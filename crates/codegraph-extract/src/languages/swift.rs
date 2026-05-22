use crate::lang_extractor;
use crate::languages::common::LangSpec;
use codegraph_core::NodeKind;

fn ts_language() -> tree_sitter::Language {
    tree_sitter_swift::LANGUAGE.into()
}

pub static SPEC: LangSpec = LangSpec {
    language_name: "swift",
    extensions: &["swift"],
    ts_language,
    decls: &[
        ("function_declaration", NodeKind::Function),
        ("class_declaration", NodeKind::Class),
        ("protocol_declaration", NodeKind::Protocol),
        ("property_declaration", NodeKind::Property),
    ],
    call_kind: Some("call_expression"),
    callee_field: Some("name"),
    callee_ident_kinds: &["simple_identifier"],
    import_kinds: &["import_declaration"],
    import_extract: None,
};

lang_extractor!(SwiftExtractor, SPEC);
