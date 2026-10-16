//! Runtime custody operates on the source-selected cleanup schedule. A nominal
//! hook borrows its receiver; it does not dispose the receiver's descendants.
//! The checked partition marks retaining hooks explicitly: their descendant
//! actions can follow on a later selected edge, not merely the next action.
//! The checked dying-owner partition publishes home retirement explicitly.
//! A replacement disposes a selected region while retaining its parent. Reserved home
//! geometry can include fields that never became values, so it cannot decide
//! which children owe cleanup. Captured outgoing projections retain their
//! backing independently of that obsolete root binding.

mod field_stores;
mod windows;

#[cfg(test)]
mod tests;

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::LiveClaim;
use crate::reference;
use crate::scalar_operations::terminal_scalar_belongs_to_type;
use crate::values::StructuralRuntimePlace;
use crate::values::{
    TerminalScalarValue, TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use semantic_vocabulary::{
    ClaimId, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::TerminalMachine;
use terminal_psi::{
    BoundaryMachineDeclaration, ClaimTransfer, CompletionReceipt, EntryClaim, NominalAffineCleanup,
    StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralResultClaimBinding, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction, find_structural_case,
};

/// Bind one authenticated completed-result claim into a live claim map. A binding inhabiting
/// a non-empty result path carries Linear multiplicity — the path itself
/// already resolves the claim to exactly one value.
pub(crate) fn bind_result_claim(
    live_claims: &mut BTreeMap<ClaimId, crate::execution::LiveClaim>,
    binding: &StructuralResultClaimBinding,
    place: PlaceId,
    multiplicity: StructuralMultiplicity,
) -> Result<(), TerminalInterpretError> {
    if live_claims
        .insert(
            binding.claim,
            crate::execution::LiveClaim {
                place: Some(place),
                path: binding.path.clone(),
                multiplicity: Some(if binding.path.is_empty() {
                    multiplicity
                } else {
                    StructuralMultiplicity::Linear
                }),
            },
        )
        .is_some()
    {
        return Err(malformed());
    }
    Ok(())
}

pub(crate) fn commit_cleanup_action(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::structural_inputs::case_membership::StructuralCaseContents,
    >,
    machines: &crate::program::ProgramDeclarations<'_, TerminalMachine>,
    structural_values: &mut BTreeMap<PlaceId, TerminalStructuralValue>,
    reference_referents: &mut BTreeMap<StructuralRuntimePlace, crate::reference::ReferenceReferent>,
    element_views: &mut BTreeMap<StructuralRuntimePlace, crate::element_views::ElementView>,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    live_claims: &mut BTreeMap<ClaimId, LiveClaim>,
    action: &TerminalAffineCleanupAction,
) -> Result<Option<(NominalAffineCleanup, TerminalStructuralValue)>, TerminalInterpretError> {
    match action {
        TerminalAffineCleanupAction::DiscardRoot(place) => {
            if live_claims
                .values()
                .any(|claim| claim.place == Some(*place))
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            if reference::discard_structural_value(
                structural_values,
                reference_referents,
                element_views,
                *place,
            )
            .is_none()
                || !remove_affine_root(frontier, *place)
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            live_claims.retain(|_, claim| claim.place != Some(*place));
        }
        TerminalAffineCleanupAction::DiscardResidual(discard) => {
            let root = crate::custody::verified_structural_place(structural_values, discard.place)?;
            // Region disposal leaves its home binding available for the checked
            // retirement action, including when the region is the whole value.
            // DiscardRoot is the separate intrinsically retiring operation.
            consume_affine_discard(
                structural_types,
                structural_values,
                structural_cases,
                frontier,
                live_claims,
                discard,
            )?;
            // A projected transfer may already retain descendants of this root.
            // Retire only this disposed subtree, never the transferred sibling.
            let path = project_structural_value(structural_types, root, &discard.path)?.path;
            reference_referents.retain(|carrier, _| {
                carrier.opaque_identity != root.opaque_identity || !carrier.path.starts_with(&path)
            });
            element_views.retain(|carrier, _| {
                carrier.opaque_identity != root.opaque_identity || !carrier.path.starts_with(&path)
            });
        }
        TerminalAffineCleanupAction::RetireHome(place) => {
            retire_cleanup_home(structural_values, frontier, live_claims, *place)?;
        }
        TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
            validate_nominal_cleanup_action(
                structural_types,
                structural_cases,
                machines,
                structural_values,
                frontier,
                live_claims,
                cleanup,
            )?;
            let root = crate::custody::verified_structural_place(structural_values, cleanup.place)?;
            let value = project_structural_value(structural_types, root, &cleanup.path)?;
            // A loan into the selected owner prevents its disposition. An
            // owned descriptor inside it is instead a descendant: the parent
            // hook borrows the intact carrier before the ordered child actions
            // dispose those descriptors, never their external referents.
            if reference_referents.values().any(|referent| {
                referent.view.opaque_identity == value.opaque_identity
                    && terminal_semantics::structural_paths_may_overlap(
                        &referent.view.path,
                        &value.path,
                    )
            }) {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            begin_nominal_cleanup(
                structural_types,
                structural_values,
                structural_cases,
                frontier,
                live_claims,
                cleanup,
            )?;
            return Ok(Some((cleanup.clone(), value)));
        }
    }
    Ok(None)
}

/// Begin an owner-authorized disposition before lending its intact value.
/// A linear parent's claim covers its contents once, not one extra claim per
/// transparent child. Once that claim is consumed, retain the selected region
/// as structural cleanup debt until the subsequent child actions finish. This
/// reservation cannot transfer a value or mint a claim; retirement still requires
/// the complete ordered disposal partition.
pub(crate) fn begin_nominal_cleanup(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::structural_inputs::case_membership::StructuralCaseContents,
    >,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    claims: &mut BTreeMap<ClaimId, LiveClaim>,
    cleanup: &NominalAffineCleanup,
) -> Result<(), TerminalInterpretError> {
    let retains_descendants = cleanup.retains_descendants;
    if cleanup.consumed_claims.is_empty() && !retains_descendants {
        consume_affine_projection(
            structural_types,
            values,
            structural_cases,
            frontier,
            &StructuralArgument {
                place: cleanup.place,
                path: cleanup.path.clone(),
                access: StructuralAccess::Owned,
            },
        )?;
        frontier.insert(StructuralAffineDiscard {
            place: cleanup.place,
            path: cleanup.path.clone(),
            structural_type: cleanup.structural_type,
        });
    }
    for claim in &cleanup.consumed_claims {
        claims.remove(claim);
    }
    if retains_descendants && !cleanup.consumed_claims.is_empty() {
        if !frontier.insert(StructuralAffineDiscard {
            place: cleanup.place,
            path: cleanup.path.clone(),
            structural_type: cleanup.structural_type,
        }) {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        exclude_linear_regions(
            structural_types,
            frontier,
            claims
                .values()
                .filter(|claim| {
                    claim.place == Some(cleanup.place) && claim.path.starts_with(&cleanup.path)
                })
                .map(|claim| (cleanup.place, claim.path.as_slice())),
            &|current| {
                crate::structural_inputs::case_membership::stored_case_at_projection(
                    structural_types,
                    values.get(&current.place)?,
                    structural_cases,
                    &current.path,
                )
            },
        )?;
    }
    Ok(())
}

/// Validate the complete cleanup transaction before the value is lent. The
/// hook designation is declaration metadata, never inferred from its use here.
pub(crate) fn validate_nominal_cleanup_action(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::structural_inputs::case_membership::StructuralCaseContents,
    >,
    machines: &crate::program::ProgramDeclarations<'_, TerminalMachine>,
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    frontier: &BTreeSet<StructuralAffineDiscard>,
    claims: &BTreeMap<ClaimId, LiveClaim>,
    cleanup: &NominalAffineCleanup,
) -> Result<(), TerminalInterpretError> {
    let invalid = || TerminalInterpretError::AffineFrontierMismatch;
    let root = values.get(&cleanup.place).ok_or_else(invalid)?;
    let value = project_structural_value(structural_types, root, &cleanup.path)?;
    let target = machines.get(&cleanup.cleanup_machine).ok_or_else(invalid)?;
    if !structural_types.contains_key(&cleanup.structural_type) {
        return Err(invalid());
    }
    let retains_descendants = cleanup.retains_descendants;
    let source_claims = claims
        .iter()
        .filter(|(identity, claim)| {
            claim.place == Some(cleanup.place)
                && if retains_descendants {
                    cleanup.consumed_claims.contains(identity)
                } else {
                    terminal_semantics::structural_paths_may_overlap(&claim.path, &cleanup.path)
                }
        })
        .collect::<Vec<_>>();
    if value.structural_type != cleanup.structural_type
        || target.attachment != Some(cleanup.structural_type)
        || target.nominal_cleanup_subject != Some(cleanup.structural_type)
        || machines
            .values()
            .filter(|candidate| candidate.nominal_cleanup_subject == Some(cleanup.structural_type))
            .count()
            != 1
        || target.result != terminal_psi::TerminalMachineResult::Unit
        || !target.parameters.is_empty()
        || !target.entry_claims.is_empty()
        || !target.content_entry_claims.is_empty()
        || target.structural_parameters.len() > 1
        || target
            .structural_parameters
            .iter()
            .any(|parameter| !parameter.is_cleanup_receiver(cleanup))
        || cleanup.consumed_claims
            != source_claims
                .iter()
                .map(|(identity, _)| **identity)
                .collect::<Vec<_>>()
        || source_claims.iter().any(|(_, claim)| {
            !claim.path.starts_with(&cleanup.path)
                || claim.multiplicity != Some(StructuralMultiplicity::Linear)
        })
    {
        return Err(invalid());
    }
    // Only a leaf action disposes a region. A parent hook borrows its intact
    // receiver and leaves descendant obligations for the following actions.
    if !retains_descendants {
        if source_claims.is_empty() {
            let mut remaining = frontier.clone();
            consume_affine_projection(
                structural_types,
                values,
                structural_cases,
                &mut remaining,
                &StructuralArgument {
                    place: cleanup.place,
                    path: cleanup.path.clone(),
                    access: StructuralAccess::Owned,
                },
            )
            .map_err(|_| invalid())?;
        } else if frontier.iter().any(|entry| {
            entry.place == cleanup.place
                && terminal_semantics::structural_paths_may_overlap(&entry.path, &cleanup.path)
        }) {
            return Err(invalid());
        }
    }
    Ok(())
}

/// A hook returns to the same pending subject. Remove its reservation only;
/// the surrounding carrier remains until its last sibling obligation ends.
pub(crate) fn complete_nominal_cleanup(
    values: &mut BTreeMap<PlaceId, TerminalStructuralValue>,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    claims: &BTreeMap<ClaimId, LiveClaim>,
    cleanup: &NominalAffineCleanup,
) -> Result<(), TerminalInterpretError> {
    if cleanup.retains_descendants {
        return Ok(());
    }
    if if cleanup.consumed_claims.is_empty() {
        !frontier.remove(&StructuralAffineDiscard {
            place: cleanup.place,
            path: cleanup.path.clone(),
            structural_type: cleanup.structural_type,
        })
    } else {
        frontier.iter().any(|entry| {
            entry.place == cleanup.place
                && terminal_semantics::structural_paths_may_overlap(&entry.path, &cleanup.path)
        }) || cleanup
            .consumed_claims
            .iter()
            .any(|claim| claims.contains_key(claim))
    } {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    }
    // A whole nominal leaf action consumes the home intrinsically, as a whole
    // discard does. Projected hooks only consume their selected region: the
    // checked dying-owner partition supplies a separate RetireHome when needed.
    if cleanup.path.is_empty() {
        retire_cleanup_home(values, frontier, claims, cleanup.place)?;
    }
    Ok(())
}

/// Retire only the explicitly selected dying home. Its checked disposal partition
/// has finished, so reserved geometry for never-established fields is not debt.
/// This event is not a wildcard disposal, claim discharge, or loan restoration;
/// backing retained by captured outgoing values survives the frame binding.
pub(crate) fn retire_cleanup_home(
    values: &mut BTreeMap<PlaceId, TerminalStructuralValue>,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    claims: &BTreeMap<ClaimId, LiveClaim>,
    place: PlaceId,
) -> Result<(), TerminalInterpretError> {
    if claims.values().any(|claim| claim.place == Some(place)) {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    }
    if values.remove(&place).is_none() {
        return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
            place,
        ));
    }
    frontier.retain(|entry| entry.place != place);
    Ok(())
}

pub(crate) fn bind_arguments(
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

pub(crate) fn bind_boundary_arguments(
    parameters: &[ScalarType],
    arguments: &[TerminalScalarValue],
) -> Result<(), TerminalInterpretError> {
    if arguments.len() != parameters.len()
        || parameters
            .iter()
            .zip(arguments)
            .any(|(parameter, argument)| *parameter != argument.scalar_type())
    {
        return Err(malformed());
    }
    Ok(())
}

/// A parameter declares the possible projected roster; only authenticated active
/// case paths require evidence for this invocation. Entry binding without case
/// backing continues to use the strict binder below.
pub(crate) fn bind_active_structural_arguments(
    parameters: &[StructuralParameterDeclaration],
    arguments: &[TerminalStructuralValue],
    types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::structural_inputs::case_membership::StructuralCaseContents,
    >,
) -> Result<BTreeMap<PlaceId, TerminalStructuralValue>, TerminalInterpretError> {
    let active = active_structural_parameters(parameters, arguments, types, cases)?;
    bind_structural_arguments(&active, arguments)
}

/// An internal edge relabels already established storage in one activation.
/// A whole owner may stay live beside a reborrow of that same source place;
/// this does not supply a second owned value or authorize two exclusive loans.
/// Entry and ordinary call binding keep the strict independent-input rule.
pub(crate) fn bind_active_block_arguments(
    parameters: &[StructuralParameterDeclaration],
    sources: &[terminal_psi::StructuralArgument],
    arguments: &[TerminalStructuralValue],
    source_parameters: &[Option<&StructuralParameterDeclaration>],
    types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::structural_inputs::case_membership::StructuralCaseContents,
    >,
) -> Result<BTreeMap<PlaceId, TerminalStructuralValue>, TerminalInterpretError> {
    if sources.len() != parameters.len() || source_parameters.len() != parameters.len() {
        return Err(malformed());
    }
    let active = active_structural_parameters(parameters, arguments, types, cases)?;
    bind_existing_structural_places(&active, arguments, sources, source_parameters)
}

fn active_structural_parameters(
    parameters: &[StructuralParameterDeclaration],
    arguments: &[TerminalStructuralValue],
    types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::structural_inputs::case_membership::StructuralCaseContents,
    >,
) -> Result<Vec<StructuralParameterDeclaration>, TerminalInterpretError> {
    if parameters.len() != arguments.len() {
        return Err(TerminalInterpretError::StructuralArgumentCount {
            expected: parameters.len(),
            actual: arguments.len(),
        });
    }
    let mut active_parameters = parameters.to_vec();
    for (parameter, argument) in active_parameters.iter_mut().zip(arguments) {
        let mut active = Vec::new();
        for row in &parameter.projected_qualifications {
            if crate::structural_inputs::case_membership::case_projection_in_value_is_active(
                types, argument, cases, &row.path,
            )? {
                active.push(row.clone());
            }
        }
        parameter.projected_qualifications = active;
    }
    Ok(active_parameters)
}

pub(crate) fn bind_structural_arguments(
    parameters: &[StructuralParameterDeclaration],
    arguments: &[TerminalStructuralValue],
) -> Result<BTreeMap<PlaceId, TerminalStructuralValue>, TerminalInterpretError> {
    bind_existing_structural_places(parameters, arguments, &[], &[])
}

/// A fresh reborrow names the owner's actual source place. A forwarded loan
/// names its already admitted borrow parameter instead; its accompanying owner
/// is still the same live affine source. The binder separately joins runtime
/// identities and paths. Copies, linear custody and two loans do not form this
/// relation, and independent inputs supply no source-parameter roster.
fn retains_source_owner(
    parameters: &[StructuralParameterDeclaration],
    arguments: &[TerminalStructuralValue],
    sources: &[terminal_psi::StructuralArgument],
    source_parameters: &[Option<&StructuralParameterDeclaration>],
    first: usize,
    second: usize,
) -> bool {
    let (owner, loan) = match (parameters[first].access, parameters[second].access) {
        (StructuralAccess::Owned, access) if access != StructuralAccess::Owned => (first, second),
        (access, StructuralAccess::Owned) if access != StructuralAccess::Owned => (second, first),
        _ => return false,
    };
    let Some(owner_source) = sources.get(owner) else {
        return false;
    };
    let Some(loan_source) = sources.get(loan) else {
        return false;
    };
    parameters[owner].multiplicity == StructuralMultiplicity::Affine
        && arguments[loan].path.starts_with(&arguments[owner].path)
        && owner_source.access == StructuralAccess::Owned
        && owner_source.path.is_empty()
        && loan_source.access == parameters[loan].access
        && (owner_source.place == loan_source.place
            || (source_parameters
                .get(owner)
                .copied()
                .flatten()
                .is_some_and(|source| {
                    source.access == StructuralAccess::Owned
                        && source.multiplicity == StructuralMultiplicity::Affine
                })
                && source_parameters
                    .get(loan)
                    .copied()
                    .flatten()
                    .is_some_and(|source| {
                        source.access != StructuralAccess::Owned
                            && source.multiplicity == StructuralMultiplicity::Unrestricted
                            && crate::reference::access_covers(source.access, loan_source.access)
                    })))
}

fn bind_existing_structural_places(
    parameters: &[StructuralParameterDeclaration],
    arguments: &[TerminalStructuralValue],
    sources: &[terminal_psi::StructuralArgument],
    source_parameters: &[Option<&StructuralParameterDeclaration>],
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
            || argument
                .projected_qualifications
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
            || parameter
                .projected_qualifications
                .iter()
                .any(|row| !argument.projected_qualifications.contains(row))
        {
            return Err(TerminalInterpretError::StructuralQualificationMissing(
                parameter.place,
            ));
        }
        for (previous_index, (previous_parameter, previous_argument)) in parameters
            [..argument_index]
            .iter()
            .zip(&arguments[..argument_index])
            .enumerate()
        {
            if previous_argument.opaque_identity != argument.opaque_identity {
                continue;
            }
            let exclusive = matches!(
                previous_parameter.access,
                terminal_psi::exclusive_borrow_pair_pattern!()
            ) || matches!(
                parameter.access,
                terminal_psi::exclusive_borrow_pair_pattern!()
            );
            let overlapping = terminal_semantics::structural_paths_may_overlap(
                &previous_argument.path,
                &argument.path,
            );
            // Disjoint members of one root are independent values, whatever
            // their multiplicity. Overlapping referents alias when either
            // value carries custody or either access is exclusive; only
            // shared reads of copyable storage may overlap.
            let custody = previous_parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted;
            if overlapping
                && (custody || exclusive)
                && !retains_source_owner(
                    parameters,
                    arguments,
                    sources,
                    source_parameters,
                    previous_index,
                    argument_index,
                )
            {
                return Err(TerminalInterpretError::StructuralArgumentAliasing(
                    argument.opaque_identity,
                ));
            }
        }
        if values.insert(parameter.place, argument.clone()).is_some() {
            return Err(malformed());
        }
    }
    Ok(values)
}

pub(crate) fn bind_structural_primitive_values(
    machine: &TerminalMachine,
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    structural_values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    arguments: &[TerminalStructuralPrimitiveValue],
    mut storage: BTreeMap<StructuralRuntimePlace, TerminalScalarValue>,
    zero_initialized_arguments: &[u32],
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
    let expected = primitive_parameters
        .keys()
        .filter(|position| !zero_initialized_arguments.contains(position))
        .count();
    if expected != arguments.len() {
        return Err(TerminalInterpretError::StructuralPrimitiveValueCount {
            expected,
            actual: arguments.len(),
        });
    }

    let mut entry_places = BTreeMap::new();
    for (position, parameter) in &primitive_parameters {
        if zero_initialized_arguments.contains(position) {
            let root = structural_values
                .get(&parameter.place)
                .ok_or_else(malformed)?;
            let place = StructuralRuntimePlace::from(root);
            if !storage.contains_key(&place) {
                return Err(malformed());
            }
            entry_places.insert(*position, place);
        }
    }
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
        let view = crate::custody::verified_structural_place(structural_values, parameter.place)?;
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

pub(crate) fn bind_affine_frontier(
    parameters: &[StructuralParameterDeclaration],
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
) -> Result<BTreeSet<StructuralAffineDiscard>, TerminalInterpretError> {
    bind_affine_frontier_types(parameters, structural_types, |place| {
        values.get(&place).map(|value| value.structural_type)
    })
}

pub(crate) fn bind_affine_frontier_types(
    parameters: &[StructuralParameterDeclaration],
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    source_type: impl Fn(PlaceId) -> Option<StructuralTypeId>,
) -> Result<BTreeSet<StructuralAffineDiscard>, TerminalInterpretError> {
    let mut frontier = BTreeSet::new();
    // Owned values and affine descriptor carriers incur disposal debt. A
    // descriptor's root is its carrier, never its borrowed elements; ordinary
    // address loans still owe no disposal of the referent they identify.
    for parameter in parameters.iter().filter(|parameter| {
        parameter.multiplicity == StructuralMultiplicity::Affine
            && (parameter.access == StructuralAccess::Owned
                || terminal_semantics::view_descriptor_parameter(
                    parameter,
                    structural_types.values(),
                ))
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
            return Err(malformed());
        }
    }
    Ok(frontier)
}

pub(crate) fn remove_affine_root(
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

/// The affine debt one whole owned value carries: its root minus the linear
/// regions its own claims cover, or nothing when it is not affine. A value
/// moved or returned whole carries exactly these rows; any other row on its
/// place means part of it was already consumed or disposed.
pub(crate) fn whole_value_frontier<'a>(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    place: PlaceId,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    claim_paths: impl IntoIterator<Item = &'a [StructuralPathSegment]>,
    case_membership: &impl Fn(&StructuralAffineDiscard) -> Option<semantic_vocabulary::StructuralCaseId>,
) -> Result<BTreeSet<StructuralAffineDiscard>, TerminalInterpretError> {
    let mut frontier = BTreeSet::new();
    if multiplicity == StructuralMultiplicity::Affine {
        frontier.insert(StructuralAffineDiscard {
            place,
            path: Vec::new(),
            structural_type,
        });
        exclude_linear_regions(
            structural_types,
            &mut frontier,
            claim_paths.into_iter().map(|path| (place, path)),
            case_membership,
        )?;
    }
    Ok(frontier)
}

/// Fresh affine storage owes only the complement of its linear regions.
/// Invocation entry and value completion share this partition; a claim can
/// never overlap a second whole-root affine disposal obligation.
pub(crate) fn exclude_linear_regions<'a>(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    claims: impl IntoIterator<Item = (PlaceId, &'a [StructuralPathSegment])>,
    case_membership: &impl Fn(&StructuralAffineDiscard) -> Option<semantic_vocabulary::StructuralCaseId>,
) -> Result<(), TerminalInterpretError> {
    let mut claims = claims.into_iter();
    let Some(first_claim) = claims.next() else {
        return Ok(());
    };
    let affine_roots = frontier
        .iter()
        .map(|entry| entry.place)
        .collect::<BTreeSet<_>>();
    for (place, path) in std::iter::once(first_claim).chain(claims) {
        if !affine_roots.contains(&place) {
            continue;
        }
        let containing = frontier
            .iter()
            .find(|entry| entry.place == place && path.starts_with(&entry.path))
            .cloned()
            .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
        frontier.remove(&containing);
        // Guarded claims split only the payload in actual stored membership.
        // A missing discriminator is an error, never proof of an inactive row.
        split_affine_frontier_at_projection(
            structural_types,
            frontier,
            containing,
            path,
            case_membership,
        )?;
    }
    Ok(())
}

