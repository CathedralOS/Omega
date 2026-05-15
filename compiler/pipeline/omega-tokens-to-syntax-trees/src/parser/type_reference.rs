use crate::parse_error::ParseError;
use crate::parser::expression::parse_expression_handle_without_struct_literals;
use crate::parser::input::{Input, ParseResult};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_type_reference_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    if input.at_punctuation(PunctuationKind::LeftParen) {
        let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
        return Ok((syntax_trees.type_references.insert_unit(), input));
    }

    if input.at_punctuation(PunctuationKind::LeftBracket) {
        let input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
        let (element_type, input) = parse_type_reference_handle(syntax_trees, input)?;
        if input.at_punctuation(PunctuationKind::Semicolon) {
            let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            let (length, input) = input.take_integer()?;
            let length = usize::try_from(length)
                .map_err(|_| input.error_here("expected non-negative array length"))?;
            let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
            return Ok((
                syntax_trees
                    .type_references
                    .insert(TypeReferenceNode::FixedArray {
                        element_type,
                        length,
                    }),
                input,
            ));
        }

        let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
        return Ok((
            syntax_trees
                .type_references
                .insert(TypeReferenceNode::Slice { element_type }),
            input,
        ));
    }

    if input.at_keyword(KeywordKind::SelfType) {
        let input = input.take_keyword(KeywordKind::SelfType, "Self")?;
        return Ok((syntax_trees.type_references.insert_self_type(), input));
    }

    let (base_name, mut input) = input.take_identifier()?;
    let mut type_reference = if input.at_punctuation(PunctuationKind::Less) {
        input = input.take_punctuation(PunctuationKind::Less, "<")?;
        let mut argument_start = Handle::invalid();
        let mut argument_count = 0u32;

        loop {
            let (argument, rest) = parse_type_reference_handle(syntax_trees, input)?;
            let handle = syntax_trees
                .type_references
                .append_type_reference_handle(argument);
            if argument_count == 0 {
                argument_start = handle;
            }
            argument_count = argument_count
                .checked_add(1)
                .expect("type reference argument span count overflow");
            input = rest;

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }

            break;
        }

        input = input.take_punctuation(PunctuationKind::Greater, ">")?;
        let arguments = if argument_count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(argument_start, argument_count)
        };
        syntax_trees
            .type_references
            .insert(TypeReferenceNode::Generic {
                base_name,
                arguments,
            })
    } else {
        syntax_trees.type_references.insert_named(base_name)
    };

    if input.at_punctuation(PunctuationKind::LeftBracket) {
        let (constraints, rest) = parse_type_constraint_handles(syntax_trees, input)?;
        input = rest;
        type_reference = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Constrained {
                base_type: type_reference,
                constraints,
            });
    }

    Ok((type_reference, input))
}

pub(super) fn parse_type_reference_handle_allowing_borrow<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    let (is_reference, is_mutable, input) = if input.at_punctuation(PunctuationKind::Ampersand) {
        let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
        if input.at_contextual("mut") {
            (true, true, input.take_contextual("mut")?)
        } else {
            (true, false, input)
        }
    } else {
        (false, false, input)
    };

    let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
    let type_reference = if is_reference {
        syntax_trees
            .type_references
            .insert(TypeReferenceNode::Reference {
                referee: type_reference,
                is_mutable,
            })
    } else {
        type_reference
    };

    Ok((type_reference, input))
}

pub(super) fn parse_type_constraint_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<TypeConstraintNode>> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
    let mut constraint_start = Handle::invalid();
    let mut constraint_count = 0u32;

    if !input.at_punctuation(PunctuationKind::RightBracket) {
        loop {
            let constraint = if input.at_contextual("range") {
                input = input.take_contextual("range")?;
                input = input.take_punctuation(PunctuationKind::Less, "<")?;
                let (minimum, rest) = parse_expression_handle_until_punctuation(
                    syntax_trees,
                    input,
                    PunctuationKind::Comma,
                )?;
                input = rest.take_punctuation(PunctuationKind::Comma, ",")?;
                let (maximum, rest) = parse_expression_handle_until_punctuation(
                    syntax_trees,
                    input,
                    PunctuationKind::Greater,
                )?;
                input = rest.take_punctuation(PunctuationKind::Greater, ">")?;
                TypeConstraintNode::Range { minimum, maximum }
            } else {
                let (name, rest) = input.take_identifier()?;
                input = rest;
                TypeConstraintNode::Named(name)
            };

            let handle = syntax_trees.type_references.append_constraint(constraint);
            if constraint_count == 0 {
                constraint_start = handle;
            }
            constraint_count = constraint_count
                .checked_add(1)
                .expect("type constraint span count overflow");

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }

            break;
        }
    }

    input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
    let constraints = if constraint_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(constraint_start, constraint_count)
    };
    Ok((constraints, input))
}

fn parse_expression_handle_until_punctuation<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    delimiter: PunctuationKind,
) -> Result<
    (
        omega_syntax_trees::expression::ExpressionHandle,
        Input<'tokens, 'source>,
    ),
    ParseError,
> {
    let (expression_input, rest) =
        input.split_at_top_level_punctuation(delimiter, "expected constrained type delimiter")?;
    let (expression, rest_after_expression) =
        parse_expression_handle_without_struct_literals(syntax_trees, expression_input)?;

    if !rest_after_expression.tokens.is_empty() {
        return Err(rest_after_expression.error_here("expected constrained type expression"));
    }

    Ok((expression, rest))
}
