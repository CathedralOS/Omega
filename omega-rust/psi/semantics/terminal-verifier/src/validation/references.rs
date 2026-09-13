//! Reference carriers own loan permission, never their primitive referent.
//!
//! Origins and immediate parents are reconstructed from operations and checked
//! call interfaces. No producer-supplied lifetime or origin table is authority.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LiveReference {
    pub(super) carrier: PlaceId,
    pub(super) root: PlaceId,
    pub(super) parent: PlaceId,
}

pub(super) fn invalid(machine: &TerminalMachine, reason: &'static str) -> ModuleError {
    ModuleError::InvalidReferenceCustody {
        machine: machine.id,
        reason,
    }
}

pub(super) fn referent(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    module.structural_types.iter().find_map(|declaration| {
        if declaration.id != structural_type {
            return None;
        }
        match declaration.shape {
            StructuralTypeShape::Reference {
                referent,
                access: StructuralAccess::MutableBorrow,
            } if super::primitive_storage::scalar_type(module, referent).is_some() => {
                Some(referent)
            }
            _ => None,
        }
    })
}

pub(super) fn carrier_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    let signature = super::structural_result_contracts::source_signature(machine, place)?;
    referent(module, signature.structural_type)
}

/// The first executable reference form is a whole mutable primitive carrier.
/// Nested storage and ingress-owned carriers require recursive custody replay.
pub(super) fn validate_machine(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), ModuleError> {
    let is_reference = |structural_type| {
        module.structural_types.iter().any(|declaration| {
            declaration.id == structural_type
                && matches!(declaration.shape, StructuralTypeShape::Reference { .. })
        })
    };
    for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
        if operation
            .result
            .structural()
            .is_some_and(|result| is_reference(result.structural_type))
            && !matches!(
                operation.kind,
                OperationKind::EstablishReference { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
            )
        {
            return Err(invalid(
                machine,
                "operation cannot establish reference custody",
            ));
        }
    }
    if machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .any(|parameter| is_reference(parameter.structural_type))
    {
        return Err(invalid(
            machine,
            "reference carrier parameters are not yet supported",
        ));
    }
    let Some(result) = machine.result.structural() else {
        return Ok(());
    };
    if !is_reference(result.structural_type) {
        if !result.reference_sources.is_empty() {
            return Err(invalid(
                machine,
                "non-reference result has reference source mappings",
            ));
        }
        return Ok(());
    }
    let Some(referent) = referent(module, result.structural_type) else {
        return Err(invalid(
            machine,
            "reference result access or referent is unsupported",
        ));
    };
    let [mapping] = result.reference_sources.as_slice() else {
        return Err(invalid(
            machine,
            "reference result requires its exact source mapping",
        ));
    };
    let source = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == mapping.source.place);
    if result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !mapping.path.is_empty()
        || !mapping.source.path.is_empty()
        || mapping.source.access != StructuralAccess::MutableBorrow
        || !source.is_some_and(|parameter| {
            parameter.structural_type == referent
                && parameter.access == StructuralAccess::MutableBorrow
                && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
                && !machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == parameter.place)
                && !machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == parameter.place)
        })
    {
        return Err(invalid(
            machine,
            "reference result does not name an exact mutable formal referent",
        ));
    }
    Ok(())
}

pub(super) fn validate_result(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let result = operation.result.structural().ok_or_else(|| {
        invalid(
            machine,
            "reference establishment requires a structural result",
        )
    })?;
    if referent(module, result.structural_type).is_none()
        || result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !machine.structural_places.iter().any(|place| {
            place.id == result.place
                && place.kind
                    == StructuralPlaceKind::OperationResult {
                        producer: operation.id,
                        structural_type: result.structural_type,
                    }
        })
    {
        return Err(invalid(
            machine,
            "invalid mutable reference operation result",
        ));
    }
    Ok(())
}

/// Static source formation; path availability and suspension are checked in the
/// same operation transaction as the owning frontier.
pub(super) fn source_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    if !matches!(
        source.access,
        StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow
    ) {
        return None;
    }
    if source.path == [StructuralPathSegment::Referent] {
        return carrier_type(module, machine, source.place);
    }
    if !source.path.is_empty() {
        return None;
    }
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source.place)
    {
        if parameter.access != StructuralAccess::MutableBorrow
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || machine
                .entry_claims
                .iter()
                .any(|claim| claim.input == source.place)
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == source.place)
        {
            return None;
        }
        super::primitive_storage::scalar_type(module, parameter.structural_type)?;
        return Some(parameter.structural_type);
    }
    let result = super::primitive_storage::local_result(machine, source.place)?;
    super::primitive_storage::scalar_type(module, result.structural_type)?;
    Some(result.structural_type)
}

