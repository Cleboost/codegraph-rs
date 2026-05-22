use crate::languages::common::LangSpec;
use crate::lang_extractor;
use codegraph_core::NodeKind;
use tree_sitter::Node;

fn ts_language() -> tree_sitter::Language { tree_sitter_c_sharp::LANGUAGE.into() }

fn import_path(n: &Node, src: &[u8]) -> Option<String> {
    n.child_by_field_name("name").and_then(|x| x.utf8_text(src).ok()).map(|s| s.to_string())
}

pub static SPEC: LangSpec = LangSpec {
    language_name: "csharp",
    extensions: &["cs"],
    ts_language,
    decls: &[
        ("class_declaration", NodeKind::Class),
        ("struct_declaration", NodeKind::Struct),
        ("interface_declaration", NodeKind::Interface),
        ("enum_declaration", NodeKind::Enum),
        ("namespace_declaration", NodeKind::Namespace),
        ("method_declaration", NodeKind::Method),
        ("constructor_declaration", NodeKind::Method),
        ("property_declaration", NodeKind::Property),
        ("field_declaration", NodeKind::Field),
        ("record_declaration", NodeKind::Class),
    ],
    call_kind: Some("invocation_expression"),
    callee_field: Some("function"),
    callee_ident_kinds: &["identifier"],
    import_kinds: &["using_directive"],
    import_extract: Some(import_path),
};

lang_extractor!(CSharpExtractor, SPEC);
