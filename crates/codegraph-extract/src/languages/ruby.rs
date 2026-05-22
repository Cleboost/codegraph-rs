use crate::lang_extractor;
use crate::languages::common::LangSpec;
use codegraph_core::NodeKind;

fn ts_language() -> tree_sitter::Language {
    tree_sitter_ruby::LANGUAGE.into()
}

pub static SPEC: LangSpec = LangSpec {
    language_name: "ruby",
    extensions: &["rb"],
    ts_language,
    decls: &[
        ("method", NodeKind::Method),
        ("singleton_method", NodeKind::Method),
        ("class", NodeKind::Class),
        ("module", NodeKind::Module),
    ],
    call_kind: Some("call"),
    callee_field: Some("method"),
    callee_ident_kinds: &["identifier", "constant"],
    import_kinds: &[],
    import_extract: None,
};

lang_extractor!(RubyExtractor, SPEC);
