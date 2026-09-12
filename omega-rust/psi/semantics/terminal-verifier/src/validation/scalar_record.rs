//! Atomic construction of exact scalar fields in declaration order.

use super::operations::require_defined;
use super::*;

pub(super) fn validate_uses(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    available: &BTreeSet<PlaceId>,
) -> Result<(), ModuleError> {
    let validate = |place| {
        if plain_return_source(module, machine, place) && !available.contains(&place) {
            Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: operation.id,
                place,
            })
        } else {
            Ok(())
        }
    };
    match &operation.kind {
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
                OperationKind::EstablishScalarRecord { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
            )
        })
        .filter_map(|operation| operation.result.structural())
        .find(|result| result.place == source)
}

pub(super) fn plain_type(module: &TerminalModule, structural_type: StructuralTypeId) -> bool {
    module.structural_types.iter().any(|declaration| {
        declaration.id == structural_type
            && matches!(&declaration.shape, StructuralTypeShape::Record { fields }
                if fields.iter().all(|field| field.relevance == terminal_psi::BindingRelevance::Relevant
                    && matches!(field.field_type, StructuralFieldType::Scalar(_) | StructuralFieldType::IeeeFloat(_))))
    })
}

pub(super) fn plain_return_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
) -> bool {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .any(|operation| {
            matches!(
                operation.kind,
                OperationKind::EstablishScalarRecord { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
            ) && operation.result.structural().is_some_and(|result| {
                result.place == source
                    && matches!(
                        result.multiplicity,
                        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                    )
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
                    && plain_type(module, result.structural_type)
            })
        })
}

pub(crate) fn fields<'module>(
    module: &'module TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<&'module [terminal_psi::StructuralFieldDeclaration], ModuleError> {
    let failure = || ModuleError::ScalarRecordResultMismatch(operation.id);
    let OperationKind::EstablishScalarRecord { fields } = &operation.kind else {
        return Err(failure());
    };
    let result = operation.result.structural().ok_or_else(failure)?;
    if !matches!(result.multiplicity, StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine)
        || !result.qualifications.is_empty() || !result.projected_qualifications.is_empty() || !result.claims.is_empty()
        || !machine.structural_places.iter().any(|place| place.id == result.place
            && matches!(place.kind, StructuralPlaceKind::OperationResult { producer, structural_type }
                if producer == operation.id && structural_type == result.structural_type))
    { return Err(failure()); }
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
        .ok_or_else(failure)?;
    let StructuralTypeShape::Record {
        fields: declarations,
    } = &declaration.shape
    else {
        return Err(failure());
    };
    if declarations.len() != fields.len()
        || declarations
            .iter()
            .zip(fields)
            .any(|(declaration, binding)| {
                declaration.id != binding.field
                    || declaration.relevance != terminal_psi::BindingRelevance::Relevant
                    || !matches!(
                        declaration.field_type,
                        StructuralFieldType::Scalar(_) | StructuralFieldType::IeeeFloat(_)
                    )
            })
    {
        return Err(failure());
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
    let OperationKind::EstablishScalarRecord { fields } = &operation.kind else {
        return Err(ModuleError::ScalarRecordResultMismatch(operation.id));
    };
    for (declaration, binding) in declarations.iter().zip(fields) {
        require_defined(binding.value, value_types, defined)?;
        if declaration.field_type.scalar_type() != value_types.get(&binding.value).copied() {
            return Err(ModuleError::ScalarRecordResultMismatch(operation.id));
        }
    }
    Ok(())
}
