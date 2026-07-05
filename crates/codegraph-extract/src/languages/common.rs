//! Shared walker used by simple language extractors.
//!
//! A language provides a [`LangSpec`] (node kinds, callee field, etc.) and the
//! common walker handles tree-sitter traversal, name extraction, signature
//! capture, `contains` edges, and import/call emission.

use crate::{ExtractResult, LocalEdge, PendingCall, RawImport};

pub type ImportExtractFn = fn(&tree_sitter::Node, &[u8]) -> Option<String>;
use codegraph_core::{EdgeKind, NodeKind, Result};
use codegraph_db::NodeDraft;
use tree_sitter::{Node, Parser, Tree};

/// Declarative configuration of a language's extractor.
pub struct LangSpec {
    pub language_name: &'static str,
    pub extensions: &'static [&'static str],
    pub ts_language: fn() -> tree_sitter::Language,
    /// (tree-sitter node kind, codegraph NodeKind) — first match wins.
    pub decls: &'static [(&'static str, NodeKind)],
    /// Tree-sitter kind of a call site. Callee is read from `callee_field`.
    pub call_kind: Option<&'static str>,
    pub callee_field: Option<&'static str>,
    /// Identifier kinds inside a callee expression (e.g. "identifier",
    /// "field_identifier"). Used to extract the called name.
    pub callee_ident_kinds: &'static [&'static str],
    /// Tree-sitter kinds that represent an import statement at the top level.
    pub import_kinds: &'static [&'static str],
    /// Optional custom import path extractor; falls back to the entire node text.
    pub import_extract: Option<ImportExtractFn>,
}

pub fn run(spec: &'static LangSpec, source: &str) -> Result<ExtractResult> {
    let lang = (spec.ts_language)();
    let mut parser = Parser::new();
    parser
        .set_language(&lang)
        .map_err(|e| crate::parse_err(format!("set_language: {e}")))?;
    let tree: Tree = parser
        .parse(source, None)
        .ok_or_else(|| crate::parse_err("parse failed"))?;
    let mut ctx = Ctx {
        spec,
        src: source.as_bytes(),
        result: ExtractResult::default(),
        parent_idx: None,
    };
    walk(&tree.root_node(), &mut ctx);
    Ok(ctx.result)
}

struct Ctx<'a> {
    spec: &'static LangSpec,
    src: &'a [u8],
    result: ExtractResult,
    parent_idx: Option<usize>,
}

fn walk(node: &Node, ctx: &mut Ctx) {
    let k = node.kind();
    let mut pushed: Option<usize> = None;

    if let Some((_, nk)) = ctx.spec.decls.iter().find(|(s, _)| *s == k) {
        pushed = push_named(ctx, node, *nk);
    } else if ctx.spec.import_kinds.contains(&k) {
        emit_import(node, ctx);
    } else if ctx.spec.call_kind == Some(k) {
        emit_call(node, ctx);
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
    let name_node = node
        .child_by_field_name("name")
        .or_else(|| name_from_declarator(node))
        .or_else(|| first_identifier(node))?;
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
    ctx.result.nodes.push(NodeDraft {
        kind,
        name,
        qualified_name: None,
        start_line: start,
        end_line: end,
        signature: sig,
        docstring: None,
        language: ctx.spec.language_name.into(),
    });
    Some(ctx.result.nodes.len() - 1)
}

/// Walk a C/C++ declarator chain to the function or variable identifier.
fn name_from_declarator<'a>(n: &Node<'a>) -> Option<Node<'a>> {
    let declarator = n.child_by_field_name("declarator")?;
    declarator_name(&declarator)
}

fn declarator_name<'a>(n: &Node<'a>) -> Option<Node<'a>> {
    match n.kind() {
        "identifier" | "field_identifier" | "destructor_name" | "operator_name" => Some(*n),
        "type_identifier" if is_conversion_declarator(n) => Some(*n),
        "function_declarator"
        | "pointer_declarator"
        | "reference_declarator"
        | "array_declarator"
        | "parenthesized_declarator"
        | "abstract_function_declarator"
        | "variadic_declarator" => declarator_child(n).and_then(|d| declarator_name(&d)),
        "qualified_identifier" => n.child_by_field_name("name"),
        _ => None,
    }
}

fn declarator_child<'a>(n: &Node<'a>) -> Option<Node<'a>> {
    if let Some(d) = n.child_by_field_name("declarator") {
        return Some(d);
    }
    let mut c = n.walk();
    let declarator = n.children(&mut c).find(|&ch| is_declarator_kind(ch.kind()));
    declarator
}

fn is_declarator_kind(kind: &str) -> bool {
    matches!(
        kind,
        "function_declarator"
            | "pointer_declarator"
            | "reference_declarator"
            | "array_declarator"
            | "parenthesized_declarator"
            | "abstract_function_declarator"
            | "variadic_declarator"
            | "identifier"
            | "field_identifier"
            | "destructor_name"
            | "operator_name"
            | "qualified_identifier"
            | "operator_cast"
    )
}

