//! Ordinary structural graph results, with exact authored field custody, and
//! primitive scalar results. A scalar-result state completes with the
//! ordinary single-state completion: the binding its final expression's
//! operation produced, or its value-only exit roster (`validate_scalar`).
use super::super::super::super::StructuralResultDeclaration;
use super::super::super::{
    StructuralPlaceDeclaration, StructuralPlaceKind, TerminalMachineResult, ValueDeclaration,
    allocate_dense, lookup_type_id, place_id, terminal_scalar_type, value_id,
};
use super::super::{CheckedTrees, LoweringError, catalogs};
use crate::emission::borrowed_window::terminal_multiplicity;
use crate::terminal_identities::lookup_domain_id_for_carrier;
use checked_trees::CheckedControlResultPlan;

pub(super) mod projected;

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
    let structural_type = lookup_type_id(&catalogs.type_ids, &result.type_identity)?;
    let reference_sources = crate::unit::attached_unit::parameters::lower_reference_result_sources(
        reference_sources,
        structural_type,
        &catalogs.structural_types,
        parameters,
    )?;
    Ok(TerminalMachineResult::Structural(
        StructuralResultDeclaration {
            reference_sources,
            place,
            structural_type,
            multiplicity: terminal_multiplicity(result.multiplicity),
            qualifications: result
                .qualifications
                .iter()
                .map(|domain| {
                    lookup_domain_id_for_carrier(&catalogs.domain_ids, *domain, structural_type)
                })
                .collect::<Result<Vec<_>, _>>()?,
            projected_qualifications:
                crate::unit::attached_unit::parameters::lower_projected_qualifications(
                    &result.projected_qualifications,
                    &catalogs.domain_ids,
                    structural_type,
                    &catalogs.structural_types,
                )?,
        },
    ))
}