/// Capture only the claims contained in a selected owned subtree. A containing
/// nominal claim is indivisible; moving one of its fields cannot split it.
pub(crate) fn projected_ownership_claims(
    claims: &BTreeMap<ClaimId, LiveClaim>,
    argument: &StructuralArgument,
) -> Result<Vec<(ClaimId, LiveClaim)>, TerminalInterpretError> {
    let mut selected = Vec::new();
    for (identity, claim) in claims {
        if claim.place != Some(argument.place) {
            continue;
        }
        if let Some(suffix) = claim.path.strip_prefix(argument.path.as_slice()) {
            let mut moved = claim.clone();
            moved.path = suffix.to_vec();
            selected.push((*identity, moved));
        } else if argument.path.starts_with(&claim.path) {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
    }
    Ok(selected)
}

/// Relocate an owned projection before its source's residual cleanup. Linear
/// regions were already excluded from the affine frontier: they move by claim
/// identity, not by pretending to be an affine discard. An affine aggregate can
/// carry both kinds of custody, so all contained complement regions move too.
/// Missing affine custody is legal only when an authenticated claim moves.
pub(crate) fn consume_projected_ownership(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::structural_inputs::case_membership::StructuralCaseContents,
    >,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    claims: &mut BTreeMap<ClaimId, LiveClaim>,
    argument: &StructuralArgument,
) -> Result<(), TerminalInterpretError> {
    if argument.access != StructuralAccess::Owned || argument.path.is_empty() {
        return Err(malformed());
    }
    let selected_claims = projected_ownership_claims(claims, argument)?;
    let affine = consume_projected_affine_complement(
        structural_types,
        values,
        structural_cases,
        frontier,
        argument,
    )?;
    if !affine && selected_claims.is_empty() {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    }
    for (identity, _) in selected_claims {
        claims.remove(&identity);
    }
    Ok(())
}

/// Stage an owned projection's affine complement without changing its claims.
/// Excluding a contained linear claim can split the complement into descendant
/// rows. Those rows move together with the selected carrier, while claim
/// identities remain the responsibility of the caller's transfer transaction.
/// Disposal still requires the intact-region check in consume_affine_projection.
pub(crate) fn consume_projected_affine_complement(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::structural_inputs::case_membership::StructuralCaseContents,
    >,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    argument: &StructuralArgument,
) -> Result<bool, TerminalInterpretError> {
    if argument.access != StructuralAccess::Owned || argument.path.is_empty() {
        return Err(malformed());
    }
    crate::structural_inputs::case_membership::validate_case_projection(
        structural_types,
        values,
        structural_cases,
        argument.place,
        &argument.path,
    )?;
    let containing = frontier
        .iter()
        .any(|entry| entry.place == argument.place && argument.path.starts_with(&entry.path));
    let contained = frontier
        .iter()
        .any(|entry| entry.place == argument.place && entry.path.starts_with(&argument.path));
    if containing {
        consume_affine_projection(
            structural_types,
            values,
            structural_cases,
            frontier,
            argument,
        )?;
    } else if contained {
        frontier.retain(|entry| {
            entry.place != argument.place || !entry.path.starts_with(&argument.path)
        });
    }
    Ok(containing || contained)
}

/// Disposal carries no claim transfer: both preflight and commit require the
/// exact region's live claims to have been discharged by preceding actions.
/// Runtime membership and frontier splitting retain selected case custody.
pub(crate) fn consume_affine_discard(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::structural_inputs::case_membership::StructuralCaseContents,
    >,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    claims: &BTreeMap<ClaimId, LiveClaim>,
    discard: &StructuralAffineDiscard,
) -> Result<(), TerminalInterpretError> {
    let root = verified_structural_place(values, discard.place)?;
    if resolve_structural_path_type(structural_types, root.structural_type, &discard.path)?
        != discard.structural_type
        || claims.values().any(|claim| {
            claim.place == Some(discard.place)
                && terminal_semantics::structural_paths_may_overlap(&claim.path, &discard.path)
        })
    {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    }
    consume_affine_projection(
        structural_types,
        values,
        structural_cases,
        frontier,
        &StructuralArgument {
            place: discard.place,
            path: discard.path.clone(),
            access: StructuralAccess::Owned,
        },
    )
}

pub(crate) fn consume_affine_projection(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::structural_inputs::case_membership::StructuralCaseContents,
    >,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    argument: &StructuralArgument,
) -> Result<(), TerminalInterpretError> {
    let root = crate::custody::verified_structural_place(values, argument.place)?;
    crate::structural_inputs::case_membership::validate_case_projection(
        structural_types,
        values,
        structural_cases,
        argument.place,
        &argument.path,
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
    let case_membership = |current: &StructuralAffineDiscard| {
        crate::structural_inputs::case_membership::stored_case_at_projection(
            structural_types,
            root,
            structural_cases,
            &current.path,
        )
    };
    split_affine_frontier_at_projection(
        structural_types,
        frontier,
        containing,
        &argument.path,
        &case_membership,
    )
}

/// Inline view fields own descriptors even though their backing is borrowed.
/// Resolve their disposal geometry without assigning debt to scalar fields or
/// bounded owned bytes, and never silently omit an unresolved descriptor.
fn cleanup_field_type(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    field: &terminal_psi::StructuralFieldType,
) -> Result<Option<StructuralTypeId>, TerminalInterpretError> {
    if let Some(structural) = field.as_structural() {
        return Ok(Some(structural));
    }
    match field.byte_sequence_carrier() {
        Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }) | None => Ok(None),
        Some(carrier) => {
            terminal_semantics::byte_carrier_field_declaration(structural_types.values(), carrier)
                .map(|declaration| Some(declaration.id))
                .ok_or(TerminalInterpretError::AffineProjectionNotRepresentable)
        }
    }
}

