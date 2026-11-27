use crate::declarations::items::{ParsedDeclaration, parse_item, starts_machine_declaration};
use crate::statements::sequence::parse_statements;
use crate::token_input::cursor::{Input, ParseResult};
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{Item, MachineBody};
use tokens::{KeywordKind, PunctuationKind};

pub(crate) fn parse_body<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    requires_result: bool,
) -> ParseResult<'tokens, 'source, Option<MachineBody>> {
    if input.at_punctuation(PunctuationKind::Semicolon) {
        return Ok((
            None,
            input.take_punctuation(PunctuationKind::Semicolon, ";")?,
        ));
    }
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let (statements, mut input) = parse_statements(syntax_trees, input, requires_result)?;
    let mut children = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if starts_machine_declaration(input) {
            let declaration_start = input;
            let (declaration, rest) = parse_item(syntax_trees, input)?;
            let ParsedDeclaration::Item(Item::Machine(machine)) = declaration else {
                return Err(declaration_start.error_here("expected a nested machine declaration"));
            };
            if machine.is_public {
                return Err(declaration_start.error_here(
                    "nested machines are lexical declarations and cannot be package-exported",
                ));
            }
            children.push(Item::Machine(machine));
            input = rest;
            continue;
        }
        if starts_state_member(input) {
            return Err(input.error_here(
                "nested `state` declarations are retired; declare a nested `machine` with its own signature and contract",
            ));
        }
        if !children.is_empty() {
            return Err(
                input.error_here("executable statements cannot follow nested machine declarations")
            );
        }
        if starts_retired_entry_member(input)
            || (input.at_keyword(KeywordKind::Public)
                && input
                    .take_keyword(KeywordKind::Public, "public")?
                    .at_contextual("entry"))
        {
            return Err(input.error_here(
                "explicit nested `entry` / `public entry` machine members are retired; put the callable signature on the `machine` head, put its entry statements directly in the machine body, and put visibility on the declaration as `public machine`",
            ));
        }

        if starts_retired_invariant_member(input) {
            return Err(input.error_here(
                "the `invariant` machine member is retired: state arrival facts use \
                 `requires`, result facts use `ensures`, and loop facts are derived from \
                 checked transitions",
            ));
        }
        return Err(input.expected_one_of_here(&["nested `machine` declaration", "`}`"]));
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let mut nested_machines = arena::HandleSpan::empty();
    for child in children {
        nested_machines.push_contiguous(syntax_trees.items.append_item(child));
    }
    Ok((
        Some(MachineBody {
            assembly_target_requirement: Default::default(),
            statements,
            nested_machines,
        }),
        input,
    ))
}

pub(crate) fn starts_machine_member(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::RightBrace)
        || input.at_keyword(KeywordKind::Public)
        || input.at_keyword(KeywordKind::Machine)
        || starts_retired_entry_member(input)
        || starts_state_member(input)
        || starts_retired_invariant_member(input)
        || starts_machine_declaration(input)
}

fn starts_retired_entry_member(input: Input<'_, '_>) -> bool {
    if !input.at_contextual("entry") {
        return false;
    }
    let after_name = input.restarted(input.tokens.get(1..).unwrap_or_default());
    if after_name
        .tokens
        .first()
        .is_some_and(crate::token_input::cursor::is_identifier_token_for_parser)
    {
        return true;
    }
    if !after_name.at_punctuation(PunctuationKind::LeftParen) {
        return false;
    }
    let mut nesting = 0u32;
    for (position, token) in after_name.tokens.iter().enumerate() {
        match token.kind {
            tokens::TokenKind::Punctuation(PunctuationKind::LeftParen) => nesting += 1,
            tokens::TokenKind::Punctuation(PunctuationKind::RightParen) => {
                nesting = nesting.saturating_sub(1);
                if nesting == 0 {
                    let tail =
                        input.restarted(after_name.tokens.get(position + 1..).unwrap_or_default());
                    return tail.at_punctuation(PunctuationKind::LeftBrace)
                        || tail.at_punctuation(PunctuationKind::Arrow)
                        || [
                            "requires", "ensures", "crashes", "reaches", "blocks", "suspends",
                        ]
                        .iter()
                        .any(|keyword| tail.at_contextual(keyword));
                }
            }
            _ => {}
        }
    }
    false
}

fn starts_retired_invariant_member(input: Input<'_, '_>) -> bool {
    if !input.at_contextual("invariant") {
        return false;
    }

    let after_keyword = input.restarted(input.tokens.get(1..).unwrap_or_default());
    if !after_keyword
        .tokens
        .first()
        .is_some_and(crate::token_input::cursor::is_identifier_token_for_parser)
    {
        return false;
    }

    input
        .restarted(after_keyword.tokens.get(1..).unwrap_or_default())
        .at_punctuation(PunctuationKind::LeftBrace)
}

fn starts_state_member(input: Input<'_, '_>) -> bool {
    if !input.at_keyword(KeywordKind::State) {
        return false;
    }

    input
        .restarted(input.tokens.get(1..).unwrap_or_default())
        .tokens
        .first()
        .is_some_and(crate::token_input::cursor::is_identifier_token_for_parser)
}
