//! Ordinary structural graph results, with exact authored field custody, and
//! primitive scalar results. A scalar-result state completes with the
//! ordinary single-state completion: the binding its final expression's
//! operation produced, or its value-only exit roster (`validate_scalar`).
use super::super::super::super::{
    CheckedComposedUnitControlTerminatorPlan, StructuralResultDeclaration,
};
use super::super::super::{
    CheckedUnitEffectOperationPlan, Multiplicity, StructuralPlaceDeclaration, StructuralPlaceKind,
    TerminalMachineResult, ValueDeclaration, allocate_dense, lookup_type_id, place_id,
    terminal_scalar_type, unsupported, value_id,
};
use super::super::{CheckedTrees, LoweringError, catalogs};
use super::{CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan};
use crate::emission::borrowed_window::terminal_multiplicity;
use crate::terminal_identities::lookup_domain_id_for_carrier;
use checked_trees::CheckedControlResultPlan;

pub(super) mod projected;

pub(super) fn signature_matches(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    result: &CheckedControlResultPlan,
) -> bool {
    match result {
        CheckedControlResultPlan::Unit => matches!(
            checked
                .type_reference_table
                .type_reference(source.return_type),
            checked_trees::types::TypeReferenceNode::Unit
        ),
        // Value refinements use the same complete predicate judgment as the
        // checked result signature. Normal-return guarantees publish them;
        // semantic atoms still require their separate result custody.
        CheckedControlResultPlan::Scalar { primitive_type } => {
            checked.primitive_type_reference(source.return_type) == Some(*primitive_type)
                && validation::scalar_result_contract_is_represented(
                    &checked.typed,
                    source.return_type,
                )
        }
        CheckedControlResultPlan::Structural(result) => {
            // A borrowed `&[T]` view result names the peeled slice carrier in
            // its identity — the contents live in the caller's frame, so the
            // owned-contents custody clause does not apply.
            // Slice views use their element-view carrier. Named records and
            // primitive referents retain the reference shell and its affine
            // permission; neither becomes owned referent storage.
            if crate::reference_custody::mutable_borrow_parts(checked, source.return_type).is_some()
            {
                // Only a slice view has a peeled carrier here.
                let identity = crate::expression_preparation::source_custody::structural::mutable_view_carrier(
                    checked,
                    source.return_type,
                )
                .map(|carrier| checked.normalized_type_identity(carrier).into_string())
                .unwrap_or_else(|| {
                    checked
                        .normalized_type_identity(source.return_type)
                        .into_string()
                });
                return identity.as_str() == result.type_identity
                    && result.multiplicity == Multiplicity::Affine
                    && view_result_qualifications(checked, source.return_type).as_ref()
                        == Some(&result.qualifications)
                    && validation::structural_result_projected_qualifications(
                        &checked.typed,
                        source.return_type,
                    )
                    .ok()
                    .as_ref()
                        == Some(&result.projected_qualifications);
            }
            if let Some(slice) = borrowed_view_slice(checked, source.return_type) {
                return checked.normalized_type_identity(slice).as_str() == result.type_identity
                    && crate::reference_custody::result_multiplicity(checked, source.return_type)
                        == result.multiplicity
                    && view_result_qualifications(checked, source.return_type).as_ref()
                        == Some(&result.qualifications)
                    && validation::structural_result_projected_qualifications(
                        &checked.typed,
                        source.return_type,
                    )
                    .ok()
                    .as_ref()
                        == Some(&result.projected_qualifications);
            }
            // A `&mut T` result is alias custody, not an owned structural
            // claim: the plan identity spells the whole reference type, the
            // binding itself is affine, and no owned qualification rows
            // exist to compare. The caller-side loan the `EstablishReference`
            // source rejoins is verified at its own emission, not here.
            if crate::reference_custody::mutable_borrow_parts(checked, source.return_type).is_some()
            {
                return checked
                    .normalized_type_identity(source.return_type)
                    .as_str()
                    == result.type_identity
                    && result.multiplicity == Multiplicity::Affine
                    && result.qualifications.is_empty()
                    && validation::structural_result_projected_qualifications(
                        &checked.typed,
                        source.return_type,
                    )
                    .ok()
                    .as_ref()
                        == Some(&result.projected_qualifications);
            }
            // A shared `&'a V` result spells the same whole-reference
            // identity convention: `add_view_type` registers under the
            // reference's own identity with the affine view-loan
            // multiplicity the checking side mints; the named referent
            // stays in the caller's frame and the binding carries loan,
            // not contents.
            if crate::reference_custody::shared_borrow_parts(checked, source.return_type).is_some()
            {
                return checked
                    .normalized_type_identity(source.return_type)
                    .as_str()
                    == result.type_identity
                    && result.multiplicity == Multiplicity::Affine
                    && view_result_qualifications(checked, source.return_type).as_ref()
                        == Some(&result.qualifications)
                    && validation::structural_result_projected_qualifications(
                        &checked.typed,
                        source.return_type,
                    )
                    .ok()
                    .as_ref()
                        == Some(&result.projected_qualifications);
            }
            // Contents classification reads the carrier beneath its
            // qualifications; the plan identity follows the shared rule.
            let Ok(carrier) = crate::unit::attached_unit::parameters::structural_carrier_type(
                checked,
                source.return_type,
            ) else {
                return false;
            };
            validation::stored_structural_type_identity(
                &checked.typed,
                source.return_type,
                &[],
                &[],
            ) == result.type_identity
                && crate::reference_custody::result_multiplicity(checked, source.return_type) == result.multiplicity
                && validation::structural_result_qualifications(&checked.typed, source.return_type).ok().as_ref() == Some(&result.qualifications)
                && validation::structural_result_projected_qualifications(
                    &checked.typed,
                    source.return_type,
                )
                .ok()
                .as_ref()
                    == Some(&result.projected_qualifications)
                // Plain storage classification excludes linear roots by design.
                // Their admission instead requires exact entry/call/return claims.
                // An affine owner of linear members carries their claims the
                // same way: its returning edges transfer them by path.
                && (result.multiplicity == Multiplicity::Linear
                    || crate::content_classes::linear_owned_contents(checked, source.return_type)
                    || crate::reference_custody::is_reference_record(checked, source.return_type)
                    || (result.multiplicity == Multiplicity::Unrestricted
                        && crate::content_classes::plain_owned_with_represented_constraints(checked, source.return_type))
                    || validation::has_contents(
                    &checked.typed,
                    carrier, validation::ContentsRequirement::CleanupOwned) || validation::has_contents(&checked.typed, carrier, validation::ContentsRequirement::OwnedOrSharedView))
        }
    }
}

