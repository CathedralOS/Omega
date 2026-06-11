use crate::parse_error::ParseError;
use crate::parser::expression::parse_expression_handle_without_struct_literals;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use omega_core::arena::HandleSpan;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableCallExpression,
    TableCastExpression, TableIndexedExpression, TableMemberExpression, TableMembershipExpression,
    TableRangeExpression, TableStructLiteral, TableStructLiteralField, TableUnaryExpression,
};
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::statement::TransitionGuardNode;
use omega_tokens::{KeywordKind, PunctuationKind};

/// The named bindings a destructure pattern arm introduces: each bound `field`
/// rewrites to `subject.field` in the arm's guard and transition-target arguments.
pub(super) struct DestructureBindings {
    pub(super) subject: ExpressionHandle,
    pub(super) fields: Vec<Identifier>,
}

pub(super) fn parse_transition_guard_node<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    subject: &[ExpressionHandle],
) -> Result<
    (
        TransitionGuardNode,
        Option<DestructureBindings>,
        Input<'tokens, 'source>,
    ),
    ParseError,
> {
    let (pattern_input, rest) =
        input.split_at_top_level_punctuation(PunctuationKind::Arrow, "expected `->`")?;
    if looks_like_version_match_arm(pattern_input) {
        return Err(pattern_input.error_here("version match arms are not implemented yet"));
    }

    if subject.len() == 1
        && let Some((guard, bindings)) =
            parse_destructure_pattern_arm(syntax_trees, pattern_input, subject[0])?
    {
        return Ok((guard, Some(bindings), rest));
    }

    let (patterns, pattern_rest) = parse_transition_pattern_list(syntax_trees, pattern_input)?;
    if !pattern_rest.tokens.is_empty() {
        return Err(pattern_rest.error_here("expected transition pattern"));
    }

    if subject.is_empty() {
        if patterns.len() == 1 {
            let guard = match patterns.into_iter().next().flatten() {
                Some(expression) => TransitionGuardNode::When(expression),
                None => TransitionGuardNode::Always,
            };
            return Ok((guard, None, rest));
        }
        return Err(input.error_here("anonymous transition blocks do not support tuple patterns"));
    }

    if patterns.len() == 1 && patterns[0].is_none() {
        return Ok((TransitionGuardNode::Always, None, rest));
    }

    if subject.len() != patterns.len() {
        return Err(input.error_here(format!(
            "transition pattern arity {} does not match subject arity {}",
            patterns.len(),
            subject.len()
        )));
    }

    let mut combined = ExpressionHandle::invalid();
    for (left, right) in subject.iter().copied().zip(patterns.into_iter()) {
        let Some(right) = right else {
            continue;
        };
        let equality =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: BinaryOperator::Equal,
                    right,
                }));
        combined = if combined.is_valid() {
            syntax_trees
                .expressions
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: combined,
                    operator: BinaryOperator::And,
                    right: equality,
                }))
        } else {
            equality
        };
    }

    Ok((
        if combined.is_valid() {
            TransitionGuardNode::When(combined)
        } else {
            TransitionGuardNode::Always
        },
        None,
        rest,
    ))
}

pub(super) fn parse_transition_expression_list<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Vec<ExpressionHandle>> {
    parse_transition_match_list(syntax_trees, input, false)
}

fn parse_transition_pattern_list<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Vec<Option<ExpressionHandle>>> {
    parse_transition_match_list(syntax_trees, input, true)
}

fn parse_transition_match_list<'tokens, 'source, T>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    allow_wildcard: bool,
) -> ParseResult<'tokens, 'source, Vec<T>>
where
    T: FromTransitionMatchComponent,
{
    if input.at_punctuation(PunctuationKind::LeftParen) {
        let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let (values, input) =
            parse_transition_match_list_after_open_paren(syntax_trees, input, allow_wildcard)?;
        let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
        Ok((values, input))
    } else {
        let (value, input) = parse_transition_match_component(syntax_trees, input, allow_wildcard)?;
        Ok((vec![value], input))
    }
}

