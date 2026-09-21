use super::{
    TerminalInterpretError, TerminalScalarCaseValue, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue, reference,
};
use crate::terminal_interpreter::execution::ExecutableMachine;
use crate::terminal_interpreter::execution::LiveClaim;
use crate::terminal_interpreter::scalar_operations::terminal_scalar_belongs_to_type;
use crate::terminal_interpreter::values::StructuralRuntimePlace;
use semantic_vocabulary::{
    ClaimId, MachineId, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    BoundaryMachineDeclaration, ClaimTransfer, CompletionReceipt, EntryClaim, NominalAffineCleanup,
    StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralResultClaimTransfer, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction,
};

pub(super) fn commit_cleanup_actions(
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    machines: &BTreeMap<MachineId, ExecutableMachine>,
    structural_values: &mut BTreeMap<PlaceId, TerminalStructuralValue>,
    reference_referents: &mut BTreeMap<StructuralRuntimePlace, TerminalStructuralValue>,
    scalar_case_values: &mut BTreeMap<PlaceId, TerminalScalarCaseValue>,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    live_claims: &mut BTreeMap<ClaimId, LiveClaim>,
    actions: &[TerminalAffineCleanupAction],
) -> Result<Vec<(NominalAffineCleanup, TerminalStructuralValue)>, TerminalInterpretError> {
    let mut nominal = Vec::new();
    for action in actions {
        match action {
            TerminalAffineCleanupAction::DiscardRoot(place) => {
                if (reference::discard_structural_value(
                    structural_values,
                    reference_referents,
                    *place,
                )
                .is_none()
                    && scalar_case_values.remove(place).is_none())
                    || !remove_affine_root(frontier, *place)
                {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                live_claims.retain(|_, claim| claim.place != Some(*place));
            }
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                let root = structural_values.get(&discard.place).ok_or(
                    TerminalInterpretError::VerifiedStructuralPlaceMissing(discard.place),
                )?;
                if discard.path.is_empty()
                    || resolve_structural_path_type(
                        structural_types,
                        root.structural_type,
                        &discard.path,
                    )? != discard.structural_type
                    || !frontier.remove(discard)
                {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                if !frontier.iter().any(|entry| entry.place == discard.place) {
                    reference::discard_structural_value(
                        structural_values,
                        reference_referents,
                        discard.place,
                    );
                }
            }
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                let value = structural_values.remove(&cleanup.place).ok_or(
                    TerminalInterpretError::VerifiedStructuralPlaceMissing(cleanup.place),
                )?;
                if value.structural_type != cleanup.structural_type
                    || !machines.contains_key(&cleanup.cleanup_machine)
                {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                nominal.push((cleanup.clone(), value));
                live_claims.retain(|_, claim| claim.place != Some(cleanup.place));
            }
        }
    }
    let pending_nominal = nominal
        .iter()
        .map(|(cleanup, value)| StructuralAffineDiscard {
            place: cleanup.place,
            path: Vec::new(),
            structural_type: value.structural_type,
        })
        .collect::<BTreeSet<_>>();
    if !structural_values.is_empty() || *frontier != pending_nominal || !live_claims.is_empty() {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    }
    Ok(nominal)
}

pub(super) fn bind_arguments(
    parameters: &[terminal_psi::ValueDeclaration],
    arguments: &[TerminalScalarValue],
) -> Result<BTreeMap<ValueId, TerminalScalarValue>, TerminalInterpretError> {
    if arguments.len() != parameters.len() {
        return Err(TerminalInterpretError::ArgumentCount {
            expected: parameters.len(),
            actual: arguments.len(),
        });
    }
    let mut values = BTreeMap::new();
    for (parameter, argument) in parameters.iter().zip(arguments) {
        if parameter.scalar_type != argument.scalar_type() {
            return Err(TerminalInterpretError::ArgumentType {
                value: parameter.id,
                expected: parameter.scalar_type,
                actual: argument.scalar_type(),
            });
        }
        if let TerminalScalarValue::Integer { scalar_type, value } = argument
            && !scalar_type.admits(*value)
        {
            return Err(TerminalInterpretError::ArgumentIntegerOutsideType {
                value: parameter.id,
            });
        }
        values.insert(parameter.id, *argument);
    }
    Ok(values)
}

pub(super) fn bind_boundary_arguments(
    parameters: &[ScalarType],
    arguments: &[TerminalScalarValue],
) -> Result<(), TerminalInterpretError> {
    if arguments.len() != parameters.len()
        || parameters
            .iter()
            .zip(arguments)
            .any(|(parameter, argument)| *parameter != argument.scalar_type())
    {
        return Err(TerminalInterpretError::VerifiedOperationMalformed);
    }
    Ok(())
}

pub(super) fn bind_structural_arguments(
    parameters: &[StructuralParameterDeclaration],
    arguments: &[TerminalStructuralValue],
) -> Result<BTreeMap<PlaceId, TerminalStructuralValue>, TerminalInterpretError> {
    if arguments.len() != parameters.len() {
        return Err(TerminalInterpretError::StructuralArgumentCount {
            expected: parameters.len(),
            actual: arguments.len(),
        });
    }
    let mut values = BTreeMap::new();
    for (argument_index, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
        if argument
            .qualifications
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(TerminalInterpretError::StructuralQualificationsNonCanonical);
        }
        if parameter.structural_type != argument.structural_type {
            return Err(TerminalInterpretError::StructuralArgumentType {
                place: parameter.place,
                expected: parameter.structural_type,
                actual: argument.structural_type,
            });
        }
        if parameter
            .qualifications
            .iter()
            .any(|domain| !argument.qualifications.contains(domain))
        {
            return Err(TerminalInterpretError::StructuralQualificationMissing(
                parameter.place,
            ));
        }
        for (previous_parameter, previous_argument) in parameters[..argument_index]
            .iter()
            .zip(&arguments[..argument_index])
        {
            if previous_argument.opaque_identity != argument.opaque_identity {
                continue;
            }
            let exclusive = matches!(
                previous_parameter.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            ) || matches!(
                parameter.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            );
            let overlapping = previous_argument.path.starts_with(&argument.path)
                || argument.path.starts_with(&previous_argument.path);
            // Multiplicity retains its existing whole-identity restriction.
            // Exclusive access additionally forbids overlapping projected
            // referents, even when both values are unrestricted.
            if previous_parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || (exclusive && overlapping)
            {
                return Err(TerminalInterpretError::StructuralArgumentAliasing(
                    argument.opaque_identity,
                ));
            }
        }
        if values.insert(parameter.place, argument.clone()).is_some() {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
    }
    Ok(values)
}

pub(super) fn bind_structural_primitive_values(
    machine: &ExecutableMachine,
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    structural_values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    arguments: &[TerminalStructuralPrimitiveValue],
) -> Result<
    (
        BTreeMap<StructuralRuntimePlace, TerminalScalarValue>,
        BTreeMap<u32, StructuralRuntimePlace>,
    ),
    TerminalInterpretError,
> {
    let primitive_parameters = machine
        .structural_parameters
        .iter()
        .enumerate()
        .filter_map(|(argument_index, parameter)| {
            matches!(
                structural_types
                    .get(&parameter.structural_type)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::PrimitiveScalar(_))
            )
            .then_some((argument_index as u32, parameter))
        })
        .collect::<BTreeMap<_, _>>();
    if primitive_parameters.len() != arguments.len() {
        return Err(TerminalInterpretError::StructuralPrimitiveValueCount {
            expected: primitive_parameters.len(),
            actual: arguments.len(),
        });
    }

    let mut storage = BTreeMap::new();
    let mut entry_places = BTreeMap::new();
    for argument in arguments {
        let parameter = primitive_parameters
            .get(&argument.argument_index)
            .copied()
            .ok_or(TerminalInterpretError::StructuralPrimitiveValueInvalid {
                argument_index: argument.argument_index,
            })?;
        let Some(StructuralTypeShape::PrimitiveScalar(expected)) = structural_types
            .get(&parameter.structural_type)
            .map(|declaration| &declaration.shape)
        else {
            return Err(TerminalInterpretError::StructuralPrimitiveValueInvalid {
                argument_index: argument.argument_index,
            });
        };
        if argument.value.scalar_type() != *expected
            || !terminal_scalar_belongs_to_type(argument.value)
        {
            return Err(TerminalInterpretError::StructuralPrimitiveValueType {
                argument_index: argument.argument_index,
                expected: *expected,
                actual: argument.value.scalar_type(),
            });
        }
        let view = structural_values.get(&parameter.place).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(parameter.place),
        )?;
        let place = StructuralRuntimePlace::from(view);
        if entry_places
            .insert(argument.argument_index, place.clone())
            .is_some()
            || storage.insert(place, argument.value).is_some()
        {
            return Err(TerminalInterpretError::StructuralPrimitiveValueInvalid {
                argument_index: argument.argument_index,
            });
        }
    }
    Ok((storage, entry_places))
}

