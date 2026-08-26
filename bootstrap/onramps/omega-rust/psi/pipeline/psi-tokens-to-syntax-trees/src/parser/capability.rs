use crate::parser::input::{Input, ParseResult};
use crate::parser::proof_fact::parse_proof_facts_until;
use crate::parser::state::parse_state_signature;
use crate::parser::type_reference::parse_type_reference_handle;
use psi_arena::{Handle, HandleSpan};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::item::{
    BoundaryLevel, CapabilityContract, CapabilityContractKind, CapabilityDefinition,
    CapabilityField, CapabilityMember, CapabilityState,
};
use psi_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_capability_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, CapabilityDefinition> {
    let (name, mut input) = input.take_identifier()?;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut member_start = Handle::invalid();
    let mut member_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_keyword(KeywordKind::State) || input.at_keyword(KeywordKind::Entry) {
            input = if input.at_keyword(KeywordKind::State) {
                input.take_keyword(KeywordKind::State, "state")?
            } else {
                input.take_keyword(KeywordKind::Entry, "entry")?
            };
            let (state, rest) = parse_capability_state(syntax_trees, input)?;
            let handle = syntax_trees
                .items
                .append_capability_member(CapabilityMember::State(state));
            if member_count == 0 {
                member_start = handle;
            }
            member_count = member_count
                .checked_add(1)
                .expect("capability member span count overflow");
            input = rest;
        } else {
            let (field_name, rest) = input.take_identifier()?;
            let rest = rest.take_punctuation(PunctuationKind::Colon, ":")?;
            let (type_reference, rest) = parse_type_reference_handle(syntax_trees, rest)?;
            input = if rest.at_punctuation(PunctuationKind::Semicolon) {
                rest.take_punctuation(PunctuationKind::Semicolon, ";")?
            } else {
                rest
            };
            let handle = syntax_trees
                .items
                .append_capability_member(CapabilityMember::Field(CapabilityField {
                    name: field_name,
                    type_reference,
                }));
            if member_count == 0 {
                member_start = handle;
            }
            member_count = member_count
                .checked_add(1)
                .expect("capability member span count overflow");
        }
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let members = if member_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(member_start, member_count)
    };
    Ok((CapabilityDefinition { name, members }, input))
}

fn parse_capability_state<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, CapabilityState> {
    let (signature, mut input) = parse_state_signature(syntax_trees, input)?;
    let mut contract_start = Handle::invalid();
    let mut contract_count = 0u32;

    if input.at_contextual("where") {
        input = input.take_contextual("where")?;
    }

    if input.at_punctuation(PunctuationKind::LeftBrace) {
        input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        while !input.at_punctuation(PunctuationKind::RightBrace) {
            let (contract, rest) = parse_capability_contract(syntax_trees, input)?;
            let handle = syntax_trees.items.append_capability_contract(contract);
            if contract_count == 0 {
                contract_start = handle;
            }
            contract_count = contract_count
                .checked_add(1)
                .expect("capability contract span count overflow");
            input = rest;
        }
        input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    } else {
        while !(input.at_keyword(KeywordKind::State)
            || input.at_keyword(KeywordKind::Entry)
            || input.at_punctuation(PunctuationKind::RightBrace)
            || input.tokens.is_empty())
        {
            let (contract, rest) = parse_capability_contract(syntax_trees, input)?;
            let handle = syntax_trees.items.append_capability_contract(contract);
            if contract_count == 0 {
                contract_start = handle;
            }
            contract_count = contract_count
                .checked_add(1)
                .expect("capability contract span count overflow");
            input = rest;
        }
    }

    let contracts = if contract_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(contract_start, contract_count)
    };

    Ok((
        CapabilityState {
            signature,
            contracts,
        },
        input,
    ))
}

fn parse_capability_contract<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, CapabilityContract> {
    if input.at_contextual("requires") {
        let clause_input = input;
        let input = input.take_contextual("requires")?;
        let ((facts, token_count), input) =
            parse_proof_facts_until(syntax_trees, input, capability_contract_terminator)?;
        let input = take_optional_semicolon(input)?;
        return Ok((
            CapabilityContract {
                kind: CapabilityContractKind::Requires,
                binding: None,
                facts,
                token_count,
                source_span: clause_input.source_span_until(input),
            },
            input,
        ));
    }

    if input.at_contextual("ensures") {
        let clause_input = input;
        let input = input.take_contextual("ensures")?;
        let ((facts, token_count), input) =
            parse_proof_facts_until(syntax_trees, input, capability_contract_terminator)?;
        let input = take_optional_semicolon(input)?;
        return Ok((
            CapabilityContract {
                kind: CapabilityContractKind::Ensures,
                binding: None,
                facts,
                token_count,
                source_span: clause_input.source_span_until(input),
            },
            input,
        ));
    }

    if input.at_contextual("boundary") {
        let clause_input = input;
        let input = input.take_contextual("boundary")?;
        let (boundary_level, input) = parse_boundary_level(input)?;
        let input = take_optional_semicolon(input)?;
        return Ok((
            CapabilityContract {
                kind: CapabilityContractKind::Boundary(boundary_level),
                binding: None,
                facts: HandleSpan::empty(),
                token_count: 1,
                source_span: clause_input.source_span_until(input),
            },
            input,
        ));
    }

    Err(input.error_here("expected capability contract"))
}

fn take_optional_semicolon<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> Result<Input<'tokens, 'source>, crate::parse_error::ParseError> {
    if input.at_punctuation(PunctuationKind::Semicolon) {
        input.take_punctuation(PunctuationKind::Semicolon, ";")
    } else {
        Ok(input)
    }
}

fn capability_contract_terminator(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::Semicolon)
        || input.at_punctuation(PunctuationKind::RightBrace)
        || input.at_keyword(KeywordKind::State)
        || input.at_keyword(KeywordKind::Entry)
        || input.at_contextual("requires")
        || input.at_contextual("ensures")
        || input.at_contextual("boundary")
        || input.tokens.is_empty()
}

fn parse_boundary_level<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, BoundaryLevel> {
    if input.at_keyword(KeywordKind::Host) {
        let input = input.take_keyword(KeywordKind::Host, "host")?;
        Ok((BoundaryLevel::Host, input))
    } else {
        let (name, input) = input.take_identifier()?;
        Ok((BoundaryLevel::Named(name), input))
    }
}
