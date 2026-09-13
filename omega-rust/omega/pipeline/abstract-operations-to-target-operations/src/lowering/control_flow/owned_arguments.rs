//! Owned aggregate actuals retain exact type, custody, and current value backing.
//! Array dimensions and nominal record layout share transport, not semantic identity.
use super::LiveDefinitions;
use crate::lowering::shared::*;

pub(super) fn is_owned_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    parameter.access == StructuralAccess::Owned
        && parameter.multiplicity != StructuralMultiplicity::Linear
        && !parameter.is_self
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && value_shape(parameter.structural_type, types).is_ok()
}

pub(super) fn argument(
    argument: &terminal_psi::StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<TargetStructuralArgument, LoweringError> {
    let invalid = || LoweringError::UnsupportedStructuralArray(declaration.structural_type);
    let expected_shape = value_shape(declaration.structural_type, types)?;
    if !is_owned_parameter(declaration, types)
        || argument.access != StructuralAccess::Owned
        || !argument.path.is_empty()
        || destination.shape != expected_shape
    {
        return Err(invalid());
    }
    let source = if let Some(home) = live.structural_homes.get(&argument.place) {
        if home.structural_type() != declaration.structural_type
            || home.multiplicity() != declaration.multiplicity
            || home.has_claims()
            || !home.qualifications().is_empty()
            || !home.projected_qualifications().is_empty()
            || home.layout
                != target_operations::TargetStructuralHomeLayout::Aggregate(expected_shape)
        {
            return Err(invalid());
        }
        target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: home.operation_result().ok_or_else(invalid)?.0,
        }
    } else {
        let parameter = prepared
            .parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
            .ok_or_else(invalid)?;
        if parameter.access != StructuralAccess::Owned
            || parameter.multiplicity != declaration.multiplicity
            || parameter.structural_type != declaration.structural_type
            || parameter.shape != expected_shape
        {
            return Err(invalid());
        }
        parameter.placement.clone().into()
    };
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: Vec::new(),
        root_structural_type: declaration.structural_type,
        structural_type: declaration.structural_type,
        shape: expected_shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source,
        destination: destination.placement.clone(),
    })
}

fn value_shape(
    structural_type: StructuralTypeId,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<ValueShape, LoweringError> {
    match types
        .get(&structural_type)
        .map(|declaration| &declaration.shape)
    {
        Some(StructuralTypeShape::Record { .. } | StructuralTypeShape::FixedArray { .. }) => {
            crate::lowering::structural_layout::structural_shape(
                structural_type,
                types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )
        }
        _ => super::scalar_arrays::shape(structural_type, types).map(|(_, _, shape)| shape),
    }
}
