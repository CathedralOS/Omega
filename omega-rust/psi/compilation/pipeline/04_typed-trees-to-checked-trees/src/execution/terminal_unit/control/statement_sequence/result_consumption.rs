//! Result and source consumption: structural result bindings are appended
//! per operation, retained results feed a call argument, and value-call
//! sources place the operation their operand evaluates into.
//! Producers and prior retirement receipts share the retained value-call
//! inventory: nesting cannot hide a completed release or authorize a second one.
//! One structural value's operand custody is one sequence: retain its call
//! plans, settle the sources those calls evaluate under the statement's
//! owned-selection policy, and adopt the anonymous producers they consume.

use super::super::super::{
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEntryClaimPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use super::super::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan, Multiplicity,
    PermissionEventSource, StatementNode, SymbolHandle,
};
use crate::execution::terminal_unit::ScalarCalleeClosure;
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::ShapeCollector;
use checked_trees::CheckedUnitStructuralReturnPlan;

/// The latest release boundary any loan in the group reaches: a descriptor
/// retires after every alternative source and every descendant can end.
fn latest_loan_release_boundary(
    order: &typed_trees::statement::StatementExecutionOrder,
    facts: &CheckFacts,
    loans: arena::HandleSpan<checked_trees::BorrowLoanFact>,
    release: impl Fn(arena::Handle<checked_trees::BorrowLoanFact>) -> Option<u32>,
) -> Option<u32> {
    facts
        .borrow
        .loan_group(loans)?
        .try_fold(0u32, |boundary, (loan, _)| {
            u32::try_from(order.latest(boundary as usize, release(loan)? as usize)?).ok()
        })
}

/// A carrier replaced in its declared home belongs to the replacement's
/// checked cleanup, not statement-entry expiry. Every alternative must name
/// that same disposition; an incomplete or mixed group cannot suppress release.
/// This remains true while scanning statements before the replacement exists
/// in the operation list. The assignment planner must still produce its normal
/// cleanup after capturing the RHS, and completion checks that disposition.
/// A descendant can keep the backing loan live beyond this carrier's death;
/// its lexical last use does not move the recorded replacement boundary.
fn replacement_owns_release(
    order: &typed_trees::statement::StatementExecutionOrder,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    loans: arena::HandleSpan<checked_trees::BorrowLoanFact>,
) -> bool {
    let Some(group) = facts.borrow.loan_group(loans) else {
        return false;
    };
    let mut replacement = None;
    for (handle, loan) in group {
        let Some(weakening) =
            validation::reference_result_custody::loan_weakening(facts, machine, state, handle)
        else {
            return false;
        };
        let checked_trees::FlowInvalidationSource::Statement { statement_index } = weakening.source
        else {
            return false;
        };
        if weakening.reason != checked_trees::FlowBorrowWeakeningReason::LocalReassigned
            || !order.precedes(loan.statement_index, statement_index)
            || replacement.is_some_and(|previous| previous != statement_index)
        {
            return false;
        }
        replacement = Some(statement_index);
    }
    replacement.is_some()
}

/// A selected call death closes this descriptor's anonymous backing before
/// the frame return. The retained graph authenticates membership; the exact
/// contextual disposition, not that membership, authorizes the owner's death.
fn normal_call_disposes_backing(
    facts: &CheckFacts,
    state: SymbolHandle,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &[CheckedUnitEffectOperationPlan],
    binding: u32,
) -> bool {
    let values = &facts.values.structural_values;
    let Some((holder, facts::PlaceRoot::Expression(expression))) = results
        .iter()
        .find(|(result, _)| result.binding_ordinal == binding)
    else {
        return false;
    };
    let Some(root) = values.root_for_expression(state, holder.statement_index, *expression) else {
        return false;
    };
    let Some(dependencies) = values.value_dependencies(root.root) else {
        return false;
    };
    operations
        .iter()
        .rev()
        .take_while(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    boundary: checked_trees::CheckedUnitCleanupBoundary::Call(_),
                    ..
                }
            )
        })
        .any(|operation| {
            let CheckedUnitEffectOperationPlan::ContinuationCleanup {
                boundary: checked_trees::CheckedUnitCleanupBoundary::Call(coordinate),
                cleanup_actions,
                ..
            } = operation
            else {
                return false;
            };
            cleanup_actions.iter().any(|action| {
                let expression = match *action.source() {
                    CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        expression,
                    } => expression,
                    CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => {
                        let Some((_, owner)) = results
                            .iter()
                            .find(|(result, _)| result.binding_ordinal == binding_ordinal)
                        else {
                            return false;
                        };
                        let Some(expression) = owner.as_expression() else {
                            return false;
                        };
                        expression
                    }
                    _ => return false,
                };
                let Some(owner) =
                    values.root_for_expression(state, coordinate.statement_index, expression)
                else {
                    return false;
                };
                owner.root != root.root
                    && dependencies.contains(&owner.root)
                    && facts.contextual_cleanup.any_state_disposition(
                        root.machine,
                        state,
                        |_, disposition| {
                            disposition.root == facts::PlaceRoot::Expression(expression)
                                && matches!(
                                    disposition.source,
                                    PermissionEventSource::Call {
                                        statement_index,
                                        call_ordinal,
                                        ..
                                    } if statement_index == coordinate.statement_index as usize
                                        && call_ordinal == coordinate.call_ordinal as usize
                                )
                        },
                    )
            })
        })
}

/// Source groups identify named loan resources. A computed descriptor has no
/// such group, so its exact result occurrence identifies its one release.
pub(in crate::execution::terminal_unit) fn release_matches(
    operation: &CheckedUnitEffectOperationPlan,
    result: &CheckedUnitStructuralResultBindingPlan,
    loans: arena::HandleSpan<checked_trees::BorrowLoanFact>,
) -> bool {
    operation.with_value_calls().any(|operation| {
        let CheckedUnitEffectOperationPlan::ReleaseReference {
            loans: released,
            source,
            path,
            ..
        } = operation
        else {
            return false;
        };
        if !path.is_empty() {
            return false;
        }
        if loans.is_empty() {
            *source
                == (CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                })
        } else {
            *released == loans
        }
    })
}

/// A whole discard closes both the carrier home and its stored descriptors.
/// Statement and exit expiry share this physical receipt: scheduling another
/// reference release after it would retire the same result twice. A projected
/// or case-conditional action does not close the whole occurrence.
/// Only statement-entry actions are final during sequencing. Call and
/// replacement actions are still provisional until temporary-action selection;
/// their transfer and replacement judgments remain separate below.
fn whole_result_discarded(
    operation: &CheckedUnitEffectOperationPlan,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> bool {
    let CheckedUnitEffectOperationPlan::ContinuationCleanup {
        boundary: checked_trees::CheckedUnitCleanupBoundary::StatementEntry { .. },
        cleanup_actions,
        ..
    } = operation
    else {
        return false;
    };
    cleanup_actions.iter().any(|action| {
        matches!(action, checked_trees::CheckedNormalCleanupAction::Discard(discard)
            if discard.source == CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            }
                && discard.path.is_empty()
                && discard.type_identity == result.type_identity
                && discard.case_conditions.is_empty())
    })
}

