//! Fail-closed rejoin of the composed carrier to checked flow and contracts.

use super::*;
use crate::attached_unit::bodies::UnitBody;

pub(in crate::attached_unit) struct AdmittedComposedUnit<'a> {
    pub(in crate::attached_unit) entry: &'a checked_trees::CheckedComposedUnitControlStatePlan,
    pub(in crate::attached_unit) leaves:
        Vec<&'a checked_trees::CheckedComposedUnitControlStatePlan>,
    pub(in crate::attached_unit) boundaries: Vec<(&'a CheckedBoundaryMachinePlan, String)>,
    pub(in crate::attached_unit) internal_targets: Vec<(UnitBody<'a>, String)>,
    pub(super) custody: custody::ComposedCustody,
}

pub(in crate::attached_unit) fn admit_composed_unit_control<'a>(
    checked: &'a CheckedTrees,
    plan: &'a checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<AdmittedComposedUnit<'a>, LoweringError> {
    let [entry, when_true, when_false] = plan.states.as_slice() else {
        return unsupported("composed Unit control requires exactly three states");
    };
    if !plan.body_qualifications.is_empty() || !entry.operations.is_empty() {
        return unsupported("composed Unit entry is outside the exact scalar-control slice");
    }
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        guard,
        when_true: true_edge,
        when_false: false_edge,
    } = &entry.terminator
    else {
        return unsupported("composed Unit entry is not the exact Boolean conditional");
    };
    let transition_ordinal = validate_bindings(checked, entry)?;
    validate_guard(guard, &entry.scalar_parameters, &entry.bindings)?;
    if checked.facts.values.scalar_expressions.expression_at(
        entry.state,
        transition_ordinal,
        CheckedScalarExpressionRole::Guard,
    ) != Some(guard)
    {
        return unsupported("composed Unit guard drifted from checked scalar facts");
    }
    if true_edge.statement_ordinal != transition_ordinal
        || false_edge.statement_ordinal
            != transition_ordinal
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "composed Unit transition coordinate overflowed",
                ))?
        || true_edge.target_state != when_true.state
        || false_edge.target_state != when_false.state
    {
        return unsupported("composed Unit successors drifted from the checked state graph");
    }
    validate_leaf(when_true)?;
    validate_leaf(when_false)?;
    let custody = custody::admit(
        checked,
        plan,
        entry,
        [when_true, when_false],
        [true_edge, false_edge],
    )?;
    if entry.state == when_true.state
        || entry.state == when_false.state
        || when_true.state == when_false.state
    {
        return unsupported("composed Unit control contains duplicate states");
    }
    validate_contract(checked, plan)?;

    let attachment = exact_attachment(checked, plan)?;

    let (boundaries, internal_targets) = admit_call_targets(
        checked,
        plan.machine,
        &[when_true, when_false],
        custody,
        attachment,
        &plan.provider_attachment_requirements,
    )?;
    Ok(AdmittedComposedUnit {
        entry,
        leaves: vec![when_true, when_false],
        boundaries,
        internal_targets,
        custody,
    })
}

fn validate_bindings(
    checked: &CheckedTrees,
    entry: &checked_trees::CheckedComposedUnitControlStatePlan,
) -> Result<u32, LoweringError> {
    match (
        entry.bindings.as_slice(),
        entry.binding_initializers.as_slice(),
    ) {
        ([], []) => Ok(0),
        ([binding], [retained_initializer])
            if entry.scalar_parameters.is_empty()
                && binding.statement_ordinal == 0
                && binding.primitive_type == PrimitiveType::U64
                && binding.destination
                    == checked_trees::CheckedScalarBindingDestination::Immutable
                && binding.value == CheckedScalarBindingValue::Expression =>
        {
            let fact_initializer = checked
                .facts
                .values
                .scalar_expressions
                .expression_at(
                    entry.state,
                    0,
                    CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
                )
                .ok_or(LoweringError::Unsupported(
                    "composed Unit local initializer lost its checked scalar fact",
                ))?;
            if retained_initializer != fact_initializer {
                return unsupported(
                    "composed Unit local initializer drifted between checked carriers",
                );
            }
            let initializer = lower_checked_scalar_expression(retained_initializer)?;
            if !matches!(
                initializer,
                LoweredDirectExpression::IntegerLiteral {
                    scalar_type: ScalarType::Integer(integer),
                    value: IntegerValue::Unsigned(_),
                } if integer == semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                ).expect("u64 is valid")
            ) {
                return unsupported(
                    "composed Unit local initializer escaped the exact closed u64 lane",
                );
            }
            Ok(1)
        }
        _ => unsupported("composed Unit local bindings escaped the exact one-binding lane"),
    }
}