pub(super) fn bind_affine_frontier(
    parameters: &[StructuralParameterDeclaration],
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
) -> Result<BTreeSet<StructuralAffineDiscard>, TerminalInterpretError> {
    bind_affine_frontier_types(parameters, |place| {
        values.get(&place).map(|value| value.structural_type)
    })
}

pub(super) fn bind_affine_frontier_types(
    parameters: &[StructuralParameterDeclaration],
    source_type: impl Fn(PlaceId) -> Option<StructuralTypeId>,
) -> Result<BTreeSet<StructuralAffineDiscard>, TerminalInterpretError> {
    let mut frontier = BTreeSet::new();
    // Match verifier frontier reconstruction: a borrowed receiver is present
    // in the signature but is never owned by this machine.
    for parameter in parameters.iter().filter(|parameter| {
        parameter.multiplicity == StructuralMultiplicity::Affine
            && !(parameter.is_self && parameter.access != StructuralAccess::Owned)
    }) {
        let structural_type = source_type(parameter.place).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(parameter.place),
        )?;
        if structural_type != parameter.structural_type
            || !frontier.insert(StructuralAffineDiscard {
                place: parameter.place,
                path: Vec::new(),
                structural_type: parameter.structural_type,
            })
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
    }
    Ok(frontier)
}

