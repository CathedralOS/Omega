//! Source and exact checked target validation shared by boundary-return emitters.

use super::super::checked_unit_target_reach_matches;
use super::{
    CheckedBoundaryMachinePlan, CheckedBoundaryScalarReturnMachinePlan, CheckedTrees,
    LoweringError, source_custody, unsupported,
};
pub(crate) fn validate_boundary_scalar_return<'a>(
    checked: &'a CheckedTrees,
    plan: &CheckedBoundaryScalarReturnMachinePlan,
) -> Result<&'a CheckedBoundaryMachinePlan, LoweringError> {
    let mut registered = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .machines
        .iter()
        .filter(|candidate| candidate.machine == plan.machine);
    if registered.next() != Some(plan) || registered.next().is_some() {
        return unsupported("boundary scalar return has no unique exact checked body registration");
    }
    source_custody::validate(checked, plan)?;
    crate::emission::call_source_custody::validate_operation(
        checked,
        plan.machine,
        plan.state,
        &plan.boundary_call,
        &plan.structural_parameters,
    )?;
    let plans = &checked.facts.flow.terminal_boundary_scalar_returns;
    let call = super::retained_boundary_call(
        plan,
        "result-bearing boundary plan does not contain a boundary call",
    )?;
    let coordinate = &call.coordinate;
    let target_machine = &call.target.machine;
    let target_state = &call.target.state;
    let target_contract_report_fingerprint = &call.target.contract_report_fingerprint;
    let service_reach = &call.target.service_reach;
    if coordinate.statement_index != 0
        || coordinate.call_ordinal != 0
        || plan.return_statement_ordinal != 1
    {
        return unsupported("result-bearing boundary call coordinates are not canonical");
    }
    // The checked Unit plan of the same machine describes this call too. Both
    // custody copies must name one call site, target and source occurrence.
    if let Some(unit) = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(plan.machine)
    {
        // The Unit plan binds the same call's scalar result: a boundary
        // call with a scalar result at the same site and target.
        let authored_site = call.source_site();
        let agrees = unit.operations.iter().any(|operation| {
            operation.call().is_some_and(|unit_call| {
                unit_call.is_boundary()
                    && unit_call.scalar_result().is_some()
                    && unit_call.coordinate == *coordinate
                    && unit_call.source_site() == authored_site
                    && unit_call.target.machine == *target_machine
                    && unit_call.target.state == *target_state
            })
        });
        if !agrees {
            return unsupported(
                "result-bearing boundary call disagrees with the machine's checked Unit plan",
            );
        }
    }
    let mut matches = plans
        .boundary_machines
        .iter()
        .filter(|boundary| boundary.machine == *target_machine);
    let boundary = matches.next().ok_or(LoweringError::Unsupported(
        "result-bearing boundary target is absent from its checked plan",
    ))?;
    if matches.next().is_some()
        || boundary.state != *target_state
        || boundary.contract_report_fingerprint != *target_contract_report_fingerprint
        || boundary.result.scalar() != Some(plan.result_type)
        || !checked_unit_target_reach_matches(*service_reach, boundary.contract_service_reach)
    {
        return unsupported("result-bearing boundary call disagrees with its exact checked target");
    }
    if !boundary.scalar_requires.is_empty() {
        return unsupported("boundary scalar return cannot discharge scalar requirements");
    }
    let exact_identity = checked
        .facts
        .contract_plans
        .for_machine(boundary.contract_owner)
        .map(|contract| (contract.report_fingerprint, contract.commitment))
        .or_else(|| {
            checked
                .facts
                .contract_plans
                .crash_capsule(boundary.contract_owner, boundary.state)
                .map(|capsule| {
                    (
                        capsule.target_contract_report_fingerprint(),
                        capsule.target_contract_commitment(),
                    )
                })
        })
        .ok_or(LoweringError::Unsupported(
            "result-bearing boundary target is missing its canonical contract identity",
        ))?;
    if (
        boundary.contract_report_fingerprint,
        boundary.contract_commitment,
    ) != exact_identity
    {
        return unsupported(
            "result-bearing boundary target contract compatibility coordinate or strong commitment drifted",
        );
    }

    Ok(boundary)
}
