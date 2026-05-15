use crate::parser::context::StateKind;
use crate::parser::input::{Input, ParseResult};
use crate::parser::statement::parse_statement_handle;
use crate::parser::transition::{
    parse_transition_block_handles, parse_transition_statement_handle,
};
use crate::parser::type_reference::{
    parse_type_reference_handle, parse_type_reference_handle_allowing_borrow,
};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{State, StateParameterHandle, StateSignature};
use omega_syntax_trees::types::TypeReferenceHandle;
use omega_tokens::{KeywordKind, PunctuationKind};

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
            parameters,
            return_type,
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
    let (return_type, mut input) = parse_optional_return_type(syntax_trees, input)?;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut statement_start = Handle::invalid();
    let mut statement_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_punctuation(PunctuationKind::Arrow) {
            let next = input.take_punctuation(PunctuationKind::Arrow, "->")?;
            let (statement, rest) = parse_transition_statement_handle(syntax_trees, next)?;
            let handle = syntax_trees.items.append_statement_handle(statement);
            if statement_count == 0 {
                statement_start = handle;
            }
            statement_count = statement_count
                .checked_add(1)
                .expect("state statement span count overflow");
            input = rest;
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
            statements,
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

            return Ok((
                syntax_trees.items.insert_state_parameter_node(
                    omega_syntax_trees::item::StateParameterNode {
                        name: Identifier::generated("self"),
                        type_reference: syntax_trees.type_references.insert_self_type(),
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
                omega_syntax_trees::item::StateParameterNode {
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

        return Ok((
            syntax_trees.items.insert_state_parameter_node(
                omega_syntax_trees::item::StateParameterNode {
                    name: Identifier::generated("self"),
                    type_reference: syntax_trees.type_references.insert_self_type(),
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
            omega_syntax_trees::item::StateParameterNode {
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

    let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
    let (borrowed_mutable, input) = if input.at_contextual("mut") {
        (true, input.take_contextual("mut")?)
    } else {
        (false, input)
    };
    let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
    Ok((
        syntax_trees.type_references.insert(
            omega_syntax_trees::types::TypeReferenceNode::Reference {
                referee: type_reference,
                is_mutable: borrowed_mutable,
            },
        ),
        borrowed_mutable,
        input,
    ))
}
