//! Admit the complete operation-body roster before allocating shared identities.
//!
//! The result retains each checked source alongside its admitted body. Emission
//! consumes that roster in order instead of searching and removing composed bodies.

use super::super::{
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, LoweringError,
    unsupported,
};
mod operations;

use super::bodies::{UnitBody, UnitPlans};
use super::composed_control::{self, callable::CallableBody};
use super::{
    RuntimeRequirementOwner, reference_results, retain_exact_unit_boundary, scalar_completion,
    structural_completion, validate_unit_operation_sequence,
};
use checked_trees::{CheckedBoundaryMachinePlan, CheckedComposedUnitControlMachinePlan};
use symbols::SymbolHandle;

pub(super) enum AdmittedBody<'a> {
    Ordinary(&'a CheckedUnitEffectMachinePlan),
    Composed {
        source: &'a CheckedComposedUnitControlMachinePlan,
        body: CallableBody<'a>,
    },
}

impl<'a> AdmittedBody<'a> {
    pub(super) fn source(&self) -> UnitBody<'a> {
        match self {
            Self::Ordinary(plan) => UnitBody::Ordinary(plan),
            Self::Composed { source, .. } => UnitBody::Composed(source),
        }
    }
}

pub(super) fn admit<'a>(
    checked: &'a CheckedTrees,
    plans: UnitPlans<'a>,
    entry: SymbolHandle,
    closure: &[SymbolHandle],
    scalar_closure: &[SymbolHandle],
    requirements_owner: RuntimeRequirementOwner,
    boundaries: &mut Vec<(&'a CheckedBoundaryMachinePlan, String)>,
) -> Result<Vec<AdmittedBody<'a>>, LoweringError> {
    let mut bodies = Vec::with_capacity(closure.len());
    for source in closure {
        let admitted = match UnitBody::find(plans, *source)? {
            UnitBody::Composed(plan) => {
                let body = composed_control::admit_callable(checked, plan)?;
                for (boundary, _) in body.boundaries() {
                    retain_exact_unit_boundary(
                        checked,
                        plans,
                        boundaries,
                        boundary.machine,
                        boundary.state,
                        boundary.contract_report_fingerprint,
                        boundary.service_reach,
                        boundary.result.clone(),
                    )?;
                }
                AdmittedBody::Composed { source: plan, body }
            }
            UnitBody::Ordinary(plan) => {
                validate_ordinary(checked, plan, entry, requirements_owner)?;
                operations::validate(checked, plans, plan, closure, scalar_closure, boundaries)?;
                AdmittedBody::Ordinary(plan)
            }
        };
        bodies.push(admitted);
    }
    Ok(bodies)
}

fn validate_ordinary(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
    entry: SymbolHandle,
    requirements_owner: RuntimeRequirementOwner,
) -> Result<(), LoweringError> {
    if machine.contract_report_fingerprint == 0 {
        return unsupported("Unit closure contains a null checked contract fingerprint");
    }
    let contract = checked
        .facts
        .contract_plans
        .for_machine(machine.machine)
        .ok_or(LoweringError::Unsupported(
            "Unit closure is missing its canonical checked contract",
        ))?;
    if machine.contract_report_fingerprint != contract.report_fingerprint
        || machine.contract_commitment != contract.commitment
    {
        return unsupported(
            "Unit closure contract compatibility coordinate or strong commitment drifted",
        );
    }
    validate_unit_operation_sequence(checked, machine)?;
    reference_results::validate_releases(checked, machine)?;
    // The nominal-cleanup owner validates a synthetic empty completion for
    // its entry, then installs the actual scalar result and full contract.
    // Ordinary entries and every transitive helper retain authored results.
    let synthetic_cleanup_entry = requirements_owner == RuntimeRequirementOwner::NominalCleanup
        && machine.machine == entry
        && machine.structural_result.is_none()
        && machine.scalar_result.is_none()
        && machine.scalar_control.is_none()
        && matches!(machine.operations.as_slice(), [CheckedUnitEffectOperationPlan::Complete { statement_index: 0, trivial_affine_local_discard_ordinals, trivial_affine_discards }] if trivial_affine_local_discard_ordinals.is_empty() && trivial_affine_discards.is_empty());
    if !synthetic_cleanup_entry {
        if machine.scalar_result.is_some() || machine.scalar_control.is_some() {
            scalar_completion::validate(checked, machine)?;
        } else {
            structural_completion::validate(checked, machine)?;
        }
    }
    crate::emission::structural_scalar_store_source::validate(checked, machine)?;
    crate::emission::call_source_custody::validate_store_and_initializer_calls(checked, machine)?;

    Ok(())
}
