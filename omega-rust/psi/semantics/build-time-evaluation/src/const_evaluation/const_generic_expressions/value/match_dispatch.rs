//! Scalar Match joins and anonymous-result landing before selective execution.

mod rational_bounds;

use super::{Shape, Value, land_anonymous, primitive};
use diagnostics::Diagnostic;
use numerics::bignum::BigRational;
use numerics::literals::LandedIntegerType;
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern},
    types::PrimitiveType,
};

#[derive(Clone, Copy)]
pub(super) struct MatchPlan {
    pub expression: ExpressionHandle,
    pub subject: Shape,
    pub result: Shape,
}

/// Invocation-local subject snapshot, never a published constant or runtime IR.
pub(super) enum MatchSubject {
    Scalar(Value),
    Anonymous(BigRational),
}

impl MatchSubject {
    pub(super) fn new(
        program: &TypedTrees,
        value: Value,
        selected: &[(ExpressionHandle, ExpressionHandle)],
        builtin: impl FnMut(ExpressionHandle) -> bool,
    ) -> Result<Self, String> {
        if let Value::Anonymous(expression) = value {
            validation::evaluate_anonymous_numeric_expression_with_selected_match_arms(
                program, expression, selected, builtin,
            )
            .map(Self::Anonymous)
            .ok_or("constant Match needs a defined anonymous subject".into())
        } else {
            Ok(Self::Scalar(value))
        }
    }

    pub(super) fn matches(
        &self,
        program: &TypedTrees,
        pattern: Value,
        selected: &[(ExpressionHandle, ExpressionHandle)],
        builtin: impl FnMut(ExpressionHandle) -> bool,
    ) -> Result<bool, String> {
        match (self, pattern) {
            (Self::Anonymous(subject), Value::Anonymous(expression)) => {
                let pattern =
                    validation::evaluate_anonymous_numeric_expression_with_selected_match_arms(
                        program, expression, selected, builtin,
                    )
                    .ok_or("constant Match needs a defined anonymous pattern")?;
                Ok(subject.cmp_value(&pattern).is_eq())
            }
            (Self::Scalar(subject), pattern) => {
                let Value::Boolean(matched) = super::apply(
                    typed_trees::expression::BinaryOperator::Equal,
                    *subject,
                    pattern,
                )?
                else {
                    return Err("constant Match equality did not produce Boolean".into());
                };
                Ok(matched)
            }
            _ => Err("constant Match subject and pattern changed scalar types".into()),
        }
    }
}

/// Check every edge before calling recursive ordinary type readers.
pub(super) fn validate_graph(program: &TypedTrees, root: ExpressionHandle) -> Result<(), String> {
    let mut pending = vec![(root, false)];
    let mut active = Vec::new();
    let mut complete = Vec::new();
    while let Some((expression, finish)) = pending.pop() {
        if finish {
            if active.pop() != Some(expression) {
                return Err("invalid constant expression traversal".into());
            }
            complete.push(expression);
            continue;
        }
        if !program.expression_table.expression_is_valid(expression) || active.contains(&expression)
        {
            return Err("invalid or cyclic constant expression".into());
        }
        if complete.contains(&expression) {
            continue;
        }
        active.push(expression);
        pending.push((expression, true));
        match program.expression_table.expression(expression) {
            ExpressionNode::Call(call) => {
                let arguments = program.expression_table.expression_handles(call.arguments);
                if arguments.len() != call.arguments.len() {
                    return Err("invalid constant call argument span".into());
                }
                // A path-qualified call has no evaluated receiver. The call
                // admission owner separately proves that static classification.
                pending.extend(arguments.iter().rev().map(|argument| (*argument, false)));
            }
            ExpressionNode::Match(dispatch) => {
                let arms = program.expression_table.match_arms(dispatch.arms);
                if arms.len() != dispatch.arms.len() || arms.is_empty() {
                    return Err("invalid or empty constant Match arm span".into());
                }
                for arm in arms.iter().rev() {
                    pending.push((arm.value, false));
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        pending.push((pattern, false));
                    }
                }
                pending.push((dispatch.subject, false));
            }
            ExpressionNode::Binary(binary) => {
                pending.push((binary.right, false));
                pending.push((binary.left, false));
            }
            ExpressionNode::Boolean(_) | ExpressionNode::Integer(_) | ExpressionNode::Float(_) => {}
            _ => return Err("unsupported node in exact scalar constant expression".into()),
        }
    }
    Ok(())
}

