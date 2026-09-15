//! Scalar Match joins and anonymous-result landing before selective execution.

mod rational_bounds;

use super::{Shape, Value, land_anonymous, primitive};
use diagnostics::Diagnostic;
use numerics::bignum::BigRational;
use numerics::literals::LandedIntegerType;
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode, MatchPattern},
    machine::Machine,
    state::State,
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
    machine: &Machine,
    state: &State,
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
    let subject = join(program, machine, state, &patterns, warnings)?;
    let result = join(program, machine, state, &results, warnings)?;
    let result = if matches!(result, Shape::Anonymous(_)) {
        Shape::Anonymous(expression)
    } else {
        result
    };
    let mut diagnostics = Vec::new();
    validation::validate_match_dispatch(
        program,
        machine,
        state,
        expression,
        dispatch,
        &mut diagnostics,
    );
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
    machine: &Machine,
    state: &State,
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
                validate_anonymous_fragments(program, machine, state, expression)?;
            }
            (Shape::Integer(left), Shape::Integer(right)) if left == right => {}
            (Shape::Anonymous(expression), Shape::Integer(carrier)) => {
                validate_landing(
                    program,
                    machine,
                    state,
                    expression,
                    primitive(carrier)?,
                    warnings,
                )?;
            }
            _ => {
                return Err(
                    "constant Match has incompatible subject, pattern or result types".into(),
                );
            }
        }
    }
    Ok(peer)
}

pub(super) fn validate_landing(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
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
            validate_anonymous_fragments(program, machine, state, expression)?;
            let fractional_history = rational_bounds::validate_integer_landing(
                program,
                expression,
                carrier,
                |operand| {
                    validation::has_builtin_binary_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        operand,
                    )
                },
            )?;
            if fractional_history {
                validate_fractional_landings(
                    program,
                    machine,
                    state,
                    expression,
                    destination,
                    warnings,
                )?;
            }
        } else {
            land_anonymous(
                program,
                machine,
                state,
                expression,
                destination,
                &[],
                warnings,
            )?;
        }
    }
    Ok(carrier)
}

/// Bounds cannot invent the exact final value required by a fractional warning.
/// With one varying child per operation, each complete result path can reuse
/// ordinary landing with exact arm edges. This visits alternatives, not their
/// Cartesian product, and never evaluates subjects or patterns. Independent
/// fractional histories still need compositional exact diagnostic evidence.
fn validate_fractional_landings(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    root: ExpressionHandle,
    destination: PrimitiveType,
    warnings: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => pending.extend(
                program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .map(|arm| arm.value),
            ),
            ExpressionNode::Binary(binary) => {
                if contains_match(program, binary.left) && contains_match(program, binary.right) {
                    return Err("anonymous constant Match landing with independent fractional histories requires exact warning evidence".into());
                }
                pending.push(binary.right);
                pending.push(binary.left);
            }
            _ => {}
        }
    }

    enum Step {
        Enter(ExpressionHandle),
        Arm(ExpressionHandle, ExpressionHandle),
        Restore(usize),
    }
    let mut pending = vec![Step::Enter(root)];
    let mut selected = Vec::new();
    while let Some(step) = pending.pop() {
        match step {
            Step::Restore(length) => selected.truncate(length),
            Step::Arm(owner, result) => {
                pending.push(Step::Restore(selected.len()));
                selected.push((owner, result));
                pending.push(Step::Enter(result));
            }
            Step::Enter(expression) => {
                match program.expression_table.expression(expression) {
                    ExpressionNode::Match(dispatch) => {
                        pending.extend(
                            program
                                .expression_table
                                .match_arms(dispatch.arms)
                                .iter()
                                .rev()
                                .map(|arm| Step::Arm(expression, arm.value)),
                        );
                        continue;
                    }
                    ExpressionNode::Binary(binary) => {
                        if contains_match(program, binary.left) {
                            pending.push(Step::Enter(binary.left));
                            continue;
                        }
                        if contains_match(program, binary.right) {
                            pending.push(Step::Enter(binary.right));
                            continue;
                        }
                    }
                    _ => {}
                }
                land_anonymous(
                    program,
                    machine,
                    state,
                    root,
                    destination,
                    &selected,
                    warnings,
                )?;
            }
        }
    }
    Ok(())
}

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

fn validate_anonymous_fragments(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
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
                validate_nonzero_divisor(program, machine, state, binary.right)?;
            }
            pending.push(binary.right);
            pending.push(binary.left);
        } else {
            validation::evaluate_anonymous_numeric_expression_with_selected_match_arms(
                program,
                expression,
                &[],
                |operand| {
                    validation::has_builtin_binary_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        operand,
                    )
                },
            )
            .ok_or("anonymous constant expression requires defined exact numeric values")?;
        }
    }
    Ok(())
}

fn validate_nonzero_divisor(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    root: ExpressionHandle,
) -> Result<(), String> {
    // Direct dispatch and surrounding arithmetic use one result-domain proof.
    // Keeping opposite signs separate avoids a special path for bare Match.
    if !rational_bounds::excludes_zero(program, root, |operand| {
        validation::has_builtin_binary_expression_meaning(program, machine, Some(state), operand)
    })? {
        return Err("anonymous constant division requires a nonzero divisor proof; its rational bounds include zero".into());
    }
    Ok(())
}

pub(super) fn validate_anonymous_comparison(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Result<(), String> {
    if contains_match(program, expression) {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
            return Err("anonymous comparison lost operands".into());
        };
        validate_anonymous_fragments(program, machine, state, binary.left)?;
        validate_anonymous_fragments(program, machine, state, binary.right)
    } else {
        super::compare_anonymous(program, machine, state, expression, &[]).map(|_| ())
    }
}

pub(super) fn coerce(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    value: Value,
    shape: Shape,
    selected: &[(ExpressionHandle, ExpressionHandle)],
    warnings: &mut Vec<Diagnostic>,
) -> Result<Value, String> {
    match (value, shape) {
        (Value::Anonymous(expression), Shape::Integer(carrier)) => land_anonymous(
            program,
            machine,
            state,
            expression,
            primitive(carrier)?,
            selected,
            warnings,
        ),
        (Value::Anonymous(_), Shape::Anonymous(_)) | (Value::Boolean(_), Shape::Boolean) => {
            Ok(value)
        }
        (Value::Landed(left, _), Shape::Integer(right)) if left == right => Ok(value),
        _ => Err("constant Match execution changed its scalar join type".into()),
    }
}
