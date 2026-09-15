use crate::parser::input::{Input, ParseResult};
use crate::parser::machine::body::statements::{BodyKind, parse_statements};
use crate::parser::type_reference::parse_type_reference_handle_allowing_borrow;
use arena::{Handle, HandleSpan};
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{
    CapabilityContract, CapabilityContractKind, State, StateParameterHandle, StateSignature,
};
use syntax_trees::types::TypeReferenceHandle;
use tokens::{KeywordKind, PunctuationKind};

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
            native_callback_parameters: Vec::new(),
            return_type,
            service_reach_is_installation_bound: false,
            service_reach_keyword_source_spans: Vec::new(),
            service_reaches: HandleSpan::empty(),
            invokes: HandleSpan::empty(),
            suspends_keyword_source_spans: Vec::new(),
            blocks_keyword_source_spans: Vec::new(),
            suspends: false,
            blocks: false,
            contracts: HandleSpan::empty(),
            default_body: HandleSpan::empty(),
            terminates_guarantee: false,
            where_facts: HandleSpan::empty(),
        },
        input,
    ))
}

pub(super) fn parse_state<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, State> {
    let (name, input) = input.take_identifier()?;

    let (parameters, input) = parse_optional_state_parameters(syntax_trees, input)?;
    let (return_type, input) = parse_optional_return_type(syntax_trees, input)?;
    let (contracts, input) = parse_state_arrival_contracts(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let (statements, input) = parse_statements(syntax_trees, input, BodyKind::ExplicitState)?;
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
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
        let keyword_source_span = Some(input.current_source_span());
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
                keyword_source_span,
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

pub(super) fn parse_state_parameter<'tokens, 'source>(
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
        let (access, input) = if input.at_contextual("mut") {
            (
                language_core::ReferenceAccess::Mutable,
                input.take_contextual("mut")?,
            )
        } else if input.at_contextual("write") {
            (
                language_core::ReferenceAccess::WriteOnly,
                input.take_contextual("write")?,
            )
        } else {
            (language_core::ReferenceAccess::Shared, input)
        };

        if input.at_keyword(KeywordKind::SelfValue) {
            let input = input.take_keyword(KeywordKind::SelfValue, "self")?;
            // Preserve the receiver's ownership mode in the type graph.
            // `is_self` identifies receiver binding and `is_mutable` drives
            // borrow access, but neither can distinguish shared `&self` from
            // consuming `self`. The reference node is the canonical ownership
            // distinction used by permission-event discovery downstream.
            let self_type = syntax_trees.tables.type_references.insert_self_type();
            let type_reference = syntax_trees.tables.type_references.insert_reference(
                self_type,
                if is_leading_mutable {
                    language_core::ReferenceAccess::Mutable
                } else {
                    access
                },
            );

            return Ok((
                syntax_trees.items.insert_state_parameter_node(
                    syntax_trees::item::StateParameterNode {
                        name: Identifier::generated("self"),
                        type_reference,
                        is_const,
                        is_mutable: access.is_exclusive() || is_leading_mutable,
                        is_self: true,
                    },
                ),
                input,
            ));
        }

        let (name, input) = input.take_identifier()?;
        // `[erased]` is spec-legal on any authored binding occurrence; the
        // signature parameter node cannot retain relevance yet, so the
        // bracket fails closed here rather than parsing and dropping.
        let ((), input) =
            crate::parser::data::parse_unsupported_binding_relevance_brackets(input, "parameter")?;
        let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
        let (type_reference, borrowed_mutable, input) =
            parse_parameter_type_reference(syntax_trees, input)?;
        return Ok((
            syntax_trees.items.insert_state_parameter_node(
                syntax_trees::item::StateParameterNode {
                    name,
                    type_reference,
                    is_const,
                    is_mutable: access.is_exclusive() || is_leading_mutable || borrowed_mutable,
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
                syntax_trees::item::StateParameterNode {
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
    // Same binding-occurrence contract as the `&name` arm above.
    let ((), input) =
        crate::parser::data::parse_unsupported_binding_relevance_brackets(input, "parameter")?;
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (type_reference, borrowed_mutable, input) =
        parse_parameter_type_reference(syntax_trees, input)?;
    Ok((
        syntax_trees
            .items
            .insert_state_parameter_node(syntax_trees::item::StateParameterNode {
                name,
                type_reference,
                is_const,
                is_mutable: is_leading_mutable || borrowed_mutable,
                is_self: false,
            }),
        input,
    ))
}

fn parse_parameter_type_reference<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<
    (TypeReferenceHandle, bool, Input<'tokens, 'source>),
    crate::parser::parse_error::ParseError,
> {
    if !input.at_punctuation(PunctuationKind::Ampersand) {
        let (type_reference, input) =
            parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
        return Ok((type_reference, false, input));
    }

    let (type_reference, input) = parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
    let borrowed_mutable = matches!(
        syntax_trees.type_references.type_reference(type_reference),
        syntax_trees::types::TypeReferenceNode::Reference {
            access: language_core::ReferenceAccess::Mutable
                | language_core::ReferenceAccess::WriteOnly,
            ..
        }
    );
    Ok((type_reference, borrowed_mutable, input))
}
