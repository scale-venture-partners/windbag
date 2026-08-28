use crate::lang::Language;
use tree_sitter::{Node, Parser};

#[derive(Debug, Clone)]
pub struct CommentBlock {
    /// 1-indexed, inclusive
    pub start_line: usize,
    /// 1-indexed, inclusive
    pub end_line: usize,
    pub text: String,
    /// Line span of the single AST sibling immediately following the
    /// block, when it starts on the very next source line. Zero means
    /// free-floating (next line is blank, EOF, another comment run, or
    /// there's no following sibling at all). Deliberately the *sibling's*
    /// span rather than a raw line-scan: scanning raw lines runs past the
    /// actual statement into enclosing closing braces/brackets, which
    /// dilutes the ratio for anything nested more than one level deep.
    pub attached_code_lines: usize,
    /// Source text of the same attached sibling `attached_code_lines` is
    /// measured from, when there is one. Used by rules that need to
    /// compare what the comment says against what the code actually does.
    pub attached_code_text: Option<String>,
    pub is_doc_comment: bool,
    /// Whether the comment came from a markup file, which
    /// `Language::is_markup` explains.
    pub is_markup: bool,
}

fn collect_comment_nodes<'a>(node: Node<'a>, out: &mut Vec<Node<'a>>) {
    if node.kind().contains("comment") {
        out.push(node);
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_comment_nodes(child, out);
    }
}

/// Extracts comment blocks from `source`. Consecutive comments (no blank
/// or code line between them) are merged into a single block, matching how
/// a human reads a paragraph-style comment as one unit.
pub fn extract_blocks(source: &str, language: Language) -> anyhow::Result<Vec<CommentBlock>> {
    match language.ts_language() {
        Some(ts_language) => extract_treesitter_blocks(source, &ts_language, language),
        // Markdown, whose `<!-- -->` shares an `html_block` node with
        // `<div>`/`<script>` in the grammar and needs a sharper separation.
        None => Ok(scan_html_comments(source)),
    }
}

fn extract_treesitter_blocks(
    source: &str,
    ts_language: &tree_sitter::Language,
    language: Language,
) -> anyhow::Result<Vec<CommentBlock>> {
    let mut parser = Parser::new();
    parser.set_language(ts_language)?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter failed to parse source"))?;

    let mut nodes = Vec::new();
    collect_comment_nodes(tree.root_node(), &mut nodes);
    nodes.sort_by_key(|n| n.start_position().row);

    let mut blocks = Vec::new();
    let mut i = 0;
    while i < nodes.len() {
        let start_row = nodes[i].start_position().row;
        let mut end_row = nodes[i].end_position().row;
        let mut texts = vec![node_text(source, &nodes[i])];
        let mut last_node = nodes[i];
        let mut j = i + 1;
        while j < nodes.len() {
            let next_start = nodes[j].start_position().row;
            // Merge only if the next comment starts on the very next
            // source line (no blank line, no code line, between them).
            if next_start == end_row + 1 {
                end_row = nodes[j].end_position().row;
                texts.push(node_text(source, &nodes[j]));
                last_node = nodes[j];
                j += 1;
            } else {
                break;
            }
        }
        let (attached_code_lines, attached_code_text) =
            attached_sibling_info(source, &last_node, end_row);
        let text = texts.join("\n");
        let is_doc_comment = language.is_doc_comment(&text);
        blocks.push(CommentBlock {
            start_line: start_row + 1,
            end_line: end_row + 1,
            text,
            attached_code_lines,
            attached_code_text,
            is_doc_comment,
            is_markup: language.is_markup(),
        });
        i = j;
    }
    Ok(blocks)
}

fn node_text(source: &str, node: &Node) -> String {
    source[node.byte_range()].to_string()
}

fn attached_sibling_info(
    source: &str,
    last_comment_node: &Node,
    end_row: usize,
) -> (usize, Option<String>) {
    match last_comment_node.next_sibling() {
        Some(sibling) if sibling.start_position().row == end_row + 1 => {
            let span = sibling.end_position().row - sibling.start_position().row + 1;
            (span, Some(node_text(source, &sibling)))
        }
        _ => (0, None),
    }
}

