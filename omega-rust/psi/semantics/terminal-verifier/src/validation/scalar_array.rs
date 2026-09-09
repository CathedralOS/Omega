//! Complete primitive-array construction and call-result return sources.

use super::operations::require_defined;
use super::*;

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
            operation.result.structural().is_some_and(|result| {
                result.place == source
                    && result.multiplicity == StructuralMultiplicity::Unrestricted
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
                    && terminal_semantics::scalar_array_leaf_shape(
                        module.structural_types.iter(),
                        result.structural_type,
                    )
                    .is_some()
            }) && match operation.kind {
                OperationKind::EstablishScalarArray { .. } => {
                    shape(module, machine, operation).is_ok()
                }
                // The complete call and its live result are validated by the
                // operation and frontier walks. Do not recursively inspect
                // callee source shapes or invent another call graph here.
                OperationKind::CallStructural { .. }
                | OperationKind::CallStructuralWithScalarArguments { .. } => true,
                _ => false,
            }
        })
}

pub(crate) fn shape(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<ScalarType, ModuleError> {
    let failure = || ModuleError::ScalarArrayResultMismatch(operation.id);
    let OperationKind::EstablishScalarArray { elements } = &operation.kind else {
        return Err(failure());
    };
    let result = operation.result.structural().ok_or_else(failure)?;
    let (scalar_type, leaf_count) = terminal_semantics::scalar_array_leaf_shape(
        module.structural_types.iter(),
        result.structural_type,
    )
    .ok_or_else(failure)?;
    if u64::try_from(elements.len()).ok() != Some(leaf_count)
        || result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
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
    Ok(scalar_type)
}

pub(super) fn operands(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let expected = shape(module, machine, operation)?;
    let OperationKind::EstablishScalarArray { elements } = &operation.kind else {
        return Err(ModuleError::ScalarArrayResultMismatch(operation.id));
    };
    for element in elements {
        require_defined(*element, value_types, defined)?;
        if value_types.get(element).copied() != Some(expected) {
            return Err(ModuleError::ScalarArrayResultMismatch(operation.id));
        }
    }
    Ok(())
}
