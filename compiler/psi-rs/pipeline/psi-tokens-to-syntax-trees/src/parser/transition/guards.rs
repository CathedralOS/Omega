use crate::parse_error::ParseError;
use crate::parser::expression::parse_expression_handle_without_struct_literals;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use psi_arena::HandleSpan;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableAtomicExpression, TableBinaryExpression,
    TableCallExpression, TableCastExpression, TableIndexedExpression, TableMemberExpression,
    TableMembershipExpression, TableRangeExpression, TableStructLiteral, TableStructLiteralField,
    TableUnaryExpression,
};
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::statement::TransitionGuardNode;
use psi_tokens::{KeywordKind, PunctuationKind};

/// One named binding a pattern arm introduces: uses of `binding` rewrite to
/// `subject.member` in the arm's guard and transition-target arguments. For
/// field destructures the two names coincide (`{ text }` binds `text` to
/// `subject.text`).
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

/// The named bindings a destructure pattern arm introduces: each bound `field`
/// rewrites to `subject.field` in the arm's guard and transition-target arguments.
pub(super) struct DestructureBindings {
    pub(super) subject: ExpressionHandle,
    pub(super) fields: Vec<DestructureBinding>,
    /// The SPELLED field set (bound AND waived) + variant + `..` flag -- the
    /// exhaustiveness law's carrier.
    pub(super) spelling: Option<ArmPatternSpelling>,
}

/// What a destructure arm SPELLED, for the exhaustiveness law: a `..`-free
/// pattern must mention every field of the record (variant `None`) or of the
/// named case's payload. The block parser encodes this into a marker let
/// (`__arm_destructure#V=<variant>#<f1>#<f2>`) that the typed-stage
/// validation resolves against the data definition.
pub(super) struct ArmPatternSpelling {
    pub(super) variant: Option<Identifier>,
    pub(super) members: Vec<Identifier>,
    pub(super) has_rest: bool,
}

pub(super) fn parse_transition_guard_node<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    subject: &[ExpressionHandle],
) -> Result<
    (
        TransitionGuardNode,
        Vec<DestructureBindings>,
        Option<Vec<Option<bool>>>,
        Input<'tokens, 'source>,
    ),
    ParseError,
> {
    let (pattern_input, rest) =
        input.split_at_top_level_punctuation(PunctuationKind::Arrow, "expected `->`")?;

    if subject.len() == 1
        && let Some((guard, bindings)) =
            parse_destructure_pattern_arm(syntax_trees, pattern_input, subject[0])?
    {
        return Ok((guard, vec![bindings], None, rest));
    }

    if subject.len() >= 2
        && let Some((guard, bindings)) =
            parse_tuple_destructure_pattern_arm(syntax_trees, pattern_input, subject)?
    {
        return Ok((guard, bindings, None, rest));
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
            return Ok((guard, Vec::new(), None, rest));
        }
        return Err(input.error_here("anonymous transition blocks do not support tuple patterns"));
    }

    if patterns.len() == 1 && patterns[0].is_none() {
        return Ok((TransitionGuardNode::Always, Vec::new(), None, rest));
    }

    if subject.len() != patterns.len() {
        return Err(input.error_here(format!(
            "transition pattern arity {} does not match subject arity {}",
            patterns.len(),
            subject.len()
        )));
    }

    // The arm's BOOL-TUPLE shape (bool literals / `_` wildcards only) feeds
    // the block-level exhaustiveness rewrite: a covering matrix's last arm
    // becomes the fall-through (ch4's canonical `(found, has_next)` example
    // has no `_ ->` arm; coverage IS the completeness proof).
    let bool_tuple: Option<Vec<Option<bool>>> = patterns
        .iter()
        .map(|pattern| match pattern {
            None => Some(None),
            Some(handle) => match syntax_trees.expressions.expression(*handle) {
                ExpressionNode::Boolean(value) => Some(Some(*value)),
                _ => None,
            },
        })
        .collect();
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
                if syntax_trees
                    .expressions
                    .identifier_path_members(*path)
                    .len()
                    == 2 =>
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
        Vec::new(),
        bool_tuple,
        rest,
    ))
}

