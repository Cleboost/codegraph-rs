use crate::languages::common::LangSpec;
use crate::lang_extractor;
use codegraph_core::NodeKind;

fn ts_language() -> tree_sitter::Language { tree_sitter_scala::LANGUAGE.into() }

pub static SPEC: LangSpec = LangSpec {
    language_name: "scala",
    extensions: &["scala", "sc"],
    ts_language,
    decls: &[
        ("function_definition", NodeKind::Function),
        ("function_declaration", NodeKind::Function),
        ("class_definition", NodeKind::Class),
        ("object_definition", NodeKind::Module),
        ("trait_definition", NodeKind::Trait),
        ("val_definition", NodeKind::Constant),
        ("var_definition", NodeKind::Variable),
    ],
    call_kind: Some("call_expression"),
    callee_field: Some("function"),
    callee_ident_kinds: &["identifier"],
    import_kinds: &["import_declaration"],
    import_extract: None,
};

lang_extractor!(ScalaExtractor, SPEC);