pub(super) fn remove_affine_root(
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    place: PlaceId,
) -> bool {
    let Some(root) = frontier
        .iter()
        .find(|entry| entry.place == place && entry.path.is_empty())
        .cloned()
    else {
        return false;
    };
    frontier.remove(&root)
}

pub(super) fn consume_affine_projection(
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    argument: &StructuralArgument,
) -> Result<(), TerminalInterpretError> {
    let root = values.get(&argument.place).ok_or(
        TerminalInterpretError::VerifiedStructuralPlaceMissing(argument.place),
    )?;
    let Some(containing) = frontier
        .iter()
        .find(|entry| {
            entry.place == argument.place && argument.path.starts_with(entry.path.as_slice())
        })
        .cloned()
    else {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    };
    if containing.path.is_empty() && containing.structural_type != root.structural_type {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    }
    frontier.remove(&containing);
    split_affine_frontier_at_projection(structural_types, frontier, containing, &argument.path)
}

pub(super) fn split_affine_frontier_at_projection(
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    current: StructuralAffineDiscard,
    projected_path: &[StructuralPathSegment],
) -> Result<(), TerminalInterpretError> {
    if current.path == projected_path {
        return Ok(());
    }
    let Some(next_segment) = projected_path.get(current.path.len()) else {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    };
    let declaration = structural_types
        .get(&current.structural_type)
        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
    let selected = match &declaration.shape {
        StructuralTypeShape::Record { fields } => {
            let mut selected = None;
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                let terminal_psi::StructuralFieldType::Structural(field_type) = field.field_type
                else {
                    continue;
                };
                let segment = StructuralPathSegment::Field(field.identity.clone());
                let mut path = current.path.clone();
                path.push(segment.clone());
                let child = StructuralAffineDiscard {
                    place: current.place,
                    path,
                    structural_type: field_type,
                };
                if &segment == next_segment {
                    selected = Some(child);
                } else if !frontier.insert(child) {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
            }
            selected
        }
        StructuralTypeShape::FixedArray {
            element,
            length,
        } if matches!(next_segment, StructuralPathSegment::FixedIndex(index) if index < length) => {
            let StructuralPathSegment::FixedIndex(selected_index) = next_segment else {
                unreachable!()
            };
            let mut selected = None;
            for index in 0..*length {
                let mut path = current.path.clone();
                path.push(StructuralPathSegment::FixedIndex(index));
                let child = StructuralAffineDiscard {
                    place: current.place,
                    path,
                    structural_type: *element,
                };
                if index == *selected_index {
                    selected = Some(child);
                } else if !frontier.insert(child) {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
            }
            selected
        }
        StructuralTypeShape::PrimitiveScalar(_)
        | StructuralTypeShape::Reference { .. }
        | StructuralTypeShape::ByteSequence(_)
        | StructuralTypeShape::FixedArray { .. }
        | StructuralTypeShape::Sum { .. }
        | StructuralTypeShape::Mixed { .. } => None,
    }
    .ok_or(TerminalInterpretError::AffineProjectionNotRepresentable)?;
    split_affine_frontier_at_projection(structural_types, frontier, selected, projected_path)
}

pub(super) fn resolve_structural_path_type(
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<StructuralTypeId, TerminalInterpretError> {
    let mut structural_type = root;
    for segment in path {
        let declaration = structural_types
            .get(&structural_type)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        structural_type = match (segment, &declaration.shape) {
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            (
                StructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. },
            ) => {
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())
                    .ok_or(TerminalInterpretError::AffineProjectionNotRepresentable)?;
                match &field.field_type {
                    terminal_psi::StructuralFieldType::Structural(next) => *next,
                    leaf => {
                        let Some(shape) = leaf.canonical_leaf_shape() else {
                            return Err(TerminalInterpretError::AffineProjectionNotRepresentable);
                        };
                        structural_types
                            .iter()
                            .find(|(_, declaration)| declaration.shape == shape)
                            .map(|(id, _)| *id)
                            .ok_or(TerminalInterpretError::AffineProjectionNotRepresentable)?
                    }
                }
            }
            _ => return Err(TerminalInterpretError::AffineProjectionNotRepresentable),
        };
    }
    Ok(structural_type)
}

