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
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan, CheckedScalarExpressionRole,
    CheckedUnitEffectOperationPlan, Multiplicity, checked_terminal_machine_name, unsupported,
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
    let true_ordinal =
        plan.coordinate
            .statement_index
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic continuation coordinate overflowed",
            ))?;
    let false_ordinal = true_ordinal
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "direct dynamic continuation coordinate overflowed",
        ))?;
    if checked.facts.values.scalar_expressions.expression_at(
        plan.caller_state,
        true_ordinal,
        CheckedScalarExpressionRole::Guard,
    ) != Some(&continuation.guard)
    {
        return unsupported("direct dynamic continuation guard drifted from checked scalar facts");
    }
    if continuation.when_true.statement_ordinal != true_ordinal
        || continuation.when_false.statement_ordinal != false_ordinal
        || continuation.when_true.target_state != when_true.state
        || continuation.when_false.target_state != when_false.state
        || plan.caller_state == when_true.state
        || plan.caller_state == when_false.state
        || when_true.state == when_false.state
        || !continuation.when_true.transfers.is_empty()
        || !continuation.when_false.transfers.is_empty()
        || !continuation.when_true.scalar_arguments.is_empty()
        || !continuation.when_false.scalar_arguments.is_empty()
        || !continuation
            .when_true
            .trivial_affine_discard_parameter_positions
            .is_empty()
        || !continuation
            .when_false
            .trivial_affine_discard_parameter_positions
            .is_empty()
    {
        return unsupported("direct dynamic continuation drifted from its checked control graph");
    }
    for edge in [&continuation.when_true, &continuation.when_false] {
        if let Some(local) = continuation.trivial_affine_local_discard {
            let Some(stored) = stored else {
                return unsupported(
                    "direct dynamic continuation fabricated a stored local discard",
                );
            };
            if stored.storage.destination_binding != local
                || checked
                    .facts
                    .flow
                    .terminal_structural_control_cleanups
                    .for_edge(
                        plan.caller_machine,
                        plan.caller_state,
                        edge.statement_ordinal,
                    )
                    .is_some()
                || !exact_stored_local_drop(checked, plan, &stored.storage)
            {
                return unsupported(
                    "stored dynamic continuation local cleanup drifted after checking",
                );
            }
        } else {
            let cleanup = checked
                .facts
                .flow
                .terminal_structural_control_cleanups
                .for_edge(
                    plan.caller_machine,
                    plan.caller_state,
                    edge.statement_ordinal,
                )
                .ok_or(LoweringError::Unsupported(
                    "direct dynamic continuation lost its checked cleanup edge",
                ))?;
            if cleanup.target_state != edge.target_state
                || !cleanup
                    .trivial_affine_discard_parameter_positions
                    .is_empty()
            {
                return unsupported(
                    "direct dynamic continuation cleanup edge drifted after checking",
                );
            }
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

fn exact_stored_local_drop(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedDynamicScalarCallPlan,
    storage: &checked_trees::DynamicDescriptorStorageFact,
) -> bool {
    // Descriptor storage establishes an affine local even though its borrowed
    // payload carries no linear debt and needs no executable destructor. Rejoin
    // its no-code disposal to that exact establishment, not the old untracked
    // Unknown provenance. Any intervening move, replacement, projected claim,
    // or additional dying root needs its own cleanup plan rather than this pair.
    let root = facts::PlaceRoot::Symbol(storage.destination_binding);
    let source = language_semantics::PermissionEventSource::Statement {
        statement_index: storage.statement_index,
    };
    let provenance = language_semantics::PermissionProvenance::Established {
        machine_symbol: plan.caller_machine,
        state_symbol: plan.caller_state,
        source,
    };
    let mut events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == plan.caller_machine
                && event.state_symbol == plan.caller_state
                && (event.root == root
                    || (event.source == language_semantics::PermissionEventSource::StateExit
                        && event.kind == language_semantics::PermissionEventKind::AffineDrop))
        })
        .map(|(_, event)| event);
    let (Some(establishment), Some(drop), None) = (events.next(), events.next(), events.next())
    else {
        return false;
    };
    establishment.kind == language_semantics::PermissionEventKind::Establish
        && establishment.source == source
        && drop.kind == language_semantics::PermissionEventKind::AffineDrop
        && drop.source == language_semantics::PermissionEventSource::StateExit
        && [establishment, drop].into_iter().all(|event| {
            event.root == root
                && event.access == language_semantics::PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Affine
                && event.claim_identity == language_semantics::PermissionClaimIdentity::Unknown
                && event.provenance == provenance
                && !event.obligation_live
                && event.segments.is_empty()
        })
}

