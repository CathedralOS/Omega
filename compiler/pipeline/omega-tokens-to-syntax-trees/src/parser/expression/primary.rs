use crate::parser::context::ExpressionContext;
use crate::parser::expression::{parse_expression_handle, parse_expression_handle_in};
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, TableStructLiteral, TableStructLiteralField,
};
use omega_syntax_trees::identifier::Identifier;
use omega_tokens::{KeywordKind, NumericLiteralKind, PunctuationKind, TokenKind};

pub(super) fn parse_primary_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    if input.at_keyword(KeywordKind::True) {
        let input = input.take_keyword(KeywordKind::True, "true")?;
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::Boolean(true)),
            input,
        ));
    }

    if input.at_keyword(KeywordKind::False) {
        let input = input.take_keyword(KeywordKind::False, "false")?;
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::Boolean(false)),
            input,
        ));
    }

    if input.at_keyword(KeywordKind::SelfValue) {
        let input = input.take_keyword(KeywordKind::SelfValue, "self")?;
        return Ok((
            syntax_trees.expressions.insert(ExpressionNode::SelfValue),
            input,
        ));
    }

    if input.tokens.first().is_some_and(|token| {
        matches!(
            token.kind,
            TokenKind::NumericLiteral(NumericLiteralKind::Integer(_))
        )
    }) {
        let (value, input) = input.take_integer()?;
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::Integer(value)),
            input,
        ));
    }

    if input.tokens.first().is_some_and(|token| {
        matches!(
            token.kind,
            TokenKind::NumericLiteral(NumericLiteralKind::Float(_))
        )
    }) {
        let (value, input) = input.take_float_text()?;
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::Float(value)),
            input,
        ));
    }

    if input
        .tokens
        .first()
        .is_some_and(|token| token.kind == TokenKind::StringLiteral)
    {
        let (value, input) = input.take_string()?;
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::String(value.into())),
            input,
        ));
    }

    if input.at_punctuation(PunctuationKind::LeftParen) {
        let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let (expression, input) =
            parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)?;
        let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
        return Ok((expression, input));
    }

    if input.at_punctuation(PunctuationKind::LeftBracket) {
        let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
        let mut values = Vec::new();

        if !input.at_punctuation(PunctuationKind::RightBracket) {
            loop {
                let (value, rest) =
                    parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)?;
                values.push(value);
                input = rest;

                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    continue;
                }

                break;
            }
        }

        let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
        let values = syntax_trees.expressions.insert_expression_handles(values);
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::ArrayLiteral(values)),
            input,
        ));
    }

    if input.at_name_like() {
        let (path, input) = parse_path_handle_span(input, |member| {
            syntax_trees
                .expressions
                .append_identifier_path_member(member)
        })?;

        if context.allows_struct_literal()
            && input.at_punctuation(PunctuationKind::LeftBrace)
            && path.count() == 1
        {
            let type_name = syntax_trees
                .expressions
                .identifier_path_members(path)
                .first()
                .cloned()
                .expect("single-member path should have one member");
            return parse_struct_literal_handle(syntax_trees, type_name, input);
        }

        return Ok((
            syntax_trees.expressions.insert(ExpressionNode::Name(path)),
            input,
        ));
    }

    Err(input.error_here("expected expression"))
}

fn parse_struct_literal_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    type_name: Identifier,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut fields = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (name, rest) = input.take_identifier()?;
        input = rest.take_punctuation(PunctuationKind::Colon, ":")?;
        let (value, rest) = parse_expression_handle(syntax_trees, input)?;
        input = rest;
        fields.push(TableStructLiteralField { name, value });

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let fields = syntax_trees.expressions.insert_struct_fields(fields);
    Ok((
        syntax_trees
            .expressions
            .insert(ExpressionNode::StructLiteral(TableStructLiteral {
                type_name,
                fields,
            })),
        input,
    ))
}
