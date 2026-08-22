use crate::parser::context::StateKind;
use crate::parser::input::{Input, ParseResult};
use crate::parser::statement::{
    parse_asm_block_statement_handles, parse_statement_handle,
    reject_retired_evidence_package_destructure, try_parse_atomic_compare_exchange_let,
    try_parse_atomic_fetch_let, try_parse_atomic_swap_let, try_parse_destructure_let,
    try_parse_evidence_package_destructure,
};
use crate::parser::transition::parse_transition_block_handles;
use crate::parser::type_reference::parse_type_reference_handle_allowing_borrow;
use psi_arena::{Handle, HandleSpan};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{
    CapabilityContract, CapabilityContractKind, State, StateParameterHandle, StateSignature,
};
use psi_syntax_trees::types::TypeReferenceHandle;
use psi_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_state_signature<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StateSignature> {
    let (name, input) = input.take_identifier()?;
    let (parameters, input) = parse_optional_state_parameters(syntax_trees, input)?;
    let (return_type, input) = parse_optional_return_type(syntax_trees, input)?;

    Ok((
        StateSignature {
            name,
            spelling: None,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            is_default: false,
            parameters,
            return_type,
            service_reach_is_installation_bound: false,
            service_reaches: HandleSpan::empty(),
            invokes: HandleSpan::empty(),
            suspends: false,
            blocks: false,
            contracts: HandleSpan::empty(),
            default_body: HandleSpan::empty(),
            terminates_guarantee: false,
        },
        input,
    ))
}

pub(super) fn parse_state<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    kind: StateKind,
) -> ParseResult<'tokens, 'source, State> {
    let (name, input) =
        if kind.allows_implicit_entry_name() && input.at_punctuation(PunctuationKind::LeftParen) {
            (Identifier::generated("entry"), input)
        } else {
            input.take_identifier()?
        };

    let (parameters, input) = parse_optional_state_parameters(syntax_trees, input)?;
    let (return_type, input) = parse_optional_return_type(syntax_trees, input)?;
    let (contracts, mut input) = parse_state_arrival_contracts(syntax_trees, input)?;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut statement_start = Handle::invalid();
    let mut statement_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        reject_retired_evidence_package_destructure(input)?;
        if input.at_punctuation(PunctuationKind::Arrow) {
            return Err(input.error_here(
                "explicit state bodies must use the `transition` keyword; bare `->` transitions are only allowed in implicit entry",
            ));
        } else if input.at_keyword(KeywordKind::Transition) || input.at_keyword(KeywordKind::Match)
        {
            let next = if input.at_keyword(KeywordKind::Transition) {
                input.take_keyword(KeywordKind::Transition, "transition")?
            } else {
                input.take_keyword(KeywordKind::Match, "match")?
            };
            let (new_statements, rest) = parse_transition_block_handles(syntax_trees, next)?;
            if !new_statements.is_empty() {
                if statement_count == 0 {
                    statement_start = new_statements.start();
                }
                statement_count = statement_count
                    .checked_add(new_statements.count())
                    .expect("state statement span count overflow");
            }
            input = rest;
        } else if input.at_contextual("asm") {
            let (new_statements, rest) = parse_asm_block_statement_handles(syntax_trees, input)?;
            if statement_count == 0 {
                statement_start = new_statements.start();
            }
            statement_count = statement_count
                .checked_add(new_statements.count())
                .expect("state statement span count overflow");
            input = rest;
        // ATOMICS STAGE 1 (ch17, M3): `let name: T = place.fetch_add(n, ord);`
        // expands to two statements (capture prior + increment).
        // RECORD PATTERNS IN LET POSITION (owner spec 2026-07-18):
        // `let { x, y as h, z as _ } = place;` expands to the marker +
        // per-field lets.
        } else if let Some((new_statements, rest)) =
            try_parse_evidence_package_destructure(syntax_trees, input)
        {
            if statement_count == 0 {
                statement_start = new_statements.start();
            }
            statement_count = statement_count
                .checked_add(new_statements.count())
                .expect("state statement span count overflow");
            input = rest;
        } else if let Some((new_statements, rest)) = try_parse_destructure_let(syntax_trees, input)
        {
            if statement_count == 0 {
                statement_start = new_statements.start();
            }
            statement_count = statement_count
                .checked_add(new_statements.count())
                .expect("state statement span count overflow");
            input = rest;
        } else if let Some((new_statements, rest)) = try_parse_atomic_fetch_let(syntax_trees, input)
        {
            if statement_count == 0 {
                statement_start = new_statements.start();
            }
            statement_count = statement_count
                .checked_add(new_statements.count())
                .expect("state statement span count overflow");
            input = rest;
        } else if let Some((new_statements, rest)) = try_parse_atomic_swap_let(syntax_trees, input)
        {
            if statement_count == 0 {
                statement_start = new_statements.start();
            }
            statement_count = statement_count
                .checked_add(new_statements.count())
                .expect("state statement span count overflow");
            input = rest;
        // ATOMICS STAGE 1 (ch17, M4): `let name: T = place.compare_exchange(expected, new_val, succ_ord, fail_ord);`
        // expands to two statements (capture prior + conditional swap).
        } else if let Some((new_statements, rest)) =
            try_parse_atomic_compare_exchange_let(syntax_trees, input)
        {
            if statement_count == 0 {
                statement_start = new_statements.start();
            }
            statement_count = statement_count
                .checked_add(new_statements.count())
                .expect("state statement span count overflow");
            input = rest;
        } else {
            let (statement, rest) = parse_statement_handle(syntax_trees, input)?;
            let handle = syntax_trees.items.append_statement_handle(statement);
            if statement_count == 0 {
                statement_start = handle;
            }
            statement_count = statement_count
                .checked_add(1)
                .expect("state statement span count overflow");
            input = rest;
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let statements = if statement_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(statement_start, statement_count)
    };
    Ok((
        State {
            name,
            parameters,
            return_type,
            contracts,
            statements,
        },
        input,
    ))
}

