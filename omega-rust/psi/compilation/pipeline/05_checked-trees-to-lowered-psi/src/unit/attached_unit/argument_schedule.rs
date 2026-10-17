//! Schedule scalar evaluation around nested structural producers without
//! changing checked call coordinates or inventing source-local bindings. Every
//! produced argument belongs to the formal that consumes its exact result
//! binding. Construction, copy, extraction and nested call results therefore
//! share one schedule. Hoisting extraction or a copy before an earlier scalar
//! argument reads absent or stale storage; delaying it observes later mutation.
//! Constructor leaves use the selective evaluator, not additional scheduled calls.
//! Address selectors join this same formal-order walk: evaluating them at the
//! final call would observe mutations from later arguments. Retained value slots
//! carry each selector through private joins without replaying its computation.

use super::{
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, LoweringError,
    byte_subslices, unsupported,
};
pub(crate) enum Step {
    CaptureBefore(u32),
    Ordinary(usize),
    Begin,
    Argument {
        operation: usize,
        ordinal: usize,
    },
    /// A window argument whose coordinates folded keeps its authored endpoint
    /// evaluations: the call no longer reads their values, but the expressions
    /// still run — their calls and writes are authored effects in order.
    Endpoint {
        operation: usize,
        argument: usize,
    },
    Subslice {
        operation: usize,
        ordinal: usize,
    },
    Selector {
        operation: usize,
        argument: usize,
        segment: usize,
    },
    Produce(usize),
    Call(usize),
    End,
}

pub(super) fn build(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
) -> Result<Vec<Step>, LoweringError> {
    let operations = &plan.operations[..plan.operations.len() - 1];
    let mut steps = build_operations(checked, plan.machine, plan.state, operations)?;
    steps.push(Step::CaptureBefore(plan.statement_count));
    Ok(steps)
}

/// Both straight-line machines and composed states retain the same authored
/// call coordinates. Their surrounding control graph does not change operand
/// order, so both routes consume this schedule over their operation sequence.
pub(super) fn build_operations(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    operations: &[CheckedUnitEffectOperationPlan],
) -> Result<Vec<Step>, LoweringError> {
    let mut steps = Vec::new();
    let mut index = 0;
    while index < operations.len() {
        if let Some(statement) = operations[index].source_statement_index() {
            steps.push(Step::CaptureBefore(statement));
        }
        // A nested boundary structural result joins its argument group below.
        if byte_subslices::contains(&operations[index])
            && !operations[index].call().is_some_and(|call| {
                call.is_boundary()
                    && call.structural_result().is_some()
                    && call.coordinate.call_ordinal != 0
            })
        {
            steps.push(Step::Begin);
            append(
                checked,
                machine,
                state,
                operations,
                index..index + 1,
                index,
                &mut Vec::new(),
                &mut Vec::new(),
                &mut steps,
            )?;
            steps.push(Step::End);
            index += 1;
            continue;
        }
        let statement = match operations[index].structural_result_binding() {
            Some(result)
                if operations[index + 1..].iter().any(|operation| {
                    operation.call().is_some_and(|call| {
                        call.coordinate.statement_index == result.statement_index
                            && call.structural_arguments.iter().any(|argument| {
                                argument.source_structural_result_binding_ordinal()
                                    == Some(result.binding_ordinal)
                            })
                    })
                }) =>
            {
                result.statement_index
            }
            _ => {
                if operations[index].call().is_some_and(|call| {
                    call.structural_arguments
                        .iter()
                        .any(|argument| argument.call_selector_segments().next().is_some())
                        || call.structural_arguments.iter().enumerate().any(
                            |(argument_ordinal, _)| {
                                !recorded_window_endpoint_roots(
                                    checked,
                                    state,
                                    call.coordinate,
                                    argument_ordinal,
                                )
                                .is_empty()
                            },
                        )
                }) {
                    steps.push(Step::Begin);
                    append(
                        checked,
                        machine,
                        state,
                        operations,
                        index..index + 1,
                        index,
                        &mut Vec::new(),
                        &mut Vec::new(),
                        &mut steps,
                    )?;
                    steps.push(Step::End);
                    index += 1;
                    continue;
                }
                steps.push(Step::Ordinary(index));
                index += 1;
                continue;
            }
        };
        let start = index;
        let root = call_group_root(operations, start, statement)?;
        steps.push(Step::Begin);
        let mut emitted = Vec::new();
        append(
            checked,
            machine,
            state,
            operations,
            start..root + 1,
            root,
            &mut Vec::new(),
            &mut emitted,
            &mut steps,
        )?;
        if !emitted.iter().copied().eq(start..root + 1) {
            return unsupported("argument schedule disagrees with checked structural call order");
        }
        steps.push(Step::End);
        index = root + 1;
    }
    Ok(steps)
}