fn parse_transition_match_list_after_open_paren<'tokens, 'source, T>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    allow_wildcard: bool,
) -> ParseResult<'tokens, 'source, Vec<T>>
where
    T: FromTransitionMatchComponent,
{
    let mut values = Vec::new();
    while !input.at_punctuation(PunctuationKind::RightParen) {
        let (value, rest) = parse_transition_match_component(syntax_trees, input, allow_wildcard)?;
        values.push(value);
        input = rest;
        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
        } else {
            break;
        }
    }
    Ok((values, input))
}

fn looks_like_version_match_arm(input: Input<'_, '_>) -> bool {
    let semantic_tokens = input
        .tokens
        .iter()
        .filter(|token| !token.is_non_semantic())
        .collect::<Vec<_>>();

    semantic_tokens.windows(3).any(|tokens| {
        tokens[0].punctuation() == Some(PunctuationKind::ColonColon)
            && tokens[1]
                .lexeme
                .as_str()
                .strip_prefix('v')
                .is_some_and(|suffix| suffix.chars().all(|ch| ch.is_ascii_digit()))
            && tokens[2].punctuation() == Some(PunctuationKind::LeftParen)
    })
}

fn parse_transition_match_component<'tokens, 'source, T>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    allow_wildcard: bool,
) -> ParseResult<'tokens, 'source, T>
where
    T: FromTransitionMatchComponent,
{
    if allow_wildcard && input.at_contextual("_") {
        let input = input.take_contextual("_")?;
        return Ok((T::from_wildcard(), input));
    }

    let (expression, input) = parse_expression_handle_without_struct_literals(syntax_trees, input)?;
    Ok((T::from_expression(expression), input))
}

/// Parse a single-subject DESTRUCTURE pattern arm, with or without an `if` guard:
///
/// - `Type::Case { field, .. } [if guard] -> ...` -- a CASE pattern: the arm
///   matches when the subject's tag is `Type::Case` (an equality guard against
///   the case name), and each bound `field` rewrites to `subject.field` (a
///   payload read) in the `if` guard and in the transition target's arguments.
/// - `Type { field, .. } if guard -> ...` -- a record destructure: no tag
///   compare (the subject IS that type); bindings rewrite the same way.
///
/// Returns `Ok(None)` when the arm is not a destructure pattern (no `{` after a
/// leading path), so the caller falls back to the plain pattern list.
fn parse_destructure_pattern_arm<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    subject: ExpressionHandle,
) -> Result<Option<(TransitionGuardNode, DestructureBindings)>, ParseError> {
    if !input.at_name_like() {
        return Ok(None);
    }

    let (pattern_input, guard_input) = match find_top_level_keyword(input, KeywordKind::If) {
        Some(if_index) => {
            let (pattern_tokens, guard_tokens_with_if) = input.tokens.split_at(if_index);
            (
                Input::new(input.source_id, pattern_tokens),
                Some(Input::new(
                    input.source_id,
                    guard_tokens_with_if
                        .get(1..)
                        .expect("if keyword split should include guard tokens"),
                )),
            )
        }
        None => (input, None),
    };

    let (path, after_path) = parse_path_handle_span(pattern_input, |member| {
        syntax_trees
            .expressions
            .append_identifier_path_member(member)
    })?;
    if !after_path.at_punctuation(PunctuationKind::LeftBrace) {
        // Without `if` this is an ordinary pattern; with `if` the arm must be a
        // destructure (a bare `pattern if guard` arm has no other meaning here).
        if guard_input.is_some() {
            return Err(after_path
                .error_here("expected `{` to open a destructure pattern before `if` guard"));
        }
        return Ok(None);
    }

    let (fields, pattern_rest) = parse_data_destructure_pattern_fields(syntax_trees, after_path)?;
    if !pattern_rest.tokens.is_empty() {
        return Err(pattern_rest.error_here("expected data destructure pattern"));
    }

    // A two-member path names a case of the subject's type: the arm matches on
    // the TAG, so the guard starts with `subject == Type::Case`.
    let mut combined = match syntax_trees.expressions.identifier_path_members(path).len() {
        1 => ExpressionHandle::invalid(),
        2 => {
            let case_reference = syntax_trees.expressions.insert(ExpressionNode::Name(path));
            syntax_trees
                .expressions
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: subject,
                    operator: BinaryOperator::Equal,
                    right: case_reference,
                }))
        }
        _ => {
            return Err(pattern_input
                .error_here("destructure pattern path must be `Type { .. }` or `Type::Case { .. }`"));
        }
    };

    if let Some(guard_input) = guard_input {
        let (guard, rest) =
            parse_expression_handle_without_struct_literals(syntax_trees, guard_input)?;
        if !rest.tokens.is_empty() {
            return Err(rest.error_here("expected transition pattern guard"));
        }
        let guard = rewrite_destructure_guard_expression(syntax_trees, guard, subject, &fields);
        combined = if combined.is_valid() {
            syntax_trees
                .expressions
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: combined,
                    operator: BinaryOperator::And,
                    right: guard,
                }))
        } else {
            guard
        };
    }

    let guard = if combined.is_valid() {
        TransitionGuardNode::When(combined)
    } else {
        TransitionGuardNode::Always
    };
    Ok(Some((guard, DestructureBindings { subject, fields })))
}

