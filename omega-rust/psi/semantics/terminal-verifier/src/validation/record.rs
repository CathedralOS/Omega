//! Complete record establishment binds declarations, never layout-compatible names.

use super::*;
use terminal_psi::{RecordFieldValue, StructuralFieldDeclaration};

pub(super) fn plain_type(module: &TerminalModule, root: StructuralTypeId) -> bool {
    fn visit(
        module: &TerminalModule,
        root: StructuralTypeId,
        active: &mut Vec<StructuralTypeId>,
        complete: &mut BTreeSet<StructuralTypeId>,
    ) -> bool {
        if complete.contains(&root) {
            return true;
        }
        if active.contains(&root) {
            return false;
        }
        let mut candidates = module
            .structural_types
            .iter()
            .filter(|item| item.id == root);
        let Some(declaration) = candidates.next() else {
            return false;
        };
        if candidates.next().is_some() {
            return false;
        }
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            return false;
        };
        active.push(root);
        let valid = fields.iter().all(|field| {
            field.relevance == terminal_psi::BindingRelevance::Relevant
                && match field.field_type {
                    StructuralFieldType::Structural(child) => {
                        visit(module, child, active, complete)
                    }
                    _ => field.field_type.scalar_type().is_some(),
                }
        });
        active.pop();
        if valid {
            complete.insert(root);
        }
        valid
    }
    visit(module, root, &mut Vec::new(), &mut BTreeSet::new())
}

pub(crate) fn fields<'a>(
    module: &'a TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<&'a [StructuralFieldDeclaration], ModuleError> {
    let failure = || ModuleError::RecordResultMismatch(operation.id);
    let OperationKind::EstablishRecord { fields } = &operation.kind else {
        return Err(failure());
    };
    let result = operation.result.structural().ok_or_else(failure)?;
    let exact_place = machine.structural_places.iter().any(|place| {
        place.id == result.place
            && matches!(place.kind,
            StructuralPlaceKind::OperationResult { producer, structural_type }
                if producer == operation.id && structural_type == result.structural_type)
    });
    let owned = matches!(
        result.multiplicity,
        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
    );
    if !owned
        || !exact_place
        || !plain_type(module, result.structural_type)
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return Err(failure());
    }
    let declaration = module
        .structural_types
        .iter()
        .find(|item| item.id == result.structural_type)
        .ok_or_else(failure)?;
    let StructuralTypeShape::Record {
        fields: declarations,
    } = &declaration.shape
    else {
        return Err(failure());
    };
    if declarations.len() != fields.len() {
        return Err(failure());
    }
    for (declaration, binding) in declarations.iter().zip(fields) {
        if declaration.id != binding.field {
            return Err(failure());
        }
        match (&declaration.field_type, &binding.value) {
            (StructuralFieldType::Structural(expected), RecordFieldValue::Structural(argument)) => {
                let source =
                    super::structural_result_contracts::source_signature(machine, argument.place)
                        .ok_or_else(failure)?;
                if argument.access != StructuralAccess::Owned
                    || !argument.path.is_empty()
                    || argument.place == result.place
                    || source.structural_type != *expected
                    || !source.qualifications.is_empty()
                    || !source.projected_qualifications.is_empty()
                    || !matches!(
                        source.multiplicity,
                        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                    )
                    || (result.multiplicity == StructuralMultiplicity::Unrestricted
                        && source.multiplicity != StructuralMultiplicity::Unrestricted)
                    || machine
                        .structural_parameters
                        .iter()
                        .chain(
                            machine
                                .blocks
                                .iter()
                                .flat_map(|block| &block.structural_parameters),
                        )
                        .any(|parameter| {
                            parameter.place == argument.place
                                && parameter.access != StructuralAccess::Owned
                        })
                    || machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .any(|producer| {
                            producer.result.structural().is_some_and(|source| {
                                source.place == argument.place && !source.claims.is_empty()
                            })
                        })
                {
                    return Err(failure());
                }
            }
            (
                field_type,
                RecordFieldValue::Scalar {
                    range_obligation, ..
                },
            ) if field_type.scalar_type().is_some() => {
                if matches!(field_type, StructuralFieldType::BoundedInteger(_))
                    != range_obligation.is_some()
                {
                    return Err(failure());
                }
            }
            _ => return Err(failure()),
        }
    }
    Ok(declarations)
}

pub(super) fn operands(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let declarations = fields(module, machine, operation)?;
    let OperationKind::EstablishRecord { fields } = &operation.kind else {
        unreachable!()
    };
    for (declaration, binding) in declarations.iter().zip(fields) {
        if let RecordFieldValue::Scalar { value, .. } = binding.value {
            super::operations::require_defined(value, value_types, defined)?;
            if declaration.field_type.scalar_type() != value_types.get(&value).copied() {
                return Err(ModuleError::RecordResultMismatch(operation.id));
            }
        }
    }
    Ok(())
}

pub(super) fn completed_source<'a>(
    module: &TerminalModule,
    machine: &'a TerminalMachine,
    place: PlaceId,
) -> Option<&'a terminal_psi::StructuralOperationResult> {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            if !matches!(
                operation.kind,
                OperationKind::EstablishRecord { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
            ) {
                return None;
            }
            operation.result.structural().filter(|result| {
                result.place == place
                    && matches!(
                        result.multiplicity,
                        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                    )
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
                    && plain_type(module, result.structural_type)
            })
        })
}

pub(super) fn validate_uses(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    available: &BTreeSet<PlaceId>,
) -> Result<(), ModuleError> {
    let validate = |place| {
        if completed_source(module, machine, place).is_some() && !available.contains(&place) {
            Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: operation.id,
                place,
            })
        } else {
            Ok(())
        }
    };
    match &operation.kind {
        OperationKind::EstablishRecord { fields } => {
            for field in fields {
                if let RecordFieldValue::Structural(argument) = &field.value {
                    validate(argument.place)?;
                }
            }
        }

        OperationKind::IntegerStructuralField { source, .. }
        | OperationKind::BooleanStructuralField { source, .. } => validate(*source)?,
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
        } => {
            for argument in structural_arguments {
                validate(argument.place)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn result(
    machine: &TerminalMachine,
    source: PlaceId,
) -> Option<&terminal_psi::StructuralOperationResult> {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::EstablishRecord { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
            )
        })
        .filter_map(|operation| operation.result.structural())
        .find(|result| result.place == source)
}

pub(super) fn plain_return_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
) -> bool {
    completed_source(module, machine, source).is_some()
        || machine.structural_parameters.iter().any(|parameter| {
            parameter.place == source
                && parameter.access == StructuralAccess::Owned
                && matches!(
                    parameter.multiplicity,
                    StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                )
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
                && plain_type(module, parameter.structural_type)
                && !machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == source)
                && !machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == source)
        })
}