pub(super) fn validate_join(
    program: &TypedTrees,
    context: super::EvaluationContext<'_>,
    expression: ExpressionHandle,
    shapes: &mut Vec<Shape>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<MatchPlan, String> {
    let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) else {
        return Err("constant Match plan lost its dispatch".into());
    };
    let arms = program.expression_table.match_arms(dispatch.arms);
    let child_count = 1 + arms
        .iter()
        .map(|arm| 1 + usize::from(matches!(arm.pattern, MatchPattern::Value(_))))
        .sum::<usize>();
    let start = shapes
        .len()
        .checked_sub(child_count)
        .ok_or("constant Match lost child types")?;
    let children = shapes.split_off(start);
    let mut children = children.into_iter();
    let subject = children.next().ok_or("constant Match lost subject type")?;
    let mut patterns = vec![subject];
    let mut results = Vec::new();
    for arm in arms {
        if matches!(arm.pattern, MatchPattern::Value(_)) {
            patterns.push(children.next().ok_or("constant Match lost pattern type")?);
        }
        results.push(children.next().ok_or("constant Match lost result type")?);
    }
    let subject = join(program, context, &patterns, warnings)?;
    let result = join(program, context, &results, warnings)?;
    let result = if matches!(result, Shape::Anonymous(_)) {
        Shape::Anonymous(expression)
    } else {
        result
    };
    let mut diagnostics = Vec::new();
    if let super::EvaluationContext::Machine(machine, state) = context {
        validation::validate_match_dispatch(
            program,
            machine,
            state,
            expression,
            dispatch,
            &mut diagnostics,
        );
    } else {
        // The scalar shape join above already checks every subject/pattern and
        // result carrier. Closed positions have no local storage selections.
        let wildcard = arms
            .iter()
            .any(|arm| matches!(arm.pattern, MatchPattern::Wildcard));
        let covered = [false, true].into_iter().all(|expected| arms.iter().any(|arm| matches!(arm.pattern, MatchPattern::Value(pattern) if matches!(program.expression_table.expression(pattern), ExpressionNode::Boolean(value) if *value == expected))));
        if !wildcard && !(matches!(subject, Shape::Boolean) && covered) {
            return Err("match requires exhaustive Boolean patterns or a wildcard".into());
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>()
            .join("; "));
    }
    Ok(MatchPlan {
        expression,
        subject,
        result,
    })
}

fn join(
    program: &TypedTrees,
    context: super::EvaluationContext<'_>,
    shapes: &[Shape],
    warnings: &mut Vec<Diagnostic>,
) -> Result<Shape, String> {
    let peer = shapes
        .iter()
        .copied()
        .find(|shape| !matches!(shape, Shape::Anonymous(_)))
        .or_else(|| shapes.first().copied())
        .ok_or("empty constant Match type join")?;
    for shape in shapes {
        match (*shape, peer) {
            (Shape::Boolean, Shape::Boolean) => {}
            (Shape::Anonymous(expression), Shape::Anonymous(_)) => {
                validate_anonymous_fragments(program, context, expression)?;
            }
            (Shape::Integer(left, _), Shape::Integer(right, _)) if left == right => {}
            (Shape::Anonymous(expression), Shape::Integer(carrier, _)) => {
                validate_landing(program, context, expression, primitive(carrier)?, warnings)?;
            }
            _ => {
                return Err(
                    "constant Match has incompatible subject, pattern or result types".into(),
                );
            }
        }
    }
    context.joined_result(program, shapes, peer)
}

