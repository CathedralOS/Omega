use crate::parser::expression::parse_expression_handle_without_struct_literals;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::proof_fact::parse_proof_facts_until;
use crate::parser::type_reference::parse_type_reference_handle;
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{BoundaryLevel, CapabilityContract, CapabilityContractKind};
use omega_tokens::PunctuationKind;

type MachineClauses = (
    bool,
    HandleSpan<omega_syntax_trees::expression::ExpressionHandle>,
    HandleSpan<Identifier>,
    HandleSpan<Identifier>,
    HandleSpan<CapabilityContract>,
);

type DecreasesClause = (
    HandleSpan<omega_syntax_trees::expression::ExpressionHandle>,
    HandleSpan<Identifier>,
);

pub(super) fn parse_machine_clauses<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, MachineClauses> {
    let mut terminates = false;
    let mut decreases = HandleSpan::empty();
    let mut decrease_order = HandleSpan::empty();
    let mut effect_start = Handle::invalid();
    let mut effect_count = 0u32;
    let mut contract_start = Handle::invalid();
    let mut contract_count = 0u32;

    while !input.at_punctuation(PunctuationKind::LeftBrace) {
        if input.at_contextual("terminates") {
            input = input.take_contextual("terminates")?;
            terminates = true;
            if starts_termination_clause_block(input) {
                let ((block_decreases, block_order), rest) =
                    parse_termination_clause_block(syntax_trees, input)?;
                input = rest;
                if !block_decreases.is_empty() {
                    decreases = block_decreases;
                    decrease_order = block_order;
                }
            }
            continue;
        }

        if input.at_contextual("decreases") {
            let ((clause_decreases, clause_order), rest) =
                parse_decreases_clause(syntax_trees, input)?;
            input = rest;
            decreases = clause_decreases;
            decrease_order = clause_order;

            continue;
        }

        if input.at_contextual("effects") {
            input = input.take_contextual("effects")?;
            while !input.at_punctuation(PunctuationKind::LeftBrace)
                && !input.at_contextual("requires")
                && !input.at_contextual("ensures")
                && !input.at_contextual("terminates")
                && !input.at_contextual("decreases")
                && !input.at_contextual("boundary")
                && !input.at_contextual("where")
                && !input.at_contextual("satisfies")
            {
                let (effect, rest) = input.take_identifier()?;
                let handle = syntax_trees.items.append_identifier_path_member(effect);
                if effect_count == 0 {
                    effect_start = handle;
                }
                effect_count = effect_count
                    .checked_add(1)
                    .expect("machine effect span count overflow");
                input = rest;

                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                }
            }
            continue;
        }

        if input.at_contextual("boundary") {
            let (boundary, rest) = parse_boundary_clause(input)?;
            input = rest;
            let handle = syntax_trees
                .items
                .append_capability_contract(CapabilityContract {
                    kind: CapabilityContractKind::Boundary(boundary),
                    facts: HandleSpan::empty(),
                    token_count: 2,
                });
            if contract_count == 0 {
                contract_start = handle;
            }
            contract_count = contract_count
                .checked_add(1)
                .expect("machine contract span count overflow");
            continue;
        }

        if input.at_contextual("requires") || input.at_contextual("ensures") {
            let kind = if input.at_contextual("requires") {
                input = input.take_contextual("requires")?;
                CapabilityContractKind::Requires
            } else {
                input = input.take_contextual("ensures")?;
                CapabilityContractKind::Ensures
            };
            let ((facts, token_count), rest) =
                parse_proof_facts_until(syntax_trees, input, |input| {
                    input.at_punctuation(PunctuationKind::LeftBrace)
                        || input.at_contextual("requires")
                        || input.at_contextual("ensures")
                        || input.at_contextual("terminates")
                        || input.at_contextual("decreases")
                        || input.at_contextual("effects")
                        || input.at_contextual("boundary")
                        || input.at_contextual("where")
                        || input.at_contextual("satisfies")
                        || input.tokens.is_empty()
                })?;
            let handle = syntax_trees
                .items
                .append_capability_contract(CapabilityContract {
                    kind,
                    facts,
                    token_count,
                });
            if contract_count == 0 {
                contract_start = handle;
            }
            contract_count = contract_count
                .checked_add(1)
                .expect("machine contract span count overflow");
            input = rest;
            continue;
        }

        let (_, rest) = input.expect_token()?;
        input = rest;
    }

    let effects = if effect_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(effect_start, effect_count)
    };
    let contracts = if contract_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(contract_start, contract_count)
    };
    Ok((
        (terminates, decreases, decrease_order, effects, contracts),
        input,
    ))
}

