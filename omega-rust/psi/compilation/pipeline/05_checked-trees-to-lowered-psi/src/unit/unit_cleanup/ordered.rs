//! Admit a selected nominal cleanup entry and root its ordinary Unit closure.
//! Signature contexts and return-action finalization are shared with members.

use super::{
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedTrees, CheckedUnitStructuralTypeShape,
    LoweringError, lower_unit_closure, unsupported,
};
use crate::producer_result::SourceMappedLowered;
use crate::unit::attached_unit::UnitClosureRequest;
use crate::unit::attached_unit::bodies::UnitPlans;
use checked_trees::is_bounded_nominal_cleanup_record;

pub(super) fn lower_ordered_nominal_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
) -> Result<SourceMappedLowered, LoweringError> {
    let plan = &nominal.machine;
    let parameter_count = plan.structural_parameters.len();
    if parameter_count == 0 || nominal.cleanups.len() != parameter_count {
        return unsupported("ordered nominal cleanup requires matched actions");
    }

    // Stage 04 adopted the nominal lane's shapes into the ordinary roster's
    // one type namespace, rejecting any conflicting registration there.
    let published = UnitPlans::published(&checked.facts.flow.terminal_unit_effects);
    // A receiver-free caller, qualified or not, retains no attachment
    // storage. A retained owner must be an empty record: the dispatcher
    // carries no attachment state across its cleanup edges.
    if let Some(attachment) = plan.attachment_type_identity.as_deref() {
        let attachment_shape =
            published
                .structural_type(attachment)
                .ok_or(LoweringError::Unsupported(
                    "ordered nominal cleanup attachment shape is absent",
                ))?;
        if !matches!(&attachment_shape.shape, CheckedUnitStructuralTypeShape::Record { fields, .. } if fields.is_empty())
        {
            return unsupported("ordered nominal cleanup attachment is not an empty record");
        }
    }
    for parameter in &plan.structural_parameters {
        let shape = published.structural_type(&parameter.type_identity).ok_or(
            LoweringError::Unsupported("ordered nominal cleanup parameter shape is absent"),
        )?;
        if !is_bounded_nominal_cleanup_record(&shape.shape) {
            return unsupported("ordered nominal cleanup parameter shape is outside the bound");
        }
    }

    super::contexts::validate_requirements(checked, nominal)?;
    super::custody::validate_checked_dispositions(checked, nominal)?;

    let mut roots = Vec::new();
    for cleanup in &nominal.cleanups {
        let contract = checked
            .facts
            .contract_plans
            .for_machine(cleanup.cleanup_machine)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target contract is absent",
            ))?;
        // The hook body is an ordinary checked Unit body; the edge invokes
        // whatever it carries.
        if contract.report_fingerprint != cleanup.cleanup_contract_report_fingerprint {
            return unsupported("ordered nominal cleanup target is not exact and bounded");
        }
        if !roots.contains(&cleanup.cleanup_machine) {
            roots.push(cleanup.cleanup_machine);
        }
    }

    // The nominal lane's dispatcher joins the ordinary roster for this
    // closure.
    let plans = UnitPlans::with_staged(&checked.facts.flow.terminal_unit_effects, plan);
    // The dispatcher plus each unique cleanup root anchors a transitive call
    // closure; ordinary machines reached by a hook body lower beside it.
    let mut unit_roots = Vec::with_capacity(roots.len() + 1);
    unit_roots.push(plan.machine);
    unit_roots.extend_from_slice(&roots);
    lower_unit_closure(
        checked,
        &UnitClosureRequest {
            plans,
            entry: plan.machine,
            unit_roots: &unit_roots,
            scalar_entry: false,
        },
    )
}
