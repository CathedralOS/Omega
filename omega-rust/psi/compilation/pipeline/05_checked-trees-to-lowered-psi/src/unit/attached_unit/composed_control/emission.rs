//! Operation emission for one composed-graph state or call leaf.
//!
//! `emit_call_operations` walks a state's checked operations in order and
//! emits every one through `attached_unit::operation_frame::OperationFrame`,
//! the per-operation emitter the ordinary machine uses. This file owns only
//! what is composed-specific: the frame over the state's environment (its
//! registry-backed result namespace, the machine-wide claim table, the
//! shared temporary roster and the primitive locals the sequence establishes)
//! and consumption of the same authored-position operand schedule as ordinary
//! machines. Structural snapshots must stay between earlier and later scalar
//! side effects even when the surrounding body has state transitions.
use super::super::super::{ClaimId, PermissionClaimIdentity, StructuralParameterDeclaration};
use super::super::argument_schedule::{self, Step};
use super::super::bodies::UnitPlans;
use super::super::operation_frame::{
    CallInputs, Callees, CallerCustody, OperationFrame, StructuralResults, StructuralTypeRoster,
};
use super::super::primitive_locals::PrimitiveLocal;
use super::super::{CheckedUnitEffectOperationPlan, ValueDeclaration, unsupported};
use super::{CheckedTrees, LoweringError, catalogs, literal_arguments};
use crate::emission::operation_emission::buffer::OperationBuffer;
use std::borrow::Cow;

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_call_operations(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    planned_operations: &[CheckedUnitEffectOperationPlan],
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    evaluation: &mut super::super::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    // The emitting machine's erased roster, for `ErasedParameter` actuals
    // forwarded through proof-only call operands.
    erased_parameters: &[ValueDeclaration],
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    // Borrowed-storage windows open and close within one straight-line
    // operation sequence. Every exit of the sequence's block — the state's
    // terminator, its successor edges and any return — follows the last
    // operation, so requiring the ledger closed here closes it on every exit.
    // No state-graph join can therefore receive an open window, and the
    // blocks argument evaluation splits off in between rejoin with the one
    // frontier this sequence carries.
    let mut windows = crate::emission::borrowed_window::BorrowedWindowLedger::default();
    let plans = UnitPlans::published(&checked.facts.flow.terminal_unit_effects);
    // The mutable primitive locals this sequence establishes. Their places
    // live in the shared private roster; this roster keys later stores to
    // them by authored symbol, as the ordinary machine's roster does.
    let mut primitive_locals = Vec::<PrimitiveLocal>::new();
    // A returned final expression evaluates in the `Return` role.
    let scalar_result = match &state.terminator {
        checked_trees::CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
            completion: checked_trees::CheckedScalarReturnPlan::Binding(binding),
        } => Some(binding),
        _ => None,
    };
    let schedule =
        argument_schedule::build_operations(checked, machine, state.state, planned_operations)?;
    let mut schedule = schedule.iter().peekable();
    let mut retained_source_count = None;
    let mut staged_scalar_slots = vec![Vec::new(); planned_operations.len()];
    let mut staged_ranges = vec![Vec::new(); planned_operations.len()];
    while let Some(step) = schedule.next() {
        let (operation_index, staged) = match *step {
            Step::CaptureBefore(statement) => {
                evaluation.prepare_observations_before(
                    checked,
                    state.state,
                    statement,
                    &catalogs.structural_types,
                    values,
                    &mut catalogs.next_place,
                    next_value,
                    operations,
                    &mut catalogs.temporary_places,
                )?;
                continue;
            }
            Step::Begin => {
                if retained_source_count.replace(values.len()).is_some() {
                    return unsupported("composed argument group was already active");
                }
                continue;
            }
            Step::End => {
                if retained_source_count.take().is_none() {
                    return unsupported("composed argument group was not active");
                }
                continue;
            }
            Step::Argument { operation, ordinal } => {
                let source_value_count = retained_source_count.ok_or(
                    LoweringError::Unsupported("composed scalar operand has no source prefix"),
                )?;
                if staged_scalar_slots[operation].len() != ordinal {
                    return unsupported("composed scalar operand ordinals are not dense");
                }
                let mut calls = catalogs.scalar_calls.emission_context();
                let value = evaluation.argument_at(
                    checked,
                    machine,
                    state.state,
                    &planned_operations[operation],
                    ordinal,
                    source_value_count,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                    &mut calls,
                )?;
                catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
                staged_scalar_slots[operation].push(values.len());
                values.push(value);
                continue;
            }
            Step::Subslice { operation, ordinal } => {
                let source_value_count = retained_source_count.ok_or(
                    LoweringError::Unsupported("composed range operand has no source prefix"),
                )?;
                if staged_ranges[operation]
                    .iter()
                    .any(|(prior, _)| *prior == ordinal)
                {
                    return unsupported("composed range operand is evaluated more than once");
                }
                let place = literal_arguments::stage_range(
                    checked,
                    state,
                    &planned_operations[operation],
                    ordinal,
                    source_value_count,
                    catalogs,
                    evaluation,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                )?;
                staged_ranges[operation].push((ordinal, place));
                continue;
            }
            Step::Call(index) | Step::Constructor(index) if retained_source_count.is_some() => {
                (index, true)
            }
            Step::Ordinary(index) if retained_source_count.is_none() => (index, false),
            _ => return unsupported("composed call schedule disagrees with its argument group"),
        };
        let operation = &planned_operations[operation_index];
        if matches!(
            operation,
            CheckedUnitEffectOperationPlan::ReleaseReference {
                defer_to_exit: true,
                ..
            }
        ) {
            continue;
        }
        let source_value_count = retained_source_count.unwrap_or(values.len());
        // Keep private argument values across constructor evaluation and any
        // split blocks. Read their rebound identities only at the consuming
        // call, never from the values they held before a later operand ran.
        let staged_scalars = staged_scalar_slots[operation_index]
            .iter()
            .map(|slot| {
                values.get(*slot).copied().ok_or(LoweringError::Unsupported(
                    "composed scalar operand lost its retained value",
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let establishes_local = matches!(
            operation,
            CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
        );
        let call_operands = if establishes_local || OperationFrame::lowers(operation) {
            None
        } else {
            Some(literal_arguments::evaluate(
                checked,
                machine,
                state,
                operation,
                staged.then_some(literal_arguments::StagedArguments {
                    scalars: &staged_scalars,
                    ranges: &staged_ranges[operation_index],
                }),
                catalogs,
                evaluation,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
            )?)
        };
        // The final call publishes into the state's ordinary scalar binding
        // namespace. Retire argument temporaries first, after copying its
        // completed operands, so the result receives its actual source slot.
        if matches!(step, Step::Call(_)) && matches!(schedule.peek(), Some(Step::End)) {
            values.truncate(source_value_count);
        }
        let mut calls = catalogs.scalar_calls.emission_context();
        calls.claims = crate::emission::claim_bindings::ClaimBindings::Fixed(claim_bindings);
        let frame = OperationFrame {
            checked,
            machine,
            state: state.state,
            scalar_result,
            scalar_parameter_count: state.scalar_parameters.len(),
            source_value_count,
            parameters,
            structural_types: match &mut catalogs.structural_types {
                Cow::Owned(types) => StructuralTypeRoster::Owned(types),
                Cow::Borrowed(types) => StructuralTypeRoster::Published(types),
            },
            type_ids: &catalogs.type_ids,
            service_ids: &catalogs.service_ids,
            primitive_locals: &primitive_locals,
            results: StructuralResults::StateGraph {
                state,
                places: &mut catalogs.result_places,
            },
            // A composed body keeps literals, construction and join places
            // in its one private temporary roster.
            private_places: &mut catalogs.temporary_places,
            windows: &mut windows,
            evaluation,
            values,
            next_place: &mut catalogs.next_place,
            next_value,
            next_block,
            next_edge,
            calls: &mut calls,
            operations,
            callees: Callees {
                plans,
                signatures: &catalogs.signatures,
                boundaries: &catalogs.boundary_parameters,
                domain_ids: &catalogs.domain_ids,
                closure: catalogs.closure,
                prepared_scalar_machines: catalogs.prepared_scalar_machines,
            },
            caller: CallerCustody {
                erased_scalar_parameters: erased_parameters,
                erased_proof_parameters: &state.erased_proof_parameters,
                local_places: &[],
            },
        };
        match &call_operands {
            None if establishes_local => {
                let local = frame.establish_primitive_local(operation)?;
                catalogs.temporary_places.push(local.declaration);
                primitive_locals.push(local);
            }
            None => frame.emit(operation)?,
            Some((arguments, byte_places)) => {
                if frame
                    .emit_call(
                        operation,
                        CallInputs {
                            evaluated_scalar_arguments: arguments.as_deref(),
                            byte_places,
                            staged: false,
                        },
                    )?
                    .is_some()
                {
                    return unsupported("composed call staged a private scalar result");
                }
            }
        }
        catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
    }
    windows.require_closed()
}