fn parse_boundary_clause<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, BoundaryLevel> {
    let input = input.take_contextual("boundary")?;
    if input.at_contextual("host") {
        let input = input.take_contextual("host")?;
        return Ok((BoundaryLevel::Host, input));
    }

    let (name, input) = input.take_identifier()?;
    Ok((BoundaryLevel::Named(name), input))
}

fn starts_termination_clause_block(input: Input<'_, '_>) -> bool {
    if !input.at_punctuation(PunctuationKind::LeftBrace) || input.tokens.is_empty() {
        return false;
    }

    let rest = Input::new(input.source_id, &input.tokens[1..]);
    rest.tokens.first().is_some_and(|token| {
        matches!(token.punctuation(), Some(PunctuationKind::RightBrace))
            || matches!(token.lexeme.as_str(), "decreases" | "increases")
    })
}

fn parse_termination_clause_block<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DecreasesClause> {
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut decreases = HandleSpan::empty();
    let mut decrease_order = HandleSpan::empty();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_contextual("decreases") {
            let ((clause_decreases, clause_order), rest) =
                parse_decreases_clause(syntax_trees, input)?;
            input = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;
            decreases = clause_decreases;
            decrease_order = clause_order;
            continue;
        }

        if input.at_contextual("increases") {
            return Err(input.error_here(
                "`increases` termination clauses are not implemented yet; use `decreases` for now",
            ));
        }

        return Err(input.expected_one_of_here(&["`decreases`", "`}`"]));
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok(((decreases, decrease_order), input))
}

fn parse_decreases_clause<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DecreasesClause> {
    let input = input.take_contextual("decreases")?;
    let (expression, mut rest) =
        parse_expression_handle_without_struct_literals(syntax_trees, input)?;
    let decreases = syntax_trees
        .expressions
        .insert_expression_handles([expression]);
    let mut decrease_order = HandleSpan::empty();

    if rest.at_punctuation(PunctuationKind::Arrow) {
        rest = rest.take_punctuation(PunctuationKind::Arrow, "->")?;
        let (order, next) = parse_path_handle_span(rest, |member| {
            syntax_trees.items.append_identifier_path_member(member)
        })?;
        decrease_order = order;
        rest = next;
    }

    Ok(((decreases, decrease_order), rest))
}

pub(super) fn parse_satisfies_traits<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<Identifier>> {
    if !input.at_contextual("satisfies") {
        return Ok((HandleSpan::empty(), input));
    }

    input = input.take_contextual("satisfies")?;
    let mut trait_start = Handle::invalid();
    let mut trait_count = 0u32;

    loop {
        let (trait_name, rest) = input.take_identifier()?;
        let rest = parse_optional_satisfies_type_arguments(syntax_trees, rest)?;
        let handle = syntax_trees.items.append_identifier_path_member(trait_name);
        if trait_count == 0 {
            trait_start = handle;
        }
        trait_count = trait_count
            .checked_add(1)
            .expect("machine satisfies span count overflow");
        input = rest;

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }

        break;
    }

    let satisfies = if trait_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(trait_start, trait_count)
    };
    Ok((satisfies, input))
}

fn parse_optional_satisfies_type_arguments<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> Result<Input<'tokens, 'source>, crate::parse_error::ParseError> {
    if !input.at_punctuation(PunctuationKind::Less) {
        return Ok(input);
    }

    input = input.take_punctuation(PunctuationKind::Less, "<")?;
    loop {
        let (_argument, rest) = parse_type_reference_handle(syntax_trees, input)?;
        input = rest;

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }

        return input.take_punctuation(PunctuationKind::Greater, ">");
    }
}
