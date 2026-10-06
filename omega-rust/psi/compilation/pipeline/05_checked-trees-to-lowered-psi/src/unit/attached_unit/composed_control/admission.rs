//! Fail-closed rejoin of the composed carrier to checked flow and contracts.
//!
//! A state's calls are admitted by the same call admission an ordinary
//! body's are (`attached_unit::admission::calls`), over a `CallerView` of
//! that state; `admit_calls` only runs it per state and names which other
//! operations a state may hold. The graph itself (`state_graph::admission`)
//! and the direct dynamic continuation below decide what else each route
//! admits: the dynamic continuation's leaves stay exact call-and-return
//! states whose boundaries carry no claims (`validate_leaf`,
//! `validate_claim_free_boundary`).
use super::super::super::{
    CheckedComposedUnitControlTerminatorPlan, CheckedUnitStructuralTypeShape,
};
use super::super::admission::{CallerView, admit_call};
use super::super::{
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan, CheckedUnitEffectOperationPlan,
    Multiplicity, checked_terminal_machine_name, unsupported,
};
use super::{CheckedTrees, LoweringError};
use crate::unit::attached_unit::bodies::{UnitBody, UnitPlans};

pub(crate) fn admit_dynamic_continuation<'a>(
    checked: &'a CheckedTrees,
    plan: &checked_trees::CheckedDynamicScalarCallPlan,
    continuation: &'a checked_trees::CheckedDynamicUnitContinuationPlan,
    stored: Option<&checked_trees::CheckedDynamicStoredDescriptorPlan>,
) -> Result<
    (
        Vec<(&'a CheckedBoundaryMachinePlan, String)>,
        Vec<(UnitBody<'a>, String)>,
    ),
    LoweringError,
> {
    let [when_true, when_false] = continuation.leaves.as_slice() else {
        return unsupported("direct dynamic continuation requires exactly two effect leaves");
    };
    validate_leaf(when_true)?;
    validate_leaf(when_false)?;
    for edge in [&continuation.when_true, &continuation.when_false] {
        if continuation.trivial_affine_local_discard.is_some() {
            if stored.is_none() {
                return unsupported(
                    "direct dynamic continuation fabricated a stored local discard",
                );
            }
        } else if checked
            .facts
            .flow
            .terminal_structural_control_cleanups
            .for_edge(
                plan.caller_machine,
                plan.caller_state,
                edge.statement_ordinal,
            )
            .is_none()
        {
            return unsupported("direct dynamic continuation lost its checked cleanup edge");
        }
    }
    let attachment =
        exact_attachment_identity(checked, &plan.caller_attachment_type_identity, true)?;
    admit_call_targets(
        checked,
        plan.caller_machine,
        &[when_true, when_false],
        Some(attachment),
        &continuation.provider_attachment_requirements,
    )
}

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

fn exact_attachment_identity<'a>(
    checked: &'a CheckedTrees,
    identity: &str,
    requires_record_storage: bool,
) -> Result<&'a checked_trees::CheckedUnitStructuralTypePlan, LoweringError> {
    let attachments = checked
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .iter()
        .filter(|candidate| candidate.identity == identity)
        .collect::<Vec<_>>();
    let [attachment] = attachments.as_slice() else {
        return unsupported("composed Unit attachment type is missing or duplicated");
    };
    if requires_record_storage
        && !matches!(
            attachment.shape,
            CheckedUnitStructuralTypeShape::Record { .. }
        )
    {
        return unsupported("composed Unit attachment is not a record");
    }

    Ok(attachment)
}