pub(super) fn validate_landing(
    program: &TypedTrees,
    context: super::EvaluationContext<'_>,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    warnings: &mut Vec<Diagnostic>,
) -> Result<LandedIntegerType, String> {
    let carrier = match destination {
        PrimitiveType::I8 => LandedIntegerType::I8,
        PrimitiveType::I16 => LandedIntegerType::I16,
        PrimitiveType::I32 => LandedIntegerType::I32,
        PrimitiveType::I64 => LandedIntegerType::I64,
        PrimitiveType::U8 => LandedIntegerType::U8,
        PrimitiveType::U16 => LandedIntegerType::U16,
        PrimitiveType::U32 => LandedIntegerType::U32,
        PrimitiveType::U64 => LandedIntegerType::U64,
        _ => return Err("anonymous constant destination is not an exact integer carrier".into()),
    };
    // A destination follows direct result edges, not anonymous arithmetic
    // operands. Landing each inner Match before an enclosing multiplication
    // would reject valid fractional intermediates. Nor does this construct a
    // Cartesian product of independent branch choices: selected calculations
    // land once during execution, after their dispatch subjects are demanded.
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) {
            pending.extend(
                program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .map(|arm| arm.value),
            );
        } else if contains_match(program, expression) {
            validate_anonymous_fragments(program, context, expression)?;
            let fractional_history = rational_bounds::validate_integer_landing(
                program,
                expression,
                carrier,
                |operand| context.has_builtin(program, operand),
            )?;
            if fractional_history {
                validate_fractional_landings(program, context, expression, destination, warnings)?;
            }
        } else {
            land_anonymous(program, context, expression, destination, &[], warnings)?;
        }
    }
    Ok(carrier)
}

/// One reachable result of a match-containing anonymous subtree: its exact
/// rational value, the first fractional intermediate the path records, and a
/// selection reproducing both. The first fractional node in left-to-right
/// evaluation order is the warning's origin; its own subtree value under the
/// path is the reported intermediate.
struct LandingSummary {
    value: BigRational,
    fractional: Option<(ExpressionHandle, BigRational)>,
    selection: Vec<(ExpressionHandle, ExpressionHandle)>,
}

/// Bounds cannot invent the exact final value required by a fractional
/// warning, so each distinct reachable (result, origin) pair replays its own
/// recorded selection through the ordinary landing, which emits the shared
/// warning unchanged. Summaries combine at the value level: a Match unions its
/// arms' entries and a binary joins two bounded entry sets, so independent
/// dispatches contribute their own evidence instead of a Cartesian enumeration
/// of arm combinations. Subjects and patterns are never evaluated.
fn validate_fractional_landings(
    program: &TypedTrees,
    context: super::EvaluationContext<'_>,
    root: ExpressionHandle,
    destination: PrimitiveType,
    warnings: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    for summary in landing_summaries(program, context, root)? {
        land_anonymous(
            program,
            context,
            root,
            destination,
            &summary.selection,
            warnings,
        )?;
    }
    Ok(())
}

