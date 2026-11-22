//! Coordinate evidence from independently prepared source and toolchain units.
#![forbid(unsafe_code)]

use omega_lowering::LoweringError;
use omega_preparation::{PolicyConstructionOccurrence, PreparedTrees};
use std::sync::Arc;

/// Check the constructor reached by each policy with a known schema and package
/// owner. Preparation retains the remaining execution demand for that occurrence.
/// The defining toolchain is lowered once per package that needs its evidence.
pub fn check_pending_policy_constructions(prepared: &mut PreparedTrees<'_>) -> Result<(), LoweringError> {
    let requests: Vec<_> = prepared
        .typed()
        .access_policies
        .iter()
        .enumerate()
        .filter_map(|(index, selection)| {
            prepared
                .policy_construction_occurrence(&selection.domain)
                .map(|occurrence| (index, occurrence))
        })
        .collect();
    let Some((first, _)) = requests.first() else { return Ok(()) };
    let library = omega_toolchain::SourcePackage::toolchain_definition();
    let prepared_library = omega_preparation::prepare_source_package_diagnosed(&library).map_err(|error| {
        LoweringError::new(
            &prepared.typed().access_policies[*first].site,
            format!("preparing the policy constructor definition: {}", error.diagnostic().message),
        )
    })?;
    let unit = omega_lowering::lower_package(&prepared_library)?;
    for (index, occurrence) in requests {
        let selection = &prepared.typed().access_policies[index];
        let inputs = omega_lowering::lower_policy_construction_inputs(prepared, selection)?;
        let definition = omega_lowering::lower_selected_policy_definition(prepared, selection)?;
        let refuse = |reason: &str| LoweringError::new(&inputs.selection_site, reason);
        let constructor = omega_checker::check_record_constructor(&unit, &inputs.constructor).map_err(refuse)?;
        let call = omega_checker::check_selected_policy_call(&definition, &constructor).map_err(refuse)?;
        let construction = omega_checker::check_policy_construction(&call, &inputs).map_err(refuse)?;
        resume_policy_construction(prepared, occurrence, &construction)?;
    }
    Ok(())
}

/// Compare the proof with inputs projected from the package receiving it.
/// Identical bytes from a different preparation have a different backing.
fn resume_policy_construction(
    prepared: &mut PreparedTrees<'_>,
    occurrence: PolicyConstructionOccurrence,
    checked: &omega_checker::CheckedPolicyConstruction,
) -> Result<(), LoweringError> {
    let proof = checked.inputs();
    let refuse = |message| LoweringError::new(&proof.selection_site, message);
    let selection = prepared
        .policy_invocation(occurrence.domain())
        .ok_or_else(|| refuse("policy construction is not pending"))?
        .selection;
    let current = omega_lowering::lower_policy_construction_inputs(prepared, selection)?;
    if current != *proof || !Arc::ptr_eq(&current.schema.storage, &proof.schema.storage) {
        return Err(refuse("checked construction differs from the receiving package's inputs"));
    }
    prepared
        .record_policy_construction_comparison(
            occurrence,
            &current.package_key,
            &current.subject_revision,
            &current.schema.storage,
            &current.selection_site,
            &current.construction_site,
        )
        .map_err(refuse)
}

#[cfg(test)]
mod tests;
