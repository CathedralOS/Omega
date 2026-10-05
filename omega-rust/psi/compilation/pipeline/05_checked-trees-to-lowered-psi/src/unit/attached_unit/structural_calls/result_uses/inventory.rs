//! Join result uses from operation bodies and retained expression calls.
//!
//! This is a custody inventory, not an emission schedule. Existing body order
//! remains intact; expression calls retain their authored completion positions.
//! Only an unconditional exact source occurrence can retire whole caller debt.

use super::*;
use crate::emission::call_source_custody::authored::nested;

pub(crate) fn with_expression_consumers(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
) -> Result<Vec<CheckedUnitEffectOperationPlan>, LoweringError> {
    let mut expressions = Vec::new();
    for preparation in crate::expression_preparation::computation_graph::state_value_calls(
        checked,
        caller.machine,
        caller.state,
    )? {
        for operation in preparation
            .operands()
            .iter()
            .chain(std::iter::once(preparation.operation()))
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        {
            let Some(call) = operation.call() else {
                continue;
            };
            let mut existing = caller
                .operations
                .iter()
                .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                .chain(expressions.iter().map(
                    |(_, operation, _): &(
                        usize,
                        &CheckedUnitEffectOperationPlan,
                        &checked_trees::CheckedStructuralValueCall,
                    )| *operation,
                ))
                .filter(|candidate| {
                    candidate
                        .call()
                        .is_some_and(|candidate| candidate.coordinate == call.coordinate)
                });
            if let Some(previous) = existing.next() {
                if previous != operation || existing.next().is_some() {
                    return unsupported("result-use inventory has conflicting call occurrences");
                }
                continue;
            }
            // An expression call enters this inventory only when it refers to an
            // existing body result. Other temporary producers belong to their own
            // operand sequence, whose source and result custody is checked there.
            let uses_body_result = call.structural_arguments.iter().any(|argument| {
                argument
                    .source_structural_result_binding_ordinal()
                    .is_some_and(|ordinal| {
                        caller.operations.iter().any(|producer| {
                            producer
                                .structural_result_binding()
                                .is_some_and(|result| result.binding_ordinal == ordinal)
                        })
                    })
            });
            if !uses_body_result {
                continue;
            }
            // Shared observation keeps its backing debt. Whole owned transfer
            // cannot use a selected-only occurrence as unconditional retirement.
            let retires_body_result = call.structural_arguments.iter().any(|argument| {
                argument.access == checked_trees::CheckedStructuralAccess::Owned
                    && argument.path.is_empty()
                    && argument
                        .source_structural_result_binding_ordinal()
                        .is_some_and(|ordinal| {
                            caller.operations.iter().any(|producer| {
                                producer.structural_result_binding().is_some_and(|result| {
                                    result.binding_ordinal == ordinal
                                        && result.multiplicity != Multiplicity::Unrestricted
                                })
                            })
                        })
            });
            let position = if retires_body_result {
                nested::unconditional_call_position(checked, caller.state, call.coordinate)?
            } else {
                // Even a non-consuming observation retains its exact authored
                // position; selected alternatives remain independent occurrences.
                nested::authored_call_position(checked, caller.state, call.coordinate)?
            };
            expressions.push((position, operation, preparation));
        }
    }
    expressions.sort_by_key(|(position, operation, _)| {
        (
            operation.call().unwrap().coordinate.statement_index,
            *position,
        )
    });
    let mut operations = caller.operations.clone();
    for (position, operation, preparation) in &expressions {
        let coordinate = operation.call().unwrap().coordinate;
        let mut insertion = operations.len();
        for (index, candidate) in operations.iter().enumerate() {
            if matches!(candidate, CheckedUnitEffectOperationPlan::Complete { .. }) {
                insertion = index;
                break;
            }
            let Some(statement) = candidate.source_statement_index() else {
                continue;
            };
            if statement > coordinate.statement_index {
                insertion = index;
                break;
            }
            if statement == coordinate.statement_index {
                let after = if let Some(call) = candidate.call() {
                    nested::authored_call_position(checked, caller.state, call.coordinate)?
                        > *position
                } else {
                    // Establishment follows evaluation of its source value.
                    true
                };
                if after {
                    insertion = index;
                    break;
                }
            }
        }
        // The original preparation supplies constructors and temporary call
        // results in their existing order. A call inside that preparation
        // sees only its completed prefix, never a later sibling's result.
        for operand in preparation.operands().iter() {
            if operand == *operation {
                break;
            }
            let existing =
                operations
                    .iter()
                    .find(|candidate| match (candidate.call(), operand.call()) {
                        (Some(left), Some(right)) => left.coordinate == right.coordinate,
                        _ => candidate
                            .structural_result_binding()
                            .zip(operand.structural_result_binding())
                            .is_some_and(|(left, right)| {
                                left.binding_ordinal == right.binding_ordinal
                            }),
                    });
            if let Some(existing) = existing {
                if existing != operand {
                    return unsupported("result consumer preparation changed an existing producer");
                }
            } else {
                operations.insert(insertion, operand.clone());
                insertion += 1;
            }
        }
        operations.insert(insertion, (*operation).clone());
    }
    // Each added call still has to agree with its exact authored operand and
    // producing local. This is the same consumer check used by body calls.
    let view = CallerView {
        operations: &operations,
        ..CallerView::ordinary(caller)
    };
    for (_, operation, _) in expressions {
        let call = operation.call().unwrap();
        let parameters =
            super::super::super::call_parameters::structural_parameters(checked, call)?;
        if call.is_boundary() {
            // Boundary entry obligations use their own completion receipts.
            validate_consumer(checked, &view, operation, parameters, &[])?;
        } else if call.scalar_result().is_some() {
            let target = crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee::find_for_unit_call(
                checked, call.target.machine,
            )?;
            validate_consumer(checked, &view, operation, parameters, target.entry_claims())?;
        } else {
            let target = crate::unit::attached_unit::bodies::UnitBody::find(
                crate::unit::attached_unit::bodies::UnitPlans::published(
                    &checked.facts.flow.terminal_unit_effects,
                ),
                call.target.machine,
            )?
            .entry()?;
            validate_consumer(checked, &view, operation, parameters, target.entry_claims)?;
        }
    }
    Ok(operations)
}
