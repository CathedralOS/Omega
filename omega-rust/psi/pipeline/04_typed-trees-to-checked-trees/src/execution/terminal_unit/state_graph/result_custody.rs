//! Result custody accounting for one composed Unit state.
//!
//! Every structural result a state's operations produce has exactly one
//! owner on each path out of the state: a later operation that consumes it,
//! a store that writes it, a successor edge that transfers it, or a cleanup
//! row that discards it. A result with no owner, or with two, refuses the
//! route; case successors of a closed sum account for it per case.
use super::{
    CheckFacts, CheckedComposedUnitControlTerminatorPlan, CheckedStructuralAccess,
    CheckedStructuralControlSuccessorPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan, Multiplicity, PermissionEventSource, StatementNode,
    TypedTrees, call_owns_result, control, local_results, terminator_call_consumes_result,
};

pub(super) fn account(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    operations: &[CheckedUnitEffectOperationPlan],
    terminator: &CheckedComposedUnitControlTerminatorPlan,
    disposable_locals: &[symbols::SymbolHandle],
    trace: &control::LocalConstructionTrace,
) -> Option<()> {
    let statements = program.statement_table.statements(state.statement_nodes);
    trace.phase("state graph: result custody accounting");
    let case_successors = match terminator {
        CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } => cases
            .iter()
            .filter_map(|case| match &case.outcome {
                checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(successor) => {
                    Some(successor)
                }
                checked_trees::CheckedClosedSumCaseOutcomePlan::Return(_) => None,
            })
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    let construction_uses = construction_local_uses(facts, state.symbol, operations)?;
    for (producer_index, operation) in operations.iter().enumerate() {
        let result = match operation {
            CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } => result,
            _ => continue,
        };
        let transferred = |edge: &CheckedStructuralControlSuccessorPlan| {
            edge.transfers.iter().filter(|transfer| matches!(transfer.source, checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } if binding_ordinal == result.binding_ordinal)).count() == 1
        };
        // An owned selection moved each candidate source's custody into
        // the join's residual parameters, which die at the same authored
        // exit. The receipt, not a transfer or disposal row on the dead
        // source place, accounts for that consumption on an ordinary
        // successor; other exits still need their own residual evidence.
        let selection_residual_source = matches!(
            terminator,
            CheckedComposedUnitControlTerminatorPlan::Jump { .. }
                | CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
                | CheckedComposedUnitControlTerminatorPlan::ConditionalReturn { .. }
                | CheckedComposedUnitControlTerminatorPlan::GuardedJumps { .. }
        ) && result.multiplicity == Multiplicity::Affine
            && facts
                .flow
                .ownership
                .owned_selections
                .iter()
                .any(|(_, receipt)| {
                    receipt.machine == machine.symbol
                        && receipt.state == state.symbol
                        && receipt.death == PermissionEventSource::StateExit
                        && facts
                            .flow
                            .ownership
                            .selection_sources
                            .span_or_empty(receipt.sources)
                            .iter()
                            .any(|source| {
                                source.statement_ordinal == result.statement_index
                                    && matches!(
                                        statements.get(result.statement_index as usize),
                                        Some(StatementNode::LocalData(local))
                                            if local.symbol == source.symbol
                                    )
                            })
                });
        let consumed = match terminator {
            CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } => {
                result.multiplicity == Multiplicity::Unrestricted
                    || return_values.iter().any(|value| {
                        value.with_value_calls().skip(1).any(|operation| {
                            terminator_call_consumes_result(operation, result.binding_ordinal)
                        })
                    })
            }
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit
            | CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. } => {
                local_results::permits_disposal(program, state, result, &[], disposable_locals)
            }
            CheckedComposedUnitControlTerminatorPlan::Jump { successor } => {
                transferred(successor)
                    || local_results::permits_disposal(
                        program,
                        state,
                        result,
                        &[successor],
                        disposable_locals,
                    )
            }
            CheckedComposedUnitControlTerminatorPlan::Conditional {
                when_true,
                when_false,
                ..
            } => {
                (transferred(when_true) && transferred(when_false))
                    || local_results::permits_disposal(
                        program,
                        state,
                        result,
                        &[when_true, when_false],
                        disposable_locals,
                    )
            }
            CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                jump,
                return_arm,
                ..
            } => {
                // The return arm consumes its producers through the
                // `EstablishStructuralValue` operands like Guarded; only
                // the named edge participates in transfer custody.
                transferred(jump)
                    || local_results::permits_disposal(
                        program,
                        state,
                        result,
                        &[jump],
                        disposable_locals,
                    )
                    || matches!(return_arm,
                    checked_trees::CheckedConditionalReturnArm::Structural(operation)
                        if operation.with_value_calls().skip(1).any(|operation| {
                            terminator_call_consumes_result(operation, result.binding_ordinal)
                        }))
            }
            CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback } => {
                let successors = arms
                    .iter()
                    .map(|arm| &arm.successor)
                    .chain(match fallback {
                        checked_trees::CheckedGuardedJumpFallbackPlan::Successor(successor) => {
                            Some(successor)
                        }
                        checked_trees::CheckedGuardedJumpFallbackPlan::Return(_) => None,
                    })
                    .collect::<Vec<_>>();
                // A `(expression)` fallback arm consumes its producers through
                // the establishment operands it carries, exactly like a
                // `ConditionalReturn`'s arm.
                successors.iter().all(|edge| transferred(edge))
                    || matches!(fallback,
                    checked_trees::CheckedGuardedJumpFallbackPlan::Return(
                        checked_trees::CheckedConditionalReturnArm::Structural(operation),
                    ) if operation.with_value_calls().skip(1).any(|operation| {
                        terminator_call_consumes_result(operation, result.binding_ordinal)
                    }))
                    || local_results::permits_disposal(
                        program,
                        state,
                        result,
                        &successors,
                        disposable_locals,
                    )
            }
            CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, cases, .. } => {
                if matches!(subject.source, CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } if binding_ordinal == result.binding_ordinal)
                {
                    case_successors.iter().all(|edge| !transferred(edge))
                } else {
                    // Like a ConditionalReturn's arm: a `(expression)` case
                    // edge consumes its producers through the establishment
                    // operands the arm carries.
                    case_successors.iter().all(|edge| transferred(edge))
                        || cases.iter().any(|case| {
                            matches!(
                                &case.outcome,
                                checked_trees::CheckedClosedSumCaseOutcomePlan::Return(
                                    checked_trees::CheckedConditionalReturnArm::Structural(operation),
                                ) if operation.with_value_calls().skip(1).any(|operation| {
                                    terminator_call_consumes_result(operation, result.binding_ordinal)
                                })
                            )
                        })
                        || local_results::permits_disposal(
                            program,
                            state,
                            result,
                            &case_successors,
                            disposable_locals,
                        )
                }
            }
            CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result: returned } => {
                matches!(returned.source, CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } if binding_ordinal == result.binding_ordinal)
            }
            _ => false,
        } || selection_residual_source;
        // A result the call owns (`call_owns_result`) is no longer owed by
        // the state's terminator. Count its exact whole owned uses in the
        // completed sequence; returning it as well would duplicate custody.
        // Affine and unrestricted locals keep their ordinary statement
        // owner until graph admission can replay their cleanup partition.
        let call_transfers = operations[producer_index + 1..]
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::StructuralCall {
                    structural_arguments,
                    ..
                }
                | CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                }
                | CheckedUnitEffectOperationPlan::ScalarCall {
                    structural_arguments,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryCall {
                    structural_arguments,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                    structural_arguments,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    structural_arguments,
                    ..
                } => Some(structural_arguments),
                _ => None,
            })
            .flatten()
            .filter(|argument| {
                call_owns_result(operation)
                    && argument.source_structural_result_binding_ordinal()
                        == Some(result.binding_ordinal)
                    && argument.access == CheckedStructuralAccess::Owned
                    && argument.path.is_empty()
            })
            .count();
        // An in-sequence continuation cleanup owns an affine result whose
        // authored statement discarded it. Count that exact owner as the
        // consumption alongside terminator disposal and call transfer.
        let cleanup_discards = operations[producer_index + 1..]
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                    affine_discards, ..
                } => Some(affine_discards),
                _ => None,
            })
            .flatten()
            .filter(|discard| {
                matches!(
                    discard.source,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal
                    } if binding_ordinal == result.binding_ordinal
                )
            })
            .count();
        // A hole-restore stores a whole owned result into borrowed
        // storage; custody moved into the field, which owns it after
        // this state. Count it with the terminator exits: a disposable
        // local stored into a hole is not a second consumption. A copy
        // of an `Unrestricted` value moves into its field the same way.
        let stored = operations[producer_index + 1..].iter().any(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::StoreStructuralField {
                    value, ..
                } if value.source_structural_result_binding_ordinal()
                    == Some(result.binding_ordinal)
                    && value.access == CheckedStructuralAccess::Owned
                    && value.path.is_empty()
            )
        });
        // A constructor takes its whole owned local operands just as a call
        // does. Statement sequencing already retires their disposal flags;
        // rejoin the exact value children here instead of requiring that the
        // operand itself also be the returned outer binding. Selected values
        // retain their separate per-arm ownership receipts.
        let construction_count = match statements.get(result.statement_index as usize) {
            Some(StatementNode::LocalData(local)) => construction_uses
                .iter()
                .filter(|(consumer_index, argument)| {
                    *consumer_index > producer_index
                        && matches!(argument.source,
                            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
                                if symbol == local.symbol)
                        && argument.type_identity == result.type_identity
                        && (result.multiplicity == Multiplicity::Unrestricted
                            || argument.access == CheckedStructuralAccess::Owned)
                        && argument.path.is_empty()
                })
                .count(),
            _ => 0,
        };
        // Copyable local leaves may be retained as shared case/record reads,
        // or as owned operands. Neither form moves the original owner; repeated
        // observations do not create or discharge an affine obligation.
        let copied_into_construction =
            result.multiplicity == Multiplicity::Unrestricted && construction_count != 0;
        let construction_transfers = if result.multiplicity == Multiplicity::Unrestricted {
            0
        } else {
            construction_count
        };
        if usize::from(consumed || stored || copied_into_construction)
            + call_transfers
            + cleanup_discards
            + construction_transfers
            != 1
        {
            return None;
        }
    }
    Some(())
}

