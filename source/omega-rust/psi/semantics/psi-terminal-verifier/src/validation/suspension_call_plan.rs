use super::*;

pub(super) fn validate_suspension_call_plans(module: &TerminalModule) -> Result<(), ModuleError> {
    if usize::try_from(module.suspension_call_plan_count).ok()
        != Some(module.suspension_call_plans.len())
        || module.suspension_call_sites.len() != module.suspension_call_plans.len()
    {
        return invalid(None, SuspensionCallPlanError::CountMismatch);
    }
    for (site, plan) in module
        .suspension_call_sites
        .iter()
        .zip(&module.suspension_call_plans)
    {
        if site.operation != plan.operation
            || site.crossing != plan.crossing
            || site.target != plan.target
            || site.frontier_commitment != psi_terminal::suspension_frontier_commitment(plan)
        {
            return invalid(Some(plan.operation), SuspensionCallPlanError::SiteMismatch);
        }
    }
    let frontiers = reconstruct_validated_structural_ownership_frontiers(module)?;
    let mut operations = BTreeSet::new();
    let mut crossings = BTreeSet::new();
    let mut previous = None;
    for plan in &module.suspension_call_plans {
        let key = (plan.operation, plan.crossing);
        if previous.is_some_and(|previous| previous >= key) {
            return invalid(Some(plan.operation), SuspensionCallPlanError::NonCanonical);
        }
        previous = Some(key);
        if !operations.insert(plan.operation) {
            return invalid(
                Some(plan.operation),
                SuspensionCallPlanError::DuplicateOperation,
            );
        }
        if !crossings.insert(plan.crossing) {
            return invalid(
                Some(plan.operation),
                SuspensionCallPlanError::DuplicateCrossing,
            );
        }
        let Some((machine, block, operation_index)) = operation_owner(module, plan.operation)
        else {
            return invalid(
                Some(plan.operation),
                SuspensionCallPlanError::UnknownOperation,
            );
        };
        let operation = &block.operations[operation_index];
        if terminal_call_target(&operation.kind) != Some(plan.target) {
            return invalid(
                Some(plan.operation),
                SuspensionCallPlanError::RedirectedToNonCall,
            );
        }
        if usize::try_from(plan.live_value_count).ok() != Some(plan.live_values.len()) {
            return invalid(Some(plan.operation), SuspensionCallPlanError::CountMismatch);
        }
        let expected_policy = plan.live_values.iter().map(|live| live.effective).fold(
            psi_language_semantics::CarryPolicy::PERMISSIVE,
            |combined, policy| combined.intersect(policy),
        );
        if plan.effective != expected_policy
            || plan.effective.suspension != psi_language_semantics::CarrySuspension::Allowed
            || plan.live_values.iter().any(|live| {
                live.effective.suspension != psi_language_semantics::CarrySuspension::Allowed
            })
        {
            return invalid(
                Some(plan.operation),
                SuspensionCallPlanError::UnderstatedPolicy,
            );
        }
        let operation_frontier = frontiers
            .machine(machine.id)
            .and_then(|frontier| frontier.operation_entry(plan.operation))
            .ok_or(ModuleError::InvalidSuspensionCallPlan {
                operation: Some(plan.operation),
                reason: SuspensionCallPlanError::InvalidClaimFrontier,
            })?;
        let mut previous_live = None;
        for live in &plan.live_values {
            if usize::try_from(live.claim_count).ok() != Some(live.claims.len()) {
                return invalid(Some(plan.operation), SuspensionCallPlanError::CountMismatch);
            }
            let live_key = (&live.place, live.storage, live.value_type, live.effective);
            if previous_live.is_some_and(|previous| previous >= live_key) {
                return invalid(Some(plan.operation), SuspensionCallPlanError::NonCanonical);
            }
            previous_live = Some(live_key);
            match (&live.place, live.value_type) {
                (
                    psi_terminal::TerminalSuspensionPlace::Scalar(value),
                    psi_terminal::TerminalSuspensionValueType::Scalar(expected),
                ) => {
                    if scalar_visible_before(machine, block, operation_index, *value)
                        != Some(expected)
                    {
                        return invalid(
                            Some(plan.operation),
                            SuspensionCallPlanError::TypeMismatch,
                        );
                    }
                    if !live.claims.is_empty() {
                        return invalid(
                            Some(plan.operation),
                            SuspensionCallPlanError::InvalidClaimFrontier,
                        );
                    }
                    if !scalar_storage_matches(
                        machine,
                        block,
                        operation_index,
                        &operation.kind,
                        *value,
                        live.storage,
                    ) {
                        return invalid(
                            Some(plan.operation),
                            SuspensionCallPlanError::InvalidCallArgument,
                        );
                    }
                }
                (
                    psi_terminal::TerminalSuspensionPlace::Structural { place, path },
                    psi_terminal::TerminalSuspensionValueType::Structural(expected),
                ) => {
                    let Some(root_type) =
                        structural_visible_before(machine, block, operation_index, *place)
                    else {
                        return invalid(
                            Some(plan.operation),
                            SuspensionCallPlanError::UnknownPlace,
                        );
                    };
                    if resolve_structural_path(module, root_type, path) != Some(expected) {
                        return invalid(
                            Some(plan.operation),
                            SuspensionCallPlanError::TypeMismatch,
                        );
                    }
                    if !structural_storage_matches(
                        machine,
                        block,
                        operation_index,
                        &operation.kind,
                        *place,
                        path,
                        live.storage,
                    ) {
                        return invalid(
                            Some(plan.operation),
                            SuspensionCallPlanError::InvalidCallArgument,
                        );
                    }
                    if live.claims.windows(2).any(|pair| pair[0] >= pair[1])
                        || live.claims.iter().any(|claim| {
                            !operation_frontier.claims().iter().any(|candidate| {
                                candidate.claim == *claim
                                    && candidate.input == Some(*place)
                                    && candidate.path.starts_with(path)
                            })
                        })
                    {
                        return invalid(
                            Some(plan.operation),
                            SuspensionCallPlanError::InvalidClaimFrontier,
                        );
                    }
                }
                _ => {
                    return invalid(Some(plan.operation), SuspensionCallPlanError::TypeMismatch);
                }
            }
        }
    }
    Ok(())
}

