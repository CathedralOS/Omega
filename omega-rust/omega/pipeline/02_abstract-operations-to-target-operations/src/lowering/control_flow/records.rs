//! Record construction retains the complete declared field tuple in one home.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use std::collections::BTreeSet;
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralMultiplicity, StructuralTypeShape,
};

pub(super) fn establish(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::EstablishRecord {
        psi_operation,
        result,
        fields,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let result_home = super::aggregate_results::home(*psi_operation, result, types)?;
    let StructuralTypeShape::Record {
        fields: declarations,
    } = &types
        .get(&result.structural_type)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
        .shape
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    if declarations.len() != fields.len() || live.structural_homes.contains_key(&result.place) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let mut consumed = BTreeSet::new();
    let mut relocations = Vec::new();
    let mut moved_leaves = BTreeSet::new();
    for (field, declaration) in fields.iter().zip(declarations) {
        if field.field != declaration.id || declaration.relevance.is_erased() {
            return Err(LoweringError::unsupported_control_flow(function.machine));
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
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
            }
            terminal_psi::RecordFieldValue::Structural(argument) => {
                let StructuralFieldType::Structural(nested) = declaration.field_type else {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                };
                let reference_bearing = super::references::contains_reference(types, nested);
                if !matches!(
                    types.get(&nested).map(|declaration| &declaration.shape),
                    Some(StructuralTypeShape::Record { .. })
                ) && !reference_bearing
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                // A bare carrier field contributes custody, not storage: its
                // operand contract comes from the live reference map, not a
                // structural home or signature parameter.
                let bare_carrier = reference_bearing
                    && matches!(
                        types.get(&nested).map(|declaration| &declaration.shape),
                        Some(StructuralTypeShape::Reference { .. })
                    );
                let multiplicity = if bare_carrier {
                    live.references
                        .get(&(argument.place, Vec::new()))
                        .filter(|leaf| leaf.result.structural_type == nested)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                        .result
                        .multiplicity
                } else if let Some(home) = live.structural_homes.get(&argument.place) {
                    if home.structural_type() != nested
                        || home.has_claims()
                        || !home.qualifications().is_empty()
                        || !home.projected_qualifications().is_empty()
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    home.multiplicity()
                } else {
                    let mut parameters = function
                        .structural_parameters
                        .iter()
                        .filter(|parameter| parameter.place == argument.place);
                    let parameter = parameters
                        .next()
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                    if parameters.next().is_some()
                        || parameter.structural_type != nested
                        || parameter.access != StructuralAccess::Owned
                        || !parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty()
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    parameter.multiplicity
                };
                if argument.access != StructuralAccess::Owned
                    || !argument.path.is_empty()
                    || multiplicity == StructuralMultiplicity::Linear
                    || (multiplicity == StructuralMultiplicity::Affine
                        && !consumed.insert(argument.place))
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                if reference_bearing {
                    super::references::record_field_leaves(
                        function,
                        types,
                        live,
                        &declaration.identity,
                        nested,
                        argument,
                        &mut relocations,
                        &mut moved_leaves,
                    )?;
                }
            }
        }
    }
    if live
        .references
        .keys()
        .any(|(carrier, _)| *carrier == result.place)
        || live
            .references
            .values()
            .any(|leaf| leaf.identity.0 == result.place)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    for place in consumed {
        live.structural_homes.remove(&place);
    }
    super::references::relocate_record_leaves(live, result.place, relocations);
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

/// An empty-record affine local occupies no bytes, so it needs no home: the
/// retained row is its establishment identity, and a whole owned call
/// argument names that row as its `StructuralHome` producer
/// (`owned_arguments::argument`). The declaration must be the module's own
/// empty record, and the place must be the trivial local that exact type
/// declares; an abandoned array-construction element keeps rejecting because
/// its prefix cleanup schedule is not realized here.
pub(super) fn establish_trivial_affine_local(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let AbstractOperation::EstablishTrivialAffineLocal {
        psi_operation,
        place,
        structural_type,
    } = operation
    else {
        return Err(invalid());
    };
    if !matches!(
        place.kind,
        semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
            structural_type: declared,
            construction: None,
            ..
        } if declared == structural_type.id
    ) || types.get(&structural_type.id).copied() != Some(structural_type)
        || !matches!(&structural_type.shape, StructuralTypeShape::Record { fields } if fields.is_empty())
        || live.structural_homes.contains_key(&place.id)
        || live
            .trivial_affine_locals
            .insert(place.id, (*psi_operation, structural_type.id))
            .is_some()
    {
        return Err(invalid());
    }
    operations.push(TargetUnitOperation::EstablishTrivialAffineLocal {
        psi_operation: *psi_operation,
        place: *place,
        structural_type: structural_type.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
