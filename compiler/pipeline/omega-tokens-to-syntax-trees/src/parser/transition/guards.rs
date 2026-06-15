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

/// One named binding a pattern arm introduces: uses of `binding` rewrite to
/// `subject.member` in the arm's guard and transition-target arguments. For
/// field destructures the two names coincide (`{ text }` binds `text` to
/// `subject.text`); a version arm's whole-value binding maps a user-chosen
/// name onto the container's internal payload field (`Counter::v1(old)` binds
/// `old` to `subject.__payload_v1`).
#[derive(Clone)]
pub(super) struct DestructureBinding {
    pub(super) binding: Identifier,
    pub(super) member: Identifier,
    /// The case variant this field is bound from (`Some("Transfer")` for a
    /// `Type::Case { .. }` pattern), so the rewritten `subject.member` access
    /// resolves to THAT variant's field even when a same-named field exists in
    /// another variant. `None` for a plain `Type { .. }` data destructure.
    pub(super) case_variant: Option<Identifier>,
}

impl DestructureBinding {
    fn field(name: Identifier, case_variant: Option<Identifier>) -> Self {
        Self {
            member: name.clone(),
            binding: name,
            case_variant,
        }
    }
}

/// The named bindings a destructure pattern arm introduces: each bound `field`
/// rewrites to `subject.field` in the arm's guard and transition-target arguments.
pub(super) struct DestructureBindings {
    pub(super) subject: ExpressionHandle,
    pub(super) fields: Vec<DestructureBinding>,
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

    // Version match arms (frozen decision 14): on a `Versioned<T>` subject the
    // paren form binds the WHOLE historical value -- `Counter::v1(old)` tests
    // the era tag and binds `old` to the payload reinterpreted as the `v1`
    // shape; the bare `Counter(current)` arm selects the CURRENT era. The
    // parser desugars structurally (era-marker membership guard + payload
    // member binding); the typed lowering resolves the era number and rejects
    // non-`Versioned` subjects.
    if subject.len() == 1
        && let Some((guard, bindings)) =
            parse_version_pattern_arm(syntax_trees, pattern_input, subject[0])?
    {
        return Ok((guard, Some(bindings), rest));
    }
    // Any OTHER spelling that embeds a `Type::vN(...)` selector (nested in a
    // larger pattern expression, non-identifier binding, ...) is still
    // rejected: there is no other meaning for a version selector call shape.
    if looks_like_version_match_arm(pattern_input) {
        return Err(pattern_input.error_here(
            "a version match arm must be the whole pattern: `Type::vN(binding) ->` or `Type(binding) ->` on a `Versioned<Type>` subject",
        ));
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
        // A bare two-member path arm (`Command::Move ->`, no binding braces) is
        // a CLASSIFICATION like its braced sibling: the arm tests the tag,
        // which is domain membership (decision 11). Desugaring to membership
        // keeps the synthesized tag test distinct from user-written equality,
        // which rejects bare payload-bearing case names.
        let equality = match syntax_trees.expressions.expression(right) {
            ExpressionNode::Name(path)
                if syntax_trees.expressions.identifier_path_members(*path).len() == 2 =>
            {
                let domain = *path;
                syntax_trees.expressions.insert(ExpressionNode::Membership(
                    TableMembershipExpression {
                        value: left,
                        domain,
                    },
                ))
            }
            _ => syntax_trees
                .expressions
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: BinaryOperator::Equal,
                    right,
                })),
        };
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