pub(super) fn bind_entry_claims(
    entry_claims: &[EntryClaim],
    content_entry_claims: &[terminal_psi::ContentEntryClaim],
    parameters: &[StructuralParameterDeclaration],
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
    let mut claims = BTreeMap::new();
    for entry_claim in entry_claims {
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.place == entry_claim.input)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        if parameter.multiplicity == StructuralMultiplicity::Unrestricted
            || !values.contains_key(&parameter.place)
            || claims
                .insert(
                    entry_claim.claim,
                    LiveClaim {
                        place: Some(parameter.place),
                        path: entry_claim.path.clone(),
                        multiplicity: Some(if entry_claim.path.is_empty() {
                            parameter.multiplicity
                        } else {
                            StructuralMultiplicity::Linear
                        }),
                    },
                )
                .is_some()
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
    }
    for entry_claim in content_entry_claims {
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.place == entry_claim.input.root);
        claims.entry(entry_claim.claim).or_insert(LiveClaim {
            place: parameter.map(|_| entry_claim.input.root),
            path: Vec::new(),
            multiplicity: parameter.map(|parameter| parameter.multiplicity),
        });
    }
    Ok(claims)
}

pub(super) fn resolve_structural_arguments(
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    arguments: &[StructuralArgument],
) -> Result<Vec<TerminalStructuralValue>, TerminalInterpretError> {
    arguments
        .iter()
        .map(|argument| {
            let mut value = values.get(&argument.place).cloned().ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(argument.place),
            )?;
            let mut structural_type = value.structural_type;
            for segment in &argument.path {
                let declaration = structural_types
                    .get(&structural_type)
                    .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                structural_type = match (segment, &declaration.shape) {
                    (
                        StructuralPathSegment::FixedIndex(index),
                        StructuralTypeShape::FixedArray { element, length },
                    ) if index < length => *element,
                    (
                        StructuralPathSegment::Field(identity),
                        StructuralTypeShape::Record { fields },
                    ) => {
                        let field = fields
                            .iter()
                            .find(|field| {
                                field.identity == *identity && !field.relevance.is_erased()
                            })
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        match &field.field_type {
                            terminal_psi::StructuralFieldType::Structural(next) => *next,
                            leaf => {
                                // A scalar leaf resolves to the canonical
                                // PrimitiveScalar declaration sharing its
                                // shape — the same referent type a `&T`
                                // shared loan binds. Other leaf carriers
                                // have no structural referent type and fail
                                // closed.
                                let Some(shape) = leaf.canonical_leaf_shape() else {
                                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                                };
                                structural_types
                                    .iter()
                                    .find(|(_, declaration)| declaration.shape == shape)
                                    .map(|(id, _)| *id)
                                    .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?
                            }
                        }
                    }
                    _ => return Err(TerminalInterpretError::VerifiedOperationMalformed),
                };
            }
            value.structural_type = structural_type;
            if !argument.path.is_empty() {
                value.qualifications.clear();
            }
            value.path.extend(argument.path.clone());
            Ok(value)
        })
        .collect()
}

