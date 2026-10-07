//! Fail-closed rejoin of the composed carrier to checked flow and contracts.
//!
//! A state's calls are admitted by the same call admission an ordinary
//! body's are (`attached_unit::admission::calls`), over a `CallerView` of
//! that state; `admit_calls` only runs it per state and names which other
//! operations a state may hold. The graph itself (`state_graph::admission`)
//! decides what else the route admits.
use super::super::admission::{CallerView, admit_call};
use super::super::{CheckedBoundaryMachinePlan, CheckedUnitEffectOperationPlan, unsupported};
use super::{CheckedTrees, LoweringError};
use crate::unit::attached_unit::bodies::UnitPlans;

pub(super) fn exact_attachment<'a>(
    checked: &'a CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<Option<&'a checked_trees::CheckedUnitStructuralTypePlan>, LoweringError> {
    let mut machines = checked
        .machines()
        .iter()
        .filter(|machine| machine.symbol == plan.machine);
    machines.next().ok_or(LoweringError::Unsupported(
        "composed Unit attachment has no authored machine",
    ))?;
    if machines.next().is_some() {
        return unsupported("composed Unit attachment has duplicate authored machines");
    }
    if plan.attachment_type_identity.is_some() || !plan.provider_attachment_requirements.is_empty()
    {
        return unsupported("free composed Unit fabricated an attachment or provider field");
    }
    Ok(None)
}

/// Admit every call the given states make through the one call admission an
/// ordinary body's calls take (`admission::calls`), each against its own
/// state's operations, parameters and entry claims, and return the boundaries
/// those calls name. The operations a state may hold besides calls are the
/// graph's to decide; any other operation is refused here.
pub(super) fn admit_calls<'a>(
    checked: &'a CheckedTrees,
    call_states: &[&'a checked_trees::CheckedComposedUnitControlStatePlan],
) -> Result<Vec<(&'a CheckedBoundaryMachinePlan, String)>, LoweringError> {
    let plans = UnitPlans::published(&checked.facts.flow.terminal_unit_effects);
    let mut boundaries = Vec::new();
    for state in call_states.iter().copied() {
        // A fused roster's call sites record their facts under the machine
        // that authored the state, not the composed plan's dispatch machine.
        let caller = CallerView::state(
            crate::declarations::state_declaration(checked, state.state)?
                .0
                .symbol,
            state,
        );
        let arm_sequences = state
            .terminator
            .return_arms()
            .into_iter()
            .map(|arm| arm.operation_dependencies().collect::<Vec<_>>())
            .filter(|sequence| !sequence.is_empty())
            .collect::<Vec<_>>();
        let edge_sequences = state
            .successors()
            .into_iter()
            .flat_map(|successor| &successor.transfers)
            .filter_map(|transfer| match &transfer.source {
                checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                    operands,
                    establishment,
                } => Some(
                    operands
                        .iter()
                        .chain(std::iter::once(establishment.as_ref()))
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            })
            .collect::<Vec<_>>();
        for operation in state.operation_dependencies() {
            // A terminal call arm runs after the state's whole operation
            // sequence, on its selected arm alone: that sequence followed by
            // the call is the one its producers and cleanup rejoin against.
            // A composite argument established on an edge runs at the same
            // point, on its selected edge alone, and its member calls may
            // read or take the state's own results through that sequence.
            let caller_sequence;
            let selected_sequence = edge_sequences
                .iter()
                .chain(&arm_sequences)
                .find(|sequence| {
                    sequence
                        .iter()
                        .any(|selected| std::ptr::eq(*selected, operation))
                });
            let caller = if let Some(selected_sequence) = selected_sequence {
                caller_sequence = state
                    .operations
                    .iter()
                    .chain(selected_sequence.iter().copied())
                    .cloned()
                    .collect::<Vec<_>>();
                CallerView {
                    operations: &caller_sequence,
                    ..caller
                }
            } else if operation.call().is_some()
                && !state
                    .operations
                    .iter()
                    .any(|body| std::ptr::eq(body, operation))
            {
                // Legacy guarded structural returns retain their call directly.
                caller_sequence = state
                    .operations
                    .iter()
                    .chain(std::iter::once(operation))
                    .cloned()
                    .collect::<Vec<_>>();
                CallerView {
                    operations: &caller_sequence,
                    ..caller
                }
            } else {
                caller
            };
            admit_call(checked, plans, &caller, operation, &mut boundaries)?;
            match operation {
                _ if operation.call().is_some() => {}
                CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                | CheckedUnitEffectOperationPlan::ByteSequenceStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                | CheckedUnitEffectOperationPlan::ElementViewWrite(_)
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                | CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(_)
                | CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(_)
                | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                // A representation view store writes through a loaned view;
                // it names no callee.
                | CheckedUnitEffectOperationPlan::RepresentationViewStore(_)
                // An atomic event names no callee.
                | CheckedUnitEffectOperationPlan::AtomicAccess(_)
                // The paired call carries the callee dependency; cleanup only
                // disposes its discarded result.
                | CheckedUnitEffectOperationPlan::ContinuationCleanup { .. }
                // A displaced field's move-out and restoring store carry no
                // callee; the replacing call carries its own dependency.
                | CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
                | CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
                | CheckedUnitEffectOperationPlan::RebindStructuralLocal { .. }
                // A view-subslice local narrows a view the state holds.
                | CheckedUnitEffectOperationPlan::EstablishByteSequence { .. }
                | CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
                // Frame-local storage minted at an authored statement names no
                // callee; borrowed references bind storage already owned here.
                | CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
                | CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                // A declared structural local's establishment mints
                // frame-local storage at its statement the same way.
                | CheckedUnitEffectOperationPlan::EstablishStructuralLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishReference { .. }
                | CheckedUnitEffectOperationPlan::ReleaseReference { .. } => {}
                _ => return unsupported("composed Unit call state contains a non-call operation"),
            }
        }
    }
    boundaries.sort_by(|left, right| left.1.cmp(&right.1));
    if boundaries.windows(2).any(|pair| pair[0].1 == pair[1].1) {
        return unsupported("composed Unit boundaries have duplicate canonical identities");
    }
    Ok(boundaries)
}

pub(super) fn validate_contract(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<(), LoweringError> {
    if checked
        .facts
        .contract_plans
        .for_machine(plan.machine)
        .is_none()
    {
        return unsupported("composed Unit control is missing its canonical checked contract");
    }
    Ok(())
}
