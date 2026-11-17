use crate::errors::parse_error::ParseError;
use crate::expressions::parse::parse_expression_handle_without_struct_literals;
use crate::syntax_storage::tables::{exprs, exprs_of};
use crate::token_input::cursor::{Input, ParseResult, parse_path_handle_span};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, MatchFieldAction, TableBinaryExpression,
    UnaryOperator,
};
use syntax_trees::statement::TableOutcomeProofSelector;
use syntax_trees::statement::TransitionGuardNode;
use tokens::{KeywordKind, PunctuationKind};

pub(crate) use crate::patterns::bindings::{
    DestructureBinding, rewrite_destructure_guard_expression,
};

pub(crate) struct DestructureBindings {
    pub(crate) subject: ExpressionHandle,
    pub(crate) fields: Vec<DestructureBinding>,
    pub(crate) pattern: Option<syntax_trees::statement::TransitionPattern>,
    pub(crate) proof_selectors: Vec<TableOutcomeProofSelector>,
}

pub(crate) fn parse_transition_guard_node<'tokens, 'source>(
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

fn retain_value_pattern(
    syntax_trees: &mut SyntaxTrees,
    subject: ExpressionHandle,
    value: ExpressionHandle,
    bindings: &mut Vec<DestructureBindings>,
) {
    let capture = exprs(syntax_trees).insert_transition_subject(subject);
    let (head, guard) = if matches!(
        exprs_of(syntax_trees).expression(value),
        ExpressionNode::Name { .. }
    ) {
        (value, ExpressionHandle::invalid())
    } else if arm_pattern_is_condition(syntax_trees, value) {
        (ExpressionHandle::invalid(), value)
    } else {
        let equality = exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
            policy: numerics::arithmetic::ArithmeticDomain::Exact,
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
        policy: numerics::arithmetic::ArithmeticDomain::Exact,
        left,
        operator: BinaryOperator::And,
        right,
    }))
}

pub(crate) fn parse_transition_expression_list<'tokens, 'source>(
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
        if guard_input.is_some() {
            return Err(after_path
                .error_here("expected `{` to open a destructure pattern before `if` guard"));
        }
        return Ok(None);
    }

    let (parsed_fields, pattern_rest) =
        crate::patterns::fields::parse_destructure_fields(syntax_trees, after_path)?;
    let fields = parsed_fields
        .fields
        .iter()
        .map(|field| {
            let binding = match &field.action {
                MatchFieldAction::Bind(binding) => Some(binding.clone()),
                _ => None,
            };
            (field.member.clone(), binding, field.component)
        })
        .collect::<Vec<_>>();
    let has_rest = parsed_fields.has_rest;
    let proof_selectors = parsed_fields.proof_selectors;
    let head = exprs(syntax_trees).insert(ExpressionNode::Name {
        members: path,
        application: None,
    });
    let capture = exprs(syntax_trees).insert_transition_subject(subject);
    let saved = exprs(syntax_trees).insert(ExpressionNode::SavedSubject(
        syntax_trees::expression::TableSavedSubject {
            component: Default::default(),
            capture,
            pattern_head: head,
        },
    ));
    let fields = fields
        .into_iter()
        .filter_map(|(member, binding, component)| {
            binding.map(|binding| DestructureBinding {
                component,
                binding,
                member,
                case_variant: None,
            })
        })
        .collect::<Vec<_>>();
    if !pattern_rest.tokens.is_empty() {
        return Err(pattern_rest.error_here("expected data destructure pattern"));
    }

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