/// A prior constructor or call transfer already moved this descriptor's
/// custody to another holder.
fn custody_transferred(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    prepared_calls: &crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    binding_ordinal: u32,
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<bool> {
    Some(
        super::reference_retirement::transferred_paths(
            program,
            facts,
            prepared_calls,
            results,
            binding_ordinal,
            operations,
        )?
        .iter()
        .any(|path| path.is_empty()),
    )
}

/// A materialized carrier owns the same captured loan group whether formed
/// locally or returned by a call. Scheduling and custody accounting must join
/// that exact holder rather than select retirement by producer family.
pub(in crate::execution::terminal_unit) struct CapturedReferenceResult<'operation> {
    pub result: &'operation CheckedUnitStructuralResultBindingPlan,
    pub loans: arena::HandleSpan<checked_trees::BorrowLoanFact>,
    /// Scalar siblings and borrows of the carrier home can outlive its last
    /// descriptor observation. Whole disposal must retain both inventories.
    whole_storage_boundary: u32,
    storage_loans: Vec<arena::Handle<checked_trees::BorrowLoanFact>>,
}

/// Every named carrier retains its own lexical uses and borrowers, regardless
/// of whether a call, constructor, or reference formation produced it. An
/// anonymous carrier has no later authored uses; the result dependency graph
/// still retains it for descriptors formed from it.
fn captured_storage_lifetime(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<(u32, Vec<arena::Handle<checked_trees::BorrowLoanFact>>)> {
    let mut owners = results.iter().filter(|(binding, _)| binding == result);
    let (_, owner) = owners.next()?;
    if owners.next().is_some() {
        return None;
    }
    let facts::PlaceRoot::Symbol(owner) = owner else {
        return Some((0, Vec::new()));
    };
    let authored = program.state_by_symbol(state)?;
    let (formation, _) = program.state_local_declaration(authored, *owner)?;
    if formation != result.statement_index as usize {
        return None;
    }
    Some((
        validation::reference_result_custody::whole_local_release_boundary(
            program,
            facts,
            machine,
            authored,
            result.statement_index,
        )?,
        validation::reference_result_custody::local_storage_loans(facts, machine, state, *owner)?,
    ))
}

pub(in crate::execution::terminal_unit) fn captured_reference_result<'operation>(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operation: &'operation CheckedUnitEffectOperationPlan,
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<CapturedReferenceResult<'operation>> {
    let direct = match operation {
        CheckedUnitEffectOperationPlan::EstablishReference { result, .. } => {
            super::reference_retirement::local_reference_loans(
                facts, machine, state, result, results,
            )
            .map(|loans| (result, loans))
        }
        CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { result, value, .. },
        ) if facts.values.structural_values.nodes.is_valid(*value)
            && validation::reference_result_custody::stored_loan_parts(
                program,
                facts
                    .values
                    .structural_values
                    .nodes
                    .get(*value)
                    .type_reference,
            )
            .is_some() =>
        {
            // A transported bare descriptor has its own captured occurrence,
            // just like an explicitly formed one. Releasing it cannot consume
            // the supplying holder or wait for a copy as though it reborrowed.
            super::reference_retirement::local_reference_loans(
                facts, machine, state, result, results,
            )
            .map(|loans| (result, loans))
        }
        _ => operation.call().and_then(|call| {
            let custody = call.structural_custody()?;
            let result = call.structural_result()?;
            // A cleanup-free aggregate can carry the exact same captured
            // loan group as a bare descriptor. Neither the group alone nor
            // physical shape grants its release: retain both the actual
            // captured custody and the recursive no-hook/no-linear judgment.
            // Nominal owners keep their ordinary selected disposition.
            validation::call_target_return_type(program, call.target.state)
                .is_some_and(|returned| {
                    validation::has_contents(
                        program,
                        returned,
                        &[],
                        validation::ContentsRequirement::DescriptorCleanup,
                    ) && (!custody.reference_loans.is_empty()
                        || validation::reference_result_custody::stored_loan_parts(
                            program, returned,
                        )
                        .is_some())
                })
                .then_some((result, custody.reference_loans))
        }),
    };
    if let Some((result, loans)) = direct {
        let (whole_storage_boundary, storage_loans) =
            captured_storage_lifetime(program, facts, machine, state, results, result)?;
        return Some(CapturedReferenceResult {
            result,
            loans,
            whole_storage_boundary,
            storage_loans,
        });
    }
    let (result, _) = operation.discardable_structural_result()?;
    let loans = super::reference_retirement::carrier_loans(facts, machine, state, result, results)?;
    if loans.is_empty() {
        return None;
    }
    let authored_state = program.state_by_symbol(state)?;
    let mut owners = results.iter().filter(|(binding, _)| binding == result);
    let (_, facts::PlaceRoot::Symbol(owner)) = owners.next()? else {
        return None;
    };
    if owners.next().is_some() {
        return None;
    }
    let (formation, local) = program.state_local_declaration(authored_state, *owner)?;
    if formation != result.statement_index as usize
        || !validation::has_contents(
            program,
            local.type_reference,
            &[],
            validation::ContentsRequirement::DescriptorCleanup,
        )
        || !super::reference_retirement::transferred_paths(
            program,
            facts,
            &crate::execution::terminal_unit::structural_value_calls::PreparedCalls::EMPTY,
            results,
            result.binding_ordinal,
            operations,
        )?
        .is_empty()
        || super::reference_retirement::unique_borrow_state(facts, machine, state)?.is_some_and(
            |borrowing| {
                facts
                    .borrow
                    .loan_group(borrowing.loans)
                    .is_some_and(|mut loans| {
                        loans.any(|(_, loan)| {
                            loan.storage_owner == facts::PlaceRoot::Symbol(*owner)
                                && loan.statement_index != formation
                        })
                    })
            },
        )
    {
        return None;
    }
    // Completion computations are retained outside the body operation list.
    // A possible owned use on one selected path keeps this occurrence with
    // that path's normal custody; an unconditional release would race its move.
    // Replacement-owned deaths remain with replacement cleanup in the callers;
    // this conservative inventory never grants a transfer or release itself.
    if result.multiplicity != Multiplicity::Unrestricted
        && crate::execution::terminal_unit::storage_transfers::has_computation_transfer(
            program, facts, state, results, result, operations,
        )?
    {
        return None;
    }
    let (whole_storage_boundary, storage_loans) =
        captured_storage_lifetime(program, facts, machine, state, results, result)?;
    Some(CapturedReferenceResult {
        result,
        loans,
        whole_storage_boundary,
        storage_loans,
    })
}