/// Borrow whole local operands once per unconditional construction tree. The
/// sequence mutation walker also enters dispatch/projection nodes to update
/// disposal flags; accounting cannot flatten those per-arm/residual receipts.
/// Keep this inventory restricted to ordinary constructor composition.
fn construction_local_uses<'a>(
    facts: &'a CheckFacts,
    state: symbols::SymbolHandle,
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<Vec<(usize, &'a checked_trees::CheckedUnitStructuralArgumentPlan)>> {
    use checked_trees::{CheckedStructuralRecordFieldValue, CheckedStructuralValueKind};

    let values = &facts.values.structural_values;
    let mut uses = Vec::new();
    let mut pending = Vec::new();
    let mut visited = Vec::new();
    for (index, operation) in operations.iter().enumerate() {
        let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, value, .. } =
            operation
        else {
            continue;
        };
        if facts
            .flow
            .ownership
            .owned_selection_at(state, result.statement_index)
            .is_some()
        {
            continue;
        }
        pending.push(*value);
        visited.clear();
        while let Some(value) = pending.pop() {
            if !values.nodes.is_valid(value) || visited.contains(&value) {
                return None;
            }
            visited.push(value);
            match &values.nodes.get(value).kind {
                CheckedStructuralValueKind::Place(argument)
                | CheckedStructuralValueKind::ScalarCasePlace {
                    source: argument, ..
                }
                | CheckedStructuralValueKind::CopiedStructuralPlace {
                    source: argument, ..
                } => {
                    uses.push((index, argument));
                }
                CheckedStructuralValueKind::Record { fields, .. }
                | CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                    for field in values.record_fields.span(*fields)? {
                        if let CheckedStructuralRecordFieldValue::Structural(value) = field.value {
                            pending.push(value);
                        }
                    }
                }
                CheckedStructuralValueKind::FixedArray { elements } => {
                    pending.extend(elements.iter().copied());
                }
                _ => {}
            }
        }
    }
    Some(uses)
}
