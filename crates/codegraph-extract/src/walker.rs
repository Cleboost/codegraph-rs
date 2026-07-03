use crate::config::{self, ExtractConfig, HeaderLanguage};
use crate::Extractor;
use camino::{Utf8Path, Utf8PathBuf};
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::sync::Arc;

pub struct FileMatch {
    pub path: Utf8PathBuf,
    pub extractor: Arc<dyn Extractor>,
}

pub type ExtMap = HashMap<&'static str, Arc<dyn Extractor>>;

pub struct WalkOptions<'a> {
    pub config: &'a ExtractConfig,
    pub project_hint: Option<HeaderLanguage>,
    pub c_extractor: Option<Arc<dyn Extractor>>,
    pub cpp_extractor: Option<Arc<dyn Extractor>>,
}

pub fn build_ext_map(extractors: &[Arc<dyn Extractor>]) -> ExtMap {
    let mut ext_map: ExtMap = HashMap::new();
    for ex in extractors {
        for e in ex.extensions() {
            ext_map.insert(*e, ex.clone());
        }
    }
    ext_map
}

fn find_extractor<'a>(
    extractors: &'a [Arc<dyn Extractor>],
    lang: &str,
) -> Option<&'a Arc<dyn Extractor>> {
    extractors.iter().find(|e| e.language() == lang)
}

pub fn walk_options<'a>(
    extractors: &'a [Arc<dyn Extractor>],
    config: &'a ExtractConfig,
    root: &Utf8Path,
) -> WalkOptions<'a> {
    let project_hint = if config.header_language == HeaderLanguage::Auto {
        config::detect_project_header_hint(root)
    } else {
        None
    };
    WalkOptions {
        config,
        project_hint,
        c_extractor: find_extractor(extractors, "c").cloned(),
        cpp_extractor: find_extractor(extractors, "cpp").cloned(),
    }
}

/// Match a single path against the extractor registry, without walking the tree.
/// Used for incremental (watcher-driven) syncs where the caller already knows
/// which paths changed.
pub fn match_extractor(
    path: &Utf8Path,
    ext_map: &ExtMap,
    opts: &WalkOptions<'_>,
) -> Option<Arc<dyn Extractor>> {
    let ext = path.extension()?;
    if ext == "h" {
        return resolve_header_extractor(path, opts);
    }
    ext_map.get(ext).cloned()
}

pub fn walk(
    root: &Utf8Path,
    extractors: &[Arc<dyn Extractor>],
    config: &ExtractConfig,
) -> Vec<FileMatch> {
    let ext_map = build_ext_map(extractors);
    let opts = walk_options(extractors, config, root);

    let mut out = Vec::new();
    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_exclude(true)
        .parents(true)
        .add_custom_ignore_filename(".codegraphignore")
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(p) = Utf8PathBuf::from_path_buf(path.to_path_buf()) else {
            continue;
        };
        let ex = if ext == "h" {
            resolve_header_extractor(&p, &opts)
        } else {
            ext_map.get(ext).cloned()
        };
        let Some(ex) = ex else {
            continue;
        };
        out.push(FileMatch {
            path: p,
            extractor: ex,
        });
    }
    out
}

fn resolve_header_extractor(path: &Utf8Path, opts: &WalkOptions<'_>) -> Option<Arc<dyn Extractor>> {
    let c = opts.c_extractor.as_ref();
    let cpp = opts.cpp_extractor.as_ref();

    match (c, cpp) {
        (None, None) => None,
        (Some(c), None) => Some(c.clone()),
        (None, Some(cpp)) => Some(cpp.clone()),
        (Some(c), Some(cpp)) => Some(resolve_header_with_both(path, opts, c, cpp)),
    }
}

fn resolve_header_with_both(
    path: &Utf8Path,
    opts: &WalkOptions<'_>,
    c: &Arc<dyn Extractor>,
    cpp: &Arc<dyn Extractor>,
) -> Arc<dyn Extractor> {
    match opts.config.header_language {
        HeaderLanguage::C => c.clone(),
        HeaderLanguage::Cpp => cpp.clone(),
        HeaderLanguage::Auto => {
            if let Some(hint) = opts.project_hint {
                return match hint {
                    HeaderLanguage::C => c.clone(),
                    HeaderLanguage::Cpp => cpp.clone(),
                    HeaderLanguage::Auto => unreachable!(),
                };
            }
            // Mixed C/C++ project: sniff file content.
            if header_looks_like_cpp(path) {
                cpp.clone()
            } else {
                c.clone()
            }
        }
    }
}

fn header_looks_like_cpp(path: &Utf8Path) -> bool {
    let Ok(bytes) = std::fs::read(path.as_std_path()) else {
        return false;
    };
    let sample = &bytes[..bytes.len().min(8192)];
    let Ok(text) = std::str::from_utf8(sample) else {
        return false;
    };
    config::is_cpp_header(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry;
    use std::io::Write;

    fn write_file(dir: &Utf8Path, name: &str, content: &str) -> Utf8PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = std::fs::File::create(path.as_std_path()).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn cpp_project_headers_use_cpp_extractor() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        write_file(&root, "src/Foo.cpp", "class Foo {};\n");
        write_file(
            &root,
            "src/Foo.h",
            "#pragma once\nnamespace tnl { class Foo {}; }\n",
        );

        let extractors = registry();
        let config = ExtractConfig::default();
        let matches = walk(&root, &extractors, &config);
        let h = matches
            .iter()
            .find(|m| m.path.ends_with("Foo.h"))
            .expect("Foo.h should be indexed");
        assert_eq!(h.extractor.language(), "cpp");
    }

    #[test]
    fn c_project_headers_use_c_extractor() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        write_file(&root, "src/foo.c", "struct foo { int x; };\n");
        write_file(
            &root,
            "src/foo.h",
            "#ifndef FOO_H\n#define FOO_H\nstruct foo { int x; };\n#endif\n",
        );

        let extractors = registry();
        let config = ExtractConfig::default();
        let matches = walk(&root, &extractors, &config);
        let h = matches
            .iter()
            .find(|m| m.path.ends_with("foo.h"))
            .expect("foo.h should be indexed");
        assert_eq!(h.extractor.language(), "c");
    }

    #[test]
    fn config_forces_cpp_headers() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        write_file(&root, "src/foo.c", "struct foo { int x; };\n");
        write_file(
            &root,
            "src/foo.h",
            "#ifndef FOO_H\n#define FOO_H\nstruct foo { int x; };\n#endif\n",
        );

        let extractors = registry();
        let config = ExtractConfig {
            header_language: HeaderLanguage::Cpp,
        };
        let matches = walk(&root, &extractors, &config);
        let h = matches
            .iter()
            .find(|m| m.path.ends_with("foo.h"))
            .expect("foo.h should be indexed");
        assert_eq!(h.extractor.language(), "cpp");
    }
}