/// End captured reference custody at entry to the next authored statement.
/// Whole carriers use ordinary affine edge disposal, which retires every leaf;
/// reference results retain one release of their complete group. Reverse production order
/// closes descendants before earlier parent carriers at a shared boundary.
#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn append_reference_releases(
    owner_candidates: &super::reference_retirement::OwnerCandidates,
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    prepared_calls: &crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
    shapes: &mut crate::execution::terminal_unit::types::ShapeCollector<'_>,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: u32,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
    trace: &super::super::super::LocalConstructionTrace,
) -> Option<()> {
    trace.phase("reference retirement: transport");
    let authored_state = program.state_by_symbol(state)?;
    let borrowing = super::reference_retirement::unique_borrow_state(facts, machine, state)?;
    if borrowing.is_none_or(|borrowing| borrowing.loans.is_empty())
        && !operations
            .iter()
            .chain(prepared_calls.operations_for(machine, state))
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
            .any(|operation| {
                operation
                    .call()
                    .and_then(|call| call.structural_custody())
                    .is_some()
            })
    {
        return Some(());
    }
    let order = program
        .statement_table
        .admitted_execution_order(authored_state.statement_nodes);
    let transported_loans = super::reference_retirement::transport::collect(
        program,
        facts,
        machine,
        authored_state,
        results,
        operations,
    )?;
    let mut pending = Vec::new();
    let mut retired = Vec::new();
    // Receiving carriers retire before their retained supplying descriptors.
    // Call inputs have the same statement boundaries as standalone producers;
    // leaving them to exit cleanup can outlive their borrowed local backing.
    for operation in operations
        .iter()
        .rev()
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
    {
        trace.phase("reference retirement: selected complements");
        let complements = super::reference_retirement::selections::complement_completions(
            &super::reference_retirement::Retirement {
                program,
                facts,
                prepared_calls,
                machine,
                state: authored_state,
                order: &order,
                statement_index,
                results,
                operations,
                completed_operations: operations.len(),
                pending: &pending,
                at_exit: false,
                uncommitted_owner: facts::PlaceRoot::Symbol(Default::default()),
                returned: None,
                transported_loans: &transported_loans,
            },
        )?;
        for (position, complement) in complements {
            trace.phase("reference retirement: complement receiver");
            let CheckedUnitEffectOperationPlan::ReleaseReference {
                selection_complements,
                ..
            } = pending.get_mut(position)?
            else {
                return None;
            };
            selection_complements.push(complement);
        }
        trace.phase("reference retirement: stored occurrences");
        let stored_releases = super::reference_retirement::stores::releases(
            &super::reference_retirement::Retirement {
                program,
                facts,
                prepared_calls,
                machine,
                state: authored_state,
                order: &order,
                statement_index,
                results,
                operations,
                completed_operations: operations.len(),
                pending: &pending,
                at_exit: false,
                uncommitted_owner: facts::PlaceRoot::Symbol(Default::default()),
                returned: None,
                transported_loans: &transported_loans,
            },
            operation,
        )?;
        pending.extend(stored_releases);
        // A cleared return-discard flag is not a physical retirement receipt.
        // Use the exact whole cleanup, preserving partial-carrier leaf expiry.
        if operation
            .discardable_structural_result()
            .is_some_and(|(result, _)| {
                operations
                    .iter()
                    .chain(&pending)
                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                    .any(|operation| whole_result_discarded(operation, result))
            })
        {
            continue;
        }
        let reference_result = captured_reference_result(
            program, facts, machine, state, results, operation, operations,
        );
        if let Some(captured) = reference_result {
            let result = captured.result;
            let loans = captured.loans;
            if replacement_owns_release(&order, facts, machine, state, loans) {
                continue;
            }
            trace.phase("reference retirement: captured custody transfer");
            if custody_transferred(
                program,
                facts,
                prepared_calls,
                results,
                result.binding_ordinal,
                operations,
            )? {
                // The receiving constructor or call owns the descriptor now.
                // Its former holder cannot release that transferred custody.
                continue;
            }

            // A descriptor retires once, after every alternative source and
            // every descendant can end. Never release an arbitrary candidate
            // early or emit one destructive operation per possible referent.
            trace.phase("reference retirement: captured loan boundary");
            let boundary = latest_loan_release_boundary(&order, facts, loans, |loan| {
                super::super::super::reference_results::release_boundary(
                    program, facts, machine, state, loan,
                )
            })?;
            trace.phase("reference retirement: captured storage boundary");
            let boundary = order.latest(
                boundary as usize,
                order.successor(result.statement_index as usize)?,
            )?;
            let boundary =
                u32::try_from(order.latest(boundary, captured.whole_storage_boundary as usize)?)
                    .ok()?;
            let retirement = super::reference_retirement::Retirement {
                program,
                facts,
                prepared_calls,
                machine,
                state: authored_state,
                order: &order,
                statement_index,
                results,
                operations,
                completed_operations: operations.len(),
                pending: &pending,
                at_exit: false,
                uncommitted_owner: facts::PlaceRoot::Symbol(Default::default()),
                returned: None,
                transported_loans: &transported_loans,
            };
            trace.phase("reference retirement: captured loan group");
            if order.at_or_before(boundary as usize, statement_index as usize)
                && retirement.carrier_children_retired(result)
                && captured
                    .storage_loans
                    .iter()
                    .all(|loan| retirement.holder_retired(*loan))
                && facts
                    .borrow
                    .loan_group(loans)?
                    .all(|(loan, _)| retirement.children_retired(loan))
                && !operations
                    .iter()
                    .chain(&pending)
                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                    .any(|operation| release_matches(operation, result, loans))
            {
                pending.push(CheckedUnitEffectOperationPlan::ReleaseReference {
                    statement_index,
                    source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: result.binding_ordinal,
                    },
                    path: Vec::new(),
                    select_evaluated_storage: false,
                    defer_to_exit: false,
                    loans,
                    selection_complements: Vec::new(),
                    owner_cleanups: arena::HandleSpan::empty(),
                    case_conditions: Default::default(),
                });
                // Explicit retirement consumes the descriptor now. Its
                // producer cannot also discard that carrier at state exit.
                if operation.discardable_structural_result().is_some() {
                    retired.push(result.binding_ordinal);
                }
            }
            // A copied aggregate can retain live scalar siblings after one
            // descriptor expires. Until whole retirement is scheduled, keep
            // the ordinary projected-leaf schedule below available.
            if captured.whole_storage_boundary == 0
                || operations
                    .iter()
                    .chain(&pending)
                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                    .any(|operation| release_matches(operation, result, loans))
            {
                continue;
            }
        }
        let Some((result, _)) = operation.discardable_structural_result() else {
            continue;
        };
        if result.multiplicity != Multiplicity::Affine
            || !order.precedes(result.statement_index as usize, statement_index as usize)
        {
            continue;
        }
        let Some(StatementNode::LocalData(local)) = program
            .statement_table
            .statements(authored_state.statement_nodes)
            .get(result.statement_index as usize)
        else {
            continue;
        };
        // Anonymous operands can share their enclosing local's statement and
        // type. Only the binding registered to that local owns its loan roster.
        if !results.iter().any(|(binding, root)| {
            binding == result && *root == facts::PlaceRoot::Symbol(local.symbol)
        }) {
            continue;
        }
        // This inventory describes a production, not every later value stored
        // in its named home. Replacement cleanup owns the old occurrence;
        // an initializer cannot acquire a second retirement after rebinding.
        if super::super::super::local_storage::occupant(
            program,
            authored_state,
            operations,
            local.symbol,
        ) != Some(result.binding_ordinal)
        {
            continue;
        }
        // A replacement creates a different captured occurrence. This roster
        // describes the initializer, so leave a replaced home to its existing
        // replacement/exit custody until that occurrence is represented here.
        if borrowing.is_some_and(|borrowing| {
            facts
                .borrow
                .loan_group(borrowing.loans)
                .is_some_and(|mut loans| {
                    loans.any(|(_, loan)| {
                        loan.owner_symbol == local.symbol
                            && loan.statement_index != result.statement_index as usize
                            && order.precedes(loan.statement_index, statement_index as usize)
                    })
                })
        }) {
            continue;
        }
        if borrowing
            .and_then(|borrowing| {
                facts.borrow.storage_loans(
                    borrowing,
                    facts::PlaceRoot::Symbol(local.symbol),
                    result.statement_index as usize,
                )
            })
            .is_some_and(|loans| loans.is_empty())
        {
            continue;
        }
        // Early descriptor expiry may retire only cleanup-free contents.
        // A nominal owner or descendant keeps its references for the checked
        // normal-disposal transaction, including the hook's receiver borrow.
        if !validation::has_contents(
            program,
            local.type_reference,
            &[],
            validation::ContentsRequirement::DescriptorCleanup,
        ) {
            continue;
        }
        // Transfers remove only their selected subtree. Other stored leaves
        // may still end at their own checked boundary; a whole-value transfer
        // leaves no initializer custody here at all.
        trace.phase("reference retirement: local custody transfer");
        let transferred_paths = super::reference_retirement::transferred_paths(
            program,
            facts,
            prepared_calls,
            results,
            result.binding_ordinal,
            operations,
        )?;
        if transferred_paths.iter().any(|path| path.is_empty()) {
            continue;
        }
        let Some(boundary) = validation::reference_result_custody::whole_local_release_boundary(
            program,
            facts,
            machine,
            authored_state,
            result.statement_index,
        ) else {
            continue;
        };
        let retains_siblings = order.precedes(statement_index as usize, boundary as usize)
            || !transferred_paths.is_empty();
        let retirement = super::reference_retirement::Retirement {
            program,
            facts,
            prepared_calls,
            machine,
            state: authored_state,
            order: &order,
            statement_index,
            results,
            operations,
            completed_operations: operations.len(),
            pending: &pending,
            at_exit: false,
            uncommitted_owner: facts::PlaceRoot::Symbol(Default::default()),
            returned: None,
            transported_loans: &transported_loans,
        };
        trace.phase("reference retirement: local loan leaves");
        let local_loans = validation::reference_result_custody::local_record_loans(
            program,
            facts,
            machine,
            authored_state,
            result.statement_index,
        )?;
        // Descriptor expiry needs an actual captured descriptor. An empty
        // roster supplies no authority to dispose established contents: a
        // zeroed, unestablished local may owe only home retirement. Ordinary
        // owned storage keeps its source-selected normal-exit partition.
        if local_loans.is_empty() {
            continue;
        }
        if !retains_siblings
            && local_loans.iter().any(|(_, group)| {
                !facts.borrow.loan_group(*group).is_some_and(|mut loans| {
                    loans.all(|(loan, _)| retirement.children_retired(loan))
                })
            })
        {
            continue;
        }
        let leaf_releases = if retains_siblings {
            local_loans
                .into_iter()
                .rev()
                .filter(|(source, group)| {
                    facts.borrow.loan_group(*group).is_some_and(|mut loans| {
                            loans.all(|(loan, _)| retirement.children_retired(loan))
                        }) && !transferred_paths.iter().any(|path| {
                            path.starts_with(&source.path) || source.path.starts_with(path)
                        }) && latest_loan_release_boundary(&order, facts, *group, |loan| {
                            validation::reference_result_custody::release_statement(
                                program, facts, machine, state, loan,
                            )
                        })
                        .is_some_and(|boundary| {
                            order.at_or_before(boundary as usize, statement_index as usize)
                        }) && !operations.iter().chain(&pending)
                        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                        .any(|operation| {
                            matches!(operation, CheckedUnitEffectOperationPlan::ReleaseReference {
                            loans: released, ..
                        } if *released == *group)
                        })
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        if retains_siblings && leaf_releases.is_empty() {
            continue;
        }
        // Lexically expired leaves can remain stored in a live aggregate.
        // Retire their actual holders before the storage they borrow. The
        // reverse producer walk makes pending cleanups available to older
        // owners here, so the same join also orders transitive dependencies.
        // A deferred owner is retried after its earliest permitted boundary.
        trace.phase("reference retirement: local storage loans");
        let storage_loans = validation::reference_result_custody::local_storage_loans(
            facts,
            machine,
            state,
            local.symbol,
        )?;
        if storage_loans
            .into_iter()
            .any(|handle| !retirement.holder_retired(handle))
        {
            continue;
        }
        // A leaf's checked weakening retires only that descriptor. Its scalar
        // siblings remain established storage; the later whole death schedule
        // is completed from the exact released-path complement during lowering.
        if retains_siblings {
            for (source, group) in leaf_releases {
                pending.push(CheckedUnitEffectOperationPlan::ReleaseReference {
                    statement_index,
                    source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: result.binding_ordinal,
                    },
                    path: source.path,
                    select_evaluated_storage: false,
                    defer_to_exit: false,
                    loans: group,
                    selection_complements: Vec::new(),
                    owner_cleanups: arena::HandleSpan::empty(),
                    case_conditions: Default::default(),
                });
            }
            continue;
        }
        retired.push(result.binding_ordinal);
        pending.push(CheckedUnitEffectOperationPlan::ContinuationCleanup {
            selected_regions: Vec::new(),
            boundary: checked_trees::CheckedUnitCleanupBoundary::StatementEntry { statement_index },
            cleanup_actions: vec![checked_trees::CheckedNormalCleanupAction::Discard(
                checked_trees::CheckedUnitPartialAffineDiscardPlan {
                    source:
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: result.binding_ordinal,
                        },
                    path: Vec::new(),
                    type_identity: result.type_identity.clone(),
                    case_conditions: Default::default(),
                },
            )],
        });
    }
    // A descriptor inside backing can itself lend another computed home.
    // Each emitted release closes an actual holder; repeat until this finite
    // storage dependency frontier stops changing, without re-evaluating values.
    loop {
        trace.phase("reference retirement: backing occurrences");
        let releases = super::reference_retirement::Retirement {
            program,
            facts,
            prepared_calls,
            machine,
            state: authored_state,
            order: &order,
            statement_index,
            results,
            operations,
            completed_operations: operations.len(),
            pending: &pending,
            at_exit: false,
            uncommitted_owner: facts::PlaceRoot::Symbol(Default::default()),
            returned: None,
            transported_loans: &transported_loans,
        }
        .backing_releases()?;
        if releases.is_empty() {
            break;
        }
        pending.extend(releases);
    }
    for binding in retired {
        trace.phase("reference retirement: producer disposal");
        let (_, discard_on_return) = operations.iter_mut().find_map(|operation| {
            operation.discardable_structural_result_by_ordinal_mut(binding)
        })?;
        *discard_on_return = false;
    }
    trace.phase("reference retirement: case conditions");
    super::reference_retirement::prepare_owner_cleanups(
        owner_candidates,
        program,
        facts,
        prepared_calls,
        shapes,
        machine,
        authored_state,
        statement_index,
        None,
        false,
        results,
        operations,
        operations.len(),
        &mut pending,
    )?;
    super::reference_retirement::prepare_case_conditions(
        program,
        facts,
        shapes,
        machine,
        state,
        parameters,
        results,
        &mut pending,
    )?;
    // This boundary closes the previous statement's loans before disposing
    // its anonymous backing. Call continuations have already selected those
    // deaths; a descriptor release appended after their hooks would keep an
    // overlapping loan live while the owner is disposed. Only trailing call
    // cleanup moves behind this independently admitted weakening: no call or
    // operand evaluation is crossed.
    let insertion = operations
        .iter()
        .rposition(|operation| {
            !matches!(operation,
        CheckedUnitEffectOperationPlan::ContinuationCleanup {
            boundary: checked_trees::CheckedUnitCleanupBoundary::Call(coordinate), ..
        } if order.precedes(coordinate.statement_index as usize, statement_index as usize))
        })
        .map_or(0, |position| position + 1);
    operations.splice(insertion..insertion, pending);
    Some(())
}

