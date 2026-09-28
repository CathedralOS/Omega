//! The structural return rows: each names an installed function, sits
//! inside its text span in canonical order, and realizes the function's
//! declared structural return exactly.

use crate::installation_record::{
    CallSignature, CallingPolicy, InstallationError, InstallationRecord, InstalledFunction,
    MachineId, SemanticCodeSite, StructuralMultiplicity, ValueClass, ValueShape,
    direct_structural_return_placement, evaluate_call_plan,
};

pub(super) fn validate_structural_returns(
    record: &InstallationRecord,
    function_by_machine: &std::collections::BTreeMap<MachineId, &InstalledFunction>,
) -> Result<(), InstallationError> {
    let mut previous_return = None;
    for installed in &record.structural_returns {
        let function = function_by_machine.get(&installed.machine).ok_or(
            InstallationError::StructuralReturnMachineMissing(installed.machine),
        )?;
        let returned = &installed.returned;
        let scalar_shapes = returned
            .scalar_parameters
            .iter()
            .map(|parameter| {
                let semantic_vocabulary::ScalarType::Integer(integer) = parameter.scalar_type
                else {
                    return None;
                };
                if integer.is_address() || !matches!(integer.bits(), 8 | 16 | 32 | 64) {
                    return None;
                }
                let bytes = integer.bits() / 8;
                Some(ValueShape::integer(bytes, bytes))
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(InstallationError::InvalidStructuralReturn(
                installed.machine,
            ))?;
        let expected_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(record.target),
            &CallSignature {
                parameters: scalar_shapes
                    .iter()
                    .copied()
                    .chain(
                        returned
                            .parameter_placements
                            .iter()
                            .map(|placement| placement.shape),
                    )
                    .collect(),
                result: Some(returned.shape),
            },
        )
        .map_err(|_| InstallationError::InvalidStructuralReturn(installed.machine))?;
        let expected_result = expected_plan.result.as_ref();
        let structural_attribution = record
            .semantic_code_attribution
            .iter()
            .filter(|attribution| attribution.machine == installed.machine)
            .collect::<Vec<_>>();
        let exact_claimful_linear = returned.scalar_parameters.is_empty()
            && returned.source.multiplicity == StructuralMultiplicity::Linear
            && returned.result.multiplicity == StructuralMultiplicity::Linear
            && returned.returned_claims.len() == 1;
        let exact_claim_free_affine =
            crate::object_artifact::replay::structural::return_record::has_claim_free_affine_identity_custody(returned);
        if previous_return.is_some_and(|previous| previous >= installed.machine)
            || !returned.result.reference_sources.is_empty()
            || returned.code_offset != 0
            || returned.byte_count != function.byte_count
            || returned.source.position != 0
            || returned.source.is_self
            || (!exact_claimful_linear && !exact_claim_free_affine)
            || returned.source.structural_type != returned.result.structural_type
            || returned.source.qualifications != returned.result.qualifications
            || returned.source.projected_qualifications
                != returned.result.projected_qualifications
            || returned.source.place == returned.result.place
            || returned.shape.class != ValueClass::Integer
            || !((returned.shape.byte_size == 8 && returned.shape.alignment == 8)
                || (9..=16).contains(&returned.shape.byte_size))
            || returned.source_placement.shape != returned.shape
            || returned.result_placement.shape != returned.shape
            || !direct_structural_return_placement(&returned.source_placement)
            || !direct_structural_return_placement(&returned.result_placement)
            || returned.parameters.first() != Some(&returned.source)
            || returned.parameters.iter().skip(1).any(|parameter| {
                parameter.place == returned.source.place
                    || parameter.place == returned.result.place
                    || !parameter.qualifications.is_empty()
            })
            || returned
                .parameters
                .iter()
                .map(|parameter| parameter.place)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != returned.parameters.len()
            || returned.trivial_affine_locals.iter().enumerate().any(|(index, (_, local, local_type))| {
                !matches!(
                    local.kind,
                    semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                        declaration_ordinal,
                        structural_type,
                        construction: None,
                    } if usize::try_from(declaration_ordinal) == Ok(index)
                        && structural_type == local_type.id
                ) || local.id == returned.source.place
                    || local.id == returned.result.place
                    || returned.parameters.iter().any(|parameter| parameter.place == local.id)
                    || local_type.identity.is_empty()
                    || !matches!(
                        local_type.shape,
                        terminal_psi::StructuralTypeShape::Record { ref fields } if fields.is_empty()
                    )
            })
            || returned
                .trivial_affine_locals
                .iter()
                .map(|(_, local, _)| local.id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != returned.trivial_affine_locals.len()
            || returned.parameter_placements.len() != returned.parameters.len()
            || expected_plan.parameters.len()
                != returned.scalar_parameters.len() + returned.parameter_placements.len()
            || expected_plan.parameters[..returned.scalar_parameters.len()]
                .iter()
                .zip(&returned.scalar_parameters)
                .any(|(placement, parameter)| placement != &parameter.placement)
            || expected_plan.parameters[returned.scalar_parameters.len()..]
                != returned.parameter_placements
            || returned.parameter_placements.first() != Some(&returned.source_placement)
            || returned
                .parameters
                .iter()
                .enumerate()
                .any(|(index, parameter)| {
                    parameter.is_self || usize::try_from(parameter.position) != Ok(index)
                })
            || returned.trivial_affine_discards
                != returned
                    .trivial_affine_locals
                    .iter()
                    .rev()
                    .map(|(_, local, _)| local.id)
                    .chain(
                        returned
                            .parameters
                            .iter()
                            .skip(1)
                            .rev()
                            .map(|parameter| parameter.place),
                    )
                    .collect::<Vec<_>>()
            || returned
                .parameters
                .iter()
                .skip(1)
                .any(|parameter| parameter.multiplicity != StructuralMultiplicity::Affine)
            || expected_result != Some(&returned.result_placement)
            || structural_attribution.len() != returned.trivial_affine_locals.len() + 1
            || returned
                .trivial_affine_locals
                .iter()
                .enumerate()
                .any(|(ordinal, (operation, _, _))| {
                    structural_attribution.get(ordinal).is_none_or(|installed| {
                        installed.attribution.site
                                != SemanticCodeSite::Operation(*operation)
                            || installed.attribution.operation_ordinal != ordinal
                            || installed.attribution.code_offset != 0
                            || installed.attribution.byte_count != 0
                    })
                })
            || structural_attribution.last().is_none_or(|installed| {
                installed.attribution.site
                        != SemanticCodeSite::Edge(returned.psi_edge)
                    || installed.attribution.operation_ordinal
                        != returned.trivial_affine_locals.len()
                    || installed.attribution.code_offset != 0
                    || installed.attribution.byte_count != returned.byte_count
            })
        {
            return Err(InstallationError::InvalidStructuralReturn(
                installed.machine,
            ));
        }
        previous_return = Some(installed.machine);
    }
    Ok(())
}
