use crate::diagnostics::parse_error::ParseError;
use crate::expressions::parse_expression::parse_expression_handle_without_struct_literals;
use crate::input::token_cursor::{Input, ParseResult, parse_path_handle_span};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableMemberExpression,
    TableMembershipExpression, UnaryOperator,
};
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::TableOutcomeProofSelector;
use syntax_trees::statement::TransitionGuardNode;
use tokens::{KeywordKind, PunctuationKind};

pub(super) use crate::patterns::bindings::{
    DestructureBinding, rewrite_destructure_guard_expression,
};

/// The named bindings a destructure pattern arm introduces: each bound `field`
/// rewrites to `subject.field` in the arm's guard and transition-target arguments.
pub(super) struct DestructureBindings {
    pub(super) subject: ExpressionHandle,
    pub(super) fields: Vec<DestructureBinding>,
    /// The SPELLED field set (bound AND waived) + variant + `..` flag -- the
    /// exhaustiveness law's carrier.
    pub(super) spelling: Option<ArmPatternSpelling>,
    /// Erased named proof outputs selected after the payload/proof `;` split.
    pub(super) proof_selectors: Vec<TableOutcomeProofSelector>,
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
    // becomes the fall-through (a matrix that covers every combination needs
    // no `_ ->` arm; coverage IS the completeness proof).
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
    for (left, right) in subject.iter().copied().zip(patterns) {
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
            // A condition-shaped pattern already evaluates to a decision:
            // `transition text.len { text.len == 0 -> .. }` guards by the
            // authored comparison rather than comparing the subject against
            // a boolean (`subject == bool` cannot type). Value patterns keep
            // their subject-equality desugar.
            _ if arm_pattern_is_condition(syntax_trees, right) => right,
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

/// Whether an arm's parsed pattern already evaluates to a decision -- a
/// comparison, boolean composition or negation, or a membership test. Such
/// arms guard the transition as spelled; equal-matching them against the
/// subject would type `subject == bool` nonsense. Everything else keeps the
/// subject-equality desugar.
fn arm_pattern_is_condition(syntax_trees: &SyntaxTrees, expression: ExpressionHandle) -> bool {
    match syntax_trees.expressions.expression(expression) {
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
            let constraint = pattern_constraint(syntax_trees, subject, pattern);
            combined = join_guard_conjunction(syntax_trees, combined, constraint);
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

    let (parsed_fields, pattern_rest) =
        crate::patterns::parse_destructure_fields(syntax_trees, after_path)?;
    let fields = parsed_fields
        .fields
        .iter()
        .map(|field| {
            let binding = match &field.action {
                syntax_trees::expression::MatchFieldAction::Bind(binding) => Some(binding.clone()),
                _ => None,
            };
            (field.member.clone(), binding)
        })
        .collect::<Vec<_>>();
    let matched_fields = parsed_fields
        .fields
        .iter()
        .filter_map(|field| match field.action {
            syntax_trees::expression::MatchFieldAction::Equal(value) => {
                Some((field.member.clone(), value))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let has_rest = parsed_fields.has_rest;
    let proof_selectors = parsed_fields.proof_selectors;
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
    if !proof_selectors.is_empty() && case_variant.is_none() {
        return Err(pattern_input.error_here(
            "outcome proof selectors require an exact `Result::Case { payload; selector: local }` arm",
        ));
    }
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
