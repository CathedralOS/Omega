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
        _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
    };
    let result_home = super::aggregate_results::home(*psi_operation, result, types)?;
    let shape = &types
        .get(&result.structural_type)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?
        .shape;
    // A mixed case binds its runtime common fields in declaration order, then
    // the selected payload: the same roster Terminal verification checked.
    // Erased common members hold no storage and take no roster position.
    let declarations: Vec<(
        Option<semantic_vocabulary::StructuralCaseId>,
        &StructuralFieldDeclaration,
    )> = match (shape, result_case) {
        (StructuralTypeShape::Record { fields, .. }, None) => fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .map(|field| (None, field))
            .collect(),
        // Erased payload members hold no storage and take no roster position.
        (StructuralTypeShape::Sum { cases }, Some(result_case)) => cases
            .iter()
            .find(|case| case.id == result_case)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?
            .fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .map(|field| (Some(result_case), field))
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
            .map(|field| (None, field))
            .chain(
                cases
                    .iter()
                    .find(|case| case.id == result_case)
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?
                    .fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .map(|field| (Some(result_case), field)),
            )
            .collect(),
        (StructuralTypeShape::FixedArray { length, .. }, None)
            if elements
                .is_some_and(|elements| u64::try_from(elements.len()).ok() == Some(*length)) =>
        {
            Vec::new()
        }
        _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
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
        .map(|(field, (case, declaration))| {
            if field.field != declaration.id || declaration.relevance.is_erased() {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let mut destination = Vec::new();
            if let Some(case) = case {
                destination.push(terminal_psi::StructuralPathSegment::Case(case));
            }
            destination.push(terminal_psi::StructuralPathSegment::Field(
                declaration.identity.clone(),
            ));
            Ok((
                &declaration.field_type,
                destination,
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
                        element_type
                            .as_ref()
                            .ok_or(LoweringError::unsupported_control_flow(function.machine))?,
                        vec![terminal_psi::StructuralPathSegment::FixedIndex(
                            index as u64,
                        )],
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
                // A view member stores its descriptor while its referent stays
                // borrowed. The source declaration below determines whether
                // that descriptor is copied or transferred once.
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
                    field_type => field_type
                        .byte_sequence_carrier()
                        .is_some_and(|carrier| carrier.is_shared_readable_view()),
                };
                let nested = match field_type {
                    StructuralFieldType::Structural(nested) => *nested,
                    _ if byte_carrier.is_some() => {
                        let nested = live
                            .structural_observation_type(function, argument.place)
                            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                        if !types.get(&nested).is_some_and(|declaration| {
                            Some(&declaration.shape)
                                == byte_carrier.map(StructuralTypeShape::ByteSequence).as_ref()
                        }) {
                            return Err(LoweringError::unsupported_control_flow(function.machine));
                        }
                        nested
                    }
                    // The declared `&[u8]` member spells its descriptor as a
                    // standalone borrowed view; the source carries that shape.
                    _ if descriptor_member => {
                        let nested = live
                            .structural_observation_type(function, argument.place)
                            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                        if !types.get(&nested).is_some_and(|declaration| {
                            declaration.shape.is_shared_readable_byte_view()
                        }) {
                            return Err(LoweringError::unsupported_control_flow(function.machine));
                        }
                        nested
                    }
                    _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
                };
                let reference_bearing = super::references::contains_reference(types, nested);
                // Assembly consumes an already established home, not a second
                // constructor selected by the child's shape. Its source and
                // destination layouts retain the exact declared child type;
                // custody below decides copying or transfer. Scalar receiving
                // obligations belong to the producer, just as field obligations
                // do for records, and are not re-proved while copying storage.
                let (selected, multiplicity) =
                    super::aggregate_results::value_input(function, types, live, argument)?;
                if selected != nested {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                // Claims travel with the exact home origin, not its byte
                // layout. Every noncopyable operand transfers once, whether
                // its obligation is affine or linear; construction must not
                // replace the child's claims with a new parent identity.
                if argument.access != StructuralAccess::Owned
                    || (byte_carrier.is_some()
                        && multiplicity != StructuralMultiplicity::Unrestricted)
                    || (multiplicity != StructuralMultiplicity::Unrestricted
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
                    super::references::subtree_leaves(
                        function,
                        types,
                        live,
                        &destination,
                        nested,
                        argument,
                        multiplicity != StructuralMultiplicity::Unrestricted,
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
    super::references::relocate_leaves(live, result.place, relocations, moved_leaves);
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
    let AbstractOperation::EstablishTrivialAffineLocal {
        psi_operation,
        place,
        structural_type,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    if !matches!(
        place.kind,
        semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
            structural_type: declared,
            construction: None,
            ..
        } if declared == structural_type.id
    ) || types.get(&structural_type.id).copied() != Some(structural_type)
        || !matches!(&structural_type.shape, StructuralTypeShape::Record { fields, .. } if fields.is_empty())
        || live.structural_homes.contains_key(&place.id)
        || live
            .trivial_affine_locals
            .insert(place.id, (*psi_operation, structural_type.id))
            .is_some()
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
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
    let AbstractOperation::EstablishStructuralLocal {
        psi_operation,
        result,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let result_home =
        established_result_home(live, psi_operation, result, types, function.machine)?;
    // A declared descriptor local's home is read as a shared view once its
    // whole-slot store installs a descriptor; verification proved that store
    // reaches every read.
    if terminal_semantics::descriptor_results::is_shared_descriptor(
        types.values().copied(),
        result.structural_type,
    ) && live
        .views
        .insert(result.place, (*psi_operation, result.structural_type))
        .is_some()
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
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
    let AbstractOperation::EstablishZeroedStructural {
        psi_operation,
        result,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let result_home =
        established_result_home(live, psi_operation, result, types, function.machine)?;
    operations.push(TargetUnitOperation::EstablishZeroedStructural {
        psi_operation: *psi_operation,
        result_home,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

/// The establishment's aggregate home, registered as live; refused for
/// `machine` when the place already holds one.
fn established_result_home(
    live: &mut LiveDefinitions,
    psi_operation: &semantic_vocabulary::OperationId,
    result: &terminal_psi::StructuralOperationResult,
    types: &StructuralTypeLookup<'_>,
    machine: semantic_vocabulary::MachineId,
) -> Result<target_operations::TargetStructuralHomeRequirement, LoweringError> {
    let result_home = super::aggregate_results::home(*psi_operation, result, types)?;
    if live
        .structural_homes
        .insert(result.place, result_home.clone())
        .is_some()
    {
        return Err(LoweringError::unsupported_control_flow(machine));
    }
    Ok(result_home)
}
