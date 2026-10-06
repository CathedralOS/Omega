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
    CheckedUnitStructuralArgumentSourcePlan, Multiplicity, PermissionEventKind,
    PermissionEventSource, StatementNode, TypedTrees, call_owns_result, control, local_results,
    terminator_call_consumes_result,
};
use checked_trees::CheckedUnitStructuralParameterPlan;

pub(super) fn account(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    operations: &mut [CheckedUnitEffectOperationPlan],
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
    let mut guarded_discards = Vec::new();
    for (producer_index, operation) in operations.iter().enumerate() {
        let Some((result, _)) = operation.discardable_structural_result() else {
            continue;
        };
        // An operand constructed for a call argument belongs to that call.
        // Its statement declares the call's result local, not the operand,
        // so the statement local's disposal permit would book a second owner
        // beside the call that takes it.
        let call_operand = matches!(
            operation,
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                operand_source: Some(
                    checked_trees::CheckedArrayConstructionSource::CallArgument { .. }
                ),
                ..
            }
        );
        // A whole in-sequence cleanup already owns this result's retirement.
        // Its lexical state-exit disposal permit cannot book a second owner;
        // moves, returns, and duplicate cleanups still count independently.
        let (cleanup_discards, retired_whole) = operations[producer_index + 1..]
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::ContinuationCleanup {
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
            .fold((0, false), |(count, retired_whole), discard| {
                (
                    count + 1,
                    retired_whole
                        || (discard.path.is_empty()
                            && discard.type_identity == result.type_identity),
                )
            });
        // A partially transferring successor partition must not hide a move
        // after unconditional retirement merely because it has another exit.
        if retired_whole
            && (terminator
                .successors()
                .iter()
                .any(|edge| edge.result_moves(result) != 0)
                || terminator.return_arms().iter().any(|arm| {
                    arm.operation_dependencies().any(|operation| {
                        operation.with_value_calls().any(|operation| {
                            terminator_call_consumes_result(operation, result.binding_ordinal)
                        })
                    })
                }))
        {
            return None;
        }
        let permits_disposal = |successors: &[&CheckedStructuralControlSuccessorPlan]| {
            !call_operand
                && !retired_whole
                && local_results::permits_disposal(
                    program,
                    state,
                    result,
                    successors,
                    disposable_locals,
                )
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
        // A mutable view's result owner is its one checked loan release. A
        // selected edge may additionally transfer that view or return a
        // reference reborrowed through it, but those mutually exclusive uses
        // validate the edge partition rather than adding another owner beside
        // the release. Resolve the source once so every control family applies
        // the same exact-root and descendant-path test.
        let view_source = match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. }
                if facts.values.structural_values.nodes.is_valid(*value) =>
            {
                match &facts.values.structural_values.nodes.get(*value).kind {
                    checked_trees::CheckedStructuralValueKind::BorrowedSliceView {
                        source, ..
                    }
                    | checked_trees::CheckedStructuralValueKind::MutableSliceView {
                        source, ..
                    } => Some(source),
                    _ => None,
                }
            }
            _ => None,
        };
        let parameter_symbol = |parameter_index: u32| {
            let parameter = structural_parameters.get(parameter_index as usize)?;
            let authored = program
                .state_parameters(state)
                .get(parameter.position as usize)?;
            false.then_some(machine.symbol).or(Some(authored.symbol))
        };
        let same_view_root = |argument: &checked_trees::CheckedUnitStructuralArgumentPlan| {
            let Some(view_source) = view_source else {
                return false;
            };
            match (&argument.source, &view_source.source) {
                (
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                ) => parameter_symbol(*parameter_index) == Some(*symbol),
                (
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
                ) => Some(*symbol) == parameter_symbol(*parameter_index),
                _ => argument.source == view_source.source,
            }
        };
        let exact_view_reborrow = |source: &checked_trees::CheckedUnitStructuralArgumentPlan| {
            let Some(view_source) = view_source else {
                return false;
            };
            same_view_root(source)
                && matches!(
                    (view_source.access, source.access),
                    (
                        CheckedStructuralAccess::MutableBorrow,
                        CheckedStructuralAccess::MutableBorrow
                            | CheckedStructuralAccess::SharedBorrow,
                    ) | (
                        CheckedStructuralAccess::SharedBorrow,
                        CheckedStructuralAccess::SharedBorrow,
                    )
                )
                && source.path.starts_with(&view_source.path)
                && (source.path.len() > view_source.path.len()
                    || source.type_identity == view_source.type_identity)
        };
        // Closed-case return establishments are retained among the state's
        // operation dependencies as well as in the terminator roster. Count
        // their one exact reborrow here so an unrestricted shared view does
        // not also claim a synthetic dominance owner. A partition containing
        // only successor transfers has no such reborrow and keeps the
        // unrestricted value's ordinary dominance disposition instead.
        let reference_reborrows = if view_source.is_some() {
            operations[producer_index + 1..]
                .iter()
                .filter(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::EstablishReference { source, .. }
                            if exact_view_reborrow(source)
                    )
                })
                .count()
        } else {
            0
        };
        // The release (`view_releases` below) owns an affine view. An edge
        // carrying the view whole continues that same loan in its target:
        // the carry is checked as at most one move per edge, and the
        // ordinary transfer count below must not book it as a second owner.
        let release_owned_view = view_source.is_some()
            && result.multiplicity == Multiplicity::Affine
            && reference_reborrows == 0;
        if release_owned_view
            && terminator
                .successors()
                .iter()
                .any(|edge| edge.result_moves(result) > 1)
        {
            return None;
        }
        // A successor moves the result either as a direct transfer or, for
        // a result with ownership debt, into one of its own operand calls.
        let transferred = |edge: &CheckedStructuralControlSuccessorPlan| {
            !release_owned_view && edge.result_moves(result) == 1
        };
        let mut closed_sum_all_reference_returns = false;
        let consumed = match terminator {
            CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } => {
                result.multiplicity == Multiplicity::Unrestricted
                    || return_values.iter().any(|value| {
                        value.with_value_calls().skip(1).any(|operation| {
                            terminator_call_consumes_result(operation, result.binding_ordinal)
                        })
                    })
            }
            // Both arms of a conditional completion return: neither
            // transfers a result, and an arm's terminal call transfers no
            // claim, so each result dies with the state as on a Unit return.
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit
            | CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. }
            | CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion { .. } => {
                permits_disposal(&[])
            }
            CheckedComposedUnitControlTerminatorPlan::Jump { successor } => {
                transferred(successor) || permits_disposal(&[successor])
            }
            CheckedComposedUnitControlTerminatorPlan::Conditional {
                when_true,
                when_false,
                ..
            } => {
                (transferred(when_true) && transferred(when_false))
                    || permits_disposal(&[when_true, when_false])
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
                    || permits_disposal(&[jump])
                    || matches!(return_arm,
                    checked_trees::CheckedConditionalReturnArm::Structural(operation)
                        if operation.with_value_calls().skip(1).any(|operation| {
                            terminator_call_consumes_result(operation, result.binding_ordinal)
                        }))
            }
            CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback } => {
                let successors = arms
                    .iter()
                    .filter_map(|arm| arm.successor())
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
                    || permits_disposal(&successors)
            }
            CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, cases, .. } => {
                if matches!(subject.source, CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } if binding_ordinal == result.binding_ordinal)
                {
                    case_successors.iter().all(|edge| !transferred(edge))
                } else {
                    // Every reachable case must own this result on its own
                    // edge. Looking only at successor cases makes an
                    // all-return roster vacuously transferred; accepting any
                    // one returning arm lets a mixed roster hide an uncovered
                    // sibling. Reference returns use the same exact view root
                    // and a descendant path. Their one loan release remains
                    // the semantic owner counted below, so this complete
                    // roster validates the partition without booking a second
                    // consumption beside that release.
                    let complete_partition = !cases.is_empty()
                        && cases.iter().all(|case| match &case.outcome {
                            checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(
                                successor,
                            ) => transferred(successor),
                            checked_trees::CheckedClosedSumCaseOutcomePlan::Return(
                                checked_trees::CheckedConditionalReturnArm::Structural(returned),
                            ) => match (view_source, returned) {
                                (
                                    Some(_),
                                    CheckedUnitEffectOperationPlan::EstablishReference {
                                        source,
                                        ..
                                    },
                                ) => exact_view_reborrow(source),
                                (None, operation) => {
                                    operation.with_value_calls().skip(1).any(|operation| {
                                        terminator_call_consumes_result(
                                            operation,
                                            result.binding_ordinal,
                                        )
                                    })
                                }
                                _ => false,
                            },
                            _ => false,
                        });
                    if view_source.is_some() {
                        if !complete_partition {
                            return None;
                        }
                        closed_sum_all_reference_returns = cases.iter().all(|case| {
                            matches!(
                                &case.outcome,
                                checked_trees::CheckedClosedSumCaseOutcomePlan::Return(
                                    checked_trees::CheckedConditionalReturnArm::Structural(
                                        CheckedUnitEffectOperationPlan::EstablishReference {
                                            source,
                                            ..
                                        },
                                    ),
                                ) if exact_view_reborrow(source)
                            )
                        });
                        result.multiplicity == Multiplicity::Unrestricted
                            && reference_reborrows == 0
                    } else {
                        complete_partition || permits_disposal(&case_successors)
                    }
                }
            }
            CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result: returned } => {
                matches!(returned.source, CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } if binding_ordinal == result.binding_ordinal)
            }
            _ => false,
        } || selection_residual_source;
        // A whole owned argument ends an affine local's custody at the call,
        // just as it ends a linear result or an inline operand's custody.
        // Sequencing retires the producer's discard flag; count the exact
        // later use here instead of still demanding state-exit disposal.
        // The final sum rejects a second move, cleanup, or returned owner.
        // Unrestricted locals remain copies, not one-use ownership debt.
        let call_transfers = operations[producer_index + 1..]
            .iter()
            .flat_map(CheckedUnitEffectOperationPlan::call_structural_arguments)
            .filter(|argument| {
                result.multiplicity != Multiplicity::Unrestricted
                    && call_owns_result(operation)
                    && argument.source_structural_result_binding_ordinal()
                        == Some(result.binding_ordinal)
                    && argument.type_identity == result.type_identity
                    && argument.access == CheckedStructuralAccess::Owned
                    && argument.path.is_empty()
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
        // A reference-loaned result's custody is the loan itself: its owner
        // is the release at the authored weakening boundary, not a
        // structural disposal.
        let reference_releases = match operation
            .call()
            .and_then(checked_trees::CheckedUnitCallPlan::structural_custody)
        {
            Some(custody) if custody.reference_loan.is_valid() => operations[producer_index + 1..]
                .iter()
                .filter(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::ReleaseReference { loan, .. }
                            if *loan == custody.reference_loan
                    )
                })
                .count(),
            _ => 0,
        };
        // A view result's custody is also its loan's release: the loan
        // minted at the establishment ends in exactly one Consume row —
        // on the selected edge that carries it when control moves, or at
        // the state exit when the scope simply closes. Count that release
        // as the owner; an owned-storage disposal never applies to a
        // reference shell. The Consume's root names the lent place, not
        // the view local, so the establishment's own claim identifies the
        // release. Only a loan-bound result needs the owner: an
        // Unrestricted view is admitted by its dominance permit, and
        // counting its release beside that permit would double-book.
        // A counted reborrow already owns the custody — the release the
        // permissions still record is the same loan's lexical end, not a
        // second owner, so it does not book twice beside the reborrow.
        let view_releases = if release_owned_view {
            facts
                .flow
                .ownership
                .permissions
                .iter()
                .filter(|(_, event)| {
                    event.kind == PermissionEventKind::Consume
                        && event.machine_symbol == machine.symbol
                        && event.state_symbol == state.symbol
                        && matches!(
                            event.claim_identity,
                            language_semantics::PermissionClaimIdentity::Established {
                                machine_symbol: claim_machine,
                                state_symbol: claim_state,
                                source: PermissionEventSource::Statement { statement_index },
                                ..
                            } if claim_machine == machine.symbol
                                && claim_state == state.symbol
                                && statement_index == result.statement_index as usize
                        )
                })
                .count()
        } else {
            0
        };
        // The checked loan release is the semantic owner of an affine view,
        // while Terminal's structural frontier owns the materialized carrier.
        // On an all-return guarded state the same one release therefore
        // restores the carrier's physical discard row; it is not a second
        // source-custody disposition in `total` below.
        let returning_view_disposal = (matches!(
            terminator,
            CheckedComposedUnitControlTerminatorPlan::Guarded { .. }
        ) || closed_sum_all_reference_returns)
            && view_releases == 1;
        // A byte replacement copies the completed live prefix and length;
        // it neither moves an affine owner nor invents a disposal receipt.
        // The store's source replay separately rejoins the exact call.
        let copied_into_byte_store = result.multiplicity == Multiplicity::Unrestricted
            && operations[producer_index + 1..].iter().any(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::ByteSequenceStore(store)
                    if store.value.completed_call_binding_ordinal() == Some(result.binding_ordinal))
            });
        // A whole unrestricted result can be copied or shared by a later call,
        // independently of whether a call or construction produced it. Neither
        // observation creates affine transfer debt; exact source occurrence,
        // producer order and loan authority are retained by the consumer.
        let observed_by_call = result.multiplicity == Multiplicity::Unrestricted
            && operations[producer_index + 1..]
                .iter()
                .flat_map(CheckedUnitEffectOperationPlan::call_structural_arguments)
                .any(|argument| {
                    argument.source_structural_result_binding_ordinal()
                        == Some(result.binding_ordinal)
                        && argument.type_identity == result.type_identity
                        && matches!(
                            argument.access,
                            CheckedStructuralAccess::Owned | CheckedStructuralAccess::SharedBorrow
                        )
                        && argument.path.is_empty()
                });
        let total = usize::from(
            consumed
                || stored
                || copied_into_construction
                || copied_into_byte_store
                || observed_by_call,
        ) + call_transfers
            + cleanup_discards
            + construction_transfers
            + reference_releases
            + reference_reborrows
            + view_releases;
        if total != 1 {
            return None;
        }
        if returning_view_disposal {
            guarded_discards.push(producer_index);
        }
    }
    for producer_index in guarded_discards {
        let (_, discard_result_on_return) = operations
            .get_mut(producer_index)?
            .discardable_structural_result_mut()?;
        *discard_result_on_return = true;
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
