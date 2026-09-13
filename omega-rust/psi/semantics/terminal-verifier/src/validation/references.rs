//! Reference carriers own loan permission, never their primitive referent.
//!
//! Origins and immediate parents are reconstructed from operations and checked
//! call interfaces. No producer-supplied lifetime or origin table is authority.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LiveReference {
    /// Formation identity is stable across owned moves; parents name this ID,
    /// never a carrier's current record location.
    pub(super) identity: PlaceId,
    pub(super) carrier: PlaceId,
    pub(super) carrier_path: Vec<StructuralPathSegment>,
    pub(super) root: PlaceId,
    pub(super) parent: PlaceId,
}

/// Inspect owned containment only. A reference's referent is not its payload.
pub(super) fn contains_reference(module: &TerminalModule, root: StructuralTypeId) -> bool {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Some(declaration) = module
            .structural_types
            .iter()
            .find(|item| item.id == current)
        else {
            continue;
        };
        let mut fields = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
            pending.extend(fields.iter().filter_map(|field| match field.field_type {
                StructuralFieldType::Structural(child) => Some(child),
                _ => None,
            }));
        };
        match &declaration.shape {
            StructuralTypeShape::Reference { .. } => return true,
            StructuralTypeShape::Record {
                fields: declarations,
            } => fields(declarations),
            StructuralTypeShape::Sum { cases } => {
                for case in cases {
                    fields(&case.fields);
                }
            }
            StructuralTypeShape::Mixed {
                fields: declarations,
                cases,
            } => {
                fields(declarations);
                for case in cases {
                    fields(&case.fields);
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => pending.push(*element),
            StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_) => {}
        }
    }
    false
}

/// Exact declaration-order reference leaves of the supported owned shape.
/// Foundation has already rejected cycles and unsupported containment kinds.
fn leaf_paths(
    module: &TerminalModule,
    root: StructuralTypeId,
    maximum_leaves: usize,
) -> Option<Vec<Vec<StructuralPathSegment>>> {
    let mut output = Vec::new();
    let mut pending = vec![(root, Vec::new())];
    while let Some((current, path)) = pending.pop() {
        // A type DAG can describe exponentially many occurrences. Existing
        // live descriptors bound the possible roster; do not expand an absent
        // roster or recurse through repeated reference-free payload subtrees.
        if !contains_reference(module, current) {
            continue;
        }
        let Some(declaration) = module
            .structural_types
            .iter()
            .find(|item| item.id == current)
        else {
            continue;
        };
        match &declaration.shape {
            StructuralTypeShape::Reference { .. } => {
                if output.len() == maximum_leaves {
                    return None;
                }
                output.push(path);
            }
            StructuralTypeShape::Record { fields } => {
                for field in fields.iter().rev() {
                    if let StructuralFieldType::Structural(child) = field.field_type {
                        let mut child_path = path.clone();
                        child_path.push(StructuralPathSegment::Field(field.identity.clone()));
                        pending.push((child, child_path));
                    }
                }
            }
            _ => {}
        }
    }
    Some(output)
}

fn projected_carrier_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    let (StructuralPathSegment::Referent, fields) = source.path.split_last()? else {
        return None;
    };
    let mut current = super::structural_result_contracts::source_signature(machine, source.place)?
        .structural_type;
    for segment in fields {
        let StructuralPathSegment::Field(identity) = segment else {
            return None;
        };
        let declaration = module
            .structural_types
            .iter()
            .find(|item| item.id == current)?;
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            return None;
        };
        let field = fields.iter().find(|field| &field.identity == identity)?;
        if field.relevance != terminal_psi::BindingRelevance::Relevant {
            return None;
        }
        let StructuralFieldType::Structural(child) = field.field_type else {
            return None;
        };
        current = child;
    }
    referent(module, current)
}

