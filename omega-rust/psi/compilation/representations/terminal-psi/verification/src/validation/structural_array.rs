//! Complete array construction uses the same exact owned operand contract as records.
use super::{
    ModuleError, OperationKind, StructuralAccess, StructuralMultiplicity, StructuralPlaceKind,
    StructuralTypeShape, TerminalMachine, TerminalModule,
};

pub(crate) fn operands(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::RecordResultMismatch(operation.id);
    let OperationKind::EstablishStructuralArray { elements } = &operation.kind else {
        return Err(invalid());
    };
    let result = operation.result.structural().ok_or_else(invalid)?;
    let Some(StructuralTypeShape::FixedArray { element, length }) = module
        .structural_types
        .iter()
        .find(|row| row.id == result.structural_type)
        .map(|row| &row.shape)
    else {
        return Err(invalid());
    };
    if !matches!(result.multiplicity, StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted)
        || !super::record::constructible_type(module, result.structural_type)
        || !result.qualifications.is_empty() || !result.projected_qualifications.is_empty() || !result.claims.is_empty()
        || u64::try_from(elements.len()).ok() != Some(*length)
        || !machine.structural_places.iter().any(|place| place.id == result.place && matches!(place.kind, StructuralPlaceKind::OperationResult { producer, structural_type } if producer == operation.id && structural_type == result.structural_type)) {
        return Err(invalid());
    }
    for argument in elements {
        let source = super::structural::result_contracts::constructor_argument_signature(
            module, machine, argument,
        )
        .ok_or_else(invalid)?;
        if argument.access != StructuralAccess::Owned
            || argument.place == result.place
            || source.structural_type != *element
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
                        && !(parameter.access == StructuralAccess::SharedBorrow
                            && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                            && argument.path.is_empty()
                            && terminal_semantics::descriptor_results::is_shared_descriptor(
                                module.structural_types.iter(),
                                parameter.structural_type,
                            ))
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
            return Err(invalid());
        }
    }
    Ok(())
}
