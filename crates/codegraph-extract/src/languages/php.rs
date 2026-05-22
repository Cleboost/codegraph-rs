use crate::languages::common::LangSpec;
use crate::lang_extractor;
use codegraph_core::NodeKind;
use tree_sitter::Node;

fn ts_language() -> tree_sitter::Language { tree_sitter_php::LANGUAGE_PHP.into() }

fn import_path(n: &Node, src: &[u8]) -> Option<String> {
    let mut c = n.walk();
    for ch in n.children(&mut c) {
        if matches!(ch.kind(), "namespace_name" | "qualified_name") {
            return ch.utf8_text(src).ok().map(|s| s.to_string());
        }
    }
    None
}

pub static SPEC: LangSpec = LangSpec {
    language_name: "php",
    extensions: &["php"],
    ts_language,
    decls: &[
        ("function_definition", NodeKind::Function),
        ("method_declaration", NodeKind::Method),
        ("class_declaration", NodeKind::Class),
        ("interface_declaration", NodeKind::Interface),
        ("trait_declaration", NodeKind::Trait),
        ("namespace_definition", NodeKind::Namespace),
    ],
    call_kind: Some("function_call_expression"),
    callee_field: Some("function"),
    callee_ident_kinds: &["name", "qualified_name"],
    import_kinds: &["namespace_use_declaration"],
    import_extract: Some(import_path),
};

lang_extractor!(PhpExtractor, SPEC);
