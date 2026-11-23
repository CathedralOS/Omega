//! Coordinate evidence from independently prepared source and toolchain units.
#![forbid(unsafe_code)]

mod policy_answers;
pub use policy_answers::{answer_policy, lower_package_with_policy_answers, PolicyAnswer};

use omega_lowering::LoweringError;
use omega_preparation::{PolicyConstructionOccurrence, PreparedTrees};
use std::sync::Arc;

/// Check frozen construction inputs against their defining Psi bodies.
pub fn check_pending_policy_constructions(prepared: &mut PreparedTrees<'_>) -> Result<(), LoweringError> {
    check_policies(prepared, None).map(|_| ())
}

/// Execute closed selected policies under the root's ceilings and retain their answers for lowering.
pub fn prepare_policy_answers(
    prepared: &mut PreparedTrees<'_>,
    ceilings: omega_interpreter::Ceilings,
) -> Result<Vec<PolicyAnswer>, LoweringError> {
    check_policies(prepared, Some(ceilings))
}

fn check_policies(
    prepared: &mut PreparedTrees<'_>,
    ceilings: Option<omega_interpreter::Ceilings>,
) -> Result<Vec<PolicyAnswer>, LoweringError> {
    let requests: Vec<_> = prepared.typed().access_policies.iter().enumerate().filter_map(|(index, selection)| {
        prepared.policy_construction_occurrence(&selection.domain).map(|occurrence| (index, occurrence))
    }).collect();
    let mut answers = Vec::new();
    let mut fallback = None;
    for (index, occurrence) in requests {
        let selection = &prepared.typed().access_policies[index];
        let inputs = omega_lowering::lower_policy_construction_inputs(prepared, selection)?;
        let definition = omega_lowering::lower_selected_policy_definition(prepared, selection)?;
        let execution_unit = omega_lowering::lower_selected_policy_unit(prepared, selection)?;
        let unit = if let Some(unit) = &execution_unit {
            unit.package()
        } else {
            if fallback.is_none() {
                fallback = Some(lower_toolchain(&selection.site)?);
            }
            fallback.as_ref().expect("initialized above")
        };
        let refuse = |reason: &str| LoweringError::new(&inputs.selection_site, reason);
        let constructor = omega_checker::check_record_constructor(unit, &inputs.constructor).map_err(refuse)?;
        let call = omega_checker::check_selected_policy_call(&definition, &constructor).map_err(refuse)?;
        let construction = omega_checker::check_policy_construction(&call, &inputs).map_err(refuse)?;
        resume_policy_construction(prepared, occurrence, &construction)?;
        if let (Some(ceilings), Some(execution_unit)) = (ceilings, &execution_unit) {
            let closed = omega_checker::check_closed_policy_call(&call).map_err(refuse)?;
            let receipt = omega_interpreter::execute_policy(&closed, &inputs.schema, ceilings)
                .map_err(|reason| refuse(&reason))?;
            answers.push(answer_policy(execution_unit, receipt)?);
        }
    }
    Ok(answers)
}

/// The toolchain's own package, which defines the policy constructor.
fn lower_toolchain(requested: &omega_lowering::Site) -> Result<omega_lowering::LoweredPackage, LoweringError> {
    let library = omega_toolchain::SourcePackage::toolchain_definition();
    let prepared = omega_preparation::prepare_source_package_diagnosed(&library)
        .map_err(|error| LoweringError::new(requested, format!("preparing the policy constructor definition: {}", error.diagnostic().message)))?;
    omega_lowering::lower_package(&prepared)
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

/// Complete the policy handoffs for a captured package and lower its prepared trees.
pub fn lower_prepared_package(prepared: &mut PreparedTrees<'_>) -> Result<omega_lowering::LoweredPackage, LoweringError> {
    let answers = prepare_policy_answers(prepared, omega_interpreter::Ceilings::default())?;
    lower_package_with_policy_answers(prepared, &answers)
}

#[cfg(test)]
mod tests;
