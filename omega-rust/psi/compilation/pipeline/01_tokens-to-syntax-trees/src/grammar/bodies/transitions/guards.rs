use crate::diagnostics::parse_error::ParseError;
use crate::grammar::expressions::parse_expression::parse_expression_handle_without_struct_literals;
use crate::storage::tables::{exprs, exprs_of};
use crate::tokens::token_cursor::{Input, ParseResult, parse_path_handle_span};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, MatchFieldAction, TableBinaryExpression,
    UnaryOperator,
};
use syntax_trees::statement::TableOutcomeProofSelector;
use syntax_trees::statement::TransitionGuardNode;
use tokens::{KeywordKind, PunctuationKind};

pub(super) use crate::grammar::patterns::bindings::{
    DestructureBinding, rewrite_destructure_guard_expression,
};

/// The named bindings a destructure pattern arm introduces: each bound `field`
/// rewrites to `subject.field` in the arm's guard and transition-target arguments.
pub(super) struct DestructureBindings {
    pub(super) subject: ExpressionHandle,
    pub(super) fields: Vec<DestructureBinding>,
    pub(super) pattern: Option<syntax_trees::statement::TransitionPattern>,
    /// Erased named proof outputs selected after the payload/proof `;` split.
    pub(super) proof_selectors: Vec<TableOutcomeProofSelector>,
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

    let (patterns, pattern_rest) = parse_transition_match_list(syntax_trees, pattern_input, true)?;
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
    // becomes the fall-through (a matrix that covers every combination needs
    // no `_ ->` arm; coverage IS the completeness proof).
    let bool_tuple: Option<Vec<Option<bool>>> = patterns
        .iter()
        .map(|pattern| match pattern {
            None => Some(None),
            Some(handle) => match exprs_of(syntax_trees).expression(*handle) {
                ExpressionNode::Boolean(value) => Some(Some(*value)),
                _ => None,
            },
        })
        .collect();
    let mut bindings = Vec::new();
    for (left, right) in subject.iter().copied().zip(patterns) {
        let Some(right) = right else {
            continue;
        };
        retain_value_pattern(syntax_trees, left, right, &mut bindings);
    }

    Ok((TransitionGuardNode::Always, bindings, bool_tuple, rest))
}

/// Whether an arm's parsed pattern already evaluates to a decision -- a
/// comparison, boolean composition or negation, or a membership test. Such
/// arms guard the transition as spelled; equal-matching them against the
/// subject would type `subject == bool` nonsense. Everything else keeps the
/// subject-equality desugar.
fn arm_pattern_is_condition(syntax_trees: &SyntaxTrees, expression: ExpressionHandle) -> bool {
    match exprs_of(syntax_trees).expression(expression) {
        ExpressionNode::Binary(binary) => matches!(
            binary.operator,
            BinaryOperator::And
                | BinaryOperator::Or
                | BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
        ),
        ExpressionNode::Membership(_) => true,
        ExpressionNode::Unary(unary) => unary.operator == UnaryOperator::LogicalNot,
        _ => false,
    }
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
                input.restarted(tuple_tokens),
                Some(
                    input.restarted(
                        guard_tokens_with_if
                            .get(1..)
                            .expect("if keyword split should include guard tokens"),
                    ),
                ),
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
            if !bindings.proof_selectors.is_empty() {
                return Err(component.error_here(
                    "outcome proof selectors are admitted only on a single exact result-case arm",
                ));
            }
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
            retain_value_pattern(syntax_trees, subject, pattern, &mut all_bindings);
        }
    }

    for (binding_index, bindings) in all_bindings.iter().enumerate() {
        for field in &bindings.fields {
            if all_bindings[..binding_index].iter().any(|earlier| {
                earlier.fields.iter().any(|other| {
                    other.binding.as_str(syntax_trees) == field.binding.as_str(syntax_trees)
                })
            }) {
                return Err(input.error_here(format!(
                    "tuple destructure binding `{}` is introduced by more than one subject; rename one binding with `as`",
                    field.binding.as_str(syntax_trees)
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
                    return Err(input
                        .restarted(&tokens[index + 1..])
                        .error_here("expected `if` or `->` after tuple transition pattern"));
                }
                components.push(input.restarted(&tokens[component_start..index]));
                return Ok(components);
            }
            Some(PunctuationKind::LeftBracket) => bracket += 1,
            Some(PunctuationKind::RightBracket) => bracket = bracket.saturating_sub(1),
            Some(PunctuationKind::LeftBrace) => brace += 1,
            Some(PunctuationKind::RightBrace) => brace = brace.saturating_sub(1),
            Some(PunctuationKind::Comma) if paren == 0 && bracket == 0 && brace == 0 => {
                components.push(input.restarted(&tokens[component_start..index]));
                component_start = index + 1;
            }
            _ => {}
        }
    }
    Err(input.error_here("expected `)` to close tuple transition pattern"))
}

/// A named value may resolve to a case, constant or lexical binding. Only
/// declaration selection can distinguish them, independently on every axis.
fn retain_value_pattern(
    syntax_trees: &mut SyntaxTrees,
    subject: ExpressionHandle,
    value: ExpressionHandle,
    bindings: &mut Vec<DestructureBindings>,
) {
    let capture = exprs(syntax_trees).insert_transition_subject(subject);
    let (head, guard) = if matches!(
        exprs_of(syntax_trees).expression(value),
        ExpressionNode::Name(_)
    ) {
        (value, ExpressionHandle::invalid())
    } else if arm_pattern_is_condition(syntax_trees, value) {
        (ExpressionHandle::invalid(), value)
    } else {
        let equality = exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
            left: subject,
            operator: BinaryOperator::Equal,
            right: value,
        }));
        (ExpressionHandle::invalid(), equality)
    };
    bindings.push(DestructureBindings {
        subject,
        fields: Vec::new(),
        pattern: Some(syntax_trees::statement::TransitionPattern {
            subject,
            capture,
            is_structural: false,
            pattern: syntax_trees::expression::TableDestructurePattern {
                head,
                guard,
                has_rest: true,
                ..Default::default()
            },
        }),
        proof_selectors: Vec::new(),
    });
}

