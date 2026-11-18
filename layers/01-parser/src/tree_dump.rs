//! The text form of the parser's output that the golden fixtures compare against.

use crate::event::Event;
use crate::parse_error::ParseError;
use crate::syntax_tree::SyntaxTree;
use omega_lexer::{LineIndex, Token};
use std::fmt::Write;

/*
    An indented outline, two spaces per level. A node that holds only tokens is
    printed on one line with them; a node that holds other nodes is printed as
    its name, with each child on a line of its own beneath it.

        Machine
          'machine'
          Path 'main'
          Params '(' ')'
          Body
            '{'
            Tail
              Literal '0'
            '}'

    Tokens are quoted with `'` so they cannot be mistaken for node names, which
    are never quoted. Nothing inside the quotes is escaped except bytes outside
    printable ASCII, written `\x{NN}` as the lexer's dump writes them; a token
    is one line of the dump, so a `'` inside one cannot be confused with the
    closing quote.
*/

/// Renders a tree for a golden fixture.
pub fn dump_tree(
    source_bytes: &[u8],
    tokens: &[Token],
    syntax_tree: &SyntaxTree,
) -> String {
    let events = syntax_tree.get_events();
    let mut meaningful_token_indices = syntax_tree.get_meaningful_token_indices().iter();
    let mut tree_dump = String::new();
    let mut depth = 0usize;
    let mut is_inside_token_only_node = false;

    let mut write_token = |tree_dump: &mut String| {
        let spelling_bytes = meaningful_token_indices
            .next()
            .and_then(|&token_index| tokens.get(token_index as usize))
            .and_then(|token| token.get_span().slice(source_bytes))
            .unwrap_or_default();

        // Writing to a String cannot fail, so the results are discarded.
        let _ = write!(tree_dump, "'{}'", escape_for_dump(spelling_bytes));
    };

    for (event_index, event) in events.iter().enumerate() {
        match event {
            Event::Open(node_kind) => {
                let _ = write!(tree_dump, "{:indent$}{node_kind:?}", "", indent = depth * 2);

                is_inside_token_only_node = holds_only_tokens(events, event_index);

                if !is_inside_token_only_node {
                    tree_dump.push('\n');
                }

                depth += 1;
            }
            Event::Close => {
                depth = depth.saturating_sub(1);

                if is_inside_token_only_node {
                    tree_dump.push('\n');
                    is_inside_token_only_node = false;
                }
            }
            Event::Token => {
                if is_inside_token_only_node {
                    tree_dump.push(' ');
                    write_token(&mut tree_dump);
                } else {
                    let _ = write!(tree_dump, "{:indent$}", "", indent = depth * 2);

                    write_token(&mut tree_dump);
                    tree_dump.push('\n');
                }
            }
        }
    }

    tree_dump
}

/// Renders a refusal for a golden fixture: the rule broken, where, what was found and what was wanted.
pub fn dump_parse_error(
    source_bytes: &[u8],
    parse_error: &ParseError,
) -> String {
    let error_span = parse_error.get_span();
    let source_position = LineIndex::new(source_bytes).get_position(error_span.get_start_offset());
    let found_text = match error_span.slice(source_bytes) {
        Some(found_bytes) if !found_bytes.is_empty() => escape_for_dump(found_bytes),
        _ => "<end of source>".to_string(),
    };

    format!(
        "error {:?} at {}:{} bytes {}..{}\n  found: {}\n  expected: {}\n  {}\n",
        parse_error.get_kind(),
        source_position.get_line(),
        source_position.get_column(),
        error_span.get_start_offset(),
        error_span.get_end_offset(),
        found_text,
        parse_error.get_expected(),
        parse_error.get_kind().get_message(parse_error.get_expected())
    )
}

/// True if the node opened at `open_event_index` closes before any other node opens inside it.
fn holds_only_tokens(
    events: &[Event],
    open_event_index: usize,
) -> bool {
    events
        .iter()
        .skip(open_event_index + 1)
        .find(|event| !matches!(event, Event::Token))
        .is_some_and(|event| matches!(event, Event::Close))
}

// @Copypasta: the lexer's dump has the same function and keeps it private. Two dumps that must
// agree on how a byte is shown should share it; that means exporting it from the lexer.
fn escape_for_dump(bytes: &[u8]) -> String {
    let mut escaped = String::with_capacity(bytes.len());

    for &byte in bytes {
        if (0x20..=0x7E).contains(&byte) {
            escaped.push(char::from(byte));
        } else {
            let _ = write!(escaped, "\\x{{{byte:02x}}}");
        }
    }

    escaped
}
