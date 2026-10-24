//! The structural field grammar shared by expression and transition dispatch.
//! Keep binding, equality and waiver distinct through parsing: only bindings
//! introduce aliases, and a waiver must not manufacture a payload read.

use crate::expressions::parse_expression::parse_expression_handle_without_struct_literals;
use crate::input::token_cursor::{Input, ParseResult};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{MatchFieldAction, TableMatchField};
use syntax_trees::statement::TableOutcomeProofSelector;
use tokens::{KeywordKind, PunctuationKind};

pub(crate) mod bindings;

pub(crate) struct DestructureFields {
    pub(crate) fields: Vec<TableMatchField>,
    pub(crate) has_rest: bool,
    pub(crate) proof_selectors: Vec<TableOutcomeProofSelector>,
}

pub(crate) fn parse_destructure_fields<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DestructureFields> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut fields = Vec::new();
    let mut has_rest = false;
    while !input.at_punctuation(PunctuationKind::RightBrace)
        && !input.at_punctuation(PunctuationKind::Semicolon)
    {
        if input.at_punctuation(PunctuationKind::DotDot) {
            has_rest = true;
            input = input.take_punctuation(PunctuationKind::DotDot, "..")?;
        } else {
            let (member, rest) = input.take_identifier()?;
            let (action, rest) = if rest.at_punctuation(PunctuationKind::Colon) {
                let rest = rest.take_punctuation(PunctuationKind::Colon, ":")?;
                let (value, rest) =
                    parse_expression_handle_without_struct_literals(syntax_trees, rest)?;
                (MatchFieldAction::Equal(value), rest)
            } else if rest.at_keyword(KeywordKind::As) {
                let rest = rest.take_keyword(KeywordKind::As, "as")?;
                if rest.at_contextual("_") {
                    (MatchFieldAction::Waive, rest.take_contextual("_")?)
                } else {
                    let (binding, rest) = rest.take_identifier()?;
                    (MatchFieldAction::Bind(binding), rest)
                }
            } else {
                (MatchFieldAction::Bind(member.clone()), rest)
            };
            fields.push(TableMatchField { member, action });
            input = rest;
        }
        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
        } else {
            break;
        }
    }

    let mut proof_selectors = Vec::<TableOutcomeProofSelector>::new();
    if input.at_punctuation(PunctuationKind::Semicolon) {
        input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        while !input.at_punctuation(PunctuationKind::RightBrace) {
            let (output_field, rest) = input.take_identifier()?;
            let (binding, rest) = if rest.at_punctuation(PunctuationKind::Colon) {
                let rest = rest.take_punctuation(PunctuationKind::Colon, ":")?;
                rest.take_identifier()?
            } else {
                (output_field.clone(), rest)
            };
            if proof_selectors.iter().any(|selector| {
                selector.output_field.as_str(syntax_trees) == output_field.as_str(syntax_trees)
            }) {
                return Err(input.error_here(format!(
                    "outcome proof selector `{}` is bound more than once in this arm",
                    output_field.as_str(syntax_trees)
                )));
            }
            if binding.as_str(syntax_trees) == "_" {
                return Err(
                    input.error_here("omit an outcome proof selector instead of binding it to `_`")
                );
            }
            if proof_selectors.iter().any(|selector| {
                selector.binding.as_str(syntax_trees) == binding.as_str(syntax_trees)
            }) {
                return Err(input.error_here(format!(
                    "outcome evidence term `{}` is bound more than once in this arm",
                    binding.as_str(syntax_trees)
                )));
            }
            proof_selectors.push(TableOutcomeProofSelector {
                output_field,
                binding,
            });
            input = rest;
            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            } else {
                break;
            }
        }
    }
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((
        DestructureFields {
            fields,
            has_rest,
            proof_selectors,
        },
        input,
    ))
}
