use crate::parser::context::ExpressionContext;
use crate::parser::expression::{
    parse_expression_handle, parse_expression_handle_in,
    parse_expression_handle_without_struct_literals,
};
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableStructLiteral,
    TableStructLiteralField,
};
use omega_syntax_trees::identifier::Identifier;
use omega_tokens::{KeywordKind, NumericLiteralKind, PunctuationKind, TokenKind};

/// Parse a value-position `match` and desugar it into pure arithmetic over the
/// existing expression nodes — no dedicated match node or codegen is needed.
///
/// For distinct constant patterns exactly one arm matches, and a comparison
/// (`scrutinee == pattern`) evaluates to 0 or 1, so
///   `match s { p1 -> v1, p2 -> v2, _ -> d }`
/// is equivalent to
///   `d + (s == p1) * (v1 - d) + (s == p2) * (v2 - d)`.
/// The default `d` is the wildcard arm's value, or the last arm's value when the
/// match is exhaustive without a wildcard (e.g. one arm per enum variant).
fn parse_match_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let input = input.take_keyword(KeywordKind::Match, "match")?;
    let (scrutinee, input) = parse_expression_handle_without_struct_literals(syntax_trees, input)?;
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;

    // Collect arms as (pattern, value); a `None` pattern is the `_` wildcard.
    let mut arms: Vec<(Option<ExpressionHandle>, ExpressionHandle)> = Vec::new();
    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (pattern, rest) = if input.at_contextual("_") {
            (None, input.take_contextual("_")?)
        } else {
            let (pattern, rest) =
                parse_expression_handle_without_struct_literals(syntax_trees, input)?;
            (Some(pattern), rest)
        };
        let rest = rest.take_punctuation(PunctuationKind::Arrow, "->")?;
        let (value, rest) = parse_expression_handle(syntax_trees, rest)?;
        // Arms may be separated by an optional comma or just whitespace.
        let rest = if rest.at_punctuation(PunctuationKind::Comma) {
            rest.take_punctuation(PunctuationKind::Comma, ",")?
        } else {
            rest
        };
        arms.push((pattern, value));
        input = rest;
    }
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;

    if arms.is_empty() {
        return Err(input.error_here("match expression must have at least one arm"));
    }

    // The default value is the wildcard arm if present, else the last arm.
    let wildcard_index = arms.iter().position(|(pattern, _)| pattern.is_none());
    let (default_value, default_index) = match wildcard_index {
        Some(index) => (arms[index].1, index),
        None => (arms[arms.len() - 1].1, arms.len() - 1),
    };

    let mut result = default_value;
    for (index, (pattern, value)) in arms.iter().enumerate() {
        if index == default_index {
            continue;
        }
        let Some(pattern) = *pattern else {
            // A second wildcard is meaningless; skip it.
            continue;
        };
        let comparison =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: scrutinee,
                    operator: BinaryOperator::Equal,
                    right: pattern,
                }));
        let delta = syntax_trees
            .expressions
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: *value,
                operator: BinaryOperator::Subtract,
                right: default_value,
            }));
        let term = syntax_trees
            .expressions
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: comparison,
                operator: BinaryOperator::Multiply,
                right: delta,
            }));
        result = syntax_trees
            .expressions
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: result,
                operator: BinaryOperator::Add,
                right: term,
            }));
    }

    Ok((result, input))
}

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

    if input.at_keyword(KeywordKind::Match) {
        return parse_match_expression_handle(syntax_trees, input);
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

    // CONCURRENCY STAGE 1: `spawn { call }` in value position yields the
    // spawned call's COMPLETED result (the synchronous-spawn desugar; see
    // `expression/spawn.rs`). `spawn` is contextual: it is only spawn syntax
    // when followed by `{`, and only where a struct literal would also be
    // legal -- so `transition spawn { ... }` over a local named `spawn` keeps
    // parsing as an identifier, exactly like the struct-literal rule.
    if context.allows_struct_literal() && input.at_contextual("spawn") {
        let after_spawn = input.take_contextual("spawn")?;
        if after_spawn.at_punctuation(PunctuationKind::LeftBrace) {
            return super::parse_spawn_block_call_handle(syntax_trees, after_spawn);
        }
    }

    if input.at_name_like() {
        let (path, input) = parse_path_handle_span(input, |member| {
            syntax_trees
                .expressions
                .append_identifier_path_member(member)
        })?;

        if context.allows_struct_literal()
            && input.at_punctuation(PunctuationKind::LeftBrace)
            && (path.count() == 1 || path.count() == 2)
        {
            let members = syntax_trees.expressions.identifier_path_members(path);
            let type_name = members
                .first()
                .cloned()
                .expect("struct literal path should have a head member");
            // A two-member path (`Command::Say { ... }`) constructs a CASE of the
            // head type with named payload fields; one member is a record literal.
            let case_name = members.get(1).cloned();
            return parse_struct_literal_handle(syntax_trees, type_name, case_name, input);
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
    case_name: Option<Identifier>,
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
                case_name,
                fields,
            })),
        input,
    ))
}
