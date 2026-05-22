use crate::{parse_err, Extractor, ExtractResult, LocalEdge, PendingCall, RawImport};
use codegraph_core::{EdgeKind, NodeKind, Result};
use codegraph_db::NodeDraft;
use tree_sitter::{Node, Parser, Tree};

pub struct TypeScriptExtractor { lang: tree_sitter::Language }
pub struct TsxExtractor { lang: tree_sitter::Language }

impl TypeScriptExtractor {
    pub fn new() -> Self { Self { lang: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into() } }
}
impl TsxExtractor {
    pub fn new() -> Self { Self { lang: tree_sitter_typescript::LANGUAGE_TSX.into() } }
}

impl Extractor for TypeScriptExtractor {
    fn language(&self) -> &'static str { "typescript" }
    fn extensions(&self) -> &'static [&'static str] { &["ts", "mts", "cts"] }
    fn ts_language(&self) -> tree_sitter::Language { self.lang.clone() }
    fn extract(&self, source: &str) -> Result<ExtractResult> { extract_ts(self.lang.clone(), source, "typescript") }
}

impl Extractor for TsxExtractor {
    fn language(&self) -> &'static str { "tsx" }
    fn extensions(&self) -> &'static [&'static str] { &["tsx"] }
    fn ts_language(&self) -> tree_sitter::Language { self.lang.clone() }
    fn extract(&self, source: &str) -> Result<ExtractResult> { extract_ts(self.lang.clone(), source, "tsx") }
}

fn extract_ts(lang: tree_sitter::Language, source: &str, lang_name: &str) -> Result<ExtractResult> {
    let mut parser = Parser::new();
    parser.set_language(&lang).map_err(|e| parse_err(format!("set_language: {e}")))?;
    let tree: Tree = parser.parse(source, None).ok_or_else(|| parse_err("parse failed"))?;
    let mut ctx = Ctx {
        src: source.as_bytes(),
        lang_name,
        result: ExtractResult::default(),
        parent_idx: None,
    };
    walk(&tree.root_node(), &mut ctx);
    Ok(ctx.result)
}

struct Ctx<'a> {
    src: &'a [u8],
    lang_name: &'a str,
    result: ExtractResult,
    parent_idx: Option<usize>,
}

fn walk(node: &Node, ctx: &mut Ctx) {
    let kind = node.kind();
    let mut pushed: Option<usize> = None;

    match kind {
        "function_declaration" | "function_expression" | "arrow_function" => {
            if let Some(idx) = push_named(ctx, node, NodeKind::Function) { pushed = Some(idx); }
        }
        "method_definition" | "method_signature" => {
            if let Some(idx) = push_named(ctx, node, NodeKind::Method) { pushed = Some(idx); }
        }
        "class_declaration" | "class" => {
            if let Some(idx) = push_named(ctx, node, NodeKind::Class) {
                pushed = Some(idx);
                emit_heritage(node, ctx, idx);
            }
        }
        "interface_declaration" => {
            if let Some(idx) = push_named(ctx, node, NodeKind::Interface) { pushed = Some(idx); }
        }
        "type_alias_declaration" => {
            if let Some(idx) = push_named(ctx, node, NodeKind::TypeAlias) { pushed = Some(idx); }
        }
        "enum_declaration" => {
            if let Some(idx) = push_named(ctx, node, NodeKind::Enum) { pushed = Some(idx); }
        }
        "variable_declarator" => {
            if let Some(idx) = push_named(ctx, node, NodeKind::Variable) { pushed = Some(idx); }
        }
        "import_statement" => {
            emit_import(node, ctx);
        }
        "call_expression" => {
            emit_call(node, ctx);
        }
        _ => {}
    }

    let prev = ctx.parent_idx;
    if let Some(idx) = pushed {
        if let Some(parent) = prev {
            ctx.result.edges.push(LocalEdge {
                from_idx: parent, to_idx: idx,
                kind: EdgeKind::Contains, line: None,
            });
        }
        ctx.parent_idx = Some(idx);
    }

    let mut c = node.walk();
    for child in node.children(&mut c) { walk(&child, ctx); }

    ctx.parent_idx = prev;
}

fn push_named(ctx: &mut Ctx, node: &Node, kind: NodeKind) -> Option<usize> {
    let name_node = node
        .child_by_field_name("name")
        .or_else(|| find_first_identifier(node));
    let name = name_node.and_then(|n| n.utf8_text(ctx.src).ok())?.to_string();
    if name.is_empty() { return None; }
    let start = node.start_position().row as u32 + 1;
    let end = node.end_position().row as u32 + 1;
    let sig_end = node.child_by_field_name("body").map(|b| b.start_byte()).unwrap_or(node.end_byte());
    let sig = std::str::from_utf8(&ctx.src[node.start_byte()..sig_end.min(ctx.src.len())])
        .ok()
        .map(|s| s.trim().lines().next().unwrap_or("").to_string());

    ctx.result.nodes.push(NodeDraft {
        kind,
        name,
        qualified_name: None,
        start_line: start,
        end_line: end,
        signature: sig,
        docstring: None,
        language: ctx.lang_name.to_string(),
    });
    Some(ctx.result.nodes.len() - 1)
}

fn find_first_identifier<'a>(n: &Node<'a>) -> Option<Node<'a>> {
    let mut c = n.walk();
    let mut found = None;
    for ch in n.children(&mut c) {
        if matches!(ch.kind(), "identifier" | "type_identifier" | "property_identifier") {
            found = Some(ch);
            break;
        }
    }
    found
}

fn emit_call(node: &Node, ctx: &mut Ctx) {
    let Some(callee) = node.child_by_field_name("function") else { return };
    let target = match callee.kind() {
        "identifier" => callee.utf8_text(ctx.src).ok().map(|s| s.to_string()),
        "member_expression" => callee
            .child_by_field_name("property")
            .and_then(|p| p.utf8_text(ctx.src).ok())
            .map(|s| s.to_string()),
        _ => None,
    };
    let Some(name) = target else { return };
    let Some(from) = ctx.parent_idx else { return };
    ctx.result.pending_calls.push(PendingCall {
        from_idx: from,
        target_name: name,
        line: node.start_position().row as u32 + 1,
    });
}

fn emit_import(node: &Node, ctx: &mut Ctx) {
    let Some(src) = node.child_by_field_name("source") else { return };
    let Ok(text) = src.utf8_text(ctx.src) else { return };
    let module = text.trim_matches(|c| c == '"' || c == '\'' || c == '`').to_string();
    let from = ctx.parent_idx.unwrap_or(usize::MAX);
    if from == usize::MAX { return; }
    ctx.result.imports.push(RawImport {
        from_idx: from,
        module,
        line: node.start_position().row as u32 + 1,
    });
}

fn emit_heritage(node: &Node, ctx: &mut Ctx, _class_idx: usize) {
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        if ch.kind() == "class_heritage" {
            // Could emit extends/implements pending references — left as TODO for resolver.
            let _ = ch;
        }
    }
}
