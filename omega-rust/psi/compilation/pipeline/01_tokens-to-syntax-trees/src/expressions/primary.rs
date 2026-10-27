use crate::expressions::context::ExpressionContext;
use crate::expressions::match_dispatch::parse_match_expression_handle;
use crate::expressions::parse_expression::{parse_expression_handle, parse_expression_handle_in};
use crate::input::token_cursor::{Input, ParseResult, parse_path_handle_span};
use crate::type_syntax::parse_type::parse_type_reference_handle;
use numerics::literals::IntegerLiteral;
use source::{SourceSpan, Span};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, TableStructLiteral, TableStructLiteralField,
};
use syntax_trees::identifier::Identifier;
use tokens::{KeywordKind, NumericLiteralKind, PunctuationKind, TokenKind};

pub(super) fn parse_primary_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    if context.allows_type_expression() && starts_lifetime_reference_type(input) {
        let start = input;
        let (reference, rest) = parse_type_reference_handle(syntax_trees, input)?;
        let expression = syntax_trees
            .expressions
            .insert(ExpressionNode::TypeExpression(reference));
        syntax_trees
            .expressions
            .set_source_span(expression, start.source_span_until(rest));
        return Ok((expression, rest));
    }
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
        if context.allows_logical_subject() {
            let source_span =
                input.source_span(input.tokens.first().expect("logical subject token"));
            let input = input.take_keyword(KeywordKind::SelfValue, "self")?;
            let member = syntax_trees
                .expressions
                .append_identifier_path_member(Identifier::new("self", source_span));
            let expression = syntax_trees.expressions.insert(ExpressionNode::Name(
                arena::HandleSpan::from_parts(member, 1),
            ));
            syntax_trees
                .expressions
                .set_source_span(expression, source_span);
            return Ok((expression, input));
        }
        return Err(input.error_here(
            "implicit `self` is retired; name the explicit parameter or domain subject",
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
        let start = input;
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
        let expression = syntax_trees
            .expressions
            .insert(ExpressionNode::ArrayLiteral(units));
        syntax_trees
            .expressions
            .set_source_span(expression, start.source_span_until(input));
        return Ok((expression, input));
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
            parse_expression_handle_in(syntax_trees, input, context.grouped())?;
        let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
        return Ok((expression, input));
    }

    if input.at_punctuation(PunctuationKind::LeftBracket) {
        if context.allows_type_expression() && starts_fixed_array_type(input) {
            let start = input;
            let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
            let expression = syntax_trees
                .expressions
                .insert(ExpressionNode::TypeExpression(type_reference));
            syntax_trees
                .expressions
                .set_source_span(expression, start.source_span_until(input));
            return Ok((expression, input));
        }
        let start = input;
        let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
        let mut values = Vec::new();

        if !input.at_punctuation(PunctuationKind::RightBracket) {
            loop {
                let (value, rest) =
                    parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)?;
                values.push(value);
                input = rest;

                let Some(next) = input.take_comma()? else {
                    break;
                };

                input = next;

                continue;
            }
        }

        let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
        let values = syntax_trees.expressions.insert_expression_handles(values);
        let expression = syntax_trees
            .expressions
            .insert(ExpressionNode::ArrayLiteral(values));
        // A constant initializer retains this exact occurrence for later
        // diagnostics and package review, including empty arrays with no child
        // from which a source location could be recovered.
        syntax_trees
            .expressions
            .set_source_span(expression, start.source_span_until(input));
        return Ok((expression, input));
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
        let start = input;
        if let Some((application, rest)) = try_applied_requirement_head(syntax_trees, start) {
            return parse_applied_requirement_call(syntax_trees, start, application, rest);
        }
        let (path, input) = parse_path_handle_span(input, |member| {
            syntax_trees
                .expressions
                .append_identifier_path_member(member)
        })?;

        if context.allows_type_expression()
            && input.at_punctuation(PunctuationKind::Less)
            && let Some((type_reference, rest)) = try_generic_type_operand(syntax_trees, start)
        {
            let expression = syntax_trees
                .expressions
                .insert(ExpressionNode::TypeExpression(type_reference));
            syntax_trees
                .expressions
                .set_source_span(expression, start.source_span_until(rest));
            return Ok((expression, rest));
        }

        if context.allows_struct_literal() && input.at_punctuation(PunctuationKind::LeftBrace) {
            let members = syntax_trees.expressions.identifier_path_members(path);
            let first = members.first().expect("constructor path has a head");
            let last = members.last().expect("constructor path has a tail");
            let mut source_span = first.source_span();
            source_span.span.end = last.source_span().span.end;
            let mut name = first.as_str(syntax_trees).to_owned();
            for member in &members[1..] {
                name.push_str("::");
                name.push_str(member.as_str(syntax_trees));
            }
            let constructor_name = Identifier::new(name, source_span);
            return parse_struct_literal_handle(syntax_trees, constructor_name, input);
        }

        let expression = syntax_trees.expressions.insert(ExpressionNode::Name(path));
        syntax_trees
            .expressions
            .set_source_span(expression, start.source_span_until(input));
        return Ok((expression, input));
    }

    Err(input.error_here("expected expression"))
}