pub(crate) fn admit_dynamic_continuation<'a>(
    checked: &'a CheckedTrees,
    plan: &checked_trees::CheckedDynamicScalarCallPlan,
    continuation: &'a checked_trees::CheckedDynamicUnitContinuationPlan,
    stored: Option<&checked_trees::CheckedStoredDynamicScalarCallPlan>,
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
        custody::ComposedCustody::Empty,
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
    custody: custody::ComposedCustody,
    attachment: Option<&checked_trees::CheckedUnitStructuralTypePlan>,
    provider_attachment_requirements: &[checked_trees::CheckedProviderAttachmentRequirementPlan],
) -> Result<
    (
        Vec<(&'a CheckedBoundaryMachinePlan, String)>,
        Vec<(UnitBody<'a>, String)>,
    ),
    LoweringError,
> {
    let (boundaries, internal_targets) = retain_call_targets(checked, machine, call_states)?;
    for (boundary, _) in &boundaries {
        custody::validate_boundary(custody, boundary)?;
    }
    let called_boundaries = call_states
        .iter()
        .flat_map(|state| &state.operations)
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { target_machine, .. } => {
                Some(*target_machine)
            }
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

pub(super) fn retain_call_targets<'a>(
    checked: &'a CheckedTrees,
    machine: symbols::SymbolHandle,
    call_states: &[&'a checked_trees::CheckedComposedUnitControlStatePlan],
) -> Result<
    (
        Vec<(&'a CheckedBoundaryMachinePlan, String)>,
        Vec<(UnitBody<'a>, String)>,
    ),
    LoweringError,
> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut boundaries = Vec::new();
    let mut internal_targets = Vec::new();
    for state in call_states.iter().copied() {
        for operation in state
            .operation_dependencies()
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        {
            match operation {
                CheckedUnitEffectOperationPlan::BoundaryCall { .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. } => {
                    retain_call_boundary(
                        checked,
                        machine,
                        state,
                        operation,
                        plans,
                        &mut boundaries,
                    )?;
                }
                CheckedUnitEffectOperationPlan::CallUnit { .. }
                | CheckedUnitEffectOperationPlan::StructuralCall { .. } => {
                    internal_calls::admission::retain_call_target(
                        checked,
                        machine,
                        state,
                        operation,
                        plans,
                        &mut internal_targets,
                    )?;
                }
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_) => {}
                _ => return unsupported("composed Unit call state contains a non-call operation"),
            }
        }
    }
    boundaries.sort_by(|left, right| left.1.cmp(&right.1));
    if boundaries.windows(2).any(|pair| pair[0].1 == pair[1].1) {
        return unsupported("composed Unit boundaries have duplicate canonical identities");
    }
    internal_targets.sort_by(|left, right| left.1.cmp(&right.1));
    for pair in internal_targets.windows(2) {
        if pair[0].1 == pair[1].1 && pair[0].0.entry()?.machine != pair[1].0.entry()?.machine {
            return unsupported(
                "composed Unit internal targets have duplicate canonical identities",
            );
        }
    }
    Ok((boundaries, internal_targets))
}

pub(super) fn validate_guard(
    guard: &CheckedScalarExpression,
    parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
    bindings: &[checked_trees::CheckedScalarBinding],
) -> Result<(), LoweringError> {
    let CheckedScalarExpression::Boolean(boolean) = guard else {
        return unsupported("composed Unit guard is not Boolean");
    };
    let admitted = match (parameters, bindings, boolean.as_ref()) {
        ([parameter], [], CheckedBooleanExpression::Parameter { position: 0 }) => {
            parameter.source_position == 0 && parameter.primitive_type == PrimitiveType::Bool
        }
        ([], [], CheckedBooleanExpression::Constant(_)) => true,
        ([], [], CheckedBooleanExpression::IntegerComparison { left, right, .. }) => {
            matches!(
                left.as_ref(),
                CheckedScalarExpression::IntegerLiteral { .. }
            ) && matches!(
                right.as_ref(),
                CheckedScalarExpression::IntegerLiteral { .. }
            )
        }
        ([], [binding], CheckedBooleanExpression::IntegerComparison { left, right, .. })
            if binding.statement_ordinal == 0
                && binding.primitive_type == PrimitiveType::U64
                && binding.destination
                    == checked_trees::CheckedScalarBindingDestination::Immutable
                && binding.value == CheckedScalarBindingValue::Expression =>
        {
            local_and_literal(left, right) || local_and_literal(right, left)
        }
        _ => false,
    };
    if !admitted {
        return unsupported("composed Unit guard escaped the exact admitted expression family");
    }
    Ok(())
}

fn local_and_literal(local: &CheckedScalarExpression, literal: &CheckedScalarExpression) -> bool {
    matches!(
        local,
        CheckedScalarExpression::Local {
            position: 0,
            primitive_type: PrimitiveType::U64,
        }
    ) && matches!(literal, CheckedScalarExpression::IntegerLiteral { .. })
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

pub(super) fn retain_call_boundary<'a>(
    checked: &'a CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &'a checked_trees::CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
    plans: &'a checked_trees::CheckedUnitEffectPlans,
    boundaries: &mut Vec<(&'a CheckedBoundaryMachinePlan, String)>,
) -> Result<(), LoweringError> {
    crate::call_source_custody::validate_operation(
        checked,
        machine,
        state.state,
        operation,
        &state.structural_parameters,
    )?;
    let (CheckedUnitEffectOperationPlan::BoundaryCall {
        coordinate,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        ..
    }
    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
        coordinate,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        ..
    }) = operation
    else {
        unreachable!("leaf shape was validated")
    };
    retain_exact_flow_call(checked, machine, state.state, *coordinate, *target_state)?;
    let expected_result = match operation {
        CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            result,
            discard_result_on_return,
            structural_arguments,
            completion_receipts,
            ..
        } => {
            let target = unique_unit_boundary(plans, *target_machine)?;
            if (!*discard_result_on_return
                && !matches!(&state.terminator, CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, .. } if subject.type_identity == result.type_identity && subject.source == (checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal: result.binding_ordinal })))
                || result.binding_ordinal as usize != state.operations.iter().filter(|operation| {
                    matches!(operation, CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                        coordinate: previous, ..
                    } if previous.statement_index < coordinate.statement_index)
                }).count()
                || !structural_arguments.is_empty()
                || !completion_receipts.is_empty()
                || result.multiplicity != Multiplicity::Affine
                || !matches!(&target.result, CheckedBoundaryMachineResultPlan::Structural {
                    type_identity, multiplicity: Multiplicity::Affine, qualifications,
                } if type_identity == &result.type_identity && qualifications.is_empty())
            {
                return unsupported("composed Unit local result escaped claim-free affine return custody");
            }
            target.result.clone()
        }
        _ => CheckedBoundaryMachineResultPlan::Unit,
    };
    retain_exact_unit_boundary(
        checked,
        plans,
        boundaries,
        *target_machine,
        *target_state,
        *target_contract_report_fingerprint,
        *service_reach,
        expected_result,
    )
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

