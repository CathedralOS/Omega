use crate::parser::input::{Input, ParseResult};
use crate::parser::state::parse_state_signature;
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::TraitDefinition;
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_trait_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    is_boundary: bool,
) -> ParseResult<'tokens, 'source, TraitDefinition> {
    let input = input.take_contextual("trait")?;
    let (name, mut input) = input.take_identifier()?;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut required_trait_start = Handle::invalid();
    let mut required_trait_count = 0u32;
    let mut machine_start = Handle::invalid();
    let mut machine_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_contextual("requires") {
            let (required_trait, rest) = parse_trait_requirement(input)?;
            let handle = syntax_trees
                .items
                .append_identifier_path_member(required_trait);
            if required_trait_count == 0 {
                required_trait_start = handle;
            }
            required_trait_count = required_trait_count
                .checked_add(1)
                .expect("trait requirement span count overflow");
            input = rest;
            continue;
        }

        input = input.take_keyword(KeywordKind::Machine, "machine")?;
        let (signature, rest) = parse_state_signature(syntax_trees, input)?;
        let handle = syntax_trees.items.insert_state_signature(&signature);
        let handle = syntax_trees.items.append_state_signature_handle(handle);
        if machine_count == 0 {
            machine_start = handle;
        }
        machine_count = machine_count
            .checked_add(1)
            .expect("trait machine signature span count overflow");
        input = skip_trait_signature_clauses(rest)?;
        input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let requires = if required_trait_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(required_trait_start, required_trait_count)
    };
    let machines = if machine_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(machine_start, machine_count)
    };
    Ok((
        TraitDefinition {
            is_boundary,
            name,
            requires,
            machines,
        },
        input,
    ))
}

fn parse_trait_requirement<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, omega_syntax_trees::identifier::Identifier> {
    let input = input.take_contextual("requires")?;
    let (required_trait, input) = input.take_identifier()?;
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((required_trait, input))
}

fn skip_trait_signature_clauses<'tokens, 'source>(
    mut input: Input<'tokens, 'source>,
) -> Result<Input<'tokens, 'source>, crate::parse_error::ParseError> {
    while !input.at_punctuation(PunctuationKind::Semicolon) {
        if input.at_punctuation(PunctuationKind::RightBrace) {
            return Err(input.expected_one_of_here(&["`;`"]));
        }

        let (_, rest) = input.expect_token()?;
        input = rest;
    }

    Ok(input)
}
