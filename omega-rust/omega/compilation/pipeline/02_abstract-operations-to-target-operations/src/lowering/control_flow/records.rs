//! Record and selected case construction retain declared field tuples in one home.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use std::collections::BTreeSet;
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
use terminal_psi::{
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeShape,
};

pub(super) fn establish(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let (psi_operation, result, fields, result_case, elements) = match operation {
        AbstractOperation::EstablishRecord {
            psi_operation,
            result,
            fields,
        } => (psi_operation, result, fields.as_slice(), None, None),
        AbstractOperation::EstablishCase {
            psi_operation,
            result,
            fields,
            result_case,
        } => (
            psi_operation,
            result,
            fields.as_slice(),
            Some(*result_case),
            None,
        ),
        AbstractOperation::EstablishStructuralArray {
            psi_operation,
            result,
            elements,
        } => (psi_operation, result, &[][..], None, Some(elements)),
        _ => return Err(invalid()),
    };
    let result_home = super::aggregate_results::home(*psi_operation, result, types)?;
    let shape = &types
        .get(&result.structural_type)
        .ok_or_else(invalid)?
        .shape;
    // A mixed case binds its runtime common fields in declaration order, then
    // the selected payload: the same roster Terminal verification checked.
    // Erased common members hold no storage and take no roster position.
    let declarations: Vec<&StructuralFieldDeclaration> = match (shape, result_case) {
        (StructuralTypeShape::Record { fields }, None) => fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect(),
        // Erased payload members hold no storage and take no roster position.
        (StructuralTypeShape::Sum { cases }, Some(result_case)) => cases
            .iter()
            .find(|case| case.id == result_case)
            .ok_or_else(invalid)?
            .fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect(),
        (
            StructuralTypeShape::Mixed {
                fields: common,
                cases,
            },
            Some(result_case),
        ) => common
            .iter()
            .filter(|field| !matches!(field.field_type, StructuralFieldType::Erased { .. }))
            .chain(
                cases
                    .iter()
                    .find(|case| case.id == result_case)
                    .ok_or_else(invalid)?
                    .fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased()),
            )
            .collect(),
        (StructuralTypeShape::FixedArray { length, .. }, None)
            if elements
                .is_some_and(|elements| u64::try_from(elements.len()).ok() == Some(*length)) =>
        {
            Vec::new()
        }
        _ => return Err(invalid()),
    };
    if declarations.len() != fields.len() || live.structural_homes.contains_key(&result.place) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let mut consumed = BTreeSet::new();
    let mut relocations = Vec::new();
    let mut moved_leaves = BTreeSet::new();
    let element_type = match shape {
        StructuralTypeShape::FixedArray { element, .. } => {
            Some(StructuralFieldType::Structural(*element))
        }
        _ => None,
    };
    enum Input<'a> {
        Field(&'a terminal_psi::RecordFieldValue),
        Element(&'a terminal_psi::StructuralArgument),
    }
    let operands = fields
        .iter()
        .zip(declarations.iter().copied())
        .map(|(field, declaration)| {
            if field.field != declaration.id || declaration.relevance.is_erased() {
                return Err(invalid());
            }
            Ok((
                &declaration.field_type,
                terminal_psi::StructuralPathSegment::Field(declaration.identity.clone()),
                Input::Field(&field.value),
            ))
        })
        .chain(
            elements
                .into_iter()
                .flatten()
                .enumerate()
                .map(|(index, argument)| {
                    Ok((
                        element_type.as_ref().ok_or_else(invalid)?,
                        terminal_psi::StructuralPathSegment::FixedIndex(index as u64),
                        Input::Element(argument),
                    ))
                }),
        );
    for operand in operands {
        let (field_type, destination, value) = operand?;
        match value {
            Input::Field(terminal_psi::RecordFieldValue::Scalar {
                value,
                range_obligation,
            }) => {
                let source = super::scalar_sources::source(*value, function, live)?;
                if field_type.scalar_type() != Some(source.scalar_type())
                    || terminal_semantics::scalar_field_requires_evidence(field_type)
                        != range_obligation.is_some()
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
            }
            Input::Field(terminal_psi::RecordFieldValue::Structural(argument))
            | Input::Element(argument) => {
                // Completed elements and structural members retain their own
                // declaration identity; direct byte fields spell the same
                // carrier inline. Resolve both before classifying storage.
                let byte_carrier = match field_type {
                    StructuralFieldType::Structural(child) => {
                        types
                            .get(child)
                            .and_then(|declaration| match declaration.shape {
                                StructuralTypeShape::ByteSequence(carrier) => Some(carrier),
                                _ => None,
                            })
                    }
                    field_type => field_type.byte_sequence_carrier(),
                }
                .filter(|carrier| {
                    matches!(
                        carrier,
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                });
                // A `&[T]`/`&[u8]` member relocates its loan's descriptor into
                // the record: the field owns the descriptor copy while the
                // referent stays borrowed. Only shared loans reach here —
                // upstream admission declines affine view members.
                let descriptor_member = match field_type {
                    StructuralFieldType::Structural(child) => {
                        types.get(child).is_some_and(|child| {
                            matches!(
                                child.shape,
                                StructuralTypeShape::ElementView { .. }
                                    | StructuralTypeShape::ByteSequence(
                                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                                    )
                            )
                        })
                    }
                    field_type => matches!(
                        field_type.byte_sequence_carrier(),
                        Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: None | Some(StructuralAccess::SharedBorrow),
                        })
                    ),
                };
                let nested = match field_type {
                    StructuralFieldType::Structural(nested) => *nested,
                    _ if byte_carrier.is_some() => {
                        let nested = live
                            .structural_observation_type(function, argument.place)
                            .ok_or_else(invalid)?;
                        if !types.get(&nested).is_some_and(|declaration| {
                            Some(&declaration.shape)
                                == byte_carrier.map(StructuralTypeShape::ByteSequence).as_ref()
                        }) {
                            return Err(invalid());
                        }
                        nested
                    }
                    // The declared `&[u8]` member spells its descriptor as a
                    // standalone borrowed view; the source carries that shape.
                    _ if descriptor_member => {
                        let nested = live
                            .structural_observation_type(function, argument.place)
                            .ok_or_else(invalid)?;
                        if !types.get(&nested).is_some_and(|declaration| {
                            matches!(
                                declaration.shape,
                                StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                                        access: None | Some(StructuralAccess::SharedBorrow),
                                    },
                                )
                            )
                        }) {
                            return Err(invalid());
                        }
                        nested
                    }
                    _ => return Err(invalid()),
                };
                let reference_bearing = super::references::contains_reference(types, nested);
                // Reference relocation currently describes record field paths;
                // a case-qualified carrier needs its own custody contract.
                if result_case.is_some() && reference_bearing {
                    return Err(invalid());
                }
                // A sum, mixed, or fixed-array member rides as one completed
                // value the way a record member does.
                if !matches!(
                    types.get(&nested).map(|declaration| &declaration.shape),
                    Some(
                        StructuralTypeShape::Record { .. }
                            | StructuralTypeShape::Sum { .. }
                            | StructuralTypeShape::Mixed { .. }
                            | StructuralTypeShape::FixedArray { .. }
                    )
                ) && byte_carrier.is_none()
                    && !descriptor_member
                    && !reference_bearing
                    && terminal_semantics::scalar_array_leaf_shape(types.values().copied(), nested)
                        .is_none()
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
                if !argument.path.iter().all(|segment| {
                    matches!(
                        segment,
                        terminal_psi::StructuralPathSegment::Field(_)
                            | terminal_psi::StructuralPathSegment::FixedIndex(_)
                    )
                }) {
                    return Err(invalid());
                }
                let multiplicity = if descriptor_member && argument.path.is_empty() {
                    // Relocating a descriptor never moves the referent: the
                    // source is the live view place (established or copied)
                    // or the block's view parameter — both spell the member's
                    // own type. Shared loans stay shared, so nothing consumes.
                    let view_type = live.structural_observation_type(function, argument.place);
                    if view_type != Some(nested) {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    StructuralMultiplicity::Unrestricted
                } else if bare_carrier && argument.path.is_empty() {
                    live.references
                        .get(&(argument.place, Vec::new()))
                        .filter(|leaf| leaf.result.structural_type == nested)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                        .result
                        .multiplicity
                } else if let Some(home) = live.structural_homes.get(&argument.place) {
                    if types.subtree(home.structural_type(), &argument.path) != Some(nested)
                        || home.access() != StructuralAccess::Owned
                        || (!argument.path.is_empty()
                            && home.multiplicity() != StructuralMultiplicity::Affine)
                        || home.has_claims()
                        || (byte_carrier.is_none() && !home.qualifications().is_empty())
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
                        || types.subtree(parameter.structural_type, &argument.path) != Some(nested)
                        || (!argument.path.is_empty()
                            && parameter.multiplicity != StructuralMultiplicity::Affine)
                        || parameter.access != StructuralAccess::Owned
                        || (byte_carrier.is_none() && !parameter.qualifications.is_empty())
                        || !parameter.projected_qualifications.is_empty()
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    parameter.multiplicity
                };
                if argument.access != StructuralAccess::Owned
                    || multiplicity == StructuralMultiplicity::Linear
                    || (byte_carrier.is_some()
                        && multiplicity != StructuralMultiplicity::Unrestricted)
                    || (multiplicity == StructuralMultiplicity::Affine
                        && (consumed.iter().any(
                            |(place, path): &(
                                semantic_vocabulary::PlaceId,
                                Vec<terminal_psi::StructuralPathSegment>,
                            )| {
                                *place == argument.place
                                    && (path.starts_with(&argument.path)
                                        || argument.path.starts_with(path))
                            },
                        ) || !consumed.insert((argument.place, argument.path.clone()))))
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                if reference_bearing && !descriptor_member {
                    super::references::record_field_leaves(
                        function,
                        types,
                        live,
                        destination.clone(),
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
    for (place, path) in consumed {
        // Projected transfer retains the original storage for its live siblings;
        // the independently checked partial frontier owns their later disposal.
        if path.is_empty() {
            live.structural_homes.remove(&place);
        }
    }
    super::references::relocate_record_leaves(live, result.place, relocations);
    live.structural_homes
        .insert(result.place, result_home.clone());
    operations.push(if let Some(elements) = elements {
        TargetUnitOperation::EstablishStructuralArray {
            psi_operation: *psi_operation,
            result_home,
            elements: elements.clone(),
        }
    } else {
        match result_case {
            Some(result_case) => TargetUnitOperation::EstablishCase {
                psi_operation: *psi_operation,
                result_home,
                result_case,
                fields: fields.to_vec(),
            },
            None => TargetUnitOperation::EstablishRecord {
                psi_operation: *psi_operation,
                result_home,
                fields: fields.to_vec(),
            },
        }
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

/// A structural local establishment names its whole aggregate home — the
/// memberwise stores that complete it arrive as their own operations, so
/// lowering only registers the exact home.
pub(super) fn establish_structural_local(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let AbstractOperation::EstablishStructuralLocal {
        psi_operation,
        result,
    } = operation
    else {
        return Err(invalid());
    };
    let result_home = super::aggregate_results::home(*psi_operation, result, types)?;
    if live
        .structural_homes
        .insert(result.place, result_home.clone())
        .is_some()
    {
        return Err(invalid());
    }
    operations.push(TargetUnitOperation::EstablishStructuralLocal {
        psi_operation: *psi_operation,
        result_home,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

/// A zeroed establishment names its whole aggregate home — the verified op
/// carries no element roster, so lowering only registers the exact home.
pub(super) fn establish_zeroed_structural(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let AbstractOperation::EstablishZeroedStructural {
        psi_operation,
        result,
    } = operation
    else {
        return Err(invalid());
    };
    let result_home = super::aggregate_results::home(*psi_operation, result, types)?;
    if live
        .structural_homes
        .insert(result.place, result_home.clone())
        .is_some()
    {
        return Err(invalid());
    }
    operations.push(TargetUnitOperation::EstablishZeroedStructural {
        psi_operation: *psi_operation,
        result_home,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
