//! Complete, atomic establishment of a selected scalar payload.

use super::operations::require_defined;
use super::*;

pub(super) fn plain_type(module: &TerminalModule, structural_type: StructuralTypeId) -> bool {
    module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .is_some_and(|declaration| {
            matches!(&declaration.shape, StructuralTypeShape::Sum { cases }
            if cases.iter().all(|case| case.fields.iter().all(|field|
                field.relevance == terminal_psi::BindingRelevance::Relevant
                    && field.field_type.scalar_type().is_some())))
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
                OperationKind::EstablishScalarCase { .. }
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
    let failure = || ModuleError::ScalarCaseResultMismatch(operation.id);
    let OperationKind::EstablishScalarCase {
        result_case,
        fields,
    } = &operation.kind
    else {
        return Err(failure());
    };
    let result = operation.result.structural().ok_or_else(failure)?;
    if !matches!(
        result.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !machine.structural_places.iter().any(|place| {
            place.id == result.place
                && matches!(place.kind,
                StructuralPlaceKind::OperationResult { producer, structural_type }
                if producer == operation.id && structural_type == result.structural_type)
        })
    {
        return Err(failure());
    }
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
        .ok_or_else(failure)?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return Err(ModuleError::ScalarCaseRequiresSum {
            operation: operation.id,
            structural_type: result.structural_type,
            result_case: *result_case,
        });
    };
    let member_failure = || ModuleError::ScalarCaseFieldMismatch {
        operation: operation.id,
        structural_type: result.structural_type,
        result_case: *result_case,
    };
    let selected = cases
        .iter()
        .find(|case| case.id == *result_case)
        .ok_or_else(member_failure)?;
    if selected.fields.len() != fields.len() {
        return Err(member_failure());
    }
    for (declaration, binding) in selected.fields.iter().zip(fields) {
        if declaration.id != binding.field
            || declaration.relevance != terminal_psi::BindingRelevance::Relevant
            || declaration.field_type.scalar_type().is_none()
            || matches!(
                declaration.field_type,
                StructuralFieldType::BoundedInteger(_)
            ) != binding.range_obligation.is_some()
        {
            return Err(failure());
        }
    }
    Ok(&selected.fields)
}

pub(super) fn operands(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let declarations = fields(module, machine, operation)?;
    let OperationKind::EstablishScalarCase { fields, .. } = &operation.kind else {
        return Err(ModuleError::ScalarCaseResultMismatch(operation.id));
    };
    for (declaration, binding) in declarations.iter().zip(fields) {
        require_defined(binding.value, value_types, defined)?;
        if declaration.field_type.scalar_type() != value_types.get(&binding.value).copied() {
            return Err(ModuleError::ScalarCaseResultMismatch(operation.id));
        }
    }
    Ok(())
}
