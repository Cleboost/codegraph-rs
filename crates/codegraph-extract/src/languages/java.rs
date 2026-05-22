use crate::lang_extractor;
use crate::languages::common::LangSpec;
use codegraph_core::NodeKind;
use tree_sitter::Node;

fn ts_language() -> tree_sitter::Language {
    tree_sitter_java::LANGUAGE.into()
}

fn import_path(n: &Node, src: &[u8]) -> Option<String> {
    let mut c = n.walk();
    for ch in n.children(&mut c) {
        if matches!(ch.kind(), "scoped_identifier" | "identifier") {
            return ch.utf8_text(src).ok().map(|s| s.to_string());
        }
    }
    None
}

pub static SPEC: LangSpec = LangSpec {
    language_name: "java",
    extensions: &["java"],
    ts_language,
    decls: &[
        ("class_declaration", NodeKind::Class),
        ("interface_declaration", NodeKind::Interface),
        ("enum_declaration", NodeKind::Enum),
        ("record_declaration", NodeKind::Class),
        ("method_declaration", NodeKind::Method),
        ("constructor_declaration", NodeKind::Method),
        ("field_declaration", NodeKind::Field),
    ],
    call_kind: Some("method_invocation"),
    callee_field: Some("name"),
    callee_ident_kinds: &["identifier"],
    import_kinds: &["import_declaration"],
    import_extract: Some(import_path),
};

lang_extractor!(JavaExtractor, SPEC);