pub(crate) fn split_affine_frontier_at_projection(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    current: StructuralAffineDiscard,
    projected_path: &[StructuralPathSegment],
    case_membership: &impl Fn(&StructuralAffineDiscard) -> Option<semantic_vocabulary::StructuralCaseId>,
) -> Result<(), TerminalInterpretError> {
    if current.path == projected_path {
        return Ok(());
    }
    let Some(next_segment) = projected_path.get(current.path.len()) else {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    };
    let declaration = structural_types
        .get(&current.structural_type)
        .ok_or(malformed())?;
    let selected = match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => {
            let mut selected = None;
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                let Some(field_type) = cleanup_field_type(structural_types, &field.field_type)? else {
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
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
            let active_case = case_membership(&current).ok_or_else(malformed)?;
            let selected_case = find_structural_case(cases, active_case).ok_or_else(malformed)?;
            let mut selected = None;
            // Common members have their ordinary paths. Payload members live
            // only below this exact Case namespace; the virtual Case itself
            // owns no second obligation and adds no physical backing prefix.
            for (case, fields) in declaration.shape.direct_fields().map(|fields| (None, fields))
                .into_iter().chain(std::iter::once((Some(selected_case.id), selected_case.fields.as_slice())))
            {
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    let Some(field_type) = cleanup_field_type(structural_types, &field.field_type)? else {
                        continue;
                    };
                    let segment = StructuralPathSegment::Field(field.identity.clone());
                    let mut path = current.path.clone();
                    if let Some(case) = case {
                        path.push(StructuralPathSegment::Case(case));
                    }
                    path.push(segment.clone());
                    let child = StructuralAffineDiscard { place: current.place, path, structural_type: field_type };
                    if projected_path.starts_with(&child.path) {
                        selected = Some(child);
                    } else if !frontier.insert(child) {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                }
            }
            selected
        }
        StructuralTypeShape::BindingCarrier
        | StructuralTypeShape::PrimitiveScalar(_)
        | StructuralTypeShape::Reference { .. }
        | StructuralTypeShape::ByteSequence(_)
        | StructuralTypeShape::ElementView { .. }
        | StructuralTypeShape::FixedArray { .. } => None,
    }
    .ok_or(TerminalInterpretError::AffineProjectionNotRepresentable)?;
    split_affine_frontier_at_projection(
        structural_types,
        frontier,
        selected,
        projected_path,
        case_membership,
    )
}