/// `Requirement<Arguments>::operation(...)` calls a requirement of one exact
/// applied trait, as `Drawable<Player>::draw` names it in a `satisfies`
/// clause. Only a generic type path followed by `::member` then `(` or a
/// static argument list takes this form; a comparison never continues with
/// `::`, so the speculation cannot steal an ordinary expression.
fn try_applied_requirement_head<'tokens, 'source>(
    syntax: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(
    syntax_trees::types::TypeReferenceHandle,
    Input<'tokens, 'source>,
)> {
    let (_, after_name) = input.expect_token().ok()?;
    if !after_name.at_punctuation(PunctuationKind::Less)
        && !after_name.at_punctuation(PunctuationKind::ColonColon)
    {
        return None;
    }
    probe_applied_requirement_head(syntax, input)
}

// Keep the table-frontier frame off ordinary names' prefix-only path.
#[inline(never)]
fn probe_applied_requirement_head<'tokens, 'source>(
    syntax: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(
    syntax_trees::types::TypeReferenceHandle,
    Input<'tokens, 'source>,
)> {
    let checkpoint = syntax.append_checkpoint();
    let accepted = (|| {
        let (reference, rest) = parse_type_reference_handle(syntax, input).ok()?;
        if !matches!(
            syntax.type_references.type_reference(reference),
            syntax_trees::types::TypeReferenceNode::Generic { .. }
        ) {
            return None;
        }
        let after_separator = rest
            .take_punctuation(PunctuationKind::ColonColon, "::")
            .ok()?;
        let (_, after_member) = after_separator.take_identifier().ok()?;
        (after_member.at_punctuation(PunctuationKind::LeftParen)
            || after_member.at_punctuation(PunctuationKind::Less))
        .then_some((reference, rest))
    })();
    if accepted.is_none() {
        syntax.rewind_appended(checkpoint);
    }
    accepted
}

/// The applied trait is the call's static receiver, so later stages resolve
/// `operation` against that exact application rather than the bare trait.
fn parse_applied_requirement_call<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    application: syntax_trees::types::TypeReferenceHandle,
    rest: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let start = input;
    let receiver = syntax_trees
        .expressions
        .insert(ExpressionNode::TypeExpression(application));
    syntax_trees
        .expressions
        .set_source_span(receiver, start.source_span_until(rest));
    let rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
    let (target, rest) = rest.take_identifier()?;
    let (machine_arguments, rest) =
        match crate::expressions::parse_postfix::try_parse_static_machine_arguments(
            syntax_trees,
            rest,
        )? {
            Some((machine_arguments, rest)) => (machine_arguments, rest),
            None => (Box::default(), rest),
        };
    let rest = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
    let ((arguments, evidence_arguments), rest) =
        crate::expressions::parse_postfix::parse_argument_list_after_open_paren_handle(
            syntax_trees,
            rest,
        )?;
    let source = target.source_span();
    let call = syntax_trees.expressions.insert(ExpressionNode::Call(
        syntax_trees::expression::TableCallExpression {
            receiver,
            target,
            target_is_static: true,
            machine_arguments,
            arguments,
            evidence_arguments,
            operational_acknowledgement: Default::default(),
        },
    ));
    syntax_trees.expressions.set_source_span(call, source);
    Ok((call, rest))
}

/// A generic type operand ends before a fact delimiter or Boolean/equality
/// operator. A following value keeps ordinary comparisons, and `(` keeps a
/// static call. Append-only speculation rewinds rejected rows so failed type parses cannot
/// publish phantom generic applications into later normalization.
fn try_generic_type_operand<'tokens, 'source>(
    syntax: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(
    syntax_trees::types::TypeReferenceHandle,
    Input<'tokens, 'source>,
)> {
    let checkpoint = syntax.append_checkpoint();
    let accepted = (|| {
        let (reference, rest) = parse_type_reference_handle(syntax, input).ok()?;
        if !matches!(
            syntax.type_references.type_reference(reference),
            syntax_trees::types::TypeReferenceNode::Generic { .. }
        ) {
            return None;
        }
        let accepted = [
            PunctuationKind::Arrow,
            PunctuationKind::EqualEqual,
            PunctuationKind::ExclamationEqual,
            PunctuationKind::PipePipe,
            PunctuationKind::AndAnd,
            PunctuationKind::Comma,
            PunctuationKind::Semicolon,
            PunctuationKind::LeftBrace,
            PunctuationKind::RightBrace,
            PunctuationKind::RightParen,
        ]
        .into_iter()
        .any(|punctuation| rest.at_punctuation(punctuation))
            || [
                "requires",
                "ensures",
                "terminates",
                "decreases",
                "reaches",
                "effects",
                "invokes",
                "suspends",
                "blocks",
                "crashes",
                "boundary",
                "where",
                "satisfies",
            ]
            .into_iter()
            .any(|keyword| rest.at_contextual(keyword));
        accepted.then_some((reference, rest))
    })();
    if accepted.is_none() {
        syntax.rewind_appended(checkpoint);
    }
    accepted
}

/// The outer semicolon distinguishes `[Element; Count]` from a value array.
/// Nested brackets, parentheses and braces own their separators; those do not
/// decide the outer operand's role. Once selected, the ordinary type parser owns all
/// syntax validation, including malformed lengths and nested element types.
fn starts_fixed_array_type(input: Input<'_, '_>) -> bool {
    let mut bracket_depth = 0usize;
    let mut parenthesis_depth = 0usize;
    let mut brace_depth = 0usize;
    for token in input.tokens {
        match token.kind {
            TokenKind::Punctuation(PunctuationKind::LeftBracket) => bracket_depth += 1,
            TokenKind::Punctuation(PunctuationKind::LeftParen) => parenthesis_depth += 1,
            TokenKind::Punctuation(PunctuationKind::RightParen) => {
                let Some(depth) = parenthesis_depth.checked_sub(1) else {
                    return false;
                };
                parenthesis_depth = depth;
            }
            TokenKind::Punctuation(PunctuationKind::LeftBrace) => brace_depth += 1,
            TokenKind::Punctuation(PunctuationKind::RightBrace) => {
                let Some(depth) = brace_depth.checked_sub(1) else {
                    return false;
                };
                brace_depth = depth;
            }
            TokenKind::Punctuation(PunctuationKind::RightBracket) => {
                if bracket_depth <= 1 {
                    return false;
                }
                bracket_depth -= 1;
            }
            TokenKind::Punctuation(PunctuationKind::Semicolon)
                if bracket_depth == 1 && parenthesis_depth == 0 && brace_depth == 0 =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn parse_struct_literal_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    constructor_name: Identifier,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let start = input;
    let type_name_span = constructor_name.source_span();
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
            constructor_name,
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

#[cfg(test)]
mod generic_type_operand_tests {}

/// Tick-qualified reference syntax has an unambiguous static type role.
/// Unqualified `&place` retains ordinary value-borrow parsing until resolution.
pub(super) fn starts_lifetime_reference_type(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::Ampersand)
        && input
            .take_punctuation(PunctuationKind::Ampersand, "&")
            .is_ok_and(|rest| rest.at_punctuation(PunctuationKind::Apostrophe))
}
