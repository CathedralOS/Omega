use crate::input::token_cursor::{Input, ParseResult};
use crate::type_syntax::parse_type::parse_type_reference_handle_allowing_borrow;
use arena::{Handle, HandleSpan};
use syntax_trees::SyntaxTrees;
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
    let mut parameters = crate::handle_spans::HandleSpanBuilder::new();

    if !input.at_punctuation(PunctuationKind::RightParen) {
        loop {
            let (parameter, rest) = parse_parameter(syntax_trees, input)?;
            let handle = syntax_trees.items.append_state_parameter_handle(parameter);
            parameters.push(handle);
            input = rest;

            let Some(next) = input.take_comma()? else {
                break;
            };

            input = next;

            continue;
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    let parameters = parameters.finish();
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

    let (receiver_access, input) = if input.at_punctuation(PunctuationKind::Ampersand) {
        let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
        let (access, input) = input.take_reference_access()?;
        (Some(access), input)
    } else {
        (None, input)
    };

    if input.at_keyword(KeywordKind::SelfValue) {
        return Err(input.error_here(
            "`self` receivers are retired; declare an explicit named parameter and type",
        ));
    }

    let (name, input) = input.take_identifier()?;
    // `[erased]` marks the binding occurrence, so it attaches to the
    // parameter name exactly as on a data field.
    // Same binding-occurrence contract for the optional `&name` prefix.
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
                is_mutable: receiver_access.is_some_and(|access| access.is_exclusive())
                    || is_leading_mutable
                    || borrowed_mutable,
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
            parameters,
            return_type,
            ..blank_state_signature()
        },
        input,
    ))
}

/// A `StateSignature` carrying only defaults — the parse sites spread it
/// and name the fields their signature really sets.
pub(crate) fn blank_state_signature() -> syntax_trees::item::StateSignature {
    syntax_trees::item::StateSignature {
        name: syntax_trees::identifier::Identifier::generated("*"),
        spelling: None,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        is_default: false,
        parameters: HandleSpan::empty(),
        native_callback_parameters: Vec::new(),
        return_type: Handle::invalid(),
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
    }
}