/// Parse a single-subject VERSION pattern arm (frozen decision 14): the paren
/// form binding the WHOLE value.
///
/// - `Type::vN(binding) -> ...` -- a historical-era arm: the guard tests the
///   subject's era tag and `binding` rewrites to the container's `vN` payload
///   field (the payload reinterpreted as the `Type::vN` shape).
/// - `Type(binding) -> ...` -- the CURRENT-era arm: tag test against the
///   current era; `binding` rewrites to the current-shape payload field.
///
/// The guard desugars to a marker MEMBERSHIP (`subject in Type::vN::<marker>`)
/// the typed lowering resolves into an era equality compare -- the era NUMBER
/// and the subject's `Versioned<Type>`-ness are unknown at parse time. `_`
/// binds nothing. Returns `Ok(None)` when the pattern is not exactly this
/// shape, so the caller falls back to the other arm forms. (A consequence of
/// claiming the bare `Type(binding)` spelling: a transition arm can no longer
/// compare the subject against a single-bare-identifier-argument CALL --
/// a shape no known program uses; spell it through a `let` if needed.)
fn parse_version_pattern_arm(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'_, '_>,
    subject: ExpressionHandle,
) -> Result<Option<(TransitionGuardNode, DestructureBindings)>, ParseError> {
    if !input.at_name_like() {
        return Ok(None);
    }
    let Ok((type_name, after_type)) = input.take_identifier() else {
        return Ok(None);
    };

    let (version_name, after_path) = if after_type.at_punctuation(PunctuationKind::ColonColon) {
        let Ok(after_separator) = after_type.take_punctuation(PunctuationKind::ColonColon, "::")
        else {
            return Ok(None);
        };
        let Ok((segment, rest)) = after_separator.take_identifier() else {
            return Ok(None);
        };
        if !omega_core::versioning::is_version_selector(segment.as_str()) {
            return Ok(None);
        }
        (Some(segment), rest)
    } else {
        (None, after_type)
    };

    if !after_path.at_punctuation(PunctuationKind::LeftParen) {
        return Ok(None);
    }
    let Ok(in_parens) = after_path.take_punctuation(PunctuationKind::LeftParen, "(") else {
        return Ok(None);
    };
    let (binding, after_binding) = if in_parens.at_contextual("_") {
        let Ok(rest) = in_parens.take_contextual("_") else {
            return Ok(None);
        };
        (None, rest)
    } else {
        let Ok((binding, rest)) = in_parens.take_identifier() else {
            return Ok(None);
        };
        (Some(binding), rest)
    };
    let Ok(after_close) = after_binding.take_punctuation(PunctuationKind::RightParen, ")") else {
        return Ok(None);
    };
    if !after_close.tokens.is_empty() {
        return Ok(None);
    }

    // Guard: `subject in Type[::vN]::<marker>` -- the marker keeps version
    // arms distinct from case/domain membership through symbol assignment;
    // the typed lowering turns it into `subject.era == <era index>`.
    let mut domain_members = vec![type_name];
    let payload_member = match &version_name {
        Some(version) => Identifier::generated(
            omega_core::versioning::versioned_payload_field_name(version.as_str()),
        ),
        None => Identifier::generated(
            omega_core::versioning::VERSIONED_CURRENT_PAYLOAD_FIELD.to_owned(),
        ),
    };
    if let Some(version) = version_name {
        domain_members.push(version);
    }
    domain_members.push(Identifier::generated(
        omega_core::versioning::VERSION_ARM_MARKER.to_owned(),
    ));

    let mut domain_start = omega_core::arena::Handle::invalid();
    let mut domain_count = 0u32;
    for member in domain_members {
        let handle = syntax_trees.expressions.append_identifier_path_member(member);
        if domain_count == 0 {
            domain_start = handle;
        }
        domain_count += 1;
    }
    let domain = HandleSpan::from_parts(domain_start, domain_count);

    let guard = TransitionGuardNode::When(syntax_trees.expressions.insert(
        ExpressionNode::Membership(TableMembershipExpression {
            value: subject,
            domain,
        }),
    ));

    let fields = match binding {
        // A version arm binds the whole historical payload (`__payload_vN`), not
        // a case variant -- no variant qualification.
        Some(binding) => vec![DestructureBinding {
            binding,
            member: payload_member,
            case_variant: None,
        }],
        None => Vec::new(),
    };

    Ok(Some((guard, DestructureBindings { subject, fields })))
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
    // A `Type::Case { .. }` pattern (two-member path) binds payload fields of that
    // specific CASE, so tag each binding with the variant -- the rewritten field
    // access must resolve to this variant's field, not a same-named field at a
    // different offset in another variant.
    let case_variant: Option<Identifier> = {
        let members = syntax_trees.expressions.identifier_path_members(path);
        if members.len() == 2 {
            members.last().cloned()
        } else {
            None
        }
    };
    let fields = fields
        .into_iter()
        .map(|name| DestructureBinding::field(name, case_variant.clone()))
        .collect::<Vec<_>>();
    if !pattern_rest.tokens.is_empty() {
        return Err(pattern_rest.error_here("expected data destructure pattern"));
    }

    // A two-member path names a case of the subject's type: the arm matches
    // on the TAG, which is DOMAIN membership (decision 11), so the guard
    // starts with `subject in Type::Case`. Desugaring to membership (not
    // `==`) keeps the synthesized tag test distinct from user-written
    // equality, which rejects bare payload-bearing case names.
    let mut combined = match syntax_trees.expressions.identifier_path_members(path).len() {
        1 => ExpressionHandle::invalid(),
        2 => syntax_trees.expressions.insert(ExpressionNode::Membership(
            TableMembershipExpression {
                value: subject,
                domain: path,
            },
        )),
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
    fields: &[DestructureBinding],
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
            domain: cast.domain,
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
            case_variant: member.case_variant,
        }),
        ExpressionNode::Mutable(inner) => ExpressionNode::Mutable(
            rewrite_destructure_guard_expression(syntax_trees, inner, subject, fields),
        ),
        ExpressionNode::Name(path) => {
            if let Some((field, case_variant)) =
                single_destructured_field_name(syntax_trees, path, fields)
            {
                ExpressionNode::Member(TableMemberExpression {
                    receiver: subject,
                    member: field,
                    case_variant,
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
    fields: &[DestructureBinding],
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
    fields: &[DestructureBinding],
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

/// The `(field member name, case variant)` a single-name reference resolves to
/// when it names a destructure binding -- so the rewritten `subject.member`
/// access carries the variant for correct payload-field offset resolution.
fn single_destructured_field_name(
    syntax_trees: &SyntaxTrees,
    path: HandleSpan<Identifier>,
    fields: &[DestructureBinding],
) -> Option<(Identifier, Option<Identifier>)> {
    let members = syntax_trees.expressions.identifier_path_members(path);
    let [member] = members else {
        return None;
    };
    fields
        .iter()
        .find(|field| &field.binding == member)
        .map(|field| (field.member.clone(), field.case_variant.clone()))
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
