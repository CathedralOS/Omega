//! Join source-selected anonymous owner deaths to completed holder lifetimes.
//! A materialized descriptor and its owned backing are separate occurrences.
//! The retained value graph identifies backing; the existing retirement query
//! closes every actual holder and transport before attaching that disposition.

use super::{Results, Retirement};
use checked_trees::{
    CheckFacts, CheckedReferenceOwnerCleanup, CheckedUnitEffectOperationPlan,
    CheckedUnitLocalBindingPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralResultBindingPlan,
};
use symbols::SymbolHandle;

struct OwnerCandidate {
    disposition: arena::Handle<checked_trees::ContextualCleanupDisposition>,
    source_call: arena::Handle<checked_trees::FlowCallFact>,
    target_identity: (SymbolHandle, SymbolHandle),
    values: Vec<checked_trees::CheckedStructuralValueHandle>,
}

/// State-local construction scratch, prepared once and borrowed at each expiry.
/// It names issued source/value relationships, never a second liveness ledger.
pub(in crate::execution::terminal_unit) struct OwnerCandidates {
    owners: Vec<OwnerCandidate>,
}

impl OwnerCandidates {
    pub(in crate::execution::terminal_unit) fn new(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Option<Self> {
        let values = &facts.values.structural_values;
        let flow = facts.flow.control.state(machine, state)?;
        let calls = facts.flow.control.calls.span(flow.calls)?;
        let roots = values
            .roots
            .iter()
            .map(|(_, root)| root)
            .filter(|root| {
                root.machine == machine
                    && root.state == state
                    && validation::reference_result_custody::stored_loan_parts(
                        program,
                        root.type_reference,
                    )
                    .is_some()
            })
            .collect::<Vec<_>>();
        let mut owners: Vec<OwnerCandidate> = Vec::new();
        for value_root in roots.into_iter().rev() {
            for handle in
                crate::execution::terminal_unit::cleanup::retained_owners::dependency_dispositions(
                    facts,
                    machine,
                    state,
                    value_root.root,
                )?
            {
                let row = facts.contextual_cleanup.dispositions.get(handle);
                let language_semantics::PermissionEventSource::Call {
                    statement_index,
                    call_ordinal,
                    target_symbol,
                } = row.source
                else {
                    return None;
                };
                let mut origins = calls.iter().enumerate().filter(|(_, call)| {
                    call.statement_index == statement_index
                        && call.call_ordinal == call_ordinal
                        && call.target_symbol == target_symbol
                });
                let (ordinal, source) = origins.next()?;
                if origins.next().is_some() {
                    return None;
                }
                // Borrowed backing and owned-projection complements retain
                // different source authority. Consume the permission producer's
                // exact borrowing occurrence; do not reconstruct that distinction
                // from a reference type or a coincident call coordinate.
                if !facts.flow.ownership.permissions.iter().any(|(_, event)| {
                    event.machine_symbol == machine
                        && event.state_symbol == state
                        && event.source == row.source
                        && event.root == row.root
                        && event.kind == language_semantics::PermissionEventKind::Establish
                        && event.access != language_semantics::PermissionAccess::Owned
                }) {
                    continue;
                }
                if !source.argument_operands().handles().any(|operand| {
                    let operand = facts.flow.control.operands.get(operand);
                    operand.state_symbol == state
                        && operand.statement_index == statement_index
                        && operand.expression == value_root.expression
                        && program.normalized_type_identity(operand.type_reference)
                            == program.normalized_type_identity(value_root.type_reference)
                }) {
                    continue;
                }
                let source_call = flow.calls.handle_at(u32::try_from(ordinal).ok()?);
                if let Some(owner) = owners.iter_mut().find(|owner| {
                    let existing = facts.contextual_cleanup.dispositions.get(owner.disposition);
                    existing.root == row.root && existing.source == row.source
                }) {
                    if !owner.values.contains(&value_root.root) {
                        owner.values.push(value_root.root);
                    }
                } else {
                    owners.push(OwnerCandidate {
                        disposition: handle,
                        source_call,
                        target_identity:
                            crate::execution::terminal_unit::calls::resolve_call_target(
                                program,
                                source.target_symbol,
                            )?
                            .identity(),
                        values: vec![value_root.root],
                    });
                }
            }
        }
        Some(Self { owners })
    }
}

struct Owner<'a> {
    disposition: arena::Handle<checked_trees::ContextualCleanupDisposition>,
    row: &'a checked_trees::ContextualCleanupDisposition,
    carriers: Vec<&'a CheckedUnitStructuralResultBindingPlan>,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit::control::statement_sequence) fn prepare_owner_cleanups(
    candidates: &OwnerCandidates,
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    prepared_calls: &crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
    shapes: &mut crate::execution::terminal_unit::types::ShapeCollector<'_>,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    returned: Option<&checked_trees::CheckedUnitStructuralReturnPlan>,
    at_exit: bool,
    results: &Results,
    operations: &[CheckedUnitEffectOperationPlan],
    completed_operations: usize,
    pending: &mut [CheckedUnitEffectOperationPlan],
) -> Option<()> {
    let mut owners = Vec::new();
    for candidate in &candidates.owners {
        let row = facts
            .contextual_cleanup
            .dispositions
            .get(candidate.disposition);
        let source_call = facts.flow.control.calls.get(candidate.source_call);
        let calls = operations
            .iter()
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
            .filter_map(CheckedUnitEffectOperationPlan::call)
            .filter(|call| {
                call.coordinate.statement_index as usize == source_call.statement_index
                    && call.coordinate.call_ordinal as usize == source_call.call_ordinal
                    && (call.target.machine, call.target.state) == candidate.target_identity
                    && call.completion() == semantic_vocabulary::BoundaryCompletion::NormalReturn
            })
            .collect::<Vec<_>>();
        let [completed_call] = calls.as_slice() else {
            continue;
        };
        let mut carriers = operations
            .iter()
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::EstablishLocal(
                    CheckedUnitLocalBindingPlan::StructuralValue { result, value, .. },
                ) if candidate.values.contains(value) => Some(result),
                CheckedUnitEffectOperationPlan::EstablishReference {
                    formation_expression,
                    result,
                    ..
                } if candidate.values.iter().any(|value| {
                    let value = facts.values.structural_values.nodes.get(*value);
                    value.expression == *formation_expression
                        && value.statement_index == result.statement_index
                }) =>
                {
                    Some(result)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        // The checked source occurrence lends through this exact materialized
        // descriptor. A receiving call result or a later stored copy is a
        // separate holder and participates through the retirement relation.
        carriers.retain(|carrier| {
            completed_call.structural_arguments.iter().any(|argument| {
                argument.access.borrow_access_kind().is_some()
                    && argument.source_structural_result_binding_ordinal()
                        == Some(carrier.binding_ordinal)
                    && argument.path.first()
                        == Some(&checked_trees::CheckedUnitStructuralPathSegment::Referent)
            })
        });
        // A release-owned disposition requires a real materialized holder.
        // Invocation-only lifetimes belong to the existing exact call
        // continuation; an incidental later release cannot supply that join.
        if carriers.is_empty() {
            continue;
        }
        owners.push(Owner {
            disposition: candidate.disposition,
            row,
            carriers,
        });
    }
    if owners.is_empty() {
        return Some(());
    }
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let transported_loans =
        super::transport::collect(program, facts, machine, state, results, operations)?;
    for index in 0..pending.len() {
        if !matches!(
            pending[index],
            CheckedUnitEffectOperationPlan::ReleaseReference { .. }
        ) {
            continue;
        }
        let mut completed = Vec::new();
        for owner in &owners {
            // A disposition is selected once at its actual physical boundary.
            // Other selected branches keep their distinct source dispositions.
            if operations.iter().chain(&pending[..=index]).any(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::ReleaseReference { owner_cleanups, .. }
                    if shapes.cleanup_control.reference_owner_cleanups.span_or_empty(*owner_cleanups)
                        .iter().any(|selected| selected.disposition == owner.disposition))
            }) {
                continue;
            }
            let retirement = Retirement {
                program,
                facts,
                prepared_calls,
                machine,
                state,
                order: &order,
                statement_index,
                results,
                operations,
                completed_operations,
                pending: &pending[..=index],
                at_exit,
                uncommitted_owner: owner.row.root,
                returned,
                transported_loans: &transported_loans,
            };
            let loans = validation::reference_result_custody::storage_loans(
                facts,
                machine,
                state.symbol,
                owner.row.root,
            )?;
            let closed = |retirement: &Retirement<'_>| {
                owner.carriers.iter().all(|carrier| {
                    retirement.carrier_retired(carrier, &mut Vec::new())
                        && retirement.carrier_children_retired(carrier)
                }) && loans.iter().all(|loan| {
                    !retirement.return_transfers_holder(facts.borrow.loans.get(*loan))
                        && retirement.holder_retired(*loan)
                        && retirement.children_retired(*loan)
                })
            };
            if !closed(&retirement) {
                continue;
            }
            // Completion must be caused by this real release prefix. A cohort
            // already consumed by a call cannot borrow an incidental later
            // descriptor release as its cleanup boundary.
            let before = Retirement {
                pending: &pending[..index],
                ..retirement
            };
            if closed(&before) {
                continue;
            }
            let facts::PlaceRoot::Expression(expression) = owner.row.root else {
                return None;
            };
            completed.push(CheckedReferenceOwnerCleanup {
                disposition: owner.disposition,
                owner: CheckedUnitStructuralArgumentPlan {
                    projection_evaluations: Default::default(),
                    source: CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        expression,
                    },
                    path: Vec::new(),
                    type_identity: shapes.add_type(owner.row.disposed_type, &[], &[])?,
                    access: checked_trees::CheckedStructuralAccess::Owned,
                },
                actions: arena::HandleSpan::empty(),
            });
        }
        if let CheckedUnitEffectOperationPlan::ReleaseReference { owner_cleanups, .. } =
            &mut pending[index]
        {
            *owner_cleanups = shapes
                .cleanup_control
                .reference_owner_cleanups
                .insert_many(completed);
        }
    }
    Some(())
}