fn find_top_level_keyword(input: Input<'_, '_>, keyword: KeywordKind) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;

    for (index, token) in input.tokens.iter().enumerate() {
        match token.punctuation() {
            Some(PunctuationKind::LeftParen) => paren_depth += 1,
            Some(PunctuationKind::RightParen) => paren_depth = paren_depth.saturating_sub(1),
            Some(PunctuationKind::LeftBracket) => bracket_depth += 1,
            Some(PunctuationKind::RightBracket) => bracket_depth = bracket_depth.saturating_sub(1),
            Some(PunctuationKind::LeftBrace) => brace_depth += 1,
            Some(PunctuationKind::RightBrace) => brace_depth = brace_depth.saturating_sub(1),
            _ => {}
        }

        if token.keyword() == Some(keyword)
            && paren_depth == 0
            && bracket_depth == 0
            && brace_depth == 0
        {
            return Some(index);
        }
    }

    None
}

/// Parse the `{ field, .. }` part of a destructure pattern (the leading path is
/// already consumed by the caller).
fn parse_data_destructure_pattern_fields<'tokens, 'source>(
    _syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Vec<Identifier>> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut fields = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_punctuation(PunctuationKind::DotDot) {
            input = input.take_punctuation(PunctuationKind::DotDot, "..")?;
        } else {
            let (field, rest) = input.take_identifier()?;
            fields.push(field);
            input = rest;
        }

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
        } else {
            break;
        }
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((fields, input))
}