fn landing_summaries(
    program: &TypedTrees,
    context: super::EvaluationContext<'_>,
    root: ExpressionHandle,
) -> Result<Vec<LandingSummary>, String> {
    enum Step {
        Enter(ExpressionHandle),
        Leave(ExpressionHandle),
    }
    let mut pending = vec![Step::Enter(root)];
    let mut results: Vec<Vec<LandingSummary>> = Vec::new();
    while let Some(step) = pending.pop() {
        match step {
            Step::Enter(expression) => match program.expression_table.expression(expression) {
                ExpressionNode::Match(dispatch) => {
                    let arms = program.expression_table.match_arms(dispatch.arms);
                    if arms.is_empty() || arms.len() != dispatch.arms.len() {
                        return Err("anonymous constant Match landing requires valid arms".into());
                    }
                    pending.push(Step::Leave(expression));
                    pending.extend(arms.iter().rev().map(|arm| Step::Enter(arm.value)));
                }
                ExpressionNode::Binary(binary) => {
                    if !context.has_builtin(program, expression) {
                        return Err(
                            "anonymous rational bounds require selected builtin meaning".into()
                        );
                    }
                    pending.push(Step::Leave(expression));
                    pending.push(Step::Enter(binary.right));
                    pending.push(Step::Enter(binary.left));
                }
                _ => {
                    let value =
                        validation::evaluate_anonymous_numeric_expression_with_selected_match_arms(
                            program,
                            expression,
                            &[],
                            |operand| context.has_builtin(program, operand),
                        )
                        .ok_or(
                            "anonymous constant expression requires defined exact numeric values",
                        )?;
                    let fractional = (matches!(
                        program.expression_table.expression(expression),
                        ExpressionNode::Float(_)
                    ) && value.to_integer_exact().is_none())
                    .then_some((expression, value.clone()));
                    results.push(vec![LandingSummary {
                        value,
                        fractional,
                        selection: Vec::new(),
                    }]);
                }
            },
            Step::Leave(expression) => match program.expression_table.expression(expression) {
                ExpressionNode::Match(dispatch) => {
                    let arms = program.expression_table.match_arms(dispatch.arms);
                    let count = arms.len();
                    if results.len() < count {
                        return Err("anonymous constant Match lost its arm results".into());
                    }
                    let selected = results.split_off(results.len() - count);
                    let mut joined = Vec::new();
                    for (arm, mut arm_results) in arms.iter().zip(selected.into_iter()) {
                        for summary in &mut arm_results {
                            summary.selection.push((expression, arm.value));
                        }
                        append_unique(&mut joined, arm_results)?;
                    }
                    results.push(joined);
                }
                ExpressionNode::Binary(binary) => {
                    let Some(right) = results.pop() else {
                        return Err("anonymous constant operation lost its right operand".into());
                    };
                    let Some(left) = results.pop() else {
                        return Err("anonymous constant operation lost its left operand".into());
                    };
                    let mut joined = Vec::new();
                    for left in &left {
                        for right in &right {
                            let value = match binary.operator {
                                BinaryOperator::Add => left.value.add(&right.value),
                                BinaryOperator::Subtract => left.value.sub(&right.value),
                                BinaryOperator::Multiply => left.value.mul(&right.value),
                                BinaryOperator::Divide => left
                                    .value
                                    .div(&right.value)
                                    .ok_or("undefined anonymous rational quotient")?,
                                _ => {
                                    return Err(
                                        "anonymous rational bounds require arithmetic meaning"
                                            .into(),
                                    );
                                }
                            };
                            let fractional = if let Some(fractional) = &left.fractional {
                                Some(fractional.clone())
                            } else if let Some(fractional) = &right.fractional {
                                Some(fractional.clone())
                            } else {
                                (value.to_integer_exact().is_none())
                                    .then_some((expression, value.clone()))
                            };
                            let mut selection = left.selection.clone();
                            selection.extend(right.selection.iter().copied());
                            push_unique(
                                &mut joined,
                                LandingSummary {
                                    value,
                                    fractional,
                                    selection,
                                },
                            )?;
                        }
                    }
                    results.push(joined);
                }
                _ => return Err("anonymous constant Match landing lost its traversal".into()),
            },
        }
    }
    if results.len() != 1 {
        return Err("anonymous constant expression did not produce one summary".into());
    }
    results
        .pop()
        .ok_or("anonymous constant expression did not produce one summary".into())
}

/// Distinct (value, fractional-origin) pairs are the warnings the authored
/// tree can emit; identical pairs share one diagnostic, so the recorded
/// selection of either path reproduces it.
fn push_unique(summaries: &mut Vec<LandingSummary>, summary: LandingSummary) -> Result<(), String> {
    let duplicate = summaries.iter().any(|existing| {
        existing.value.cmp_value(&summary.value).is_eq()
            && match (&existing.fractional, &summary.fractional) {
                (None, None) => true,
                (Some((left_origin, left_value)), Some((right_origin, right_value))) => {
                    left_origin == right_origin && left_value.cmp_value(right_value).is_eq()
                }
                _ => false,
            }
    });
    if !duplicate {
        if summaries.len() >= MAX_LANDING_SUMMARIES {
            return Err(
                "anonymous constant Match landing has more exact fractional-warning paths than the bounded summary"
                    .into(),
            );
        }
        summaries.push(summary);
    }
    Ok(())
}