pub(super) fn exact_attachment<'a>(
    checked: &'a CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<Option<&'a checked_trees::CheckedUnitStructuralTypePlan>, LoweringError> {
    let mut machines = checked
        .machines()
        .iter()
        .filter(|machine| machine.symbol == plan.machine);
    let machine = machines.next().ok_or(LoweringError::Unsupported(
        "composed Unit attachment has no authored machine",
    ))?;
    if machines.next().is_some() {
        return unsupported("composed Unit attachment has duplicate authored machines");
    }
    if machine.attached_data.is_none() {
        if plan.attachment_type_identity.is_some()
            || !plan.provider_attachment_requirements.is_empty()
        {
            return unsupported("free composed Unit fabricated an attachment or provider field");
        }
        return Ok(None);
    }
    let retained_identity =
        plan.attachment_type_identity
            .as_deref()
            .ok_or(LoweringError::Unsupported(
                "attached composed Unit omitted its authored attachment",
            ))?;
    let program = &checked.typed;
    let mut attachments = program
        .data_definitions()
        .iter()
        .filter(|data| machine.attached_data.as_ref() == Some(&data.name));
    let attachment = attachments.next().ok_or(LoweringError::Unsupported(
        "composed Unit has no authored attachment",
    ))?;
    if attachments.next().is_some() || !program.data_type_parameters(attachment).is_empty() {
        return unsupported("composed Unit attachment is ambiguous or generic");
    }
    // Reproduce the checked shape identity from its resolved declaration, not
    // from the retained plan's unverified spelling or an unrelated record.
    let mut identity = String::from("named(name(");
    for character in program
        .symbols
        .display_path(attachment.symbol, "::")
        .chars()
    {
        if matches!(character, '\\' | '(' | ')' | ',') {
            identity.push('\\');
        }
        identity.push(character);
    }
    identity.push_str("))");
    if identity != retained_identity {
        return unsupported("composed Unit attachment disagrees with its authored owner");
    }
    // A namespaced constructor can have an exact nominal owner without any
    // runtime receiver storage. Only actual receiver/provider use requires a
    // record attachment; never manufacture storage for a sum's namespace.
    let requires_record_storage = !plan.provider_attachment_requirements.is_empty()
        || program.machine_states(machine).iter().any(|state| {
            program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.is_self)
        })
        || plan.states.iter().any(|state| {
            state
                .structural_parameters
                .iter()
                .any(|parameter| parameter.is_self)
        });
    exact_attachment_identity(checked, retained_identity, requires_record_storage).map(Some)
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
    let boundaries = admit_calls(checked, machine, call_states)?;
    for (boundary, _) in &boundaries {
        validate_claim_free_boundary(boundary)?;
    }
    let internal_targets = unit_targets(checked, machine, call_states)?;
    // Only signature-directed boundary calls are provider obligations; a
    // boundary-declaration call (`target_machine != target_state`) settles
    // through the called machine's own boundary seam.
    let called_boundaries = call_states
        .iter()
        .flat_map(|state| &state.operations)
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall {
                target_machine,
                target_state,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                target_machine,
                target_state,
                ..
            } => (target_machine == target_state).then_some(*target_machine),
            _ => None,
        })
        .collect::<Vec<_>>();
    if let Some(attachment) = attachment {
        for state in call_states {
            for operation in &state.operations {
                super::super::provider_attachments::validate_call_source(
                    checked,
                    machine,
                    state.state,
                    operation,
                    provider_attachment_requirements,
                )?;
            }
        }
        super::super::provider_attachments::validate_provider_attachment_requirements(
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
    machine: symbols::SymbolHandle,
    call_states: &[&'a checked_trees::CheckedComposedUnitControlStatePlan],
) -> Result<Vec<(&'a CheckedBoundaryMachinePlan, String)>, LoweringError> {
    let plans = UnitPlans::published(&checked.facts.flow.terminal_unit_effects);
    let mut boundaries = Vec::new();
    for state in call_states.iter().copied() {
        let caller = CallerView::state(machine, state);
        for operation in state.operation_dependencies() {
            admit_call(checked, plans, &caller, operation, &mut boundaries)?;
            match operation {
                CheckedUnitEffectOperationPlan::BoundaryCall { .. }
                | CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
                | CheckedUnitEffectOperationPlan::CallUnit { .. }
                | CheckedUnitEffectOperationPlan::StructuralCall { .. }
                | CheckedUnitEffectOperationPlan::ScalarCall { .. }
                | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                // The paired call carries the callee dependency; cleanup only
                // disposes its discarded result.
                | CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
                // A displaced field's move-out and restoring store carry no
                // callee; the replacing call carries its own dependency.
                | CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
                | CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
                // A view-subslice local narrows a view the state holds.
                | CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. } => {}
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
        let (CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. }
        | CheckedUnitEffectOperationPlan::StructuralCall { target_machine, .. }) = operation
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
    let contract = checked
        .facts
        .contract_plans
        .for_machine(plan.machine)
        .ok_or(LoweringError::Unsupported(
            "composed Unit control is missing its canonical checked contract",
        ))?;
    if plan.contract_report_fingerprint == 0
        || plan.contract_report_fingerprint != contract.report_fingerprint
        || plan.contract_commitment != contract.commitment
    {
        return unsupported("composed Unit contract identity drifted after checking");
    }
    Ok(())
}

pub(super) fn validate_leaf(
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
) -> Result<(), LoweringError> {
    if !state.scalar_parameters.is_empty()
        || !state.bindings.is_empty()
        || !state.binding_initializers.is_empty()
        || !matches!(
            state.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::BoundaryCall { .. }
                | CheckedUnitEffectOperationPlan::CallUnit { .. }]
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
