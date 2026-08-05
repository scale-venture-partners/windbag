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
    pub is_doc_comment: bool,
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

/// Extracts comment blocks from `source`. Consecutive comment nodes
/// (no blank or code line between them) are merged into a single block,
/// matching how a human reads a paragraph-style comment as one unit.
pub fn extract_blocks(source: &str, language: Language) -> anyhow::Result<Vec<CommentBlock>> {
    let mut parser = Parser::new();
    parser.set_language(&language.ts_language())?;
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
        let attached_code_lines = attached_sibling_span(&last_node, end_row);
        let text = texts.join("\n");
        let is_doc_comment = language.is_doc_comment(&text);
        blocks.push(CommentBlock {
            start_line: start_row + 1,
            end_line: end_row + 1,
            text,
            attached_code_lines,
            is_doc_comment,
        });
        i = j;
    }
    Ok(blocks)
}

fn node_text(source: &str, node: &Node) -> String {
    source[node.byte_range()].to_string()
}

fn attached_sibling_span(last_comment_node: &Node, end_row: usize) -> usize {
    match last_comment_node.next_sibling() {
        Some(sibling) if sibling.start_position().row == end_row + 1 => {
            sibling.end_position().row - sibling.start_position().row + 1
        }
        _ => 0,
    }
}
