//! Detect reads, stores and transfers crossing a package witness boundary.
use super::*;

pub(super) fn nominal_uses(
    module: &TerminalModule,
    ty: StructuralTypeId,
    witness: StructuralTypeId,
) -> bool {
    let identity = |id| {
        module
            .structural_type(id)
            .and_then(|row| module.identities.text(row.identity))
    };
    identity(ty)
        .zip(identity(witness))
        .is_some_and(|(carrier, witness)| {
            application_identity::type_identity::package_applications::uses(carrier, witness)
        })
}

// Identify every operation that enters a public package field. A producer
// cannot extract the uninstantiated reference or overwrite just the callback
// by leaving its opening out of the evidence table. Whole-package transport
// needs no conversion and does not enter a field.
pub(super) fn uses_witness(
    module: &TerminalModule,
    field: &StructuralFieldType,
    witness: StructuralTypeId,
) -> bool {
    let mut pending = vec![field.clone()];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(field) = pending.pop() {
        if !seen.insert(field.clone()) {
            continue;
        }
        match field {
            StructuralFieldType::Scalar(ScalarType::Callable(id)) => {
                if let Some(contract) = module.execution.callables.contract(id) {
                    pending.extend(
                        contract
                            .structural_parameters
                            .iter()
                            .map(|p| StructuralFieldType::Structural(p.structural_type)),
                    );
                    if let Some(result) = contract.proof_result.structural() {
                        pending.push(StructuralFieldType::Structural(result.structural_type));
                    }
                }
            }
            StructuralFieldType::Structural(id) => {
                if id == witness || nominal_uses(module, id, witness) {
                    return true;
                }
                if let Some(row) = module.structural_type(id) {
                    match &row.shape {
                        StructuralTypeShape::Reference { referent, .. } => {
                            pending.push(StructuralFieldType::Structural(*referent))
                        }
                        StructuralTypeShape::FixedArray { element, .. } => {
                            pending.push(StructuralFieldType::Structural(*element))
                        }
                        shape => {
                            pending.extend(shape.fields().map(|field| field.field_type.clone()))
                        }
                    }
                }
            }
            _ => (),
        }
    }
    false
}

pub(super) fn field_needs_opening(
    module: &TerminalModule,
    record: StructuralTypeId,
    field: &StructuralFieldDeclaration,
) -> bool {
    let Ok(schema) = schema(module, record) else {
        return false;
    };
    match &field.field_type {
        StructuralFieldType::Scalar(ScalarType::Callable(contract)) => {
            module.execution.callables.contract(*contract).is_none()
                || schema
                    .witnesses
                    .iter()
                    .any(|&witness| uses_witness(module, &field.field_type, witness))
        }
        StructuralFieldType::Structural(ty) => {
            let Some(row) = module.structural_type(*ty) else {
                return true;
            };
            // An unrestricted tag or metadata record carries no package
            // permission of its own. A linear field can additionally carry
            // the witness in custody qualifications on its enclosing path.
            !row.is_copyable() && !matches!(row.shape, StructuralTypeShape::Reference { .. })
                || schema
                    .witnesses
                    .iter()
                    .any(|&witness| uses_witness(module, &field.field_type, witness))
        }
        _ => false,
    }
}

/// The package member that owns this loan and the path to its reference
/// within the member's concrete initializer.
pub(super) struct HiddenReferenceField<'path> {
    pub record: StructuralTypeId,
    pub field: StructuralFieldId,
    pub path: &'path [StructuralPathSegment],
    pub access: StructuralAccess,
    pub referent: StructuralTypeId,
}

pub(super) fn hidden_reference_field<'path>(
    module: &TerminalModule,
    root: StructuralTypeId,
    path: &'path [StructuralPathSegment],
) -> Option<HiddenReferenceField<'path>> {
    let types = &module.types.structural_types;
    let reference = crate::inline_descriptor_type(types.iter(), root, path)?;
    let StructuralTypeShape::Reference { referent, access } = reference.shape else {
        return None;
    };
    for (position, segment) in path.iter().enumerate() {
        let StructuralPathSegment::Field(identity) = segment else {
            continue;
        };
        let record = crate::runtime_structural_path_tip(types.iter(), root, &path[..position])?.id;
        let Ok(package) = schema(module, record) else {
            continue;
        };
        let field = fields(module, record)
            .ok()?
            .iter()
            .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
        if field_needs_opening(module, record, field) {
            return package
                .witnesses
                .contains(&referent)
                .then_some(HiddenReferenceField {
                    record,
                    field: field.id,
                    path: &path[position + 1..],
                    access,
                    referent,
                });
        }
    }
    None
}

