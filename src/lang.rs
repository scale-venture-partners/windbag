use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    Hcl,
    Rust,
    Html,
    Yaml,
    Markdown,
    Sql,
}

impl Language {
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("py") => Some(Language::Python),
            Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => Some(Language::JavaScript),
            Some("ts") | Some("tsx") => Some(Language::TypeScript),
            Some("tf") | Some("tfvars") | Some("hcl") => Some(Language::Hcl),
            Some("rs") => Some(Language::Rust),
            Some("html") | Some("htm") => Some(Language::Html),
            Some("yml") | Some("yaml") => Some(Language::Yaml),
            Some("md") | Some("markdown") => Some(Language::Markdown),
            Some("sql") => Some(Language::Sql),
            _ => None,
        }
    }

    /// The tree-sitter grammar for this language, or `None` for one whose
    /// comments are found by a hand-rolled scanner instead.
    pub fn ts_language(&self) -> Option<tree_sitter::Language> {
        match self {
            Language::Python => Some(tree_sitter_python::LANGUAGE.into()),
            Language::JavaScript => Some(tree_sitter_javascript::LANGUAGE.into()),
            Language::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            Language::Hcl => Some(tree_sitter_hcl::LANGUAGE.into()),
            Language::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
            Language::Html => Some(tree_sitter_html::LANGUAGE.into()),
            Language::Yaml => Some(tree_sitter_yaml::LANGUAGE.into()),
            Language::Markdown | Language::Sql => None,
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
            Language::Python
            | Language::Hcl
            | Language::Html
            | Language::Yaml
            | Language::Markdown
            | Language::Sql => false,
        }
    }

    /// Markup files carry prose and configuration rather than code. A
    /// three-line comment above a one-line config key is idiomatic there,
    /// so the length rule has no complexity budget to measure against.
    pub fn is_markup(&self) -> bool {
        matches!(self, Language::Html | Language::Yaml | Language::Markdown)
    }
}