/// Parse a tuple arm containing one or more record/case destructures:
/// `(Packet::Data { byte }, Header { version }, _) [if predicate]`.
/// Each component is paired with its own subject, while bindings from every
/// component are in scope for the shared `if` guard and transition target.
fn parse_tuple_destructure_pattern_arm<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    subjects: &[ExpressionHandle],
) -> Result<Option<(TransitionGuardNode, Vec<DestructureBindings>)>, ParseError> {
    if !input.at_punctuation(PunctuationKind::LeftParen) {
        return Ok(None);
    }

    let (tuple_input, guard_input) = match find_top_level_keyword(input, KeywordKind::If) {
        Some(if_index) => {
            let (tuple_tokens, guard_tokens_with_if) = input.tokens.split_at(if_index);
            (
                Input::new(input.source_id, tuple_tokens),
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
    let components = split_tuple_pattern_components(tuple_input)?;
    if !components.iter().any(|component| {
        component
            .tokens
            .iter()
            .any(|token| token.punctuation() == Some(PunctuationKind::LeftBrace))
    }) {
        return Ok(None);
    }
    if components.len() != subjects.len() {
        return Err(input.error_here(format!(
            "transition pattern arity {} does not match subject arity {}",
            components.len(),
            subjects.len()
        )));
    }

    let mut combined = ExpressionHandle::invalid();
    let mut all_bindings = Vec::new();
    for (component, subject) in components.into_iter().zip(subjects.iter().copied()) {
        if component.at_contextual("_") && component.tokens.len() == 1 {
            continue;
        }
        if let Some((guard, bindings)) =
            parse_destructure_pattern_arm(syntax_trees, component, subject)?
        {
            if let TransitionGuardNode::When(expression) = guard {
                combined = join_guard_conjunction(syntax_trees, combined, expression);
            }
            all_bindings.push(bindings);
            continue;
        }

        let (pattern, rest) = parse_transition_match_component::<Option<ExpressionHandle>>(
            syntax_trees,
            component,
            true,
        )?;
        if !rest.tokens.is_empty() {
            return Err(rest.error_here("expected tuple transition pattern component"));
        }
        if let Some(pattern) = pattern {
            let constraint = pattern_constraint(syntax_trees, subject, pattern);
            combined = join_guard_conjunction(syntax_trees, combined, constraint);
        }
    }

    for (binding_index, bindings) in all_bindings.iter().enumerate() {
        for field in &bindings.fields {
            if all_bindings[..binding_index].iter().any(|earlier| {
                earlier
                    .fields
                    .iter()
                    .any(|other| other.binding.as_str() == field.binding.as_str())
            }) {
                return Err(input.error_here(format!(
                    "tuple destructure binding `{}` is introduced by more than one subject; rename one binding with `as`",
                    field.binding.as_str()
                )));
            }
        }
    }

    if let Some(guard_input) = guard_input {
        let (guard, rest) =
            parse_expression_handle_without_struct_literals(syntax_trees, guard_input)?;
        if !rest.tokens.is_empty() {
            return Err(rest.error_here("expected transition pattern guard"));
        }
        let guard = all_bindings.iter().fold(guard, |guard, bindings| {
            rewrite_destructure_guard_expression(
                syntax_trees,
                guard,
                bindings.subject,
                &bindings.fields,
            )
        });
        combined = join_guard_conjunction(syntax_trees, combined, guard);
    }

    Ok(Some((
        if combined.is_valid() {
            TransitionGuardNode::When(combined)
        } else {
            TransitionGuardNode::Always
        },
        all_bindings,
    )))
}

fn split_tuple_pattern_components<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> Result<Vec<Input<'tokens, 'source>>, ParseError> {
    let tokens = input.tokens;
    if tokens.first().and_then(|token| token.punctuation()) != Some(PunctuationKind::LeftParen) {
        return Err(input.error_here("expected `(` to open tuple transition pattern"));
    }

    let mut paren = 0usize;
    let mut bracket = 0usize;
    let mut brace = 0usize;
    let mut component_start = 1usize;
    let mut components = Vec::new();
    for (index, token) in tokens.iter().enumerate().skip(1) {
        match token.punctuation() {
            Some(PunctuationKind::LeftParen) => paren += 1,
            Some(PunctuationKind::RightParen) if paren > 0 => paren -= 1,
            Some(PunctuationKind::RightParen) if bracket == 0 && brace == 0 => {
                if tokens[index + 1..]
                    .iter()
                    .any(|token| !token.is_non_semantic())
                {
                    return Err(Input::new(input.source_id, &tokens[index + 1..])
                        .error_here("expected `if` or `->` after tuple transition pattern"));
                }
                components.push(Input::new(input.source_id, &tokens[component_start..index]));
                return Ok(components);
            }
            Some(PunctuationKind::LeftBracket) => bracket += 1,
            Some(PunctuationKind::RightBracket) => bracket = bracket.saturating_sub(1),
            Some(PunctuationKind::LeftBrace) => brace += 1,
            Some(PunctuationKind::RightBrace) => brace = brace.saturating_sub(1),
            Some(PunctuationKind::Comma) if paren == 0 && bracket == 0 && brace == 0 => {
                components.push(Input::new(input.source_id, &tokens[component_start..index]));
                component_start = index + 1;
            }
            _ => {}
        }
    }
    Err(input.error_here("expected `)` to close tuple transition pattern"))
}

fn pattern_constraint(
    syntax_trees: &mut SyntaxTrees,
    subject: ExpressionHandle,
    pattern: ExpressionHandle,
) -> ExpressionHandle {
    match syntax_trees.expressions.expression(pattern).clone() {
        ExpressionNode::Name(path)
            if syntax_trees.expressions.identifier_path_members(path).len() == 2 =>
        {
            syntax_trees
                .expressions
                .insert(ExpressionNode::Membership(TableMembershipExpression {
                    value: subject,
                    domain: path,
                }))
        }
        _ => syntax_trees
            .expressions
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: subject,
                operator: BinaryOperator::Equal,
                right: pattern,
            })),
    }
}