pub(super) fn admit_call_targets<'a>(
    checked: &'a CheckedTrees,
    machine: symbols::SymbolHandle,
    call_states: &[&'a checked_trees::CheckedComposedUnitControlStatePlan],
    attachment: Option<&checked_trees::CheckedUnitStructuralTypePlan>,
    provider_attachment_requirements: &[checked_trees::CheckedProviderAttachmentRequirementPlan],
) -> Result<
    (
        Vec<(&'a CheckedBoundaryMachinePlan, String)>,
        Vec<(UnitBody<'a>, String)>,
    ),
    LoweringError,
> {
    let boundaries = admit_calls(checked, call_states)?;
    for (boundary, _) in &boundaries {
        validate_claim_free_boundary(boundary)?;
    }
    let internal_targets = unit_targets(checked, machine, call_states)?;
    if let Some(attachment) = attachment {
        let mut called_boundaries = Vec::new();
        for state in call_states {
            for operation in &state.operations {
                let uses_attachment = super::super::provider_attachments::validate_call_source(
                    checked,
                    machine,
                    state.state,
                    operation,
                    &state.structural_parameters,
                    provider_attachment_requirements,
                )?;
                // Scalar-result calls keep their separate attachment route.
                if uses_attachment
                    && let Some(call) = operation.call()
                    && matches!(
                        call.result(),
                        checked_trees::CheckedUnitCallResult::Unit
                            | checked_trees::CheckedUnitCallResult::Structural { .. }
                    )
                {
                    called_boundaries.push(call.target.machine);
                }
            }
        }
        super::super::provider_attachments::validate_provider_attachment_requirements(
            checked,
            attachment,
            provider_attachment_requirements,
            &called_boundaries,
        )?;
    } else if !provider_attachment_requirements.is_empty() {
        return unsupported("free composed Unit cannot retain provider attachment requirements");
    }
    Ok((boundaries, internal_targets))
}

pub(super) fn validate_claim_free_boundary(
    boundary: &CheckedBoundaryMachinePlan,
) -> Result<(), LoweringError> {
    // Borrowed structural inputs ride the same argument/transfer replay the
    // ordinary path emits; their domain requirements lower to the emitted
    // boundary's `requires` rows and result qualifications mint caller-side
    // establishments, so neither disqualifies a claim-free boundary.
    if boundary.attachment_type_identity.is_some()
        || boundary.structural_parameters.iter().any(|parameter| {
            !parameter.projected_qualifications.is_empty()
                || parameter.fused_service_erasure.is_some()
                || !matches!(
                    parameter.access,
                    checked_trees::CheckedStructuralAccess::SharedBorrow
                        | checked_trees::CheckedStructuralAccess::MutableBorrow
                )
        })
        || !(boundary.result.is_unit()
            || matches!(
                &boundary.result,
                CheckedBoundaryMachineResultPlan::Structural {
                    multiplicity: Multiplicity::Affine | Multiplicity::Unrestricted,
                    ..
                }
            ))
    {
        return unsupported(
            "composed Unit boundary escaped claim-free borrowed-input/result custody",
        );
    }
    Ok(())
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

/// The Unit bodies a standalone composed catalog must lower for these
/// states' calls, member calls included, in canonical identity order.
fn unit_targets<'a>(
    checked: &'a CheckedTrees,
    machine: symbols::SymbolHandle,
    call_states: &[&'a checked_trees::CheckedComposedUnitControlStatePlan],
) -> Result<Vec<(UnitBody<'a>, String)>, LoweringError> {
    let plans = UnitPlans::published(&checked.facts.flow.terminal_unit_effects);
    let mut targets = Vec::<(UnitBody<'a>, String)>::new();
    for operation in call_states
        .iter()
        .flat_map(|state| state.operation_dependencies())
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
    {
        // An ordinary call with a Unit or structural result enters a Unit body.
        let Some(target_machine) = operation
            .call()
            .filter(|call| !call.is_boundary() && call.scalar_result().is_none())
            .map(|call| &call.target.machine)
        else {
            continue;
        };
        // A standalone catalog lowers its callees as a closure of their own;
        // its root is not one of them.
        if *target_machine == machine {
            return unsupported("composed internal Unit call is recursive");
        }
        if targets
            .iter()
            .any(|(target, _)| target.machine() == *target_machine)
        {
            continue;
        }
        targets.push((
            UnitBody::find(plans, *target_machine)?,
            checked_terminal_machine_name(checked, *target_machine)?.to_owned(),
        ));
    }
    targets.sort_by(|left, right| left.1.cmp(&right.1));
    if targets.windows(2).any(|pair| pair[0].1 == pair[1].1) {
        return unsupported("composed Unit internal targets have duplicate canonical identities");
    }
    Ok(targets)
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

pub(super) fn validate_leaf(
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
) -> Result<(), LoweringError> {
    if !state.scalar_parameters.is_empty()
        || !state.bindings.is_empty()
        || !state.binding_initializers.is_empty()
        // One Unit call, ordinary or boundary.
        || !matches!(
            state.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::Call(call)]
                if *call.result() == checked_trees::CheckedUnitCallResult::Unit
        )
        || !matches!(
            state.terminator,
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit
        )
    {
        return unsupported("composed Unit leaf is outside the exact call-and-return slice");
    }
    Ok(())
}
