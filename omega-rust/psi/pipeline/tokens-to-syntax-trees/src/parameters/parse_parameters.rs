use crate::input::token_cursor::{Input, ParseResult};
use crate::type_syntax::parse_type::parse_type_reference_handle_allowing_borrow;
use arena::{Handle, HandleSpan};
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{StateParameterHandle, StateSignature};
use syntax_trees::types::TypeReferenceHandle;
use tokens::{KeywordKind, PunctuationKind};

pub(crate) fn parse_optional_parameters<'tokens, 'source>(
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
            let (parameter, rest) = parse_parameter(syntax_trees, input)?;
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

pub(crate) fn parse_optional_return_type<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    if !input.at_punctuation(PunctuationKind::Arrow) {
        return Ok((TypeReferenceHandle::invalid(), input));
    }

    let input = input.take_punctuation(PunctuationKind::Arrow, "->")?;
    parse_type_reference_handle_allowing_borrow(syntax_trees, input)
}

pub(crate) fn parse_parameter<'tokens, 'source>(
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
                        relevance: language_core::BindingRelevance::Relevant,
                    },
                ),
                input,
            ));
        }

        let (name, input) = input.take_identifier()?;
        // `[erased]` marks the binding occurrence, so it attaches to the
        // parameter name exactly as on a data field.
        let (relevance, input) =
            crate::parameters::binding_properties::parse_binding_relevance_brackets(
                input,
                "parameter",
            )?;
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
                    relevance,
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
                    relevance: language_core::BindingRelevance::Relevant,
                },
            ),
            input,
        ));
    }

    let (name, input) = input.take_identifier()?;
    // Same binding-occurrence contract as the `&name` arm above.
    let (relevance, input) =
        crate::parameters::binding_properties::parse_binding_relevance_brackets(
            input,
            "parameter",
        )?;
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
                relevance,
            }),
        input,
    ))
}

fn parse_parameter_type_reference<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<
    (TypeReferenceHandle, bool, Input<'tokens, 'source>),
    crate::diagnostics::parse_error::ParseError,
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
pub(crate) fn parse_callable_signature<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StateSignature> {
    let (name, input) = input.take_identifier()?;
    let (parameters, input) = parse_optional_parameters(syntax_trees, input)?;
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
