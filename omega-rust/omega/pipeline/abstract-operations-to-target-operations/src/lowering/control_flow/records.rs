//! Record construction retains the complete declared field tuple in one home.
use super::LiveDefinitions;
use crate::lowering::shared::*;

pub(super) fn establish(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    let AbstractOperation::EstablishRecord {
        psi_operation,
        result,
        fields,
    } = operation
    else {
        return Err(invalid());
    };
    let result_home = super::aggregate_results::home(*psi_operation, result, types)?;
    let StructuralTypeShape::Record {
        fields: declarations,
    } = &types
        .get(&result.structural_type)
        .ok_or_else(invalid)?
        .shape
    else {
        return Err(invalid());
    };
    if declarations.len() != fields.len() || live.structural_homes.contains_key(&result.place) {
        return Err(invalid());
    }
    let mut consumed = BTreeSet::new();
    for (field, declaration) in fields.iter().zip(declarations) {
        if field.field != declaration.id || declaration.relevance.is_erased() {
            return Err(invalid());
        }
        match &field.value {
            terminal_psi::RecordFieldValue::Scalar {
                value,
                range_obligation,
            } => {
                let source = super::scalar_sources::source(*value, function, live)?;
                if declaration.field_type.scalar_type() != Some(source.scalar_type())
                    || matches!(
                        declaration.field_type,
                        StructuralFieldType::BoundedInteger(_)
                    ) != range_obligation.is_some()
                {
                    return Err(invalid());
                }
            }
            terminal_psi::RecordFieldValue::Structural(argument) => {
                let StructuralFieldType::Structural(nested) = declaration.field_type else {
                    return Err(invalid());
                };
                if !matches!(
                    types.get(&nested).map(|declaration| &declaration.shape),
                    Some(StructuralTypeShape::Record { .. })
                ) {
                    return Err(invalid());
                }
                let multiplicity = if let Some(home) = live.structural_homes.get(&argument.place) {
                    if home.structural_type() != nested
                        || home.has_claims()
                        || !home.qualifications().is_empty()
                        || !home.projected_qualifications().is_empty()
                    {
                        return Err(invalid());
                    }
                    home.multiplicity()
                } else {
                    let mut parameters = function
                        .structural_parameters
                        .iter()
                        .filter(|parameter| parameter.place == argument.place);
                    let parameter = parameters.next().ok_or_else(invalid)?;
                    if parameters.next().is_some()
                        || parameter.structural_type != nested
                        || parameter.access != StructuralAccess::Owned
                        || !parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty()
                    {
                        return Err(invalid());
                    }
                    parameter.multiplicity
                };
                if argument.access != StructuralAccess::Owned
                    || !argument.path.is_empty()
                    || multiplicity == StructuralMultiplicity::Linear
                    || (multiplicity == StructuralMultiplicity::Affine
                        && !consumed.insert(argument.place))
                {
                    return Err(invalid());
                }
            }
        }
    }
    for place in consumed {
        live.structural_homes.remove(&place);
    }
    live.structural_homes
        .insert(result.place, result_home.clone());
    operations.push(TargetUnitOperation::EstablishRecord {
        psi_operation: *psi_operation,
        result_home,
        fields: fields.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

pub(super) fn is_reference(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    types: &StructuralTypeLookup<'_>,
) -> bool {
    matches!(
        parameter.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && matches!(
            types
                .get(&parameter.structural_type)
                .map(|declaration| &declaration.shape),
            Some(StructuralTypeShape::Record { .. })
        )
}

pub(super) fn argument(
    argument: &terminal_psi::StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralArgument, LoweringError> {
    let invalid = || LoweringError::UnknownStructuralType(declaration.structural_type);
    if !is_reference(declaration, types) || argument.access != declaration.access {
        return Err(invalid());
    }
    let (root_type, source) = if let Some(home) = live.structural_homes.get(&argument.place) {
        if home.has_claims()
            || home.multiplicity() == StructuralMultiplicity::Linear
            || !home.qualifications().is_empty()
            || !home.projected_qualifications().is_empty()
        {
            return Err(invalid());
        }
        (
            home.structural_type(),
            target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: home.operation_result().ok_or_else(invalid)?.0,
            },
        )
    } else {
        let parameter = prepared
            .parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
            .ok_or_else(invalid)?;
        if !matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::Owned
        ) && parameter.access != argument.access
        {
            return Err(invalid());
        }
        (
            parameter.structural_type,
            parameter.placement.clone().into(),
        )
    };
    let (selected, shape, offset) = if argument.path.is_empty() {
        (
            root_type,
            crate::lowering::structural_layout::structural_shape(
                root_type,
                types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )?,
            0,
        )
    } else {
        crate::lowering::structural_layout::resolve_structural_field_path(
            root_type,
            &argument.path,
            types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?
    };
    if selected != declaration.structural_type {
        return Err(invalid());
    }
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: root_type,
        structural_type: selected,
        shape: calling_conventions::ValueShape::borrowed_reference(
            shape.byte_size,
            shape.alignment,
        ),
        source_byte_offset: offset,
        fixed_array_length: None,
        element_stride: None,
        source,
        destination: destination.placement.clone(),
    })
}