// Loans released by the state's natural exit carry their weakening at a
// boundary the per-statement loop never visits (the transition statement, or
// one past the last ordinary statement). The completion still needs the
// `ReleaseReference` op so the result-binding cleanup sees the disposal.
pub(in crate::execution::terminal_unit) fn append_exit_reference_releases(
    owner_candidates: &super::reference_retirement::OwnerCandidates,
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    prepared_calls: &crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
    shapes: &mut crate::execution::terminal_unit::types::ShapeCollector<'_>,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    machine: SymbolHandle,
    state: SymbolHandle,
    returned: Option<&CheckedUnitStructuralReturnPlan>,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
    trace: &super::super::LocalConstructionTrace,
) -> Option<()> {
    let authored_state = program.state_by_symbol(state)?;
    let borrowing = super::reference_retirement::unique_borrow_state(facts, machine, state)?;
    if borrowing.is_none_or(|borrowing| borrowing.loans.is_empty())
        && !operations
            .iter()
            .chain(prepared_calls.operations_for(machine, state))
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
            .any(|operation| {
                operation
                    .call()
                    .and_then(|call| call.structural_custody())
                    .is_some()
            })
    {
        return Some(());
    }
    let order = program
        .statement_table
        .admitted_execution_order(authored_state.statement_nodes);
    let mut pending = Vec::new();
    trace.phase("statement sequence: completion carrier inventory");
    // Retained call operands own real descriptor occurrences. Their output
    // transfers into the enclosing value, but borrowed input holders still
    // retire after that receiving value and before their backing homes.
    for operation in operations
        .iter()
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
    {
        if let Some(captured) = captured_reference_result(
            program, facts, machine, state, results, operation, operations,
        ) {
            let result = captured.result;
            let loans = captured.loans;
            trace.statement(Some(result.statement_index));
            if operations
                .iter()
                .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                .any(|operation| whole_result_discarded(operation, result))
            {
                continue;
            }
            if replacement_owns_release(&order, facts, machine, state, loans) {
                continue;
            }
            // The exit boundary consumes the same current custody as an
            // ordinary statement. A prior constructor/call transfer already
            // moved this descriptor to another holder.
            if custody_transferred(
                program,
                facts,
                prepared_calls,
                results,
                result.binding_ordinal,
                operations,
            )? {
                continue;
            }
            // Completion transfers this exact carrier and its retained ingress
            // rather than releasing it before the return. Earlier weakening
            // still belongs to statement scheduling; only a loan live through
            // this state's final statement can cross its result boundary.
            if returned.is_some_and(|returned| {
                super::reference_retirement::returned_reference_binding(&order, returned, result)
            }) {
                trace.phase("statement sequence: returned carrier loan boundary");
                trace.statement(Some(result.statement_index));
                let authored = program.state_by_symbol(state)?;
                let statement_count = program
                    .statement_table
                    .statements(authored.statement_nodes)
                    .len();
                if !facts.borrow.loan_group(loans)?.all(|(loan, _)| {
                    super::super::super::reference_results::release_boundary(
                        program, facts, machine, state, loan,
                    )
                    .is_some_and(|boundary| boundary as usize == statement_count)
                }) {
                    return None;
                }
                continue;
            }
            trace.phase("statement sequence: completion carrier release boundary");
            trace.statement(Some(result.statement_index));
            let boundary = latest_loan_release_boundary(&order, facts, loans, |loan| {
                super::super::super::reference_results::release_statement(
                    program, facts, machine, state, loan,
                )
            })?;
            let boundary = order.latest(
                boundary as usize,
                order.successor(result.statement_index as usize)?,
            )?;
            let boundary =
                u32::try_from(order.latest(boundary, captured.whole_storage_boundary as usize)?)
                    .ok()?;
            if !operations
                .iter()
                .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                .any(|operation| release_matches(operation, result, loans))
                && !pending
                    .iter()
                    .any(|(_, operation, _)| release_matches(operation, result, loans))
            {
                pending.push((
                    boundary,
                    CheckedUnitEffectOperationPlan::ReleaseReference {
                        statement_index: boundary,
                        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: result.binding_ordinal,
                        },
                        path: Vec::new(),
                        select_evaluated_storage: false,
                        // Completion evaluates and captures its result before
                        // closing frame-local descriptors. A shared result
                        // copy can still retain a supplying local even after
                        // its authored holder's last use. The selected exit
                        // therefore owns these dispositions, just as it owns
                        // descriptors used by transition operands.
                        defer_to_exit: true,
                        loans,
                        selection_complements: Vec::new(),
                        owner_cleanups: arena::HandleSpan::empty(),
                        case_conditions: Default::default(),
                    },
                    captured.storage_loans,
                ));
            }
        }
    }
    pending.sort_by_key(|(boundary, _, _)| order.position(*boundary as usize));
    let statement_index = authored_state.statement_nodes.count();
    trace.phase("statement sequence: completion carrier transport");
    let transported_loans = super::reference_retirement::transport::collect(
        program,
        facts,
        machine,
        authored_state,
        results,
        operations,
    )?;
    // Lexical last use does not order descriptor death: a parent's last
    // authored use can form a child that lives until this same completion.
    // Reuse statement retirement's exact holder dependencies, allowing only
    // already scheduled exit releases to satisfy them. Computed backing
    // holders participate in the same fixed point rather than a later pass.
    trace.phase("statement sequence: completion carrier retirement order");
    let mut scheduled = Vec::new();
    loop {
        let retirement = super::reference_retirement::Retirement {
            program,
            facts,
            prepared_calls,
            machine,
            state: authored_state,
            order: &order,
            statement_index,
            results,
            operations,
            completed_operations: operations.len(),
            pending: &scheduled,
            at_exit: true,
            uncommitted_owner: facts::PlaceRoot::Symbol(Default::default()),
            returned,
            transported_loans: &transported_loans,
        };
        let complements =
            super::reference_retirement::selections::complement_completions(&retirement)?;
        if !complements.is_empty() {
            for (position, complement) in complements {
                let CheckedUnitEffectOperationPlan::ReleaseReference {
                    selection_complements,
                    ..
                } = scheduled.get_mut(position)?
                else {
                    return None;
                };
                selection_complements.push(complement);
            }
            continue;
        }
        let mut next = None;
        for (index, (_, operation, storage_loans)) in pending.iter().enumerate() {
            let CheckedUnitEffectOperationPlan::ReleaseReference { loans, source, .. } = operation
            else {
                return None;
            };
            let CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } =
                source
            else {
                return None;
            };
            let (result, _) = results
                .iter()
                .find(|(result, _)| result.binding_ordinal == *binding_ordinal)?;
            if retirement.carrier_children_retired(result)
                && storage_loans
                    .iter()
                    .all(|loan| retirement.holder_retired(*loan))
                && facts
                    .borrow
                    .loan_group(*loans)?
                    .all(|(loan, _)| retirement.children_retired(loan))
            {
                next = Some(index);
                break;
            }
        }
        if let Some(index) = next {
            scheduled.push(pending.remove(index).1);
            continue;
        }
        let mut releases = retirement.backing_releases()?;
        // A final store can establish a new named descriptor after the last
        // ordinary statement boundary. Its RHS transferred into that home;
        // retiring only result carriers would strand the receiving occurrence
        // until after its backing's source-state death. Reuse the same exact
        // formation groups and dependency checks at this selected completion.
        for operation in operations
            .iter()
            .rev()
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        {
            releases.extend(super::reference_retirement::stores::releases(
                &retirement,
                operation,
            )?);
        }
        if releases.is_empty() {
            if !pending.is_empty() {
                return None;
            }
            break;
        }
        scheduled.extend(releases);
    }
    let insertion = operations
        .iter()
        .rposition(|operation| {
            !matches!(
                operation,
                CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    boundary: checked_trees::CheckedUnitCleanupBoundary::Call(_),
                    ..
                }
            )
        })
        .map_or(0, |position| position + 1);
    for index in 0..scheduled.len() {
        if let CheckedUnitEffectOperationPlan::ReleaseReference {
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal },
            path,
            ..
        } = &scheduled[index]
            && path.is_empty()
            && normal_call_disposes_backing(facts, state, results, operations, *binding_ordinal)
        {
            let captured = operations
                .iter()
                .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                .filter_map(|operation| {
                    captured_reference_result(
                        program, facts, machine, state, results, operation, operations,
                    )
                })
                .find(|captured| captured.result.binding_ordinal == *binding_ordinal)?;
            // Exit readiness may depend on releases deferred until after
            // outgoing values are captured. Recheck against only immediate
            // dispositions before moving this descriptor ahead of owner hooks.
            // The existing child-first order makes earlier promotions visible;
            // a deferred descendant or returned holder cannot justify one.
            let retirement = super::reference_retirement::Retirement {
                program,
                facts,
                prepared_calls,
                machine,
                state: authored_state,
                order: &order,
                statement_index,
                results,
                operations,
                completed_operations: insertion,
                pending: &scheduled[..index],
                at_exit: false,
                uncommitted_owner: facts::PlaceRoot::Symbol(Default::default()),
                returned: None,
                transported_loans: &transported_loans,
            };
            if retirement.carrier_children_retired(captured.result)
                && captured
                    .storage_loans
                    .iter()
                    .all(|loan| retirement.holder_retired(*loan))
                && facts
                    .borrow
                    .loan_group(captured.loans)?
                    .all(|(loan, _)| retirement.children_retired(loan))
                && let CheckedUnitEffectOperationPlan::ReleaseReference { defer_to_exit, .. } =
                    &mut scheduled[index]
            {
                *defer_to_exit = false;
            }
        }
    }
    for release in &scheduled {
        if let CheckedUnitEffectOperationPlan::ReleaseReference {
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal },
            defer_to_exit: false,
            ..
        } = release
            && let Some((_, discard)) = operations.iter_mut().find_map(|operation| {
                operation.discardable_structural_result_by_ordinal_mut(*binding_ordinal)
            })
        {
            // Calls also carry an automatic discard flag. EstablishReference
            // has none: its explicit release is already the sole disposition.
            *discard = false;
        }
    }
    super::reference_retirement::prepare_owner_cleanups(
        owner_candidates,
        program,
        facts,
        prepared_calls,
        shapes,
        machine,
        authored_state,
        statement_index,
        returned,
        true,
        results,
        operations,
        insertion,
        &mut scheduled,
    )?;
    trace.phase("statement sequence: completion carrier case conditions");
    super::reference_retirement::prepare_case_conditions(
        program,
        facts,
        shapes,
        machine,
        state,
        parameters,
        results,
        &mut scheduled,
    )?;
    operations.splice(insertion..insertion, scheduled);
    Some(())
}