fn join_guard_conjunction(
    syntax_trees: &mut SyntaxTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> ExpressionHandle {
    if !left.is_valid() {
        return right;
    }
    syntax_trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator: BinaryOperator::And,
            right,
        }))
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
    // A leading `(` is a tuple/pattern list ONLY when it is a PATTERN context
    // (`allow_wildcard`, e.g. `(A, B) ->` / `(_) ->`) or the group carries a
    // top-level comma (`(a, b)`). A guard SUBJECT `( expr )` with no top-level
    // comma -- `(self.x as i8) < 0`, `(a + b) > c`, `(a || b) && c` -- is a single
    // parenthesized expression: fall through to the general expression parser
    // (`parse_transition_match_component`), which handles the leading paren.
    if input.at_punctuation(PunctuationKind::LeftParen)
        && (allow_wildcard || input.leading_paren_group_has_top_level_comma())
    {
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
/// - `Type { field, fixed: value, .. } [if guard] -> ...` -- a record
///   destructure: no tag compare (the subject IS that type); bare fields bind,
///   while `fixed: value` contributes `subject.fixed == value` to the arm
///   guard. Case payload patterns use the same field-value spelling.
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

    let ((fields, matched_fields, has_rest), pattern_rest) =
        parse_data_destructure_pattern_fields(syntax_trees, after_path)?;
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
    // Waived fields (`as _`) are spelled but introduce NO binding; renamed
    // fields (`as name`) bind the new name to the same `subject.member` read.
    let spelled_members: Vec<Identifier> =
        fields.iter().map(|(member, _)| member.clone()).collect();
    let fields = fields
        .into_iter()
        .filter_map(|(member, binding)| {
            binding.map(|binding| DestructureBinding {
                binding,
                member,
                case_variant: case_variant.clone(),
            })
        })
        .collect::<Vec<_>>();
    let spelling = Some(ArmPatternSpelling {
        variant: case_variant.clone(),
        members: spelled_members,
        has_rest,
    });
    if !pattern_rest.tokens.is_empty() {
        return Err(pattern_rest.error_here("expected data destructure pattern"));
    }

    // A two-member path names a case of the subject's type: the arm matches
    // on the TAG, which is DOMAIN membership (decision 11), so the guard
    // starts with `subject in Type::Case`. Desugaring to membership (not
    // `==`) keeps the synthesized tag test distinct from user-written
    // equality, which rejects bare payload-bearing case names.
    let mut combined =
        match syntax_trees.expressions.identifier_path_members(path).len() {
            1 => ExpressionHandle::invalid(),
            2 => syntax_trees.expressions.insert(ExpressionNode::Membership(
                TableMembershipExpression {
                    value: subject,
                    domain: path,
                },
            )),
            _ => {
                return Err(pattern_input.error_here(
                    "destructure pattern path must be `Type { .. }` or `Type::Case { .. }`",
                ));
            }
        };

    // A value-bearing field pattern is ordinary equality over an attenuated
    // field projection.  Keeping it as a real guard means the existing proof
    // and flow machinery establishes `subject.field == expected` inside the
    // arm; no pattern-only fact channel exists.
    for (member, expected) in matched_fields {
        let projected =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver: subject,
                    member,
                    case_variant: case_variant.clone(),
                }));
        let equality =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: projected,
                    operator: BinaryOperator::Equal,
                    right: expected,
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
    Ok(Some((
        guard,
        DestructureBindings {
            subject,
            fields,
            spelling,
        },
    )))
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

