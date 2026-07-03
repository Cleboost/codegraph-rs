use camino::Utf8Path;
use serde::Deserialize;
use std::fs;

/// How `.h` header files should be parsed when both C and C++ extractors are available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeaderLanguage {
    /// Detect from project layout and file content.
    #[default]
    Auto,
    C,
    Cpp,
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    languages: LanguagesSection,
}

#[derive(Debug, Default, Deserialize)]
struct LanguagesSection {
    /// `"auto"`, `"c"`, or `"cpp"`.
    #[serde(default)]
    headers: Option<String>,
}

/// Project-level extraction settings (`.codegraph/config.toml`).
#[derive(Debug, Clone, Default)]
pub struct ExtractConfig {
    pub header_language: HeaderLanguage,
}

impl ExtractConfig {
    pub fn load(root: &Utf8Path) -> Self {
        let path = root.join(".codegraph").join("config.toml");
        Self::load_from(&path)
    }

    pub fn load_from(path: &Utf8Path) -> Self {
        let Ok(text) = fs::read_to_string(path.as_std_path()) else {
            return Self::default();
        };
        let Ok(file) = toml::from_str::<ConfigFile>(&text) else {
            return Self::default();
        };
        Self {
            header_language: parse_header_language(file.languages.headers.as_deref()),
        }
    }
}

fn parse_header_language(raw: Option<&str>) -> HeaderLanguage {
    match raw.unwrap_or("auto").trim().to_ascii_lowercase().as_str() {
        "c" => HeaderLanguage::C,
        "cpp" | "c++" | "cxx" => HeaderLanguage::Cpp,
        _ => HeaderLanguage::Auto,
    }
}

/// Default `config.toml` written on `codegraph init`.
pub const DEFAULT_CONFIG_TOML: &str = r#"# CodeGraph project configuration
# See https://github.com/Cleboost/codegraph-rs

[languages]
# How to parse .h header files: "auto", "c", or "cpp".
# "auto" detects C++ projects from .cpp/.hpp files and C++ syntax in headers.
headers = "auto"
"#;

/// Quick project scan: returns a hint when the tree is clearly C-only or C++-only.
pub fn detect_project_header_hint(root: &Utf8Path) -> Option<HeaderLanguage> {
    let mut c_files = 0u32;
    let mut cpp_files = 0u32;

    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_exclude(true)
        .parents(true)
        .add_custom_ignore_filename(".codegraphignore")
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) else {
            continue;
        };
        match ext {
            "c" => c_files += 1,
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => cpp_files += 1,
            _ => {}
        }
    }

    if cpp_files > 0 && c_files == 0 {
        Some(HeaderLanguage::Cpp)
    } else if c_files > 0 && cpp_files == 0 {
        Some(HeaderLanguage::C)
    } else {
        None
    }
}

/// Heuristic: does this header look like C++ from its source text?
pub fn is_cpp_header(source: &str) -> bool {
    let sample = &source[..source.len().min(8192)];
    const MARKERS: &[&str] = &[
        "namespace ",
        "class ",
        "template ",
        "typename ",
        "constexpr ",
        "noexcept",
        "public:",
        "private:",
        "protected:",
        "operator ",
        "std::",
        "extern \"C\"",
        "using ",
        "::",
    ];
    MARKERS.iter().any(|m| sample.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config_headers() {
        let cfg = toml::from_str::<ConfigFile>(
            r#"
[languages]
headers = "cpp"
"#,
        )
        .unwrap();
        assert_eq!(
            parse_header_language(cfg.languages.headers.as_deref()),
            HeaderLanguage::Cpp
        );
    }

    #[test]
    fn sniff_cpp_header() {
        assert!(is_cpp_header(
            "#pragma once\nnamespace tnl { class String {}; }\n"
        ));
        assert!(!is_cpp_header(
            "#ifndef FOO_H\n#define FOO_H\nstruct foo { int x; };\n#endif\n"
        ));
    }
}
