use crate::lang_extractor;
use crate::languages::common::LangSpec;
use codegraph_core::NodeKind;
use tree_sitter::Node;

fn ts_language() -> tree_sitter::Language {
    tree_sitter_cpp::LANGUAGE.into()
}

fn import_path(n: &Node, src: &[u8]) -> Option<String> {
    let mut c = n.walk();
    for ch in n.children(&mut c) {
        if matches!(ch.kind(), "string_literal" | "system_lib_string") {
            return ch.utf8_text(src).ok().map(|s| {
                s.trim_matches(|c| c == '"' || c == '<' || c == '>')
                    .to_string()
            });
        }
    }
    None
}

pub static SPEC: LangSpec = LangSpec {
    language_name: "cpp",
    extensions: &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
    ts_language,
    decls: &[
        ("function_definition", NodeKind::Function),
        ("class_specifier", NodeKind::Class),
        ("struct_specifier", NodeKind::Struct),
        ("union_specifier", NodeKind::Struct),
        ("namespace_definition", NodeKind::Namespace),
        ("enum_specifier", NodeKind::Enum),
        ("template_declaration", NodeKind::TypeAlias),
    ],
    call_kind: Some("call_expression"),
    callee_field: Some("function"),
    callee_ident_kinds: &["identifier", "field_identifier"],
    import_kinds: &["preproc_include"],
    import_extract: Some(import_path),
};

lang_extractor!(CppExtractor, SPEC);