/// A selected continuation owns both the chosen result and the conditional
/// complement until the actual death edge. Original producers must not also
/// publish unconditional return drops for those same owners.
pub(in crate::execution::terminal_unit) fn retain_selected_sources(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statement: u32,
    operations: &mut [CheckedUnitEffectOperationPlan],
) -> Option<()> {
    let Some((_, receipt)) = facts
        .flow
        .ownership
        .owned_selection_at(state.symbol, statement)
    else {
        return Some(());
    };
    for source in facts
        .flow
        .ownership
        .selection_sources
        .span(receipt.sources)?
    {
        // A state-entry source (an owned parameter) has no producing
        // operation to unmark: its residual custody already rides the
        // receipt's selected edges instead of an unconditional return drop.
        if program
            .state_parameter_by_symbol(state, source.symbol)
            .is_some()
        {
            continue;
        }
        let storage =
            super::super::super::local_storage::source(program, state, operations, source.symbol)?;
        let CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } = storage
        else {
            // Rebinding already transferred the RHS producer to current named
            // storage. Selection owns that occurrence; its immutable birth
            // has no unconditional return debt to retire again.
            if matches!(
                storage,
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
            ) {
                continue;
            }
            return None;
        };
        let mut producers = operations
            .iter()
            .enumerate()
            .filter_map(|(index, operation)| {
                operation
                    .statement_structural_result_binding()
                    .filter(|result| result.binding_ordinal == binding_ordinal)
                    .map(|result| (index, result))
            });
        let (producer, result) = producers.next()?;
        if producers.next().is_some()
            || !matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Linear
            )
            || result.statement_index != source.statement_ordinal
        {
            return None;
        }
        let linear = result.multiplicity == Multiplicity::Linear;
        if linear && source.retains_storage {
            // A whole selected transfer settles linear storage. An unselected
            // owner or projected complement still requires its own explicit
            // disposition; clearing a producer flag cannot supply one.
            return None;
        }
        // The checked selection receipt owns this exact current source.
        // Retain its ordinary producer until the selected edges consume it.
        // A linear producer already has no implicit discard; that absence
        // does not make its represented storage unavailable for transport.
        // Some producers have no automatic whole-result disposal at all.
        // Their actual completed binding is still a retained source; selection
        // must not require a disposal flag to establish current custody.
        if let Some((_, discard)) =
            operations[producer].discardable_structural_result_by_ordinal_mut(binding_ordinal)
        {
            if linear && *discard {
                return None;
            }
            *discard = false;
        }
    }
    Some(())
}