/// The same checked operand schedule is consumed by statement and expression bodies.
pub(crate) fn operand_preparation(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    operands: &checked_trees::CheckedOperandSequence,
    consumer: &CheckedUnitEffectOperationPlan,
) -> Result<Vec<Step>, LoweringError> {
    let operations = operands
        .iter()
        .cloned()
        .chain(std::iter::once(consumer.clone()))
        .collect::<Vec<_>>();
    let mut emitted = Vec::new();
    let mut steps = Vec::new();
    append(
        checked,
        machine,
        state,
        &operations,
        0..operations.len(),
        operations.len() - 1,
        &mut Vec::new(),
        &mut emitted,
        &mut steps,
    )?;
    if !emitted.iter().copied().eq(0..operations.len()) {
        return unsupported("call preparation contains an unrelated or reordered producer");
    }
    Ok(steps)
}

/// Source ordinals identify occurrences, not an execution stream's root. A
/// state arrival, return arm, or ordinary statement may retain a different
/// portion of that occurrence tree. Exact produced binding uses identify the
/// enclosing calls without classifying producer spelling or renumbering them.
fn call_group_root(
    operations: &[CheckedUnitEffectOperationPlan],
    start: usize,
    statement: u32,
) -> Result<usize, LoweringError> {
    let mut root = start;
    loop {
        let Some(result) = operations[root].structural_result_binding() else {
            return Ok(root);
        };
        let mut consumers = (root + 1..operations.len()).filter(|index| {
            operations[*index].call().is_some_and(|consumer| {
                consumer.coordinate.statement_index == statement
                    && consumer.structural_arguments.iter().any(|argument| {
                        argument.source_structural_result_binding_ordinal()
                            == Some(result.binding_ordinal)
                    })
            })
        });
        let Some(consumer) = consumers.next() else {
            return Ok(root);
        };
        if consumers.next().is_some() {
            return unsupported("argument result has multiple enclosing call occurrences");
        }
        root = consumer;
    }
}