pub(super) fn direct_scalar_field_type(
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
) -> Option<ScalarType> {
    let declaration = structural_types.get(&structural_type)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    fields.iter().find_map(|candidate| {
        (candidate.id == field && !candidate.relevance.is_erased())
            .then_some(&candidate.field_type)
            .and_then(|field_type| match field_type {
                terminal_psi::StructuralFieldType::Scalar(scalar_type) => Some(*scalar_type),
                terminal_psi::StructuralFieldType::BoundedInteger(bounded) => {
                    Some(ScalarType::Integer(bounded.integer_type()))
                }
                terminal_psi::StructuralFieldType::IeeeFloat(format) => {
                    Some(ScalarType::IeeeFloat(*format))
                }
                terminal_psi::StructuralFieldType::ByteSequence(_)
                | terminal_psi::StructuralFieldType::Structural(_)
                | terminal_psi::StructuralFieldType::Erased { .. } => None,
            })
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn transfer_claims(
    caller_claims: &BTreeMap<ClaimId, LiveClaim>,
    _caller_values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    caller_arguments: &[StructuralArgument],
    transfers: &[ClaimTransfer],
    callee_parameters: &[StructuralParameterDeclaration],
    callee_entry_claims: &[EntryClaim],
    callee_content_entry_claims: &[terminal_psi::ContentEntryClaim],
    callee_values: &BTreeMap<PlaceId, TerminalStructuralValue>,
) -> Result<(BTreeMap<ClaimId, LiveClaim>, BTreeMap<ClaimId, LiveClaim>), TerminalInterpretError> {
    if transfers.len() != callee_entry_claims.len() {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }
    if caller_arguments
        .iter()
        .zip(callee_parameters)
        .any(|(argument, parameter)| {
            !argument.path.is_empty()
                && (callee_entry_claims
                    .iter()
                    .any(|claim| claim.input == parameter.place && !claim.path.is_empty())
                    || callee_content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == parameter.place))
        })
    {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }
    let mut expected_by_argument = BTreeMap::<u32, Vec<&EntryClaim>>::new();
    for entry_claim in callee_entry_claims {
        let (index, parameter) = callee_parameters
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.place == entry_claim.input)
            .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
        if parameter.multiplicity == StructuralMultiplicity::Unrestricted
            || !callee_values.contains_key(&parameter.place)
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        expected_by_argument
            .entry(index as u32)
            .or_default()
            .push(entry_claim);
    }
    let mut actual_by_argument = BTreeMap::<u32, Vec<&ClaimTransfer>>::new();
    for transfer in transfers {
        if transfer.argument_index as usize >= caller_arguments.len() {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        actual_by_argument
            .entry(transfer.argument_index)
            .or_default()
            .push(transfer);
    }
    if expected_by_argument.keys().collect::<Vec<_>>()
        != actual_by_argument.keys().collect::<Vec<_>>()
    {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }

    let mut remaining = caller_claims.clone();
    let mut callee_claims = BTreeMap::new();
    for (argument_index, expected) in expected_by_argument {
        let actual = &actual_by_argument[&argument_index];
        if actual.len() != expected.len() {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let caller_place = caller_arguments[argument_index as usize].place;
        let argument_path = &caller_arguments[argument_index as usize].path;
        for (transfer, entry_claim) in actual.iter().zip(expected) {
            let caller_claim = remaining
                .remove(&transfer.claim)
                .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
            let expected_caller_path = argument_path
                .iter()
                .cloned()
                .chain(entry_claim.path.iter().cloned())
                .collect::<Vec<_>>();
            if caller_claim.place != Some(caller_place) || caller_claim.path != expected_caller_path
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            let parameter = callee_parameters
                .get(argument_index as usize)
                .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
            if callee_claims
                .insert(
                    entry_claim.claim,
                    LiveClaim {
                        place: Some(parameter.place),
                        path: entry_claim.path.clone(),
                        multiplicity: Some(if entry_claim.path.is_empty() {
                            parameter.multiplicity
                        } else {
                            StructuralMultiplicity::Linear
                        }),
                    },
                )
                .is_some()
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
        }
    }
    for entry_claim in callee_content_entry_claims {
        callee_claims.entry(entry_claim.claim).or_insert(LiveClaim {
            place: None,
            path: Vec::new(),
            multiplicity: None,
        });
    }
    Ok((remaining, callee_claims))
}

pub(super) fn rebind_structural_result_claims(
    caller_claims: &BTreeMap<ClaimId, LiveClaim>,
    callee_claims: &BTreeMap<ClaimId, LiveClaim>,
    source: PlaceId,
    result: &StructuralOperationResult,
    transfers: &[StructuralResultClaimTransfer],
    returned_claims: &[ClaimId],
) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
    let mut bindings = BTreeMap::new();
    for binding in &result.claims {
        if bindings
            .insert(binding.claim, binding.path.as_slice())
            .is_some()
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
    }
    if bindings.len() != transfers.len() || returned_claims.len() != transfers.len() {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }

    let expected_callee = returned_claims.iter().copied().collect::<BTreeSet<_>>();
    if expected_callee.len() != returned_claims.len() {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }
    let mut mapped_callee = BTreeSet::new();
    let mut mapped_caller = BTreeSet::new();
    let mut rebound = caller_claims.clone();
    for transfer in transfers {
        if !mapped_callee.insert(transfer.callee_claim)
            || !mapped_caller.insert(transfer.caller_claim)
            || !expected_callee.contains(&transfer.callee_claim)
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let Some(returned) = callee_claims.get(&transfer.callee_claim) else {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        };
        let Some(path) = bindings.get(&transfer.caller_claim) else {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        };
        let expected_multiplicity = if path.is_empty() {
            result.multiplicity
        } else {
            StructuralMultiplicity::Linear
        };
        if returned.place != Some(source)
            || returned.path != *path
            || returned.multiplicity != Some(expected_multiplicity)
            || rebound
                .insert(
                    transfer.caller_claim,
                    LiveClaim {
                        place: Some(result.place),
                        path: path.to_vec(),
                        multiplicity: Some(expected_multiplicity),
                    },
                )
                .is_some()
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
    }
    if mapped_callee != expected_callee
        || mapped_caller != bindings.keys().copied().collect::<BTreeSet<_>>()
    {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }
    Ok(rebound)
}

pub(super) fn validate_boundary_requirements(
    boundary: &BoundaryMachineDeclaration,
    arguments: &[TerminalStructuralValue],
) -> Result<(), TerminalInterpretError> {
    for requirement in &boundary.requires {
        let argument = arguments
            .get(requirement.argument_index as usize)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        if !argument.qualifications.contains(&requirement.domain) {
            return Err(TerminalInterpretError::BoundaryQualificationMissing {
                boundary: boundary.id,
                argument_index: requirement.argument_index,
                domain: requirement.domain,
            });
        }
    }
    Ok(())
}

pub(super) fn complete_claims(
    caller_claims: &BTreeMap<ClaimId, LiveClaim>,
    caller_arguments: &[StructuralArgument],
    receipts: &[CompletionReceipt],
    _boundary_parameters: &[StructuralParameterDeclaration],
) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
    let expected = caller_arguments
        .iter()
        .enumerate()
        .flat_map(|(index, argument)| {
            caller_claims.iter().filter_map(move |(claim, live)| {
                (live.place == Some(argument.place)
                    && (argument.path.is_empty() || live.path == argument.path))
                    .then_some((index as u32, *claim))
            })
        })
        .collect::<BTreeSet<_>>();
    let mut remaining = caller_claims.clone();
    let mut actual = BTreeSet::new();
    for receipt in receipts {
        if !actual.insert((receipt.argument_index, receipt.claim))
            || !expected.contains(&(receipt.argument_index, receipt.claim))
        {
            return Err(TerminalInterpretError::CompletionReceiptMismatch);
        }
        let argument = caller_arguments
            .get(receipt.argument_index as usize)
            .ok_or(TerminalInterpretError::CompletionReceiptMismatch)?;
        let claim = remaining
            .remove(&receipt.claim)
            .ok_or(TerminalInterpretError::CompletionReceiptMismatch)?;
        if claim.place != Some(argument.place)
            || (!argument.path.is_empty() && claim.path != argument.path)
        {
            return Err(TerminalInterpretError::CompletionReceiptMismatch);
        }
    }
    if actual != expected {
        return Err(TerminalInterpretError::CompletionReceiptMismatch);
    }
    Ok(remaining)
}

pub(super) fn has_live_linear_claims(claims: &BTreeMap<ClaimId, LiveClaim>) -> bool {
    claims
        .values()
        .any(|claim| claim.multiplicity == Some(StructuralMultiplicity::Linear))
}
