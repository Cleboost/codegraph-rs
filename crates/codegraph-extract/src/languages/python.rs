use crate::{parse_err, ExtractResult, Extractor, LocalEdge, PendingCall, RawImport};
use codegraph_core::{EdgeKind, NodeKind, Result};
use codegraph_db::NodeDraft;
use tree_sitter::{Node, Parser, Tree};

pub struct PythonExtractor {
    lang: tree_sitter::Language,
}
impl Default for PythonExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonExtractor {
    pub fn new() -> Self {
        Self {
            lang: tree_sitter_python::LANGUAGE.into(),
        }
    }
}

impl Extractor for PythonExtractor {
    fn language(&self) -> &'static str {
        "python"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["py", "pyi"]
    }
    fn ts_language(&self) -> tree_sitter::Language {
        self.lang.clone()
    }
    fn extract(&self, source: &str) -> Result<ExtractResult> {
        let mut p = Parser::new();
        p.set_language(&self.lang)
            .map_err(|e| parse_err(format!("set_language: {e}")))?;
        let tree: Tree = p
            .parse(source, None)
            .ok_or_else(|| parse_err("parse failed"))?;
        let mut ctx = Ctx {
            src: source.as_bytes(),
            result: ExtractResult::default(),
            parent_idx: None,
        };
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
        "function_definition" => {
            pushed = push_named(ctx, node, NodeKind::Function);
        }
        "class_definition" => {
            pushed = push_named(ctx, node, NodeKind::Class);
        }
        "import_statement" | "import_from_statement" => {
            emit_import(node, ctx);
        }
        "call" => {
            emit_call(node, ctx);
        }
        _ => {}
    }
    let prev = ctx.parent_idx;
    if let Some(idx) = pushed {
        if let Some(p) = prev {
            ctx.result.edges.push(LocalEdge {
                from_idx: p,
                to_idx: idx,
                kind: EdgeKind::Contains,
                line: None,
            });
        }
        ctx.parent_idx = Some(idx);
    }
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        walk(&ch, ctx);
    }
    ctx.parent_idx = prev;
}

fn push_named(ctx: &mut Ctx, node: &Node, kind: NodeKind) -> Option<usize> {
    let name_node = node.child_by_field_name("name")?;
    let name = name_node.utf8_text(ctx.src).ok()?.to_string();
    if name.is_empty() {
        return None;
    }
    let start = node.start_position().row as u32 + 1;
    let end = node.end_position().row as u32 + 1;
    let body = node
        .child_by_field_name("body")
        .map(|b| b.start_byte())
        .unwrap_or(node.end_byte());
    let sig = std::str::from_utf8(&ctx.src[node.start_byte()..body.min(ctx.src.len())])
        .ok()
        .map(|s| s.trim().lines().next().unwrap_or("").to_string());

    // Docstring: first string literal in body block.
    let docstring = node
        .child_by_field_name("body")
        .and_then(|b| extract_docstring(&b, ctx.src));

    ctx.result.nodes.push(NodeDraft {
        kind,
        name,
        qualified_name: None,
        start_line: start,
        end_line: end,
        signature: sig,
        docstring,
        language: "python".into(),
    });
    Some(ctx.result.nodes.len() - 1)
}

fn extract_docstring(body: &Node, src: &[u8]) -> Option<String> {
    let mut c = body.walk();
    let first = body.children(&mut c).next()?;
    let stmt = if first.kind() == "expression_statement" {
        first
    } else {
        return None;
    };
    let mut cc = stmt.walk();
    let s = stmt.children(&mut cc).next()?;
    if s.kind() == "string" {
        let text = s.utf8_text(src).ok()?;
        Some(
            text.trim_matches(|c: char| c == '"' || c == '\'')
                .to_string(),
        )
    } else {
        None
    }
}

fn emit_call(node: &Node, ctx: &mut Ctx) {
    let Some(callee) = node.child_by_field_name("function") else {
        return;
    };
    let name = match callee.kind() {
        "identifier" => callee.utf8_text(ctx.src).ok().map(|s| s.to_string()),
        "attribute" => callee
            .child_by_field_name("attribute")
            .and_then(|a| a.utf8_text(ctx.src).ok())
            .map(|s| s.to_string()),
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

fn emit_import(node: &Node, ctx: &mut Ctx) {
    let module = if node.kind() == "import_from_statement" {
        node.child_by_field_name("module_name")
            .and_then(|n| n.utf8_text(ctx.src).ok())
            .map(|s| s.to_string())
    } else {
        let mut c = node.walk();
        let mut found = None;
        for ch in node.children(&mut c) {
            if ch.kind() == "dotted_name" {
                found = ch.utf8_text(ctx.src).ok().map(|s| s.to_string());
                break;
            }
        }
        found
    };
    let Some(m) = module else { return };
    let from = ctx.parent_idx.unwrap_or(usize::MAX);
    if from == usize::MAX {
        return;
    }
    ctx.result.imports.push(RawImport {
        from_idx: from,
        module: m,
        line: node.start_position().row as u32 + 1,
    });
}