/// The slice a shared-borrow `&[T]` result refers to, peeling constraint and
/// reference shells. Mutable and write-only borrows stay with their own
/// custody families.
fn borrowed_view_slice(
    checked: &CheckedTrees,
    mut reference: checked_trees::types::TypeReferenceHandle,
) -> Option<checked_trees::types::TypeReferenceHandle> {
    let mut referee = loop {
        match checked.type_reference_table.type_reference(reference) {
            checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference = *base_type
            }
            checked_trees::types::TypeReferenceNode::Reference {
                referee,
                access: language_core::ReferenceAccess::Shared,
                ..
            } => break *referee,
            _ => return None,
        }
    };
    // Constraint shells inside the borrow qualify the referent's storage, not
    // the loan's shape: `&([u8] in Utf8)` refers to a slice exactly as `&[u8]`
    // does, and the domain rides the carrier the plan spells.
    loop {
        match checked.type_reference_table.type_reference(referee) {
            checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                referee = *base_type
            }
            checked_trees::types::TypeReferenceNode::Slice { .. } => return Some(referee),
            _ => return None,
        }
    }
}

/// Domain qualifications a view result declares on its own carrier:
/// constraints above the borrow directly, and on a byte view the
/// inside-borrow domains too — the lowered view type peels the reference so
/// `&[u8] in Utf8` registers `Utf8` under the slice identity the result
/// spells. Other referent domains stay inside the loan.
fn view_result_qualifications(
    checked: &CheckedTrees,
    reference: checked_trees::types::TypeReferenceHandle,
) -> Option<Vec<language_semantics::SemanticDomainId>> {
    validation::view_result_carrier_qualifications(&checked.typed, reference).ok()
}