pub(super) fn rewrite_destructure_guard_expression(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
    subject: ExpressionHandle,
    fields: &[Identifier],
) -> ExpressionHandle {
    let rewritten = match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::ArrayLiteral(values) => {
            let source_values = syntax_trees.expressions.expression_handles(values).to_vec();
            let values = source_values
                .iter()
                .map(|value| {
                    rewrite_destructure_guard_expression(syntax_trees, *value, subject, fields)
                })
                .collect::<Vec<_>>();
            ExpressionNode::ArrayLiteral(syntax_trees.expressions.insert_expression_handles(values))
        }
        ExpressionNode::Binary(binary) => ExpressionNode::Binary(TableBinaryExpression {
            left: rewrite_destructure_guard_expression(syntax_trees, binary.left, subject, fields),
            operator: binary.operator,
            right: rewrite_destructure_guard_expression(
                syntax_trees,
                binary.right,
                subject,
                fields,
            ),
        }),
        ExpressionNode::Call(call) => ExpressionNode::Call(TableCallExpression {
            receiver: rewrite_optional_expression(syntax_trees, call.receiver, subject, fields),
            target: call.target,
            arguments: rewrite_expression_span(syntax_trees, call.arguments, subject, fields),
        }),
        ExpressionNode::Cast(cast) => ExpressionNode::Cast(TableCastExpression {
            value: rewrite_destructure_guard_expression(syntax_trees, cast.value, subject, fields),
            target_type: cast.target_type,
        }),
        ExpressionNode::Indexed(indexed) => ExpressionNode::Indexed(TableIndexedExpression {
            collection: rewrite_destructure_guard_expression(
                syntax_trees,
                indexed.collection,
                subject,
                fields,
            ),
            index: rewrite_destructure_guard_expression(
                syntax_trees,
                indexed.index,
                subject,
                fields,
            ),
        }),
        ExpressionNode::Membership(membership) => {
            ExpressionNode::Membership(TableMembershipExpression {
                value: rewrite_destructure_guard_expression(
                    syntax_trees,
                    membership.value,
                    subject,
                    fields,
                ),
                domain: membership.domain,
            })
        }
        ExpressionNode::Member(member) => ExpressionNode::Member(TableMemberExpression {
            receiver: rewrite_destructure_guard_expression(
                syntax_trees,
                member.receiver,
                subject,
                fields,
            ),
            member: member.member,
        }),
        ExpressionNode::Mutable(inner) => ExpressionNode::Mutable(
            rewrite_destructure_guard_expression(syntax_trees, inner, subject, fields),
        ),
        ExpressionNode::Name(path) => {
            if let Some(field) = single_destructured_field_name(syntax_trees, path, fields) {
                ExpressionNode::Member(TableMemberExpression {
                    receiver: subject,
                    member: field,
                })
            } else {
                ExpressionNode::Name(path)
            }
        }
        ExpressionNode::Range(range) => ExpressionNode::Range(TableRangeExpression {
            start: rewrite_optional_expression(syntax_trees, range.start, subject, fields),
            end: rewrite_optional_expression(syntax_trees, range.end, subject, fields),
            end_inclusive: range.end_inclusive,
        }),
        ExpressionNode::StructLiteral(struct_literal) => {
            let source_fields = syntax_trees
                .expressions
                .struct_fields(struct_literal.fields)
                .to_vec();
            let struct_fields = source_fields
                .iter()
                .map(|field| TableStructLiteralField {
                    name: field.name.clone(),
                    value: rewrite_destructure_guard_expression(
                        syntax_trees,
                        field.value,
                        subject,
                        fields,
                    ),
                })
                .collect::<Vec<_>>();
            ExpressionNode::StructLiteral(TableStructLiteral {
                type_name: struct_literal.type_name,
                case_name: struct_literal.case_name,
                fields: syntax_trees.expressions.insert_struct_fields(struct_fields),
            })
        }
        ExpressionNode::Unary(unary) => ExpressionNode::Unary(TableUnaryExpression {
            operator: unary.operator,
            operand: rewrite_destructure_guard_expression(
                syntax_trees,
                unary.operand,
                subject,
                fields,
            ),
        }),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::SelfValue
        | ExpressionNode::String(_) => syntax_trees.expressions.expression(expression).clone(),
    };

    syntax_trees.expressions.insert(rewritten)
}

fn rewrite_optional_expression(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
    subject: ExpressionHandle,
    fields: &[Identifier],
) -> ExpressionHandle {
    if expression.is_valid() {
        rewrite_destructure_guard_expression(syntax_trees, expression, subject, fields)
    } else {
        expression
    }
}

fn rewrite_expression_span(
    syntax_trees: &mut SyntaxTrees,
    expressions: HandleSpan<ExpressionHandle>,
    subject: ExpressionHandle,
    fields: &[Identifier],
) -> HandleSpan<ExpressionHandle> {
    let source_expressions = syntax_trees
        .expressions
        .expression_handles(expressions)
        .to_vec();
    let expressions = source_expressions
        .iter()
        .map(|expression| {
            rewrite_destructure_guard_expression(syntax_trees, *expression, subject, fields)
        })
        .collect::<Vec<_>>();
    syntax_trees
        .expressions
        .insert_expression_handles(expressions)
}

fn single_destructured_field_name(
    syntax_trees: &SyntaxTrees,
    path: HandleSpan<Identifier>,
    fields: &[Identifier],
) -> Option<Identifier> {
    let members = syntax_trees.expressions.identifier_path_members(path);
    let [member] = members else {
        return None;
    };
    fields.iter().find(|field| *field == member).cloned()
}

trait FromTransitionMatchComponent {
    fn from_expression(expression: ExpressionHandle) -> Self;
    fn from_wildcard() -> Self;
}

impl FromTransitionMatchComponent for ExpressionHandle {
    fn from_expression(expression: ExpressionHandle) -> Self {
        expression
    }

    fn from_wildcard() -> Self {
        ExpressionHandle::invalid()
    }
}

impl FromTransitionMatchComponent for Option<ExpressionHandle> {
    fn from_expression(expression: ExpressionHandle) -> Self {
        Some(expression)
    }

    fn from_wildcard() -> Self {
        None
    }
}