pub(super) fn enters_package(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operand: terminal_psi::OperationProjection<'_>,
) -> bool {
    let Some(root) = machine.structural_place_type(operand.root) else {
        return false;
    };
    (0..=operand.path.len()).any(|i| {
        let tip = crate::runtime_structural_path_tip(
            module.types.structural_types.iter(),
            root,
            &operand.path[..i],
        );
        let Some(row) = tip else { return false };
        let Ok(fields) = fields(module, row.id) else {
            return false;
        };
        let field = match operand.path.get(i) {
            Some(StructuralPathSegment::Field(identity)) => {
                fields.iter().find(|f| f.identity == *identity)
            }
            None => operand
                .field
                .and_then(|id| fields.iter().find(|f| f.id == id)),
            _ => None,
        };
        field.is_some_and(|field| field_needs_opening(module, row.id, field))
    })
}

pub(super) fn check_field_accesses(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    event: Option<&TerminalPackageEvent>,
    checked: &CheckedPackageSites,
) -> Result<(), &'static str> {
    use terminal_psi::StructuralStorageUse as Use;
    // Last use ends the stored permission itself. Ordinary affine checking
    // still requires an owned, live carrier and restores each actual lender.
    if let OperationKind::ReleaseReference { source, path } = &operation.kind
        && let Some(root) = machine.structural_place_type(*source)
        && hidden_reference_field(module, root, path).is_some()
    {
        return Ok(());
    }
    let tested_argument = |projection: terminal_psi::OperationProjection<'_>| {
        let OperationKind::CallMachine {
            structural_arguments,
            ..
        } = &operation.kind
        else {
            return false;
        };
        structural_arguments.iter().enumerate().any(|(i, arg)| {
            projection.field.is_none()
                && arg.place == projection.root
                && crate::structural_paths_select_same_location(&arg.path, projection.path)
                && checked
                    .borrowed_argument(machine.id, operation.id, i)
                    .is_some()
        })
    };
    for (operand, _) in operation.kind.storage_uses() {
        match operand {
            Use::Projection(projection) if enters_package(module, machine, projection) => {
                match event {
                    Some(TerminalPackageEvent::Project { .. }) => (),
                    Some(TerminalPackageEvent::Store { .. })
                        if checked.stored_field_type(machine.id, operation.id).is_some() => {}
                    Some(TerminalPackageEvent::Invoke { .. }) if tested_argument(projection) => (),
                    _ => {
                        return Err("operation enters a package field without its checked opening");
                    }
                }
            }
            Use::Element { descriptor, .. } if enters_package(module, machine, descriptor) => {
                return Err("element operation reads an unopened package field");
            }
            Use::Canonical { root, path } | Use::ObservedField { root, path, .. } => {
                let Some(ty) = machine.structural_place_type(root) else {
                    continue;
                };
                for i in 0..=path.len() {
                    let field = match path.get(i) {
                        Some(semantic_vocabulary::CanonicalStructuralPathSegment::Field(id)) => {
                            Some(*id)
                        }
                        None => match operand {
                            Use::ObservedField { field, .. } => Some(field),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(id) = field
                        && let Some(row) = crate::canonical_structural_path_tip(
                            module.types.structural_types.iter(),
                            ty,
                            &path[..i],
                        )
                        && fields(module, row.id).is_ok_and(|fields| {
                            fields
                                .iter()
                                .find(|f| f.id == id)
                                .is_some_and(|field| field_needs_opening(module, row.id, field))
                        })
                    {
                        return Err(
                            "canonical operation enters a package field without its opening",
                        );
                    }
                }
            }
            _ => (),
        }
    }
    Ok(())
}
