//! Admit a selected nominal cleanup entry and root its ordinary Unit closure.
//! Signature contexts and return-action finalization are shared with members.

use super::{
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedTrees, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralTypeShape, LoweredPsi, LoweringError, Multiplicity,
    is_bounded_nominal_cleanup_record, lower_unit_closure, unique_unit_machine, unsupported,
};
use crate::unit::attached_unit::UnitClosureRequest;
use crate::unit::attached_unit::bodies::UnitPlans;

pub(super) fn lower_ordered_nominal_affine_unit_cleanup_machine(
    checked: &CheckedTrees,
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
) -> Result<LoweredPsi, LoweringError> {
    let plan = &nominal.machine;
    let parameter_count = plan.structural_parameters.len();
    if parameter_count == 0 || nominal.cleanups.len() != parameter_count {
        return unsupported("ordered nominal cleanup requires matched actions");
    }
    let [
        CheckedUnitEffectOperationPlan::Complete {
            statement_index: 0,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        },
    ] = plan.operations.as_slice()
    else {
        return unsupported("ordered nominal cleanup caller operation sequence drifted");
    };
    if checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .any(|candidate| candidate.machine == plan.machine)
        || !plan.trivial_affine_locals.is_empty()
        || !plan.entry_claims.is_empty()
        || !plan.body_qualifications.is_empty()
        || !trivial_affine_local_discard_ordinals.is_empty()
        || !trivial_affine_discards.is_empty()
    {
        return unsupported("ordered nominal cleanup caller signature drifted");
    }
    for (position, parameter) in plan.structural_parameters.iter().enumerate() {
        let cleanup = &nominal.cleanups[parameter_count - position - 1];
        if usize::try_from(parameter.position).ok() != Some(position)
            || usize::try_from(cleanup.source_parameter_index).ok() != Some(position)
            || parameter.is_self
            || parameter.multiplicity != Multiplicity::Affine
            || !parameter.qualifications.is_empty()
            || cleanup.type_identity != parameter.type_identity
            || cleanup.cleanup_machine == plan.machine
            || cleanup.cleanup_contract_report_fingerprint == 0
        {
            return unsupported("ordered nominal cleanup parameter join drifted");
        }
    }

    let nominal_types = &checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .structural_types;
    if nominal_types
        .iter()
        .any(|candidate| candidate.identity.is_empty())
        || nominal_types.iter().enumerate().any(|(index, candidate)| {
            nominal_types[..index]
                .iter()
                .any(|earlier| earlier.identity == candidate.identity)
        })
    {
        return unsupported("ordered nominal cleanup structural types are empty or duplicated");
    }
    // A receiver-free caller, qualified or not, retains no attachment
    // storage. A retained owner must be an empty record: the dispatcher
    // carries no attachment state across its cleanup edges.
    if let Some(attachment) = plan.attachment_type_identity.as_deref() {
        let attachment_shape = nominal_types
            .iter()
            .find(|candidate| candidate.identity == attachment)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup attachment shape is absent",
            ))?;
        if !matches!(&attachment_shape.shape, CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty())
        {
            return unsupported("ordered nominal cleanup attachment is not an empty record");
        }
    }
    for parameter in &plan.structural_parameters {
        let shape = nominal_types
            .iter()
            .find(|candidate| candidate.identity == parameter.type_identity)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup parameter shape is absent",
            ))?;
        if !is_bounded_nominal_cleanup_record(&shape.shape) {
            return unsupported("ordered nominal cleanup parameter shape is outside the bound");
        }
    }

    super::contexts::validate_requirements(checked, nominal)?;

    let published = UnitPlans::published(&checked.facts.flow.terminal_unit_effects);
    let mut roots = Vec::new();
    for cleanup in &nominal.cleanups {
        let target = unique_unit_machine(published, cleanup.cleanup_machine)?;
        let contract = checked
            .facts
            .contract_plans
            .for_machine(cleanup.cleanup_machine)
            .ok_or(LoweringError::Unsupported(
                "ordered nominal cleanup target contract is absent",
            ))?;
        // The hook body is an ordinary checked Unit body; the edge invokes
        // whatever it carries. Selection stays pinned — this owner's attached
        // `T::drop` state, the recorded contract identity, no scalar
        // arguments, and at most the exclusive borrowed receiver the edge lends.
        super::validate_nominal_cleanup_target(target, cleanup)?;
        if contract.report_fingerprint != cleanup.cleanup_contract_report_fingerprint {
            return unsupported("ordered nominal cleanup target is not exact and bounded");
        }
        if !roots.contains(&cleanup.cleanup_machine) {
            roots.push(cleanup.cleanup_machine);
        }
    }

    // The nominal lane's dispatcher and shapes join the ordinary roster for
    // this closure; a shape both lanes publish must agree.
    for shape in nominal_types {
        if published
            .structural_type(&shape.identity)
            .is_some_and(|existing| existing != shape)
        {
            return unsupported("ordered nominal cleanup structural type conflicts");
        }
    }
    let plans = UnitPlans::with_staged(
        &checked.facts.flow.terminal_unit_effects,
        plan,
        nominal_types,
    );
    // The dispatcher plus each unique cleanup root anchors a transitive call
    // closure; ordinary machines reached by a hook body lower beside it.
    let mut unit_roots = Vec::with_capacity(roots.len() + 1);
    unit_roots.push(plan.machine);
    unit_roots.extend_from_slice(&roots);
    let lowered = lower_unit_closure(
        checked,
        &UnitClosureRequest {
            plans,
            entry: plan.machine,
            unit_roots: &unit_roots,
            external: None,
            scalar_entry: false,
        },
    )?
    .lowered;
    Ok(lowered)
}