pub(super) fn retain_exact_flow_call(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    target: symbols::SymbolHandle,
) -> Result<(), LoweringError> {
    let states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter_map(|(_, candidate)| {
            (candidate.machine_symbol == machine && candidate.state_symbol == state)
                .then_some(candidate)
        })
        .collect::<Vec<_>>();
    let [flow] = states.as_slice() else {
        return unsupported("composed Unit leaf does not rejoin one checked flow state");
    };
    let statement_index = usize::try_from(coordinate.statement_index).map_err(|_| {
        LoweringError::Unsupported("composed Unit statement coordinate exceeds usize")
    })?;
    let call_ordinal = usize::try_from(coordinate.call_ordinal)
        .map_err(|_| LoweringError::Unsupported("composed Unit call coordinate exceeds usize"))?;
    let authored = crate::call_source_custody::authored::locate_source(checked, state, coordinate)?;
    if authored.target_state != target {
        return unsupported("composed Unit call disagrees with its authored resolved target");
    }
    let mut calls = checked
        .facts
        .flow
        .control
        .calls
        .span_or_empty(flow.calls)
        .iter()
        .filter(|call| {
            call.statement_index == statement_index && call.call_ordinal == call_ordinal
        });
    if calls
        .next()
        .is_none_or(|call| call.target_symbol != authored.source_target)
        || calls.next().is_some()
    {
        return unsupported("composed Unit boundary call drifted from checked flow");
    }
    Ok(())
}
