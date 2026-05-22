use crate::{parse_err, Extractor, ExtractResult, LocalEdge, PendingCall, RawImport};
use codegraph_core::{EdgeKind, NodeKind, Result};
use codegraph_db::NodeDraft;
use tree_sitter::{Node, Parser, Tree};

pub struct RustExtractor { lang: tree_sitter::Language }

impl RustExtractor {
    pub fn new() -> Self { Self { lang: tree_sitter_rust::LANGUAGE.into() } }
}

impl Extractor for RustExtractor {
    fn language(&self) -> &'static str { "rust" }
    fn extensions(&self) -> &'static [&'static str] { &["rs"] }
    fn ts_language(&self) -> tree_sitter::Language { self.lang.clone() }
    fn extract(&self, source: &str) -> Result<ExtractResult> {
        let mut p = Parser::new();
        p.set_language(&self.lang).map_err(|e| parse_err(format!("set_language: {e}")))?;
        let tree: Tree = p.parse(source, None).ok_or_else(|| parse_err("parse failed"))?;
        let mut ctx = Ctx { src: source.as_bytes(), result: ExtractResult::default(), parent_idx: None };
        walk(&tree.root_node(), &mut ctx);
        Ok(ctx.result)
    }
}

struct Ctx<'a> {
    src: &'a [u8],
    result: ExtractResult,
    parent_idx: Option<usize>,
}

fn walk(node: &Node, ctx: &mut Ctx) {
    let mut pushed: Option<usize> = None;
    match node.kind() {
        "function_item" => { pushed = push_named(ctx, node, NodeKind::Function); }
        "struct_item"    => { pushed = push_named(ctx, node, NodeKind::Struct); }
        "enum_item"      => { pushed = push_named(ctx, node, NodeKind::Enum); }
        "trait_item"     => { pushed = push_named(ctx, node, NodeKind::Trait); }
        "impl_item"      => {
            // Treat impls as containers via the type name.
            pushed = push_named(ctx, node, NodeKind::Namespace);
        }
        "mod_item"       => { pushed = push_named(ctx, node, NodeKind::Module); }
        "const_item"     => { pushed = push_named(ctx, node, NodeKind::Constant); }
        "static_item"    => { pushed = push_named(ctx, node, NodeKind::Variable); }
        "type_item"      => { pushed = push_named(ctx, node, NodeKind::TypeAlias); }
        "use_declaration"=> { emit_use(node, ctx); }
        "call_expression"=> { emit_call(node, ctx); }
        _ => {}
    }

    let prev = ctx.parent_idx;
    if let Some(idx) = pushed {
        if let Some(p) = prev {
            ctx.result.edges.push(LocalEdge {
                from_idx: p, to_idx: idx,
                kind: EdgeKind::Contains, line: None,
            });
        }
        ctx.parent_idx = Some(idx);
    }

    let mut c = node.walk();
    for ch in node.children(&mut c) { walk(&ch, ctx); }
    ctx.parent_idx = prev;
}

fn push_named(ctx: &mut Ctx, node: &Node, kind: NodeKind) -> Option<usize> {
    let name_node = node
        .child_by_field_name("name")
        .or_else(|| node.child_by_field_name("type"))?;
    let name = name_node.utf8_text(ctx.src).ok()?.to_string();
    if name.is_empty() { return None; }
    let start = node.start_position().row as u32 + 1;
    let end = node.end_position().row as u32 + 1;
    let body = node.child_by_field_name("body").map(|b| b.start_byte()).unwrap_or(node.end_byte());
    let sig = std::str::from_utf8(&ctx.src[node.start_byte()..body.min(ctx.src.len())])
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
        language: "rust".into(),
    });
    Some(ctx.result.nodes.len() - 1)
}

fn emit_call(node: &Node, ctx: &mut Ctx) {
    let Some(callee) = node.child_by_field_name("function") else { return };
    let name = match callee.kind() {
        "identifier" => callee.utf8_text(ctx.src).ok().map(|s| s.to_string()),
        "field_expression" => callee.child_by_field_name("field").and_then(|f| f.utf8_text(ctx.src).ok()).map(|s| s.to_string()),
        "scoped_identifier" => callee.child_by_field_name("name").and_then(|n| n.utf8_text(ctx.src).ok()).map(|s| s.to_string()),
        _ => None,
    };
    let Some(n) = name else { return };
    let Some(from) = ctx.parent_idx else { return };
    ctx.result.pending_calls.push(PendingCall {
        from_idx: from,
        target_name: n,
        line: node.start_position().row as u32 + 1,
    });
}

fn emit_use(node: &Node, ctx: &mut Ctx) {
    if let Ok(text) = node.utf8_text(ctx.src) {
        // Crude: take token after "use " up to ; or as.
        let s = text.trim().trim_start_matches("use ").trim_end_matches(';');
        let module = s.split_whitespace().next().unwrap_or(s).to_string();
        let from = ctx.parent_idx.unwrap_or(usize::MAX);
        if from == usize::MAX { return; }
        ctx.result.imports.push(RawImport {
            from_idx: from,
            module,
            line: node.start_position().row as u32 + 1,
        });
    }
}