pub(crate) fn resolve_structural_path_type(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<StructuralTypeId, TerminalInterpretError> {
    resolve_structural_path_type_with_leaf(
        structural_types,
        root,
        path,
        terminal_psi::StructuralFieldType::canonical_leaf_type,
    )
}

/// Collection execution needs the selected carrier, not a catalog alias for an
/// inline field. Equal-capacity qualified owners remain distinct declarations;
/// the original root and path continue to identify the live backing.
pub(crate) fn resolve_structural_path_shape<'types>(
    structural_types: &crate::program::ProgramDeclarations<'types, StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<std::borrow::Cow<'types, StructuralTypeShape>, TerminalInterpretError> {
    if let Some(carrier) = inline_structural_field(structural_types, root, path)
        .and_then(|field| field.field_type.byte_sequence_carrier())
    {
        return Ok(std::borrow::Cow::Owned(StructuralTypeShape::ByteSequence(
            carrier,
        )));
    }
    terminal_semantics::runtime_structural_path_tip(structural_types.values(), root, path)
        .map(|declaration| std::borrow::Cow::Borrowed(&declaration.shape))
        .ok_or(TerminalInterpretError::AffineProjectionNotRepresentable)
}

fn resolve_structural_path_type_with_leaf(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    leaf_type: fn(&terminal_psi::StructuralFieldType) -> Option<terminal_psi::StructuralFieldType>,
) -> Result<StructuralTypeId, TerminalInterpretError> {
    // An inline leaf uses the consuming operation's complete contract before
    // consulting standalone storage identities. A plain shared observation
    // must stay plain even when this catalog also retains an exact bounded
    // referent for a separate exclusive loan of the same field.
    if let Some(field) = inline_structural_field(structural_types, root, path)
        && !matches!(
            field.field_type,
            terminal_psi::StructuralFieldType::Structural(_)
        )
    {
        let declaration = if let Some(leaf) = leaf_type(&field.field_type) {
            leaf.leaf_declaration(structural_types.values())
        } else {
            // An owned byte field names its backing independently of inline
            // scalar admission. View descriptors do not name that backing.
            field
                .field_type
                .byte_sequence_carrier()
                .and_then(|carrier| {
                    terminal_semantics::byte_carrier_field_declaration(
                        structural_types.values(),
                        carrier,
                    )
                })
        };
        return declaration
            .map(|declaration| declaration.id)
            .ok_or(TerminalInterpretError::AffineProjectionNotRepresentable);
    }
    terminal_semantics::runtime_structural_path_tip(structural_types.values(), root, path)
        .map(|declaration| declaration.id)
        .ok_or(TerminalInterpretError::AffineProjectionNotRepresentable)
}

/// Resolve an inline field endpoint without manufacturing a standalone type.
/// Both storage presentation and explicit leaf-copy presentation use this
/// exact field/case geometry; only their endpoint carrier differs.
pub(crate) fn inline_structural_field<'a>(
    structural_types: &crate::program::ProgramDeclarations<'a, StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'a terminal_psi::StructuralFieldDeclaration> {
    terminal_semantics::runtime_structural_field(structural_types.values(), root, path)
}

pub(crate) fn bind_entry_claims(
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
            .ok_or(malformed())?;
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
            return Err(malformed());
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

pub(crate) fn resolve_structural_arguments(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    arguments: &[StructuralArgument],
) -> Result<Vec<TerminalStructuralValue>, TerminalInterpretError> {
    arguments
        .iter()
        .map(|argument| {
            let value = crate::custody::verified_structural_place(values, argument.place)?;
            project_structural_value(structural_types, value, &argument.path)
        })
        .collect()
}

/// The value admitted at `place` — admission proves the key exists, so a
/// miss reports a defect through the verified arm.
pub(crate) fn verified_structural_place<'v, V>(
    values: &'v BTreeMap<PlaceId, V>,
    place: PlaceId,
) -> Result<&'v V, TerminalInterpretError> {
    values
        .get(&place)
        .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
            place,
        ))
}

