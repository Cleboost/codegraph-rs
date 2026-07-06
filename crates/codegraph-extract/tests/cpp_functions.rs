//! Regression tests for C++ free function extraction (issue #8).

use codegraph_extract::languages::cpp::CppExtractor;
use codegraph_extract::Extractor;

fn extract_names(source: &str) -> Vec<String> {
    let result = CppExtractor::new().extract(source).unwrap();
    result
        .nodes
        .into_iter()
        .filter(|n| n.kind == codegraph_core::NodeKind::Function)
        .map(|n| n.name)
        .collect()
}

#[test]
fn cpp_out_of_class_ctor_with_specifiers_issue_9() {
    let source = include_str!("fixtures/issue9_attr_specifiers.h");
    let result = CppExtractor::new().extract(source).unwrap();
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == codegraph_core::NodeKind::Function)
        .map(|n| (n.name.clone(), n.signature.clone().unwrap_or_default()))
        .collect();

    assert_eq!(fns.len(), 12, "expected 12 out-of-class definitions");

    let expected = [
        (
            "ConstexprWidget",
            "constexpr ConstexprWidget<T>::ConstexprWidget()",
        ),
        (
            "ConstexprWidget",
            "constexpr ConstexprWidget<T>::ConstexprWidget(const ConstexprWidget &other)",
        ),
        (
            "ConstexprWidget",
            "constexpr ConstexprWidget<T>::ConstexprWidget(ConstexprWidget &&other)",
        ),
        ("~ConstexprWidget", "ConstexprWidget<T>::~ConstexprWidget()"),
        (
            "NodiscardWidget",
            "[[nodiscard]] NodiscardWidget<T>::NodiscardWidget()",
        ),
        (
            "NodiscardWidget",
            "[[nodiscard]] NodiscardWidget<T>::NodiscardWidget(const NodiscardWidget &other)",
        ),
        (
            "NodiscardWidget",
            "[[nodiscard]] NodiscardWidget<T>::NodiscardWidget(NodiscardWidget &&other)",
        ),
        ("~NodiscardWidget", "NodiscardWidget<T>::~NodiscardWidget()"),
        (
            "CustomWidget",
            "_CUSTOM_ATTRIBUTE CustomWidget<T>::CustomWidget()",
        ),
        (
            "CustomWidget",
            "_CUSTOM_ATTRIBUTE CustomWidget<T>::CustomWidget(const CustomWidget &other)",
        ),
        (
            "CustomWidget",
            "_CUSTOM_ATTRIBUTE CustomWidget<T>::CustomWidget(CustomWidget &&other)",
        ),
        ("~CustomWidget", "CustomWidget<T>::~CustomWidget()"),
    ];

    for (name, sig) in expected {
        assert!(
            fns.iter().any(|(n, s)| n == name && s == sig),
            "missing {name:?} with signature {sig:?}, got {fns:?}"
        );
    }
}

#[test]
fn cpp_free_functions_use_function_name_not_return_type() {
    let source = r#"
namespace repro_ns {

void alpha_void_plain() {}

void bravo_void_params(int x, double y) {}

int charlie_int_plain(int x) { return x; }

std::pair<int, int> delta_pair_plain(int a, int b) { return {a, b}; }

int *echo_pointer_plain(int *p) { return p; }

const int &foxtrot_const_ref_plain(const int &x) { return x; }

auto golf_auto_plain(int x) { return x; }

auto hotel_auto_trailing_plain(int x) -> int { return x; }

template <typename T> T india_template_T_return(T x) { return x; }

template <typename T> void juliet_template_void_return(T x) {}

template <typename T> int kilo_template_int_return(T x) { return 42; }

template <typename T> auto lima_template_auto_return(T x) { return x; }

template <typename T> auto mike_template_auto_trailing(T x) -> int {
  return 42;
}

template <typename T> std::pair<T, T> november_template_pair_return(T a, T b) {
  return {a, b};
}

template <typename T, typename = std::enable_if_t<std::is_integral_v<T>>>
T oscar_sfinae_return(T x) {
  return x;
}

template <typename T> T papa_noexcept_return(T x) noexcept { return x; }

constexpr int quebec_constexpr_plain(int x) { return x * 2; }

[[nodiscard]] int romeo_nodiscard_plain(int x) { return x; }

inline int sierra_inline_plain(int x) { return x; }

static int tango_static_plain(int x) { return x; }

} // namespace repro_ns
"#;

    let names = extract_names(source);
    let expected = [
        "alpha_void_plain",
        "bravo_void_params",
        "charlie_int_plain",
        "delta_pair_plain",
        "echo_pointer_plain",
        "foxtrot_const_ref_plain",
        "golf_auto_plain",
        "hotel_auto_trailing_plain",
        "india_template_T_return",
        "juliet_template_void_return",
        "kilo_template_int_return",
        "lima_template_auto_return",
        "mike_template_auto_trailing",
        "november_template_pair_return",
        "oscar_sfinae_return",
        "papa_noexcept_return",
        "quebec_constexpr_plain",
        "romeo_nodiscard_plain",
        "sierra_inline_plain",
        "tango_static_plain",
    ];

    for name in expected {
        assert!(
            names.iter().any(|n| n == name),
            "missing function {name:?}, got {names:?}"
        );
    }
    assert!(
        !names.iter().any(|n| n == "T"),
        "return type must not be used as name, got {names:?}"
    );
}
