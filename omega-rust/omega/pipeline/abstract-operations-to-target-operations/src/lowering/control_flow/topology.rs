//! Select block-owned graph lowering from represented operations and custody.
use super::*;

pub(in crate::lowering) fn requires_graph(
    function: &AbstractFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    if function.result.scalar().is_some() {
        return true;
    }
    if function
        .operations
        .iter()
        .any(|operation| matches!(operation, AbstractOperation::IeeeFloatCompare { .. }))
    {
        return true;
    }
    if function
        .structural_parameters
        .iter()
        .any(|parameter| super::scalar_arrays::is_owned_parameter(parameter, types))
    {
        return true;
    }
    if function.result.structural().is_some_and(|result| {
        result.multiplicity == StructuralMultiplicity::Affine
            && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
    }) {
        return true;
    }
    if function.operations.iter().any(|operation| {
        matches!(
            operation,
            AbstractOperation::EstablishPrimitiveLocal { .. }
                | AbstractOperation::EstablishScalarArray { .. }
                | AbstractOperation::EstablishScalarCase { .. }
                | AbstractOperation::PrimitiveLocalStore { .. }
                | AbstractOperation::PrimitiveScalarRead { .. }
        )
    }) {
        return true;
    }
    if function.operations.iter().any(|operation| {
        matches!(operation, AbstractOperation::CallStructuralScalar { structural_arguments, .. }
            if structural_arguments.is_empty())
    }) {
        return true;
    }
    if function.operations.iter().any(|operation| {
        matches!(operation, AbstractOperation::CallStructural { result, .. }
            if matches!(types.get(&result.structural_type).map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Sum { .. } | StructuralTypeShape::FixedArray { .. })))
    }) {
        return true;
    }
    false
}