/// The one structural value `argument` resolves to — a batch of one.
pub(crate) fn resolve_single_structural_argument(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    argument: &StructuralArgument,
) -> Result<TerminalStructuralValue, TerminalInterpretError> {
    resolve_structural_arguments(structural_types, values, std::slice::from_ref(argument))?
        .pop()
        .ok_or_else(malformed)
}

/// Resolve a typed projection onto its original runtime backing. Case steps
/// choose a declaration namespace but add no physical storage component.
/// The execution loop validates every selected discriminator before dispatch;
/// this geometry helper itself grants no access or membership authority.
pub(crate) fn project_structural_value(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    root: &TerminalStructuralValue,
    path: &[StructuralPathSegment],
) -> Result<TerminalStructuralValue, TerminalInterpretError> {
    project_structural_value_with_leaf(
        structural_types,
        root,
        path,
        terminal_psi::StructuralFieldType::canonical_leaf_type,
    )
}

/// A shared loan observes the original restricted field without acquiring its
/// owning declaration or writable storage authority. Its backing path and
/// qualifications are selected by the same geometry as ordinary projections.
pub(crate) fn project_shared_structural_observation(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    root: &TerminalStructuralValue,
    path: &[StructuralPathSegment],
) -> Result<TerminalStructuralValue, TerminalInterpretError> {
    project_structural_value_with_leaf(
        structural_types,
        root,
        path,
        terminal_psi::StructuralFieldType::shared_observation_type,
    )
}