/// Parse a state's explicit arrival contract. Unlike a machine signature, a
/// state has no exit contract or behavior surface: `requires` is the induction
/// hypothesis that every named incoming edge must establish.
fn parse_state_arrival_contracts<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<CapabilityContract>> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    while input.at_contextual("requires") {
        input = input.take_contextual("requires")?;
        let (binding, fact_input) = if let Ok((binding, after_binding)) = input.take_identifier()
            && after_binding.at_punctuation(PunctuationKind::Colon)
        {
            (
                Some(binding),
                after_binding.take_punctuation(PunctuationKind::Colon, ":")?,
            )
        } else {
            (None, input)
        };
        let ((facts, token_count), rest) =
            crate::parser::proof_fact::parse_proof_facts_until_with_machine_semicolon(
                syntax_trees,
                fact_input,
                |input| {
                    input.at_punctuation(PunctuationKind::LeftBrace)
                        || input.at_contextual("requires")
                        || input.at_contextual("ensures")
                        || input.at_contextual("reaches")
                        || input.at_contextual("effects")
                        || input.at_contextual("terminates")
                        || input.tokens.is_empty()
                },
                true,
            )?;
        if binding.is_some() && facts.count() != 1 {
            return Err(fact_input
                .error_here("a named state requires clause must contain exactly one proposition"));
        }
        let handle = syntax_trees
            .items
            .append_capability_contract(CapabilityContract {
                kind: CapabilityContractKind::Requires,
                binding,
                facts,
                token_count,
            });
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("state arrival contract span count overflow");
        input = rest;
    }

    if input.at_contextual("ensures")
        || input.at_contextual("reaches")
        || input.at_contextual("effects")
        || input.at_contextual("terminates")
    {
        return Err(input.error_here(
            "state signatures admit only arrival `requires`; put exit guarantees, service reach, and termination policy on the owning machine",
        ));
    }

    Ok((
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        },
        input,
    ))
}

pub(super) fn parse_optional_state_parameters<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<StateParameterHandle>> {
    if !input.at_punctuation(PunctuationKind::LeftParen) {
        return Ok((HandleSpan::empty(), input));
    }

    let mut input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
    let mut parameter_start = Handle::invalid();
    let mut parameter_count = 0u32;

    if !input.at_punctuation(PunctuationKind::RightParen) {
        loop {
            let (parameter, rest) = parse_state_parameter(syntax_trees, input)?;
            let handle = syntax_trees.items.append_state_parameter_handle(parameter);
            if parameter_count == 0 {
                parameter_start = handle;
            }
            parameter_count = parameter_count
                .checked_add(1)
                .expect("state parameter span count overflow");
            input = rest;

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }

            break;
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    let parameters = if parameter_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(parameter_start, parameter_count)
    };
    Ok((parameters, input))
}

