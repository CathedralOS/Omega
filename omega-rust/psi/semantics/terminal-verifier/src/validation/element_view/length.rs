//! Exact whole immutable element-view observation; control flow owns availability.

use crate::validation::{
    IntegerSign, IntegerType, ModuleError, OperationKind, PlaceId, ScalarType, StructuralAccess,
    StructuralMultiplicity, StructuralPlaceKind, StructuralTypeShape, TerminalMachine,
    TerminalModule, ValueId,
};

pub(in crate::validation) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
) -> Result<(), ModuleError> {
    let expected =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
    if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != expected)
    {
        return Err(ModuleError::ElementViewLengthRequiresU64Result(
            operation.id,
        ));
    }
    validate_source(module, machine, operation, source, || {
        ModuleError::InvalidElementViewLengthSource {
            operation: operation.id,
            source,
        }
    })?;
    Ok(())
}

/// One immutable element view retains one exact source for every observation.
/// Returns the view's declared element type when the source is a live
/// `ElementView` place under shared-borrow custody.
pub(in crate::validation) fn validate_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    _operation: &terminal_psi::Operation,
    source: PlaceId,
    invalid: impl Fn() -> ModuleError,
) -> Result<semantic_vocabulary::StructuralTypeId, ModuleError> {
    let place = machine
        .structural_places
        .iter()
        .find(|place| place.id == source)
        .ok_or_else(&invalid)?;
    let structural_type = match place.kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            let parameter = machine
                .structural_parameters
                .iter()
                .find(|parameter| {
                    parameter.place == source
                        && parameter.position == position
                        && parameter.is_self == is_self
                })
                .ok_or_else(&invalid)?;
            if parameter.access != StructuralAccess::SharedBorrow
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(invalid());
            }
            parameter.structural_type
        }
        StructuralPlaceKind::BlockParameter { .. } => {
            let parameter =
                crate::validation::block_views::parameter(machine, source).ok_or_else(&invalid)?;
            if parameter.access != StructuralAccess::SharedBorrow
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(invalid());
            }
            parameter.structural_type
        }
        StructuralPlaceKind::OperationResult { .. } => {
            crate::validation::element_view::subslice::borrowed_result(machine, source)
                .ok_or_else(&invalid)?
                .structural_type
        }
        _ => return Err(invalid()),
    };
    let element = module
        .structural_types
        .iter()
        .find_map(|declaration| match declaration.shape {
            StructuralTypeShape::ElementView { element } if declaration.id == structural_type => {
                Some(element)
            }
            _ => None,
        })
        .ok_or_else(&invalid)?;
    Ok(element)
}

/// A `length` operand is exact only when an `ElementViewLength` of the same
/// source produces it; a parameter, alias, or equal integer cannot replace it.
pub(in crate::validation) fn is_exact_length(
    machine: &TerminalMachine,
    source: PlaceId,
    length: ValueId,
) -> bool {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .any(|candidate| {
            candidate
                .result
                .scalar()
                .is_some_and(|result| result.id == length)
                && matches!(candidate.kind,
                    OperationKind::ElementViewLength { source: measured } if measured == source)
        })
}