fn invalid<T>(
    operation: Option<OperationId>,
    reason: SuspensionCallPlanError,
) -> Result<T, ModuleError> {
    Err(ModuleError::InvalidSuspensionCallPlan { operation, reason })
}

fn operation_owner(
    module: &TerminalModule,
    operation: OperationId,
) -> Option<(&TerminalMachine, &psi_terminal::Block, usize)> {
    module.machines.iter().find_map(|machine| {
        machine.blocks.iter().find_map(|block| {
            block
                .operations
                .iter()
                .position(|candidate| candidate.id == operation)
                .map(|index| (machine, block, index))
        })
    })
}

fn terminal_call_target(
    kind: &OperationKind,
) -> Option<psi_terminal::TerminalSuspensionCallTarget> {
    match kind {
        OperationKind::Call { callee, .. }
        | OperationKind::CallUnit { callee, .. }
        | OperationKind::CallStructuralScalar { callee, .. }
        | OperationKind::CallStructural { callee, .. }
        | OperationKind::CallStructuralWithScalarArguments { callee, .. } => {
            Some(psi_terminal::TerminalSuspensionCallTarget::Machine(*callee))
        }
        OperationKind::BoundaryCall { boundary, .. } => Some(
            psi_terminal::TerminalSuspensionCallTarget::Boundary(*boundary),
        ),
        OperationKind::CallDynamicScalar {
            descriptor_ordinal, ..
        }
        | OperationKind::CallDynamicUnit {
            descriptor_ordinal, ..
        } => Some(
            psi_terminal::TerminalSuspensionCallTarget::DynamicDescriptor {
                ordinal: *descriptor_ordinal,
            },
        ),
        OperationKind::CallDynamicParameterScalar {
            parameter_ordinal,
            requirement_slot,
            ..
        }
        | OperationKind::CallDynamicParameterUnit {
            parameter_ordinal,
            requirement_slot,
            ..
        } => Some(
            psi_terminal::TerminalSuspensionCallTarget::DynamicParameter {
                parameter_ordinal: *parameter_ordinal,
                requirement_slot: *requirement_slot,
            },
        ),
        _ => None,
    }
}