/// Parse the `{ field, fixed: value, .. }` part of a destructure pattern (the leading path is
/// already consumed by the caller). Arm position SHARES the record-pattern
/// field grammar (owner spec 2026-07-18): `field as name` renames the binding
/// and `field as _` waives it (spelled but unbound). `..` stays the arm-only
/// rest escape (predates the spec; the LET form has no `..` -- its
/// exhaustiveness law makes waivers explicit instead). Each entry is
/// `(member, binding)`: `binding = None` for a waived field. A `field: value`
/// entry is spelled for exhaustiveness but introduces no binding; it contributes
/// a field-equality guard returned in the second vector.
fn parse_data_destructure_pattern_fields<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<
    'tokens,
    'source,
    (
        Vec<(Identifier, Option<Identifier>)>,
        Vec<(Identifier, ExpressionHandle)>,
        bool,
    ),
> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut fields = Vec::new();
    let mut matched_fields = Vec::new();
    let mut has_rest = false;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_punctuation(PunctuationKind::DotDot) {
            has_rest = true;
            input = input.take_punctuation(PunctuationKind::DotDot, "..")?;
        } else {
            let (field, rest) = input.take_identifier()?;
            if rest.at_punctuation(PunctuationKind::Colon) {
                let after_colon = rest.take_punctuation(PunctuationKind::Colon, ":")?;
                let (expected, rest) =
                    parse_expression_handle_without_struct_literals(syntax_trees, after_colon)?;
                fields.push((field.clone(), None));
                matched_fields.push((field, expected));
                input = rest;
                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    continue;
                }
                break;
            }
            let mut binding = Some(field.clone());
            let mut rest = rest;
            if rest.at_keyword(KeywordKind::As) {
                let after_as = rest.take_keyword(KeywordKind::As, "as")?;
                if after_as.at_contextual("_") {
                    binding = None;
                    rest = after_as.take_contextual("_")?;
                } else {
                    let (renamed, after_renamed) = after_as.take_identifier()?;
                    if renamed.as_str() == "_" {
                        binding = None;
                    } else {
                        binding = Some(renamed);
                    }
                    rest = after_renamed;
                }
            }
            fields.push((field, binding));
            input = rest;
        }

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
        } else {
            break;
        }
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok(((fields, matched_fields, has_rest), input))
}

pub(super) fn rewrite_destructure_guard_expression(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
    subject: ExpressionHandle,
    fields: &[DestructureBinding],
) -> ExpressionHandle {
    let rewritten = match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Atomic(atomic) => ExpressionNode::Atomic(TableAtomicExpression {
            value: rewrite_destructure_guard_expression(
                syntax_trees,
                atomic.value,
                subject,
                fields,
            ),
            result: if atomic.result.is_valid() {
                rewrite_destructure_guard_expression(syntax_trees, atomic.result, subject, fields)
            } else {
                ExpressionHandle::invalid()
            },
            ordering: atomic.ordering,
        }),
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
            machine_arguments: call.machine_arguments,
            arguments: rewrite_expression_span(syntax_trees, call.arguments, subject, fields),
            evidence_arguments: call.evidence_arguments,
            operational_acknowledgement: call.operational_acknowledgement,
        }),
        ExpressionNode::Cast(cast) => ExpressionNode::Cast(TableCastExpression {
            value: rewrite_destructure_guard_expression(syntax_trees, cast.value, subject, fields),
            target_type: cast.target_type,
            target_label: cast.target_label,
            domain: cast.domain,
            semantic_domain: cast.semantic_domain,
            semantic_domain_arguments: cast.semantic_domain_arguments,
            form: cast.form,
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
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => syntax_trees.expressions.expression(expression).clone(),
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