/// Discharge only producers owned by this sequence. Call-owned operand
/// preparation has its own result namespace segment; its enclosing sequence
/// owns earlier locals. Each owner updates its own disposal debt, while source
/// custody independently rejoins every operand across that boundary.
pub(in crate::execution::terminal_unit) fn consume_retained_results(
    operations: &mut [CheckedUnitEffectOperationPlan],
    consumer: &CheckedUnitEffectOperationPlan,
) -> Option<()> {
    let Some(call) = consumer.call() else {
        return Some(());
    };
    for argument in &call.structural_arguments {
        let Some(binding) = argument.source_structural_result_binding_ordinal() else {
            continue;
        };
        if operations.iter().any(|operation| {
            operation
                .structural_result_binding()
                .is_some_and(|result| result.binding_ordinal == binding)
        }) {
            consume_result(
                operations,
                binding,
                argument.access,
                !argument.path.is_empty(),
            )?;
        }
    }
    Some(())
}

pub(in crate::execution::terminal_unit) fn consume_value_call_sources(
    facts: &CheckFacts,
    results: &mut Vec<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)>,
    operations: &mut [CheckedUnitEffectOperationPlan],
    calls: &[checked_trees::CheckedStructuralValueCall],
) -> Option<()> {
    for call in calls {
        // Every retained producer has an exact source occurrence, including
        // call inputs/results inside a selected value. This is a catalogue,
        // not a claim that every alternative executes: retirement still needs
        // an actual release or complete receiving relation for that occurrence.
        for operand in call
            .operands()
            .iter()
            .chain(call.operation())
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        {
            let Some(result) = operand.structural_result_binding() else {
                continue;
            };
            let expression = if let Some(producer) = operand.call() {
                let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
                    producer.source_site()?
                else {
                    return None;
                };
                expression
            } else {
                match operand {
                    CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                            value, ..
                        },
                    ) => facts.values.structural_values.nodes.get(*value).expression,
                    CheckedUnitEffectOperationPlan::EstablishReference {
                        formation_expression,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::MoveStructuralField {
                        expression: formation_expression,
                        ..
                    } => *formation_expression,
                    _ => return None,
                }
            };
            let root = facts::PlaceRoot::Expression(expression);
            if let Some((existing, existing_root)) = results
                .iter()
                .find(|(existing, _)| existing.binding_ordinal == result.binding_ordinal)
            {
                if existing != result || *existing_root != root {
                    return None;
                }
                continue;
            }
            results.push((result.clone(), root));
        }
        for operation in call
            .operands()
            .iter()
            .chain(call.operation())
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        {
            if let CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
            ) = operation
            {
                consume_value_places(facts, *value, results, operations)?;
            }
            consume_retained_results(operations, operation)?;
        }
    }
    Some(())
}