pub(super) fn validate_structural(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    ordinal: usize,
) -> Result<(), LoweringError> {
    use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
    let (
        CheckedControlResultPlan::Structural(_),
        CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result },
    ) = (&plan.result, &state.terminator)
    else {
        return unsupported("structural return has no result signature");
    };
    match result.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
            let (operation, binding) = state
                .operations
                .iter()
                .find_map(|operation| {
                    let result = match operation {
                        // A `&mut` view tail return's producer is the reference
                        // establishment minted at the transition.
                        CheckedUnitEffectOperationPlan::EstablishReference { result, .. }
                        | CheckedUnitEffectOperationPlan::EstablishViewSubslice {
                            result, ..
                        }
                        | CheckedUnitEffectOperationPlan::EstablishByteSequence {
                            result, ..
                        } => Some(result),
                        _ => super::result_custody::result(operation),
                    }?;
                    (result.binding_ordinal == binding_ordinal).then_some((operation, result))
                })
                .ok_or(LoweringError::Unsupported(
                    "structural return producer absent",
                ))?;
            if binding.statement_index as usize == ordinal {
                // A call carries the custody checking recorded for it, and a
                // structural value carries its own construction.
                if operation
                    .call()
                    .is_some_and(|call| call.structural_result().is_some())
                    || matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                            | CheckedUnitEffectOperationPlan::EstablishReference { .. }
                    )
                {
                } else if matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishByteSequence { .. }
                ) {
                    super::super::super::byte_locals::validate(checked, state.state, operation)?;
                } else if matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
                ) {
                    super::super::super::view_ranges::binding_site(
                        checked,
                        state.state,
                        operation,
                    )?;
                } else {
                    return unsupported(
                        "structural return tail producer requires exact value custody",
                    );
                }
            } else {
                let Some(checked_trees::statement::StatementNode::LocalData(_)) = checked
                    .statement_table
                    .statements(source.statement_nodes)
                    .get(binding.statement_index as usize)
                else {
                    return unsupported("structural return does not name its produced local");
                };
            }
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let parameter = state
                .structural_parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural return parameter absent",
                ))?;
            if checked
                .state_parameters(source)
                .get(parameter.position as usize)
                .is_none()
            {
                return unsupported("structural return source parameter absent");
            }
        }
        // A literal-backed `&[u8]` return carries its bytes as the whole
        // ingress.
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. } => {}
        _ => return unsupported("structural return requires whole established custody"),
    }
    Ok(())
}

pub(super) fn result(
    checked: &CheckedTrees,
    entry_state: symbols::SymbolHandle,
    plan: &CheckedControlResultPlan,
    reference_sources: &[checked_trees::CheckedReferenceResultSourcePlan],
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    catalogs: &mut catalogs::ComposedCatalogs,
    places: &mut Vec<StructuralPlaceDeclaration>,
) -> Result<TerminalMachineResult, LoweringError> {
    let result = match plan {
        CheckedControlResultPlan::Unit => return Ok(TerminalMachineResult::Unit),
        CheckedControlResultPlan::Scalar { primitive_type } => {
            let declared = crate::unit::attached_unit::parameters::unit_scalar_result_type(
                checked,
                entry_state,
                terminal_scalar_type(*primitive_type)?,
                catalogs
                    .prepared_scalar_qualifications
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph scalar result has no qualification namespace",
                    ))?,
            )?;
            return Ok(TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: declared.qualifications,
                id: value_id(allocate_dense(&mut catalogs.next_value)?),
                scalar_type: declared.scalar_type,
            }));
        }
        CheckedControlResultPlan::Structural(result) => result,
    };
    let place = place_id(allocate_dense(&mut catalogs.next_place)?);
    places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Result,
    });
    let reference_sources = crate::unit::attached_unit::parameters::lower_reference_result_sources(
        reference_sources,
        &catalogs.structural_types,
        parameters,
    )?;
    Ok(TerminalMachineResult::Structural(
        StructuralResultDeclaration {
            reference_sources,
            place,
            structural_type: lookup_type_id(&catalogs.type_ids, &result.type_identity)?,
            multiplicity: terminal_multiplicity(result.multiplicity),
            qualifications: result
                .qualifications
                .iter()
                .map(|domain| {
                    lookup_domain_id_for_carrier(
                        &catalogs.domain_ids,
                        *domain,
                        lookup_type_id(&catalogs.type_ids, &result.type_identity)?,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
            projected_qualifications:
                crate::unit::attached_unit::parameters::lower_projected_qualifications(
                    &result.projected_qualifications,
                    &catalogs.domain_ids,
                    lookup_type_id(&catalogs.type_ids, &result.type_identity)?,
                    &catalogs.structural_types,
                )?,
        },
    ))
}
