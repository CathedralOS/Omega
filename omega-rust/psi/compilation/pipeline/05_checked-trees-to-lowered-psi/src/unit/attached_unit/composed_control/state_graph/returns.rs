//! Ordinary structural graph results, with exact authored field custody, and
//! primitive scalar results. A scalar-result state completes with the
//! ordinary single-state completion: the binding its final expression's
//! operation produced, or its value-only exit roster (`validate_scalar`).
use super::super::super::super::{
    CheckedComposedUnitControlTerminatorPlan, StructuralResultDeclaration,
};
use super::super::super::{
    CheckedUnitEffectOperationPlan, StructuralPlaceDeclaration, StructuralPlaceKind,
    TerminalMachineResult, ValueDeclaration, allocate_dense, lookup_type_id, place_id,
    terminal_scalar_type, unsupported, value_id,
};
use super::super::{CheckedTrees, LoweringError, catalogs};
use super::{CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan};
use crate::emission::borrowed_window::terminal_multiplicity;
use crate::terminal_identities::lookup_domain_id_for_carrier;
use checked_trees::CheckedControlResultPlan;

pub(super) mod projected;

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
