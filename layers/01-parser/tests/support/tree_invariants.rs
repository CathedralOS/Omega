//! Checks on a syntax tree that do not depend on the grammar being right.

use omega_lexer::{Punctuation, Token, TokenKind, lex};
use omega_parser::{NodeKind, SyntaxTree, TreeEvent};

/*
    Whatever the parser decided about how tokens group, five things must hold
    of any tree it returns, and none of them needs the grammar to state:

      1. The events are one well-formed tree: a single root, every `Open`
         matched by a `Close`.
      2. No node was left `Unfinished`, and no node other than the root is
         empty.
      3. The tree holds every meaningful token of the file exactly once, in
         order: its token indices are precisely the positions of the lexer's
         non-trivia tokens.
      4. Brackets close inside the node that opened them. If a `(` is a direct
         or indirect child of some node, so is its `)`. A parser that grouped
         `f(a` into one node and `)` into the next has misread the source, and
         this catches it without knowing what `f(a)` means.
      5. Printing the leaves and lexing the result recovers the token kinds
         and spellings. Printing reduces each gap to one space and keeps
         touching tokens together (including the two halves of a right shift).

    Returns a description of the first violation, or None.
*/
pub fn describe_tree_problem(
    source_bytes: &[u8],
    tokens: &[Token],
    syntax_tree: &SyntaxTree,
) -> Option<String> {
    let events = syntax_tree.get_events();
    let meaningful_token_indices = syntax_tree.get_meaningful_token_indices();

    let expected_token_indices: Vec<u32> = tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| !token.get_kind().is_trivia())
        .map(|(token_index, _)| token_index as u32)
        .collect();

    if meaningful_token_indices != expected_token_indices.as_slice() {
        return Some(format!(
            "the tree numbers {} tokens but the lexer produced {} meaningful ones, or they are in a different order",
            meaningful_token_indices.len(),
            expected_token_indices.len()
        ));
    }

    if !matches!(events.first(), Some(TreeEvent::Open(_))) {
        return Some("the tree does not start by opening a root node".to_string());
    }

    // One entry per open node: how many brackets were open when it started, and how many tokens it has seen.
    let mut open_nodes: Vec<(NodeKind, usize, usize)> = Vec::new();
    let mut open_brackets: Vec<Punctuation> = Vec::new();
    let mut token_event_count = 0usize;
    let mut printed_source = Vec::with_capacity(source_bytes.len());
    let mut previous_token_end = None;

    for (event_index, event) in events.iter().enumerate() {
        match event {
            TreeEvent::Open(node_kind) => {
                if *node_kind == NodeKind::Unfinished {
                    return Some(format!("event {event_index} opens a node that was never given a kind"));
                }

                if open_nodes.is_empty() && event_index != 0 {
                    return Some(format!("event {event_index} opens a second root"));
                }

                open_nodes.push((*node_kind, open_brackets.len(), 0));
            }
            TreeEvent::Close => {
                let Some((node_kind, bracket_count_at_open, node_token_count)) = open_nodes.pop() else {
                    return Some(format!("event {event_index} closes a node that was never opened"));
                };

                if open_brackets.len() != bracket_count_at_open {
                    return Some(format!("a {node_kind:?} node ends with a bracket still open inside it"));
                }

                if node_token_count == 0 && !open_nodes.is_empty() {
                    return Some(format!("a {node_kind:?} node holds no tokens"));
                }

                // A node's tokens are its parent's tokens too.
                if let Some((_, _, parent_token_count)) = open_nodes.last_mut() {
                    *parent_token_count += node_token_count;
                }
            }
            TreeEvent::Token => {
                let Some((node_kind, bracket_count_at_open, node_token_count)) = open_nodes.last_mut() else {
                    return Some(format!("event {event_index} is a token outside every node"));
                };

                *node_token_count += 1;

                let Some(token) = meaningful_token_indices
                    .get(token_event_count)
                    .and_then(|&token_index| tokens.get(token_index as usize))
                else {
                    return Some(format!("token event {event_index} has no corresponding source token"));
                };

                let span = token.get_span();
                if previous_token_end.is_some_and(|end| end < span.get_start_offset()) {
                    printed_source.push(b' ');
                }
                printed_source.extend_from_slice(
                    span.slice(source_bytes)
                        .expect("Expected a lexer span inside its source."),
                );
                previous_token_end = Some(span.get_end_offset());

                token_event_count += 1;

                let TokenKind::Punctuation(punctuation) = token.get_kind() else {
                    continue;
                };

                if let Some(closing_bracket) = get_closing_bracket(punctuation) {
                    open_brackets.push(closing_bracket);
                } else if is_closing_bracket(punctuation) {
                    if open_brackets.len() <= *bracket_count_at_open {
                        return Some(format!(
                            "a {node_kind:?} node holds a `{}` whose opening bracket is outside it",
                            punctuation.get_spelling()
                        ));
                    }

                    if open_brackets.pop() != Some(punctuation) {
                        return Some(format!("a `{}` closes a different kind of bracket", punctuation.get_spelling()));
                    }
                }
            }
        }
    }

    if !open_nodes.is_empty() {
        return Some(format!("{} nodes were never closed", open_nodes.len()));
    }

    if token_event_count != expected_token_indices.len() {
        return Some(format!(
            "the tree holds {token_event_count} tokens but the source has {} meaningful ones ({} bytes)",
            expected_token_indices.len(),
            source_bytes.len()
        ));
    }

    let printed_tokens = match lex(&printed_source) {
        Ok(tokens) => tokens,
        Err(error) => return Some(format!("the printed leaves do not lex: {error}")),
    };
    let meaningful_tokens = |token: &&Token| !token.get_kind().is_trivia();
    let original = tokens
        .iter()
        .filter(meaningful_tokens)
        .map(|token| token_signature(token, source_bytes));
    let printed = printed_tokens
        .iter()
        .filter(meaningful_tokens)
        .map(|token| token_signature(token, &printed_source));
    if original.ne(printed) {
        return Some("lexing the printed leaves changed the meaningful tokens".into());
    }
    None
}

/// Numeric suffix offsets move with the token when comments and whitespace are removed.
fn token_signature<'a>(
    token: &Token,
    source: &'a [u8],
) -> (TokenKind, &'a [u8]) {
    let span = token.get_span();
    let kind = match token.get_kind() {
        TokenKind::Integer { radix, suffix_start_offset } => TokenKind::Integer {
            radix,
            suffix_start_offset: suffix_start_offset - span.get_start_offset(),
        },
        TokenKind::Float { suffix_start_offset } => TokenKind::Float {
            suffix_start_offset: suffix_start_offset - span.get_start_offset(),
        },
        kind => kind,
    };
    (
        kind,
        span.slice(source)
            .expect("Expected a lexer span inside its source."),
    )
}

fn get_closing_bracket(punctuation: Punctuation) -> Option<Punctuation> {
    match punctuation {
        Punctuation::LeftParenthesis => Some(Punctuation::RightParenthesis),
        Punctuation::LeftBracket => Some(Punctuation::RightBracket),
        Punctuation::LeftBrace => Some(Punctuation::RightBrace),
        _ => None,
    }
}

fn is_closing_bracket(punctuation: Punctuation) -> bool {
    matches!(punctuation, Punctuation::RightParenthesis | Punctuation::RightBracket | Punctuation::RightBrace)
}