#[allow(clippy::too_many_arguments)]
fn append(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    operations: &[CheckedUnitEffectOperationPlan],
    group: std::ops::Range<usize>,
    index: usize,
    active: &mut Vec<usize>,
    emitted: &mut Vec<usize>,
    steps: &mut Vec<Step>,
) -> Result<(), LoweringError> {
    if active.contains(&index) || emitted.contains(&index) || !group.contains(&index) {
        return unsupported("nested argument schedule repeats or escapes its producer group");
    }
    if operations[index].call().is_none() && operations[index].structural_result_binding().is_some()
    {
        emitted.push(index);
        steps.push(Step::Produce(index));
        return Ok(());
    }
    let Some(call) = operations[index].call() else {
        return unsupported("nested argument schedule requires an ordinary or boundary call");
    };
    let (coordinate, scalar_arguments, structural_arguments) = (
        call.coordinate,
        call.scalar_operands(),
        call.structural_arguments.as_slice(),
    );
    let authored = crate::call_sites::locate(checked, state, coordinate)?;
    let target = crate::declarations::target_signature(checked, machine, authored.source_target)?;
    let parameters = target.parameters;
    // Authored positions and retained argument slots are different namespaces.
    // The callee's checked signature decides which structural formals survive;
    // neither attachment syntax nor the number of supplied arguments can do so.
    let structural_positions =
        super::call_parameters::structural_positions(checked, call, &target)?;
    if parameters.len() != authored.scalar_arguments.len() + authored.structural_arguments.len()
        || scalar_arguments.len() != authored.scalar_arguments.len()
        || structural_arguments.len() != structural_positions.len()
    {
        return unsupported("nested argument schedule has no exact positional signature");
    }
    active.push(index);
    let mut scalar_ordinal = 0;
    // Walk both lanes in authored evaluation order; a retained receiver can
    // consume a structural slot without an explicit argument expression.
    let mut structural_ordinal = 0;
    for (position, parameter) in parameters.iter().enumerate() {
        if checked
            .primitive_type_reference(parameter.type_reference)
            .is_some()
        {
            steps.push(Step::Argument {
                operation: index,
                ordinal: scalar_ordinal,
            });
            scalar_ordinal += 1;
            continue;
        }
        if structural_positions
            .get(structural_ordinal)
            .copied()
            .map(|position| position as usize)
            != Some(position)
        {
            continue;
        }
        let argument_ordinal = structural_ordinal;
        structural_ordinal += 1;
        if byte_subslices::arguments(&operations[index])
            .get(argument_ordinal)
            .is_some_and(|argument| byte_subslices::range(argument).is_some())
        {
            steps.push(Step::Subslice {
                operation: index,
                ordinal: argument_ordinal,
            });
            continue;
        }
        let has_recorded_endpoints =
            !recorded_window_endpoint_roots(checked, state, call.coordinate, argument_ordinal)
                .is_empty();
        // The plan names the produced value this argument consumes; a producer
        // in this group is scheduled first, and a result bound earlier in the
        // body needs no scheduling.
        let joined = structural_arguments
            .get(argument_ordinal)
            .and_then(|argument| argument.source_structural_result_binding_ordinal())
            .and_then(|binding| {
                group.clone().find(|producer| {
                    operations[*producer]
                        .structural_result_binding()
                        .is_some_and(|result| result.binding_ordinal == binding)
                })
            });
        if let Some(producer) = joined {
            append(
                checked,
                machine,
                state,
                operations,
                group.clone(),
                producer,
                active,
                emitted,
                steps,
            )?;
        }
        for (segment, _) in structural_arguments[argument_ordinal].call_selector_segments() {
            steps.push(Step::Selector {
                operation: index,
                argument: argument_ordinal,
                segment,
            });
        }
        // A folded window keeps no descriptor to narrow, yet its authored
        // endpoint expressions still run before later arguments can mutate.
        if has_recorded_endpoints {
            steps.push(Step::Endpoint {
                operation: index,
                argument: argument_ordinal,
            });
        }
    }
    active.pop();
    emitted.push(index);
    steps.push(Step::Call(index));
    Ok(())
}

/// The recorded window endpoint computations of one structural argument at a
/// call's site, in start-then-end order. A descriptor range evaluates its
/// endpoints when it narrows; a folded fixed window presents fixed coordinates
/// instead, so its consumer must still run these roots for their authored
/// effects. Only argument expressions the caller admitted as ranges record
/// endpoint roots, so an argument without them contributes nothing.
pub(crate) fn recorded_window_endpoint_roots(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    argument_ordinal: usize,
) -> Vec<(
    checked_trees::CheckedScalarExpressionRole,
    checked_trees::CheckedScalarComputationHandle,
)> {
    let (Ok(call_ordinal), Ok(argument_ordinal)) = (
        u32::try_from(coordinate.call_ordinal),
        u32::try_from(argument_ordinal),
    ) else {
        return Vec::new();
    };
    let site = checked_trees::CheckedSubsliceSite::CallArgument {
        call_ordinal,
        argument_ordinal,
    };
    [
        checked_trees::CheckedScalarExpressionRole::SubsliceStart { site },
        checked_trees::CheckedScalarExpressionRole::SubsliceEnd { site },
    ]
    .into_iter()
    .filter_map(|role| {
        let root = checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .map(|(_, root)| root)
            .find(|root| {
                root.state == state
                    && root.statement_ordinal == coordinate.statement_index
                    && root.role == role
            })?;
        Some((role, root.root))
    })
    .collect()
}