const OPEN: &str = "<!--";
const CLOSE: &str = "-->";

/// Byte ranges covered by fenced code blocks, fence lines included, so a
/// `<!-- -->` shown as sample markup inside one is content rather than a
/// comment.
fn fenced_ranges(source: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut open: Option<(char, usize, usize)> = None;
    let mut offset = 0usize;
    for line in source.split_inclusive('\n') {
        let line_end = offset + line.len();
        if let Some(fence) = fence_marker(line) {
            match open {
                // A closing fence matches the opening character, runs at
                // least as long, and carries no info string.
                Some((open_ch, open_len, start))
                    if fence.ch == open_ch && fence.run >= open_len && !fence.has_info =>
                {
                    ranges.push(start..line_end);
                    open = None;
                }
                None if fence.opens() => open = Some((fence.ch, fence.run, offset)),
                _ => {}
            }
        }
        offset = line_end;
    }
    // An unclosed fence runs to end of file, the way a renderer treats it.
    if let Some((_, _, start)) = open {
        ranges.push(start..source.len());
    }
    ranges
}

struct Fence {
    ch: char,
    run: usize,
    has_info: bool,
    /// A backtick info string may not contain a backtick, which is what
    /// keeps an inline span like ```` ``a `b` `` ```` from opening a block.
    info_has_backtick: bool,
}

impl Fence {
    fn opens(&self) -> bool {
        !(self.ch == '`' && self.info_has_backtick)
    }
}

/// The fence run on a line: up to three spaces of indent, then three or
/// more backticks or tildes.
fn fence_marker(line: &str) -> Option<Fence> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let ch = rest.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let run = rest.chars().take_while(|c| *c == ch).count();
    if run < 3 {
        return None;
    }
    let info = rest[run..].trim();
    Some(Fence {
        ch,
        run,
        has_info: !info.is_empty(),
        info_has_backtick: info.contains('`'),
    })
}

/// Extracts `<!-- ... -->` comments, skipping anything inside a fenced
/// code block, and merges runs that sit on consecutive lines. Documentation
/// routinely shows a comment as sample markup inside a fence, so the fence
/// bounds are what separates a comment from content here.
fn scan_html_comments(source: &str) -> Vec<CommentBlock> {
    let fenced = fenced_ranges(source);
    let line_of = LineIndex::new(source);

    let mut blocks: Vec<CommentBlock> = Vec::new();
    let mut pos = 0usize;
    while let Some(rel) = source[pos..].find(OPEN) {
        let start = pos + rel;
        if fenced.iter().any(|r| r.contains(&start)) {
            pos = start + OPEN.len();
            continue;
        }
        // An unterminated `<!--` is prose naming the marker far more often
        // than it is a comment, and reading it as one would run every phrase
        // rule across the rest of the document. Nothing after it can close
        // either, so the scan is done.
        let Some(rel_end) = source[start + OPEN.len()..].find(CLOSE) else {
            break;
        };
        let end = start + OPEN.len() + rel_end + CLOSE.len();
        let start_line = line_of.line(start);
        let end_line = line_of.line(end.saturating_sub(1));
        let text = source[start..end].to_string();

        match blocks.last_mut() {
            Some(prev) if prev.end_line + 1 == start_line => {
                prev.end_line = end_line;
                prev.text.push('\n');
                prev.text.push_str(&text);
            }
            _ => blocks.push(CommentBlock {
                start_line,
                end_line,
                text,
                attached_code_lines: 0,
                attached_code_text: None,
                is_doc_comment: false,
                is_markup: true,
            }),
        }
        pos = end;
    }
    blocks
}

/// Byte offset to 1-indexed line number, over the sorted line starts.
struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    fn new(source: &str) -> Self {
        let mut line_starts = vec![0usize];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self { line_starts }
    }

    fn line(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(i) => i + 1,
            Err(i) => i,
        }
    }
}