pub(super) fn parse_optional_return_type<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    if !input.at_punctuation(PunctuationKind::Arrow) {
        return Ok((TypeReferenceHandle::invalid(), input));
    }

    let input = input.take_punctuation(PunctuationKind::Arrow, "->")?;
    parse_type_reference_handle_allowing_borrow(syntax_trees, input)
}

fn parse_state_parameter<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StateParameterHandle> {
    let (is_const, input) = if input.at_contextual("const") {
        (true, input.take_contextual("const")?)
    } else {
        (false, input)
    };
    let (is_leading_mutable, input) = if input.at_contextual("mut") {
        (true, input.take_contextual("mut")?)
    } else {
        (false, input)
    };

    if input.at_punctuation(PunctuationKind::Ampersand) {
        let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
        let (is_mutable, input) = if input.at_contextual("mut") {
            (true, input.take_contextual("mut")?)
        } else {
            (false, input)
        };

        if input.at_keyword(KeywordKind::SelfValue) {
            let input = input.take_keyword(KeywordKind::SelfValue, "self")?;
            // Preserve the receiver's ownership mode in the type graph.
            // `is_self` identifies receiver binding and `is_mutable` drives
            // borrow access, but neither can distinguish shared `&self` from
            // consuming `self`. The reference node is the canonical ownership
            // distinction used by permission-event discovery downstream.
            let self_type = syntax_trees.tables.type_references.insert_self_type();
            let type_reference = syntax_trees
                .tables
                .type_references
                .insert_reference(self_type, is_mutable || is_leading_mutable);

            return Ok((
                syntax_trees.items.insert_state_parameter_node(
                    psi_syntax_trees::item::StateParameterNode {
                        name: Identifier::generated("self"),
                        type_reference,
                        is_const,
                        is_mutable: is_mutable || is_leading_mutable,
                        is_self: true,
                    },
                ),
                input,
            ));
        }

        let (name, input) = input.take_identifier()?;
        let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
        let (type_reference, borrowed_mutable, input) =
            parse_parameter_type_reference(syntax_trees, input)?;
        return Ok((
            syntax_trees.items.insert_state_parameter_node(
                psi_syntax_trees::item::StateParameterNode {
                    name,
                    type_reference,
                    is_const,
                    is_mutable: is_mutable || is_leading_mutable || borrowed_mutable,
                    is_self: false,
                },
            ),
            input,
        ));
    }

    if input.at_keyword(KeywordKind::SelfValue) {
        let input = input.take_keyword(KeywordKind::SelfValue, "self")?;
        let type_reference = syntax_trees.tables.type_references.insert_self_type();

        return Ok((
            syntax_trees.items.insert_state_parameter_node(
                psi_syntax_trees::item::StateParameterNode {
                    name: Identifier::generated("self"),
                    type_reference,
                    is_const,
                    is_mutable: is_leading_mutable,
                    is_self: true,
                },
            ),
            input,
        ));
    }

    let (name, input) = input.take_identifier()?;
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (type_reference, borrowed_mutable, input) =
        parse_parameter_type_reference(syntax_trees, input)?;
    Ok((
        syntax_trees.items.insert_state_parameter_node(
            psi_syntax_trees::item::StateParameterNode {
                name,
                type_reference,
                is_const,
                is_mutable: is_leading_mutable || borrowed_mutable,
                is_self: false,
            },
        ),
        input,
    ))
}

fn parse_parameter_type_reference<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<(TypeReferenceHandle, bool, Input<'tokens, 'source>), crate::parse_error::ParseError> {
    if !input.at_punctuation(PunctuationKind::Ampersand) {
        let (type_reference, input) =
            parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
        return Ok((type_reference, false, input));
    }

    let (type_reference, input) = parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
    let borrowed_mutable = matches!(
        syntax_trees.type_references.type_reference(type_reference),
        psi_syntax_trees::types::TypeReferenceNode::Reference {
            is_mutable: true,
            ..
        }
    );
    Ok((type_reference, borrowed_mutable, input))
}