pub(super) fn validate_establishment(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: &StructuralArgument,
) -> Result<(), ModuleError> {
    validate_result(module, machine, operation)?;
    let result = operation
        .result
        .structural()
        .ok_or_else(|| invalid(machine, "reference result is absent"))?;
    if source.access != StructuralAccess::MutableBorrow
        || source_type(module, machine, source) != referent(module, result.structural_type)
    {
        return Err(invalid(
            machine,
            "reference formation exceeds its source authority",
        ));
    }
    Ok(())
}

pub(super) fn validate_uses(
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    available: &BTreeSet<PlaceId>,
) -> Result<(), ModuleError> {
    if let OperationKind::EstablishReference { source } = &operation.kind {
        let parameter = machine
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == source.place);
        if !parameter && !available.contains(&source.place) {
            return Err(invalid(
                machine,
                "reference source has not been established on every arrival",
            ));
        }
    }
    Ok(())
}

pub(super) fn call_source(
    callee: &TerminalMachine,
    arguments: &[StructuralArgument],
) -> Option<StructuralArgument> {
    let [mapping] = callee.result.structural()?.reference_sources.as_slice() else {
        return None;
    };
    let position = callee
        .structural_parameters
        .iter()
        .position(|parameter| parameter.place == mapping.source.place)?;
    let mut source = arguments.get(position)?.clone();
    source.path.extend(mapping.source.path.iter().cloned());
    source.access = mapping.source.access;
    Some(source)
}

pub(super) fn validate_call(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &terminal_psi::Operation,
) -> Result<bool, ModuleError> {
    let (
        callee,
        arguments,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
    ) = match &operation.kind {
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        } => (
            *callee,
            &[][..],
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        ),
        OperationKind::CallStructuralWithScalarArguments {
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => (
            *callee,
            arguments.as_slice(),
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        ),
        _ => return Ok(false),
    };
    let Some(callee) = machines.get(&callee).copied() else {
        return Ok(false);
    };
    let Some(result) = callee
        .result
        .structural()
        .filter(|result| referent(module, result.structural_type).is_some())
    else {
        return Ok(false);
    };
    validate_result(module, machine, operation)?;
    let actual = operation
        .result
        .structural()
        .ok_or_else(|| invalid(machine, "reference call result is absent"))?;
    if !super::structural_result_contracts::call_result_matches(actual, result)
        || !claim_transfers.is_empty()
        || !returned_claim_transfers.is_empty()
        || !callee.entry_claims.is_empty()
        || !callee.content_entry_claims.is_empty()
        || arguments.len() != callee.parameters.len()
        || requirement_obligations.len() != callee.contract.requires.len()
    {
        return Err(invalid(
            machine,
            "reference call does not match its complete callee interface",
        ));
    }
    super::structural_operations::validate_structural_arguments(module, machine, structural_arguments, &callee.structural_parameters, operation.id, true,
        super::structural_operations::StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults)?;
    super::structural_operations::validate_unit_call_contract_places(callee, operation.id)?;
    super::structural_operations::validate_unit_call_claim_transfers(
        module,
        machine,
        callee,
        structural_arguments,
        claim_transfers,
        operation.id,
    )?;
    super::structural_operations::validate_service_reach(
        operation.id,
        &machine.published_service_ceiling,
        &callee.published_service_ceiling,
    )?;
    super::structural_operations::validate_unit_call_crash_continuations(
        module,
        machine,
        callee,
        arguments,
        structural_arguments,
        crash_continuations,
        operation.id,
    )?;
    Ok(true)
}

fn normalized_source(
    machine: &TerminalMachine,
    live: &[LiveReference],
    source: &StructuralArgument,
) -> Result<(PlaceId, PlaceId), ModuleError> {
    if source.path == [StructuralPathSegment::Referent] {
        let parent = live
            .iter()
            .find(|reference| reference.carrier == source.place)
            .ok_or_else(|| invalid(machine, "reference carrier is no longer live"))?;
        if live
            .iter()
            .any(|reference| reference.parent == source.place)
        {
            return Err(invalid(
                machine,
                "reference parent remains suspended by a live child",
            ));
        }
        Ok((parent.root, parent.carrier))
    } else if source.path.is_empty() {
        if live.iter().any(|reference| reference.root == source.place) {
            return Err(invalid(
                machine,
                "original referent remains suspended by a live reference",
            ));
        }
        Ok((source.place, source.place))
    } else {
        Err(invalid(
            machine,
            "projected reference custody is not yet supported",
        ))
    }
}