fn append_unique(
    summaries: &mut Vec<LandingSummary>,
    arm_results: Vec<LandingSummary>,
) -> Result<(), String> {
    for summary in arm_results {
        // Duplicate (value, origin) pairs would emit an identical diagnostic;
        // the bound still applies because distinct arms can exceed it joined.
        push_unique(summaries, summary)?;
    }
    Ok(())
}

/// The summary bound keeps the emitted warning roster finite: each retained
/// entry still replays one exact selection through the ordinary landing.
const MAX_LANDING_SUMMARIES: usize = 4096;

fn contains_match(program: &TypedTrees, root: ExpressionHandle) -> bool {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(_) => return true,
            ExpressionNode::Binary(binary) => {
                pending.push(binary.right);
                pending.push(binary.left);
            }
            _ => {}
        }
    }
    false
}

pub(super) fn validate_anonymous_fragments(
    program: &TypedTrees,
    context: super::EvaluationContext<'_>,
    root: ExpressionHandle,
) -> Result<(), String> {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) {
            pending.extend(
                program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .map(|arm| arm.value),
            );
        } else if contains_match(program, expression) {
            let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
            else {
                return Err("invalid anonymous result graph".into());
            };
            if binary.operator == typed_trees::expression::BinaryOperator::Divide {
                validate_nonzero_divisor(program, context, binary.right)?;
            }
            pending.push(binary.right);
            pending.push(binary.left);
        } else {
            validation::evaluate_anonymous_numeric_expression_with_selected_match_arms(
                program,
                expression,
                &[],
                |operand| context.has_builtin(program, operand),
            )
            .ok_or("anonymous constant expression requires defined exact numeric values")?;
        }
    }
    Ok(())
}

fn validate_nonzero_divisor(
    program: &TypedTrees,
    context: super::EvaluationContext<'_>,
    root: ExpressionHandle,
) -> Result<(), String> {
    // Direct dispatch and surrounding arithmetic use one result-domain proof.
    // Keeping opposite signs separate avoids a special path for bare Match.
    if !rational_bounds::excludes_zero(program, root, |operand| {
        context.has_builtin(program, operand)
    })? {
        return Err("anonymous constant division requires a nonzero divisor proof; its rational bounds include zero".into());
    }
    Ok(())
}

pub(super) fn validate_anonymous_comparison(
    program: &TypedTrees,
    context: super::EvaluationContext<'_>,
    expression: ExpressionHandle,
) -> Result<(), String> {
    if contains_match(program, expression) {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
            return Err("anonymous comparison lost operands".into());
        };
        validate_anonymous_fragments(program, context, binary.left)?;
        validate_anonymous_fragments(program, context, binary.right)
    } else {
        super::compare_anonymous(program, context, expression, &[]).map(|_| ())
    }
}

pub(super) fn coerce(
    program: &TypedTrees,
    context: super::EvaluationContext<'_>,
    value: Value,
    shape: Shape,
    selected: &[(ExpressionHandle, ExpressionHandle)],
    warnings: &mut Vec<Diagnostic>,
) -> Result<Value, String> {
    match (value, shape) {
        (Value::Anonymous(expression), Shape::Integer(carrier, _)) => land_anonymous(
            program,
            context,
            expression,
            primitive(carrier)?,
            selected,
            warnings,
        ),
        (Value::Anonymous(_), Shape::Anonymous(_)) | (Value::Boolean(_), Shape::Boolean) => {
            Ok(value)
        }
        (Value::Landed(left, _), Shape::Integer(right, _)) if left == right => Ok(value),
        _ => Err("constant Match execution changed its scalar join type".into()),
    }
}