fn join_guard_conjunction(
    syntax_trees: &mut SyntaxTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> ExpressionHandle {
    if !left.is_valid() {
        return right;
    }
    exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
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
                input.restarted(pattern_tokens),
                Some(
                    input.restarted(
                        guard_tokens_with_if
                            .get(1..)
                            .expect("if keyword split should include guard tokens"),
                    ),
                ),
            )
        }
        None => (input, None),
    };

    let (path, after_path) = parse_path_handle_span(pattern_input, |member| {
        exprs(syntax_trees).append_identifier_path_member(member)
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

    let (parsed_fields, pattern_rest) =
        crate::grammar::patterns::parse_destructure_fields(syntax_trees, after_path)?;
    let fields = parsed_fields
        .fields
        .iter()
        .map(|field| {
            let binding = match &field.action {
                MatchFieldAction::Bind(binding) => Some(binding.clone()),
                _ => None,
            };
            (field.member.clone(), binding)
        })
        .collect::<Vec<_>>();
    let has_rest = parsed_fields.has_rest;
    let proof_selectors = parsed_fields.proof_selectors;
    let head = exprs(syntax_trees).insert(ExpressionNode::Name(path));
    let capture = exprs(syntax_trees).insert_transition_subject(subject);
    let saved = exprs(syntax_trees).insert(ExpressionNode::SavedSubject(
        syntax_trees::expression::TableSavedSubject {
            capture,
            pattern_head: head,
        },
    ));
    // Waived fields (`as _`) are spelled but introduce NO binding; renamed
    // fields (`as name`) bind the new name to the same `subject.member` read.
    let fields = fields
        .into_iter()
        .filter_map(|(member, binding)| {
            binding.map(|binding| DestructureBinding {
                binding,
                member,
                case_variant: None,
            })
        })
        .collect::<Vec<_>>();
    if !pattern_rest.tokens.is_empty() {
        return Err(pattern_rest.error_here("expected data destructure pattern"));
    }

    // Head classification belongs to declaration selection. Record and case
    // patterns retain the same source shape regardless of path length.
    let mut combined = ExpressionHandle::invalid();
    if let Some(guard_input) = guard_input {
        let (guard, rest) =
            parse_expression_handle_without_struct_literals(syntax_trees, guard_input)?;
        if !rest.tokens.is_empty() {
            return Err(rest.error_here("expected transition pattern guard"));
        }
        let guard = rewrite_destructure_guard_expression(syntax_trees, guard, saved, &fields);
        combined = join_guard_conjunction(syntax_trees, combined, guard);
    }

    let guard = TransitionGuardNode::Always;
    Ok(Some((
        guard,
        DestructureBindings {
            subject: saved,
            fields,
            pattern: Some(syntax_trees::statement::TransitionPattern {
                subject,
                capture,
                is_structural: true,
                pattern: syntax_trees::expression::TableDestructurePattern {
                    application_type: Default::default(),
                    head,
                    fields: exprs(syntax_trees).insert_match_fields(parsed_fields.fields),
                    has_rest,
                    guard: combined,
                    proof_selectors: syntax_trees
                        .statements
                        .insert_outcome_proof_selectors(proof_selectors.iter().cloned()),
                },
            }),
            proof_selectors,
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