pub(crate) fn is_reference_projection(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &StructuralArgument,
) -> bool {
    projected_carrier_type(module, machine, source).is_some()
        && source.access != StructuralAccess::Owned
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

/// Local records transfer established permission. Aggregate ingress and returns
/// remain fenced until those boundaries reconstruct the complete leaf roster.
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
        if let Some(result) = operation.result.structural()
            && contains_reference(module, result.structural_type)
            && !is_reference(result.structural_type)
            && (!matches!(operation.kind, OperationKind::EstablishRecord { .. })
                || result.multiplicity != StructuralMultiplicity::Affine
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty())
        {
            return Err(invalid(
                machine,
                "stored references require an affine record establishment",
            ));
        }
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
        .any(|parameter| contains_reference(module, parameter.structural_type))
    {
        return Err(invalid(
            machine,
            "reference carrier parameters are not yet supported",
        ));
    }
    // Binding any subtree of a reference-bearing root would need partial-move
    // custody, even when the selected target parameter itself has no references.
    for block in &machine.blocks {
        let arguments: &[StructuralArgument] = match &block.terminator {
            Terminator::Jump {
                structural_arguments,
                ..
            } => structural_arguments,
            _ => &[],
        };
        let conditional_arguments = match &block.terminator {
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => Some(
                when_true
                    .structural_arguments
                    .iter()
                    .chain(&when_false.structural_arguments),
            ),
            _ => None,
        };
        if arguments
            .iter()
            .chain(conditional_arguments.into_iter().flatten())
            .any(|argument| {
                super::structural_result_contracts::source_signature(machine, argument.place)
                    .is_some_and(|source| contains_reference(module, source.structural_type))
            })
        {
            return Err(invalid(
                machine,
                "reference-bearing continuation bindings are not yet supported",
            ));
        }
    }
    let Some(result) = machine.result.structural() else {
        return Ok(());
    };
    if !is_reference(result.structural_type) {
        if contains_reference(module, result.structural_type) {
            return Err(invalid(
                machine,
                "stored reference results are not yet supported",
            ));
        }
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
    if !source.path.is_empty() {
        return projected_carrier_type(module, machine, source);
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
    if let Some((StructuralPathSegment::Referent, carrier_path)) = source.path.split_last() {
        let parent = live
            .iter()
            .find(|reference| {
                reference.carrier == source.place && reference.carrier_path == carrier_path
            })
            .ok_or_else(|| invalid(machine, "reference carrier is no longer live"))?;
        if live
            .iter()
            .any(|reference| reference.parent == parent.identity)
        {
            return Err(invalid(
                machine,
                "reference parent remains suspended by a live child",
            ));
        }
        Ok((parent.root, parent.identity))
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
        .position(|reference| reference.carrier == source && reference.carrier_path.is_empty())
        .ok_or_else(|| invalid(machine, "release requires one live reference carrier"))?;
    if live
        .iter()
        .any(|reference| reference.parent == live[position].identity)
    {
        return Err(invalid(machine, "a reference cannot end before its child"));
    }
    live.remove(position);
    Ok(())
}

/// Whole-owner disposal visits reference leaves in reverse declaration order.
/// Validate the entire schedule before ending any loan; a live external child
/// (or an incorrectly ordered contained child) keeps its parent suspended.
pub(super) fn discard_owned(
    module: &TerminalModule,
    machine: &TerminalMachine,
    live: &mut Vec<LiveReference>,
    source: PlaceId,
) -> Result<(), ModuleError> {
    let Some(signature) = super::structural_result_contracts::source_signature(machine, source)
    else {
        return Ok(());
    };
    let paths = leaf_paths(module, signature.structural_type, live.len()).ok_or_else(|| {
        invalid(
            machine,
            "discard type requires more reference leaves than are live",
        )
    })?;
    let mut released = BTreeSet::new();
    for path in paths.iter().rev() {
        let reference = live
            .iter()
            .find(|reference| reference.carrier == source && reference.carrier_path == *path)
            .ok_or_else(|| {
                invalid(
                    machine,
                    "discard requires every owned reference leaf to be live",
                )
            })?;
        if live
            .iter()
            .any(|child| child.parent == reference.identity && !released.contains(&child.identity))
        {
            return Err(invalid(machine, "a reference cannot end before its child"));
        }
        released.insert(reference.identity);
    }
    if live
        .iter()
        .any(|reference| reference.carrier == source && !released.contains(&reference.identity))
    {
        return Err(invalid(
            machine,
            "discard reference roster differs from its owned type",
        ));
    }
    live.retain(|reference| !released.contains(&reference.identity));
    Ok(())
}

/// Record construction moves carriers, not referents. Exact static bindings
/// and the owning frontier are checked in this same operation transaction;
/// relocation cannot turn an owned move into a second, child loan.
fn establish_record(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    live: &mut [LiveReference],
) -> Result<(), ModuleError> {
    let declarations = super::record::fields(module, machine, operation)?;
    let OperationKind::EstablishRecord { fields } = &operation.kind else {
        return Err(invalid(
            machine,
            "record transfer requires a record establishment",
        ));
    };
    let result = operation
        .result
        .structural()
        .ok_or_else(|| invalid(machine, "record result is absent"))?;
    if live
        .iter()
        .any(|reference| reference.carrier == result.place || reference.identity == result.place)
    {
        return Err(invalid(
            machine,
            "record destination still owns live reference custody",
        ));
    }
    let mut relocations = Vec::new();
    let mut moved = BTreeSet::new();
    for (declaration, field) in declarations.iter().zip(fields) {
        let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
            continue;
        };
        let signature =
            super::structural_result_contracts::source_signature(machine, argument.place)
                .ok_or_else(|| invalid(machine, "record operand has no structural source"))?;
        let paths = leaf_paths(module, signature.structural_type, live.len()).ok_or_else(|| {
            invalid(
                machine,
                "record operand requires more reference leaves than are live",
            )
        })?;
        for path in paths {
            let position = live
                .iter()
                .position(|reference| {
                    reference.carrier == argument.place && reference.carrier_path == path
                })
                .ok_or_else(|| {
                    invalid(machine, "record operand does not own its reference leaf")
                })?;
            if !moved.insert(position) {
                return Err(invalid(
                    machine,
                    "record cannot duplicate reference custody",
                ));
            }
            let mut destination = vec![StructuralPathSegment::Field(declaration.identity.clone())];
            destination.extend(path);
            relocations.push((position, destination));
        }
        if live.iter().enumerate().any(|(position, reference)| {
            reference.carrier == argument.place && !moved.contains(&position)
        }) {
            return Err(invalid(
                machine,
                "record operand reference roster differs from its type",
            ));
        }
    }
    for (position, path) in relocations {
        live[position].carrier = result.place;
        live[position].carrier_path = path;
    }
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
        OperationKind::EstablishRecord { .. } => {
            return establish_record(module, machine, operation, live);
        }
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
        if super::structural_result_contracts::source_signature(machine, argument.place)
            .is_some_and(|source| contains_reference(module, source.structural_type))
            || live
                .iter()
                .any(|reference| reference.root == argument.place)
        {
            if !is_reference_projection(module, machine, argument) {
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
                .any(|argument| is_reference_projection(module, machine, argument))
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
        if live.iter().any(|reference| {
            reference.carrier == result.place || reference.identity == result.place
        }) {
            return Err(invalid(machine, "reference carrier is already live"));
        }
        live.push(LiveReference {
            identity: result.place,
            carrier: result.place,
            carrier_path: Vec::new(),
            root,
            parent,
        });
        live.sort_by_key(|reference| reference.identity);
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
        .find(|reference| reference.carrier == source && reference.carrier_path.is_empty())
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
