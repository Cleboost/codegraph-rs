use crate::lang_extractor;
use crate::languages::common::LangSpec;
use codegraph_core::NodeKind;

fn ts_language() -> tree_sitter::Language {
    tree_sitter_lua::LANGUAGE.into()
}

pub static SPEC: LangSpec = LangSpec {
    language_name: "lua",
    extensions: &["lua"],
    ts_language,
    decls: &[
        ("function_declaration", NodeKind::Function),
        ("function_definition", NodeKind::Function),
        ("local_function", NodeKind::Function),
    ],
    call_kind: Some("function_call"),
    callee_field: Some("name"),
    callee_ident_kinds: &["identifier"],
    import_kinds: &[],
    import_extract: None,
};

lang_extractor!(LuaExtractor, SPEC);