/// Select an inline endpoint's presentation after the live address walker has
/// resolved descriptor crossings. This geometry operation grants no access.
pub(crate) fn project_structural_value_with_leaf(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    root: &TerminalStructuralValue,
    path: &[StructuralPathSegment],
    leaf_type: fn(&terminal_psi::StructuralFieldType) -> Option<terminal_psi::StructuralFieldType>,
) -> Result<TerminalStructuralValue, TerminalInterpretError> {
    let mut value = root.clone();
    value.structural_type = resolve_structural_path_type_with_leaf(
        structural_types,
        root.structural_type,
        path,
        leaf_type,
    )?;
    (value.qualifications, value.projected_qualifications) =
        terminal_psi::StructuralPathQualification::select(
            &root.qualifications,
            &root.projected_qualifications,
            path,
        );
    value.path.extend(
        path.iter()
            .filter(|segment| !matches!(segment, StructuralPathSegment::Case(_)))
            .cloned(),
    );
    Ok(value)
}

/// Resolve the common member namespace shared by records and mixed shapes.
/// A mixed shape's current case does not change its common-field declaration;
/// payload fields remain selected separately through discriminator evidence.
pub(crate) fn direct_scalar_field_type(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
) -> Option<ScalarType> {
    let declaration = structural_types.get(&structural_type)?;
    let Some(fields) = declaration.shape.direct_fields() else {
        return None;
    };
    fields.iter().find_map(|candidate| {
        (candidate.id == field && !candidate.relevance.is_erased())
            .then_some(&candidate.field_type)
            .and_then(terminal_psi::StructuralFieldType::scalar_type)
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn transfer_claims(
    structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    structural_cases: &BTreeMap<
        StructuralRuntimePlace,
        crate::structural_inputs::case_membership::StructuralCaseContents,
    >,
    caller_claims: &BTreeMap<ClaimId, LiveClaim>,
    caller_arguments: &[StructuralArgument],
    transfers: &[ClaimTransfer],
    callee_parameters: &[StructuralParameterDeclaration],
    callee_entry_claims: &[EntryClaim],
    callee_content_entry_claims: &[terminal_psi::ContentEntryClaim],
    callee_values: &BTreeMap<PlaceId, TerminalStructuralValue>,
) -> Result<(BTreeMap<ClaimId, LiveClaim>, BTreeMap<ClaimId, LiveClaim>), TerminalInterpretError> {
    if caller_arguments.len() != callee_parameters.len()
        || transfers
            .iter()
            .map(|transfer| transfer.claim)
            .collect::<BTreeSet<_>>()
            .len()
            != transfers.len()
    {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }
    if caller_arguments
        .iter()
        .zip(callee_parameters)
        .any(|(argument, parameter)| {
            !argument.path.is_empty()
                && callee_content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == parameter.place)
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
        if !crate::structural_inputs::case_membership::case_projection_is_active(
            structural_types,
            callee_values,
            structural_cases,
            entry_claim.input,
            &entry_claim.path,
        )? {
            continue;
        }
        expected_by_argument
            .entry(index as u32)
            .or_default()
            .push(entry_claim);
    }
    let mut actual_by_argument = BTreeMap::<u32, Vec<&ClaimTransfer>>::new();
    for transfer in transfers {
        let parameter = callee_parameters
            .get(transfer.argument_index as usize)
            .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
        if !callee_entry_claims
            .iter()
            .any(|entry| entry.input == parameter.place && entry.path == transfer.path)
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let active = crate::structural_inputs::case_membership::case_projection_is_active(
            structural_types,
            callee_values,
            structural_cases,
            parameter.place,
            &transfer.path,
        )?;
        // Absence is justified by the stored tag and declared guard, never by
        // a missing identity. A live claim cannot hide behind an inactive row.
        if !active {
            if caller_claims.contains_key(&transfer.claim) {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            continue;
        }
        if !caller_claims.contains_key(&transfer.claim) {
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
        // Caller and callee claim identities are independent machine-local
        // namespaces, each in its own canonical order, so a transfer joins the
        // entry claim at its own path below the argument, never by position.
        let mut unmatched = expected;
        for transfer in actual {
            let caller_claim = remaining
                .remove(&transfer.claim)
                .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
            if caller_claim.place != Some(caller_place) {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            let suffix = caller_claim
                .path
                .strip_prefix(argument_path.as_slice())
                .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
            if suffix != transfer.path {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            let position = unmatched
                .iter()
                .position(|entry_claim| entry_claim.path == suffix)
                .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
            let entry_claim = unmatched.swap_remove(position);
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
    for argument in caller_arguments
        .iter()
        .filter(|argument| argument.access == StructuralAccess::Owned)
    {
        if remaining.values().any(|claim| {
            claim.place == Some(argument.place)
                && (claim.path.starts_with(&argument.path)
                    || argument.path.starts_with(&claim.path))
        }) {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
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

/// Bind the claims a structural return hands its caller. Each caller result
/// binding continues the callee claim the returning edge holds at the same
/// path beneath the returned value. Claim identities are machine-local, so
/// only the path joins them; a callee may hold different claims at one path
/// on different returning edges. Every returned claim lands on exactly one
/// binding.
pub(crate) fn rebind_structural_result_claims(
    caller_claims: &BTreeMap<ClaimId, LiveClaim>,
    callee_claims: &BTreeMap<ClaimId, LiveClaim>,
    source: PlaceId,
    result: &StructuralOperationResult,
    returned_claims: &[StructuralResultClaimBinding],
) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
    let mut unmatched = returned_claims
        .iter()
        .map(|binding| binding.claim)
        .collect::<BTreeSet<_>>();
    if unmatched.len() != returned_claims.len() || returned_claims.len() != result.claims.len() {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }
    let mut rebound = caller_claims.clone();
    for binding in &result.claims {
        let expected_multiplicity = if binding.path.is_empty() {
            result.multiplicity
        } else {
            StructuralMultiplicity::Linear
        };
        let mut held = returned_claims.iter().filter(|returned| {
            returned.path == binding.path
                && callee_claims
                    .get(&returned.claim)
                    .is_some_and(|live| live.place == Some(source) && live.path == returned.path)
        });
        let Some(callee_claim) = held.next() else {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        };
        if held.next().is_some()
            || !unmatched.remove(&callee_claim.claim)
            || callee_claims
                .get(&callee_claim.claim)
                .is_none_or(|live| live.multiplicity != Some(expected_multiplicity))
            || rebound
                .insert(
                    binding.claim,
                    LiveClaim {
                        place: Some(result.place),
                        path: binding.path.clone(),
                        multiplicity: Some(expected_multiplicity),
                    },
                )
                .is_some()
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
    }
    if !unmatched.is_empty() {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }
    Ok(rebound)
}

/// `argument` names the opaque value at one declared boundary position; a
/// position without one (an owned array copy) cannot carry a qualification.
pub(crate) fn validate_boundary_requirements<'value>(
    boundary: &BoundaryMachineDeclaration,
    argument: impl Fn(usize) -> Option<&'value TerminalStructuralValue>,
) -> Result<(), TerminalInterpretError> {
    for requirement in &boundary.requires {
        let argument = argument(requirement.argument_index as usize).ok_or(malformed())?;
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

pub(crate) fn complete_claims(
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
                if live.place != Some(argument.place) {
                    return None;
                }
                live.path
                    .strip_prefix(argument.path.as_slice())
                    .map(|path| (index as u32, *claim, path))
            })
        })
        .collect::<BTreeSet<_>>();
    let mut remaining = caller_claims.clone();
    let mut actual = BTreeSet::new();
    for receipt in receipts {
        let occurrence = (
            receipt.argument_index,
            receipt.claim,
            receipt.path.as_slice(),
        );
        if !actual.insert(occurrence) || !expected.contains(&occurrence) {
            return Err(TerminalInterpretError::CompletionReceiptMismatch);
        }
        let argument = caller_arguments
            .get(receipt.argument_index as usize)
            .ok_or(TerminalInterpretError::CompletionReceiptMismatch)?;
        let claim = remaining
            .remove(&receipt.claim)
            .ok_or(TerminalInterpretError::CompletionReceiptMismatch)?;
        if claim.place != Some(argument.place)
            || claim.path.strip_prefix(argument.path.as_slice()) != Some(receipt.path.as_slice())
        {
            return Err(TerminalInterpretError::CompletionReceiptMismatch);
        }
    }
    if actual != expected {
        return Err(TerminalInterpretError::CompletionReceiptMismatch);
    }
    Ok(remaining)
}
