use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    Hcl,
    Rust,
}

impl Language {
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("py") => Some(Language::Python),
            Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => Some(Language::JavaScript),
            Some("ts") | Some("tsx") => Some(Language::TypeScript),
            Some("tf") | Some("tfvars") | Some("hcl") => Some(Language::Hcl),
            Some("rs") => Some(Language::Rust),
            _ => None,
        }
    }

    pub fn ts_language(&self) -> tree_sitter::Language {
        match self {
            Language::Python => tree_sitter_python::LANGUAGE.into(),
            Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Language::Hcl => tree_sitter_hcl::LANGUAGE.into(),
            Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        }
    }

    /// Comment-block prefixes that mark real documentation (JSDoc, Rust's
    /// `///`/`//!`/`/**`/`/*!`), exempt from the verbose-comment length
    /// rule the same way Python docstrings are exempt (docstrings aren't
    /// `comment` nodes at all in tree-sitter's Python grammar, so they
    /// never reach this check in the first place).
    pub fn is_doc_comment(&self, text: &str) -> bool {
        let trimmed = text.trim_start();
        match self {
            Language::JavaScript | Language::TypeScript => trimmed.starts_with("/**"),
            Language::Rust => {
                trimmed.starts_with("///")
                    || trimmed.starts_with("//!")
                    || trimmed.starts_with("/**")
                    || trimmed.starts_with("/*!")
            }
            Language::Python | Language::Hcl => false,
        }
    }
}
