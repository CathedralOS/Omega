//! Detect reads, stores and transfers crossing a package witness boundary.
use super::*;

// Identify every operation that enters a public package field. A producer
// cannot extract the uninstantiated reference or overwrite just the callback
// by leaving its opening out of the evidence table. Whole-package transport
// needs no conversion and does not enter a field.
pub(super) fn uses_witness(
    module: &TerminalModule,
    field: &StructuralFieldType,
    witness: StructuralTypeId,
) -> bool {
    match field {
        StructuralFieldType::Scalar(ScalarType::Callable(id)) => module.execution.callables.contract(*id).is_some_and(|contract|
            contract.structural_parameters.iter().any(|parameter| parameter.structural_type == witness)
            || contract.proof_result.structural().is_some_and(|result| result.structural_type == witness)),
        StructuralFieldType::Structural(id) => *id == witness || module.structural_type(*id)
            .is_some_and(|row| matches!(row.shape,StructuralTypeShape::Reference {referent,..} if referent == witness)),
        _ => false,
    }
}

fn field_needs_opening(
    module: &TerminalModule,
    record: StructuralTypeId,
    field: &StructuralFieldDeclaration,
) -> bool {
    let Ok(schema) = schema(module, record) else {
        return false;
    };
    match &field.field_type {
        StructuralFieldType::Scalar(ScalarType::Callable(contract)) => module
            .execution
            .callables
            .contract(*contract)
            .is_none_or(|contract| {
                contract
                    .structural_parameters
                    .iter()
                    .any(|parameter| schema.witnesses.contains(&parameter.structural_type))
                    || contract
                        .proof_result
                        .structural()
                        .is_some_and(|result| schema.witnesses.contains(&result.structural_type))
            }),
        StructuralFieldType::Structural(ty) => {
            match module.structural_type(*ty).map(|row| &row.shape) {
                Some(StructuralTypeShape::Reference { referent, .. }) => {
                    schema.witnesses.contains(referent)
                }
                _ => true,
            }
        }
        _ => false,
    }
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
                        if checked.stored_type(machine.id, operation.id).is_some() => {}
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