pub(in crate::execution::terminal_unit) fn consume_results(
    operations: &mut [CheckedUnitEffectOperationPlan],
    consumer: &CheckedUnitEffectOperationPlan,
) -> Option<()> {
    {
        for (binding_ordinal, access, projected) in consumer
            .structural_operands()
            .iter()
            .filter_map(|argument| {
                argument
                    .source_structural_result_binding_ordinal()
                    .map(|ordinal| (ordinal, argument.access, !argument.path.is_empty()))
            })
        {
            consume_result(operations, binding_ordinal, access, projected)?;
        }
    }
    // A whole-result field store consumes the minted binding the same way a
    // call argument does: Owned access retires the producer's disposal debt.
    if let Some(value) = consumer.installed_value()
        && let Some(binding_ordinal) = value.source_structural_result_binding_ordinal()
    {
        consume_result(
            operations,
            binding_ordinal,
            value.access,
            !value.path.is_empty(),
        )?;
    }
    Some(())
}

pub(in crate::execution::terminal_unit) fn consume_result(
    operations: &mut [CheckedUnitEffectOperationPlan],
    binding_ordinal: u32,
    access: CheckedStructuralAccess,
    projected: bool,
) -> Option<()> {
    let mut producers = operations.iter_mut().filter(|operation| {
        operation
            .structural_result_binding()
            .is_some_and(|result| result.binding_ordinal == binding_ordinal)
    });
    let producer = producers.next()?;
    if producers.next().is_some() {
        return None;
    }
    // A detached field owns its moved custody, not a newly minted automatic
    // return discard. Whole and projected uses keep the same owned-value
    // rules; source ownership and residual cleanup account for every child.
    // Its original place's restoration debt remains in OpenWindows.
    if matches!(
        producer,
        CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
    ) {
        return Some(());
    }
    // Copying a whole value or a selected subtree leaves the producer's
    // storage and disposal schedule intact. That follows its checked result
    // multiplicity, not whether a call, constructor or boundary produced it.
    // Do this before descriptor retirement: a copyable aggregate may carry
    // shared loans without transferring its original descriptor occurrences.
    if producer
        .structural_result_binding()
        .is_some_and(|result| result.multiplicity == Multiplicity::Unrestricted)
    {
        return Some(());
    }
    let reference_carrier = matches!(
        producer,
        CheckedUnitEffectOperationPlan::EstablishReference { .. }
    ) || producer
        .call()
        .and_then(checked_trees::CheckedUnitCallPlan::structural_custody)
        .is_some_and(|custody| !custody.reference_loans.is_empty());
    if reference_carrier {
        // Formation and call results own the same permission carrier. Lending
        // its referent leaves disposal with the producer; it does not require
        // an owned-result discard flag or transfer the borrowed backing.
        // Argument admission retains exact access, path and loan lifetime.
        // A whole owned operand transfers the descriptor itself, including
        // its loan and later retirement, to the receiving value. Lending the
        // referent keeps that descriptor in this holder instead. A returned
        // slice already is the descriptor, while a stored reference crosses
        // its referent path. Both leave disposal here; argument admission owns
        // that type/path distinction, not the cleanup scheduling flag.
        return match access {
            CheckedStructuralAccess::Owned => !projected,
            CheckedStructuralAccess::SharedBorrow
            | CheckedStructuralAccess::MutableBorrow
            | CheckedStructuralAccess::WriteOnlyBorrow => true,
        }
        .then_some(());
    }
    if matches!(
        access,
        CheckedStructuralAccess::SharedBorrow
            | CheckedStructuralAccess::MutableBorrow
            | CheckedStructuralAccess::WriteOnlyBorrow
    ) {
        // A borrow argument rejoins the producer's loan, not its disposal
        // debt: borrow custody rides the selected edge's claim partition, so
        // a result whose return-discard the edge already owns, or a producer
        // that never mints one (an exclusive view subslice), borrows the same
        // as one still carrying the flag.
        return Some(());
    }
    let (result, discard_result_on_return) = producer.discardable_structural_result_mut()?;
    if result.multiplicity != Multiplicity::Affine {
        // Linear results never acquire automatic disposal debt. Moving a
        // whole result continues its checked call claim frontier; source flow
        // and independent Terminal replay reject a second use of that claim.
        if result.multiplicity == Multiplicity::Linear
            && !*discard_result_on_return
            && access == CheckedStructuralAccess::Owned
            && !projected
        {
            return Some(());
        }
        return None;
    }
    // This flag schedules disposal; it is not the owner's liveness bit.
    // Composed states clear it before planning their selected edges because
    // those edges own the exact cleanup/transfer partition. An affine move
    // still consumes the same producer there. Source permission events reject
    // duplicate moves and uses after consumption, regardless of this flag.
    // A projected transfer leaves its root owner alive until the exact
    // complement is committed on the consumer continuation.
    if !projected {
        *discard_result_on_return = false;
    }
    Some(())
}

