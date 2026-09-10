use crate::parser::context::ExpressionContext;
use crate::parser::expression::{
    parse_expression_handle, parse_expression_handle_in,
    parse_expression_handle_without_struct_literals,
};
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::type_reference::parse_type_reference_handle;
use numerics::literals::IntegerLiteral;
use source::{SourceSpan, Span};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, TableStructLiteral, TableStructLiteralField,
};
use syntax_trees::identifier::Identifier;
use tokens::{KeywordKind, NumericLiteralKind, PunctuationKind, TokenKind};

/// Retain authored dispatch order and source custody for semantic checking.
fn parse_match_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let start = input;
    let input = input.take_keyword(KeywordKind::Match, "match")?;
    let (subject, input) = parse_expression_handle_without_struct_literals(syntax_trees, input)?;
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut arms = Vec::new();
    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let arm_start = input;
        let (pattern, rest) = if input.at_contextual("_") {
            (
                syntax_trees::expression::MatchPattern::Wildcard,
                input.take_contextual("_")?,
            )
        } else {
            let (pattern, rest) =
                parse_expression_handle_without_struct_literals(syntax_trees, input)?;
            (syntax_trees::expression::MatchPattern::Value(pattern), rest)
        };
        let rest = rest.take_punctuation(PunctuationKind::Arrow, "->")?;
        let (value, rest) = parse_expression_handle(syntax_trees, rest)?;
        arms.push(syntax_trees::expression::TableMatchArm {
            pattern,
            value,
            source_span: arm_start.source_span_until(rest),
        });
        input = if rest.at_punctuation(PunctuationKind::Comma) {
            rest.take_punctuation(PunctuationKind::Comma, ",")?
        } else {
            rest
        };
    }
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    if arms.is_empty() {
        return Err(input.error_here("match expression must have at least one arm"));
    }
    let arms = syntax_trees.expressions.insert_match_arms(arms);
    let expression = syntax_trees.expressions.insert(ExpressionNode::Match(
        syntax_trees::expression::TableMatchExpression { subject, arms },
    ));
    syntax_trees
        .expressions
        .set_source_span(expression, start.source_span_until(input));
    Ok((expression, input))
}
pub(super) fn parse_primary_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    if input.at_keyword(KeywordKind::True) {
        let source_span = input
            .tokens
            .first()
            .map(|token| input.source_span(token))
            .expect("recognized boolean literal has a source token");
        let input = input.take_keyword(KeywordKind::True, "true")?;
        let expression = syntax_trees
            .expressions
            .insert(ExpressionNode::Boolean(true));
        syntax_trees
            .expressions
            .set_source_span(expression, source_span);
        return Ok((expression, input));
    }

    if input.at_keyword(KeywordKind::False) {
        let source_span = input
            .tokens
            .first()
            .map(|token| input.source_span(token))
            .expect("recognized boolean literal has a source token");
        let input = input.take_keyword(KeywordKind::False, "false")?;
        let expression = syntax_trees
            .expressions
            .insert(ExpressionNode::Boolean(false));
        syntax_trees
            .expressions
            .set_source_span(expression, source_span);
        return Ok((expression, input));
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

    // `zero_value<T>()` is a proof-only representation observation, not an
    // ordinary generic machine call (angle-bracket call arguments select
    // machine symbols). Preserve its complete nested type reference.
    if input.at_contextual("zero_value") {
        let after_name = input.take_contextual("zero_value")?;
        if after_name.at_punctuation(PunctuationKind::Less) {
            let after_less = after_name.take_punctuation(PunctuationKind::Less, "<")?;
            let (type_reference, rest) = parse_type_reference_handle(syntax_trees, after_less)?;
            let rest = rest.take_punctuation(PunctuationKind::Greater, ">")?;
            let rest = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let rest = rest.take_punctuation(PunctuationKind::RightParen, ")")?;
            return Ok((
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::ZeroValue(type_reference)),
                rest,
            ));
        }
    }

    if input.tokens.first().is_some_and(|token| {
        matches!(
            token.kind,
            TokenKind::NumericLiteral(NumericLiteralKind::Integer(_))
        )
    }) {
        let source_span = input
            .tokens
            .first()
            .map(|token| input.source_span(token))
            .expect("recognized integer literal has a source token");
        let (literal, input) = input.take_integer_literal()?;
        let expression = syntax_trees
            .expressions
            .insert(ExpressionNode::Integer(literal));
        syntax_trees
            .expressions
            .set_source_span(expression, source_span);
        return Ok((expression, input));
    }

    if input.tokens.first().is_some_and(|token| {
        matches!(
            token.kind,
            TokenKind::NumericLiteral(NumericLiteralKind::Float(_))
        )
    }) {
        let source_span = input.current_source_span();
        let (value, input) = input.take_float_text()?;
        let expression = syntax_trees
            .expressions
            .insert(ExpressionNode::Float(value));
        syntax_trees
            .expressions
            .set_source_span(expression, source_span);
        return Ok((expression, input));
    }

    // `utf16"..."`: UTF-16 text sugar for UEFI/Windows CHAR16 data. Desugars in
    // the PARSER to the ordinary integer ARRAY LITERAL of the string's UTF-16
    // code units (surrogate pairs for non-BMP), so it fits every `[u16; N]`
    // context an array literal fits -- no new tree node, no backend work. The
    // prefix is contextual: a bare identifier followed by a string literal is
    // never otherwise valid, so `utf16` stays usable as an ordinary name.
    if input.at_contextual("utf16") && input.at_contextual_then_string("utf16") {
        let input = input.take_contextual("utf16")?;
        let (value, input) = input.take_string()?;
        let units: Vec<_> = value
            .encode_utf16()
            .map(|unit| {
                syntax_trees.expressions.insert(ExpressionNode::Integer(
                    IntegerLiteral::from_value(i64::from(unit)),
                ))
            })
            .collect();
        let units = syntax_trees.expressions.insert_expression_handles(units);
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::ArrayLiteral(units)),
            input,
        ));
    }

    if input
        .tokens
        .first()
        .is_some_and(|token| token.kind == TokenKind::StringLiteral)
    {
        let (value, input) = input.take_string_bytes()?;
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::String(value)),
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

    // TASK RUNTIME TR1: the synchronous `spawn` fiction is retired. Keep
    // `spawn` contextual so ordinary identifiers retain their spelling, but
    // reject the former block shape at its source instead of lowering it to a
    // blocking call.
    if context.allows_struct_literal() && input.at_contextual("spawn") {
        let after_spawn = input.take_contextual("spawn")?;
        if after_spawn.at_punctuation(PunctuationKind::LeftBrace) {
            return Err(input.error_here(
                "`spawn { ... }` is retired: start an ordinary named machine through an \
                 admitted task-runtime capability (`runtime.start<Worker::run>(...)`) and \
                 keep the returned linear `Task<T>`",
            ));
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
    let start = input;
    let type_name_span = type_name.source_span();
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
    let expression = syntax_trees
        .expressions
        .insert(ExpressionNode::StructLiteral(TableStructLiteral {
            type_name,
            case_name,
            fields,
        }));
    let literal_tail_span = start.source_span_until(input);
    debug_assert_eq!(type_name_span.source_id, literal_tail_span.source_id);
    syntax_trees.expressions.set_source_span(
        expression,
        SourceSpan::new(
            type_name_span.source_id,
            Span::new(type_name_span.span.start, literal_tail_span.span.end),
        ),
    );
    Ok((expression, input))
}