fn scalar_call_arguments(kind: &OperationKind) -> &[ValueId] {
    match kind {
        OperationKind::Call { arguments, .. }
        | OperationKind::CallStructuralScalar { arguments, .. }
        | OperationKind::CallStructuralWithScalarArguments { arguments, .. }
        | OperationKind::BoundaryCall { arguments, .. } => arguments,
        _ => &[],
    }
}

fn scalar_storage_matches(
    machine: &TerminalMachine,
    block: &psi_terminal::Block,
    operation_index: usize,
    kind: &OperationKind,
    value: ValueId,
    storage: psi_terminal::TerminalSuspensionStorage,
) -> bool {
    match storage {
        psi_terminal::TerminalSuspensionStorage::Persistent => false,
        psi_terminal::TerminalSuspensionStorage::Parameter => machine
            .parameters
            .iter()
            .chain(block.parameters.iter())
            .any(|parameter| parameter.id == value),
        psi_terminal::TerminalSuspensionStorage::Local => block.operations[..operation_index]
            .iter()
            .filter_map(|operation| operation.result.scalar_ref())
            .any(|result| result.id == value),
        psi_terminal::TerminalSuspensionStorage::CallArgument => {
            scalar_call_arguments(kind).contains(&value)
        }
    }
}

fn structural_storage_matches(
    machine: &TerminalMachine,
    block: &psi_terminal::Block,
    operation_index: usize,
    kind: &OperationKind,
    place: PlaceId,
    path: &[StructuralPathSegment],
    storage: psi_terminal::TerminalSuspensionStorage,
) -> bool {
    match storage {
        psi_terminal::TerminalSuspensionStorage::Persistent => machine
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == place && parameter.is_self),
        psi_terminal::TerminalSuspensionStorage::Parameter => machine
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == place && !parameter.is_self),
        psi_terminal::TerminalSuspensionStorage::Local => block.operations[..operation_index]
            .iter()
            .filter_map(|operation| operation.result.structural())
            .any(|result| result.place == place),
        psi_terminal::TerminalSuspensionStorage::CallArgument => structural_call_arguments(kind)
            .iter()
            .any(|argument| argument.place == place && argument.path == path),
    }
}

fn structural_call_arguments(kind: &OperationKind) -> &[StructuralArgument] {
    match kind {
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        }
        | OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments,
        _ => &[],
    }
}

fn scalar_visible_before(
    machine: &TerminalMachine,
    block: &psi_terminal::Block,
    operation_index: usize,
    value: ValueId,
) -> Option<ScalarType> {
    machine
        .parameters
        .iter()
        .chain(block.parameters.iter())
        .chain(
            block.operations[..operation_index]
                .iter()
                .filter_map(|operation| operation.result.scalar_ref()),
        )
        .find_map(|declaration| (declaration.id == value).then_some(declaration.scalar_type))
}

fn structural_visible_before(
    machine: &TerminalMachine,
    block: &psi_terminal::Block,
    operation_index: usize,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    machine
        .structural_parameters
        .iter()
        .find_map(|parameter| (parameter.place == place).then_some(parameter.structural_type))
        .or_else(|| {
            block.operations[..operation_index]
                .iter()
                .filter_map(|operation| operation.result.structural())
                .find_map(|result| (result.place == place).then_some(result.structural_type))
        })
}
