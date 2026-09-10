//! Ordered primitive-array construction retains each leaf in one aggregate home.
//! Its semantic type remains an array; byte layout adds neither a record nor a tag.
use super::LiveDefinitions;
use crate::lowering::shared::*;

pub(super) fn is_owned_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    parameter.access == StructuralAccess::Owned
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && !parameter.is_self
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && shape(parameter.structural_type, types).is_ok()
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
    let expected_shape = shape(declaration.structural_type, types)?.2;
    if !is_owned_parameter(declaration, types)
        || argument.access != StructuralAccess::Owned
        || !argument.path.is_empty()
        || destination.shape != expected_shape
    {
        return Err(invalid());
    }
    let source = if let Some(home) = live.structural_homes.get(&argument.place) {
        if home.result.structural_type != declaration.structural_type
            || home.result.multiplicity != declaration.multiplicity
            || !home.result.claims.is_empty()
            || !home.result.qualifications.is_empty()
            || !home.result.projected_qualifications.is_empty()
            || home.layout
                != target_operations::TargetStructuralHomeLayout::Aggregate(expected_shape)
        {
            return Err(invalid());
        }
        target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: home.defining_operation,
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

pub(in crate::lowering) fn shape(
    structural_type: StructuralTypeId,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<(ScalarType, u64, ValueShape), LoweringError> {
    let invalid = || LoweringError::UnsupportedStructuralArray(structural_type);
    let mut current = structural_type;
    let mut count = Some(1_u64);
    let mut empty = false;
    let mut array = false;
    for _ in 0..types.len() {
        match types.get(&current).ok_or_else(invalid)?.shape {
            StructuralTypeShape::FixedArray { element, length } => {
                array = true;
                empty |= length == 0;
                count = count.and_then(|count| count.checked_mul(length));
                current = element;
            }
            StructuralTypeShape::PrimitiveScalar(scalar_type) if array => {
                let leaf = crate::lowering::scalar_abi::fixed_native_scalar_shape(scalar_type)
                    .ok_or_else(invalid)?;
                let count = if empty { 0 } else { count.ok_or_else(invalid)? };
                let size = count
                    .checked_mul(u64::from(leaf.byte_size))
                    .and_then(|size| u16::try_from(size).ok())
                    .ok_or_else(invalid)?;
                return Ok((
                    scalar_type,
                    count,
                    // Zero-byte ABI values have canonical alignment one; the
                    // retained structural type still owns dimensions and carrier.
                    ValueShape::integer(size, if size == 0 { 1 } else { leaf.alignment }),
                ));
            }
            _ => return Err(invalid()),
        }
    }
    Err(invalid())
}

pub(super) fn establish(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    let AbstractOperation::EstablishScalarArray {
        psi_operation,
        result,
        elements,
    } = operation
    else {
        return Err(invalid());
    };
    let (scalar_type, count, _) = shape(result.structural_type, types)?;
    if u64::try_from(elements.len()).ok() != Some(count)
        || elements.iter().any(|value| {
            !super::scalar_sources::source(*value, function, live)
                .is_ok_and(|source| source.scalar_type() == scalar_type)
        })
    {
        return Err(invalid());
    }
    let result_home = super::aggregate_results::home(*psi_operation, result, types)?;
    if live
        .structural_homes
        .insert(result.place, result_home.clone())
        .is_some()
    {
        return Err(invalid());
    }
    operations.push(TargetUnitOperation::EstablishScalarArray {
        psi_operation: *psi_operation,
        result_home,
        elements: elements.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
