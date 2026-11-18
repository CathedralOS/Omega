//! Checks on a syntax tree that do not depend on the grammar being right.

// Each test binary compiles this module separately and uses a different part of it.
#![allow(dead_code)]

use omega_lexer::{Punctuation, Token, TokenKind};
use omega_parser::{Event, NodeKind, SyntaxTree};

/*
    Whatever the parser decided about how tokens group, four things must hold
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

    if !matches!(events.first(), Some(Event::Open(_))) {
        return Some("the tree does not start by opening a root node".to_string());
    }

    // One entry per open node: how many brackets were open when it started, and how many tokens it has seen.
    let mut open_nodes: Vec<(NodeKind, usize, usize)> = Vec::new();
    let mut open_brackets: Vec<Punctuation> = Vec::new();
    let mut token_event_count = 0usize;

    for (event_index, event) in events.iter().enumerate() {
        match event {
            Event::Open(node_kind) => {
                if *node_kind == NodeKind::Unfinished {
                    return Some(format!("event {event_index} opens a node that was never given a kind"));
                }

                if open_nodes.is_empty() && event_index != 0 {
                    return Some(format!("event {event_index} opens a second root"));
                }

                open_nodes.push((*node_kind, open_brackets.len(), 0));
            }
            Event::Close => {
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
            Event::Token => {
                let Some((node_kind, bracket_count_at_open, node_token_count)) = open_nodes.last_mut() else {
                    return Some(format!("event {event_index} is a token outside every node"));
                };

                *node_token_count += 1;

                let token_kind = meaningful_token_indices
                    .get(token_event_count)
                    .and_then(|&token_index| tokens.get(token_index as usize))
                    .map(|token| token.get_kind());

                token_event_count += 1;

                let Some(TokenKind::Punctuation(punctuation)) = token_kind else {
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

    None
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
