//! A positional data pattern, `(first, _, ..)`, names no field. The parser
//! cannot know which field each place selects, because that depends on the
//! subject's record. It records what was written: one row per component
//! with its place counted from zero, and one more row for a final `..` that
//! spans exactly those two bytes. Typing gives each row its field.
//!
//! `wiki/spec/language/patterns.md`, "Data and case decomposition": "A
//! positional pattern binds fields by position; binding names need not match
//! declaration names", and "An explicit `..` may waive the remaining
//! components."

use crate::errors::parse_error::ParseError;
use crate::token_input::cursor::{Input, ParseResult};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{MatchFieldAction, TableMatchField};
use syntax_trees::identifier::Identifier;
use tokens::PunctuationKind;

pub(crate) struct PositionalPattern {
    /// The components in written order, then the `..` row when there is one.
    pub(crate) rows: Vec<TableMatchField>,
    pub(crate) has_rest: bool,
}

/// Whether the tokens at `input` open a positional pattern that an arm's
/// `->` or `if` follows, or that ends the tokens of an arm already cut at its
/// `->`. `(value)` and `(low..high)` stay parenthesized expressions: only a
/// comma between components, a lone `..` or an empty group makes the
/// parentheses a pattern.
pub(crate) fn starts_positional_arm(input: Input<'_, '_>) -> bool {
    if !input.at_punctuation(PunctuationKind::LeftParen) {
        return false;
    }
    let mut depth = 0usize;
    let mut separated = false;
    let mut rest_markers = 0usize;
    let mut semantic = 0usize;
    let mut remaining = input.tokens.iter().filter(|token| !token.is_trivia());
    for token in remaining.by_ref() {
        match token.punctuation() {
            Some(
                PunctuationKind::LeftParen
                | PunctuationKind::LeftBracket
                | PunctuationKind::LeftBrace,
            ) => depth += 1,
            Some(
                PunctuationKind::RightParen
                | PunctuationKind::RightBracket
                | PunctuationKind::RightBrace,
            ) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
            Some(PunctuationKind::Comma) if depth == 1 => separated = true,
            Some(PunctuationKind::DotDot) if depth == 1 => rest_markers += 1,
            _ => {}
        }
        semantic += 1;
    }
    // `semantic` counts the opening parenthesis and what it encloses.
    let empty = semantic == 1;
    let only_rest = semantic == 2 && rest_markers == 1;
    (separated || empty || only_rest)
        && remaining.next().is_none_or(|token| {
            token.punctuation() == Some(PunctuationKind::Arrow)
                || token.keyword() == Some(tokens::KeywordKind::If)
        })
}

/// The components between `(` and `)`. Each is a binding name or `_`, and a
/// `..` may come last. The cursor starts at the `(`.
pub(crate) fn parse_positional_pattern<'tokens, 'source>(
    syntax_trees: &SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, PositionalPattern> {
    let opened = input;
    let mut input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
    let mut rows: Vec<TableMatchField> = Vec::new();
    let mut has_rest = false;
    let mut separated = false;
    while !input.at_punctuation(PunctuationKind::RightParen) {
        if input.at_punctuation(PunctuationKind::DotDot) {
            let (marker, rest) = input.take_rest_marker()?;
            if !rest.at_punctuation(PunctuationKind::RightParen) {
                return Err(rest.error_here(
                    "`..` waives the remaining components, so it is the last thing in a positional pattern",
                ));
            }
            rows.push(TableMatchField {
                component: language_core::PatternComponent::Fields,
                selector: language_core::PatternSelector::Rest,
                member: marker,
                action: MatchFieldAction::Waive,
            });
            has_rest = true;
            input = rest;
            break;
        }
        let position = u32::try_from(rows.len())
            .map_err(|_| input.error_here("a positional pattern has too many components"))?;
        let written = input.current_source_span();
        let (action, rest) = component(syntax_trees, input, &rows)?;
        rows.push(TableMatchField {
            component: language_core::PatternComponent::Fields,
            selector: language_core::PatternSelector::Position(position),
            // No field is named here. An empty name keeps every lookup by
            // name from taking the binding's spelling for a field.
            member: Identifier::new("", written),
            action,
        });
        input = match rest.take_comma()? {
            Some(next) => {
                separated = true;
                next
            }
            None => {
                if !rest.at_punctuation(PunctuationKind::RightParen) {
                    return Err(
                        rest.error_here("expected `,` or `)` after a positional pattern component")
                    );
                }
                rest
            }
        };
    }
    let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    if rows.len() == 1 && !has_rest && !separated {
        // Parentheses around one name group it elsewhere in the language,
        // so this spelling would not say whether a field is decomposed.
        return Err(opened.error_here(
            "one name in parentheses is not a positional pattern: write `(name,)` to decompose \
             a record with one field, or bind the whole value with `let name = ...`",
        ));
    }
    Ok((PositionalPattern { rows, has_rest }, input))
}

/// One component: `_` waives its field and a name binds it.
fn component<'tokens, 'source>(
    syntax_trees: &SyntaxTrees,
    input: Input<'tokens, 'source>,
    earlier: &[TableMatchField],
) -> ParseResult<'tokens, 'source, MatchFieldAction> {
    let refused = || {
        input.error_here(
            "a positional pattern component is a binding name, `_`, or a final `..`; \
             to test a field's value, name it in a `Type { field: value, .. }` pattern",
        )
    };
    if input.at_contextual("_") {
        return Ok((MatchFieldAction::Waive, input.take_contextual("_")?));
    }
    if input.at_contextual("true") || input.at_contextual("false") {
        return Err(refused());
    }
    let (binding, rest) = input.take_identifier().map_err(|_: ParseError| refused())?;
    if !rest.at_punctuation(PunctuationKind::Comma)
        && !rest.at_punctuation(PunctuationKind::RightParen)
    {
        return Err(refused());
    }
    let repeated = earlier.iter().any(|row| {
        matches!(&row.action, MatchFieldAction::Bind(other)
            if other.as_str(syntax_trees) == binding.as_str(syntax_trees))
    });
    if repeated {
        return Err(input.error_here(format!(
            "pattern binding `{}` is introduced more than once in this pattern",
            binding.as_str(syntax_trees)
        )));
    }
    Ok((MatchFieldAction::Bind(binding), rest))
}