fn is_conversion_declarator(n: &Node) -> bool {
    n.parent()
        .map(|p| p.kind() == "operator_cast")
        .unwrap_or(false)
}

fn first_identifier<'a>(n: &Node<'a>) -> Option<Node<'a>> {
    let mut c = n.walk();
    let mut found = None;
    for ch in n.children(&mut c) {
        if matches!(
            ch.kind(),
            "identifier"
                | "type_identifier"
                | "field_identifier"
                | "property_identifier"
                | "simple_identifier"
        ) {
            found = Some(ch);
            break;
        }
    }
    found
}

fn emit_call(node: &Node, ctx: &mut Ctx) {
    let Some(field) = ctx.spec.callee_field else {
        return;
    };
    let Some(callee) = node.child_by_field_name(field) else {
        return;
    };
    let name = if ctx.spec.callee_ident_kinds.contains(&callee.kind()) {
        callee.utf8_text(ctx.src).ok().map(|s| s.to_string())
    } else {
        first_identifier_of_kinds(&callee, ctx.spec.callee_ident_kinds, ctx.src)
    };
    let Some(n) = name else { return };
    let Some(from) = ctx.parent_idx else { return };
    ctx.result.pending_calls.push(PendingCall {
        from_idx: from,
        target_name: n,
        line: node.start_position().row as u32 + 1,
    });
}

fn first_identifier_of_kinds(n: &Node, kinds: &[&str], src: &[u8]) -> Option<String> {
    let mut c = n.walk();
    let mut last = None;
    let mut stack = vec![n.children(&mut c).collect::<Vec<_>>()];
    while let Some(level) = stack.last_mut() {
        if let Some(ch) = level.pop() {
            if kinds.contains(&ch.kind()) {
                if let Ok(t) = ch.utf8_text(src) {
                    last = Some(t.to_string());
                }
            }
            let mut cc = ch.walk();
            let next: Vec<_> = ch.children(&mut cc).collect();
            if !next.is_empty() {
                stack.push(next);
            }
        } else {
            stack.pop();
        }
    }
    last
}

fn emit_import(node: &Node, ctx: &mut Ctx) {
    let module = if let Some(f) = ctx.spec.import_extract {
        f(node, ctx.src)
    } else {
        node.utf8_text(ctx.src).ok().map(|s| s.trim().to_string())
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

/// Convenience macro: define an `Extractor` impl that delegates to a `LangSpec`.
#[macro_export]
macro_rules! lang_extractor {
    ($struct:ident, $spec:expr) => {
        #[derive(Default)]
        pub struct $struct;
        impl $struct {
            pub fn new() -> Self {
                Self
            }
        }
        impl $crate::Extractor for $struct {
            fn language(&self) -> &'static str {
                $spec.language_name
            }
            fn extensions(&self) -> &'static [&'static str] {
                $spec.extensions
            }
            fn ts_language(&self) -> tree_sitter::Language {
                ($spec.ts_language)()
            }
            fn extract(&self, source: &str) -> codegraph_core::Result<$crate::ExtractResult> {
                $crate::languages::common::run(&$spec, source)
            }
        }
    };
}

#[cfg(all(test, feature = "lang-cpp"))]
mod tests {
    use super::*;
    use tree_sitter::{Node, Parser};

    fn find_kind<'a>(node: &Node<'a>, kind: &str) -> Option<Node<'a>> {
        if node.kind() == kind {
            return Some(*node);
        }
        let mut c = node.walk();
        for ch in node.children(&mut c) {
            if let Some(found) = find_kind(&ch, kind) {
                return Some(found);
            }
        }
        None
    }

    #[test]
    fn name_from_declarator_finds_void_function() {
        let src = "void alpha_void_plain() {}";
        let mut p = Parser::new();
        p.set_language(&tree_sitter_cpp::LANGUAGE.into()).unwrap();
        let tree = p.parse(src, None).unwrap();
        let fd = find_kind(&tree.root_node(), "function_definition").unwrap();
        let name = name_from_declarator(&fd).unwrap();
        assert_eq!(name.utf8_text(src.as_bytes()).unwrap(), "alpha_void_plain");
    }

    #[test]
    fn name_from_declarator_finds_reference_return_function() {
        let src = "const int &foxtrot_const_ref_plain(const int &x) { return x; }";
        let mut p = Parser::new();
        p.set_language(&tree_sitter_cpp::LANGUAGE.into()).unwrap();
        let tree = p.parse(src, None).unwrap();
        let fd = find_kind(&tree.root_node(), "function_definition").unwrap();
        let name = name_from_declarator(&fd).unwrap();
        assert_eq!(
            name.utf8_text(src.as_bytes()).unwrap(),
            "foxtrot_const_ref_plain"
        );
    }

    #[test]
    fn run_extracts_void_function() {
        use crate::languages::cpp::SPEC;
        let result = run(&SPEC, "void alpha_void_plain() {}").unwrap();
        assert!(
            result.nodes.iter().any(|n| n.name == "alpha_void_plain"),
            "nodes: {:?}",
            result.nodes.iter().map(|n| &n.name).collect::<Vec<_>>()
        );
    }
}