pub(super) fn release(
    machine: &TerminalMachine,
    live: &mut Vec<LiveReference>,
    source: PlaceId,
) -> Result<(), ModuleError> {
    let position = live
        .iter()
        .position(|reference| reference.carrier == source)
        .ok_or_else(|| invalid(machine, "release requires one live reference carrier"))?;
    if live.iter().any(|reference| reference.parent == source) {
        return Err(invalid(machine, "a reference cannot end before its child"));
    }
    live.remove(position);
    Ok(())
}

pub(super) fn check_root_access(
    machine: &TerminalMachine,
    live: &[LiveReference],
    place: PlaceId,
) -> Result<(), ModuleError> {
    if live
        .iter()
        .any(|reference| reference.root == place || reference.carrier == place)
    {
        return Err(invalid(
            machine,
            "operation accesses suspended referent or treats reference carrier as owned data",
        ));
    }
    Ok(())
}

pub(super) fn apply_operation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &terminal_psi::Operation,
    live: &mut Vec<LiveReference>,
) -> Result<(), ModuleError> {
    match &operation.kind {
        OperationKind::ReleaseReference { source } => return release(machine, live, *source),
        OperationKind::PrimitiveScalarRead { source } => check_root_access(machine, live, *source)?,
        OperationKind::WriteOnlyPrimitiveStore { destination, .. } => {
            check_root_access(machine, live, *destination)?
        }
        OperationKind::EstablishPrimitiveLocal { .. } => {
            if let Some(result) = operation.result.structural() {
                check_root_access(machine, live, result.place)?;
            }
        }
        _ => {}
    }
    let arguments = match &operation.kind {
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
        } => structural_arguments.as_slice(),
        _ => &[],
    };
    let mut normalized_arguments = Vec::new();
    for argument in arguments {
        if carrier_type(module, machine, argument.place).is_some()
            || live
                .iter()
                .any(|reference| reference.root == argument.place)
        {
            if argument.path != [StructuralPathSegment::Referent]
                || argument.access == StructuralAccess::Owned
            {
                return Err(invalid(
                    machine,
                    "call cannot access a suspended root or move its referent",
                ));
            }
            let (root, _) = normalized_source(machine, live, argument)?;
            normalized_arguments.push((root, argument.access));
        } else {
            normalized_arguments.push((argument.place, argument.access));
        }
    }
    for (position, (root, access)) in normalized_arguments.iter().enumerate() {
        if normalized_arguments[position + 1..]
            .iter()
            .any(|(other, other_access)| {
                root == other
                    && (*access != StructuralAccess::SharedBorrow
                        || *other_access != StructuralAccess::SharedBorrow)
            })
            && arguments
                .iter()
                .any(|argument| carrier_type(module, machine, argument.place).is_some())
        {
            return Err(invalid(
                machine,
                "call arguments overlap through reference origins",
            ));
        }
    }
    let source = match &operation.kind {
        OperationKind::EstablishReference { source } => Some(source.clone()),
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            callee,
            structural_arguments,
            ..
        } if operation
            .result
            .structural()
            .is_some_and(|result| referent(module, result.structural_type).is_some()) =>
        {
            Some(
                call_source(machines[callee], structural_arguments)
                    .ok_or_else(|| invalid(machine, "reference call source mapping is absent"))?,
            )
        }
        _ => None,
    };
    if let Some(source) = source {
        let (root, parent) = normalized_source(machine, live, &source)?;
        let result = operation
            .result
            .structural()
            .ok_or_else(|| invalid(machine, "reference establishment result is absent"))?;
        if live
            .iter()
            .any(|reference| reference.carrier == result.place)
        {
            return Err(invalid(machine, "reference carrier is already live"));
        }
        live.push(LiveReference {
            carrier: result.place,
            root,
            parent,
        });
        live.sort_by_key(|reference| reference.carrier);
    }
    Ok(())
}

pub(super) fn transfer_return(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
    live: &mut Vec<LiveReference>,
) -> Result<bool, ModuleError> {
    if carrier_type(module, machine, source).is_none() {
        return Ok(false);
    }
    let reference = live
        .iter()
        .find(|reference| reference.carrier == source)
        .ok_or_else(|| invalid(machine, "returned reference carrier is not live"))?;
    let result = machine
        .result
        .structural()
        .ok_or_else(|| invalid(machine, "reference return has no declaration"))?;
    let [mapping] = result.reference_sources.as_slice() else {
        return Err(invalid(
            machine,
            "reference return source mapping is absent",
        ));
    };
    if reference.root != mapping.source.place || reference.parent != reference.root {
        return Err(invalid(
            machine,
            "reference return escapes local custody or differs from its formal source",
        ));
    }
    release(machine, live, source)?;
    Ok(true)
}