/// Sequence one structural value's operand custody into the operation stream:
/// retain the value's call plans, settle the sources those calls evaluate
/// under the statement's owned-selection policy, and adopt the exact anonymous
/// producers into the receiving results. Returns the plans the value's
/// establishment consumes.
///
/// A selected continuation owns both the chosen result and the conditional
/// complement until the actual death edge, so a value rooted in an owned
/// selection retains its sources instead of consuming them unconditionally.
/// That policy belongs to this sequence, not to each producer site.
#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn prepare_value_operands(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    statement_ordinal: u32,
    results: &mut Vec<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)>,
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
    structural_count: &mut usize,
    root: checked_trees::CheckedStructuralValueHandle,
    trace: &LocalConstructionTrace,
    windows: &mut super::super::super::borrowed_windows::OpenWindows,
) -> Option<(
    Vec<checked_trees::CheckedStructuralValueCall>,
    checked_trees::CheckedValueCompletion,
)> {
    let (calls, completion) = super::super::structural_operands::value_calls(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        state,
        structural_parameters,
        trivial_affine_locals,
        entry_claims,
        results,
        operations,
        structural_count,
        root,
        trace,
        windows,
    )?;
    if completion == checked_trees::CheckedValueCompletion::NoNormal {
        // Only invocations actually reached consume their arguments. The
        // enclosing construction never completes and transfers no whole root.
        consume_value_call_sources(facts, results, operations, &calls)?;
        return Some((calls, completion));
    }
    if facts
        .flow
        .ownership
        .owned_selection_at(state.symbol, statement_ordinal)
        .is_some()
    {
        retain_selected_sources(program, facts, state, statement_ordinal, operations)?;
    } else {
        consume_value_places(facts, root, results, operations)?;
    }
    consume_value_call_sources(facts, results, operations, &calls)?;
    Some((calls, completion))
}

pub(in crate::execution::terminal_unit) fn consume_value_places(
    facts: &CheckFacts,
    root: checked_trees::CheckedStructuralValueHandle,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut [CheckedUnitEffectOperationPlan],
) -> Option<()> {
    let plans = &facts.values.structural_values;
    let mut pending = vec![(root, false)];
    let mut visited = Vec::new();
    while let Some((value, projected)) = pending.pop() {
        if !plans.nodes.is_valid(value) || visited.contains(&value) {
            return None;
        }
        visited.push(value);
        match &plans.nodes.get(value).kind {
            checked_trees::CheckedStructuralValueKind::NativeCodeLayout { .. } => {}
            checked_trees::CheckedStructuralValueKind::QuotientCall { arguments, .. } => {
                pending.extend(arguments.iter().rev().map(|value| (*value, false)));
            }
            checked_trees::CheckedStructuralValueKind::Place(argument) => {
                if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                } = argument.source
                {
                    let mut matching = results
                        .iter()
                        .filter(|(_, source)| *source == facts::PlaceRoot::Symbol(symbol));
                    let (result, _) = matching.next()?;
                    if matching.next().is_some() || result.type_identity != argument.type_identity {
                        return None;
                    }
                    consume_result(
                        operations,
                        result.binding_ordinal,
                        argument.access,
                        projected || !argument.path.is_empty(),
                    )?;
                }
            }
            checked_trees::CheckedStructuralValueKind::Record { fields, .. }
            | checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                for field in plans.record_fields.span(*fields)? {
                    if let checked_trees::CheckedStructuralRecordFieldValue::Structural(value) =
                        field.value
                    {
                        pending.push((value, false));
                    }
                }
            }
            checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => {
                pending.extend(
                    plans
                        .dispatch_arms
                        .span(*arms)?
                        .iter()
                        .map(|arm| (arm.value, projected)),
                );
            }
            checked_trees::CheckedStructuralValueKind::Projection {
                source,
                path,
                copies_selected_value,
                ..
            } => {
                // A whole-value projection (including qualification) moves
                // the same owner. Only a selected child or copied observation
                // leaves residual custody. Carry that distinction through
                // nested projections instead of recognizing one place shape.
                pending.push((
                    *source,
                    projected || !path.is_empty() || *copies_selected_value,
                ));
            }
            checked_trees::CheckedStructuralValueKind::ViewSubslice { source, .. } => {
                pending.push((*source, true));
            }
            checked_trees::CheckedStructuralValueKind::QuotientAdaptation { source, .. } => {
                pending.push((*source, projected));
            }
            checked_trees::CheckedStructuralValueKind::FixedArray { elements } => {
                pending.extend(elements.iter().map(|value| (*value, false)));
            }
            checked_trees::CheckedStructuralValueKind::Reference { source_value, .. } => {
                if source_value.is_valid() {
                    pending.push((*source_value, false));
                }
            }
            checked_trees::CheckedStructuralValueKind::BorrowedSliceView { .. }
            | checked_trees::CheckedStructuralValueKind::MutableSliceView { .. }
            | checked_trees::CheckedStructuralValueKind::Call { .. }
            | checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. }
            | checked_trees::CheckedStructuralValueKind::ZeroedScalarArray { .. }
            | checked_trees::CheckedStructuralValueKind::ScalarArray { .. }
            | checked_trees::CheckedStructuralValueKind::Scalar { .. }
            | checked_trees::CheckedStructuralValueKind::ZeroedStructural { .. }
            | checked_trees::CheckedStructuralValueKind::ByteLiteralView { .. }
            | checked_trees::CheckedStructuralValueKind::Case(_) => {}
        }
    }
    Some(())
}
