//! Source and exact checked target validation shared by boundary-return emitters.

use super::*;

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
    crate::call_source_custody::validate_operation(
        checked,
        plan.machine,
        plan.state,
        &plan.boundary_call,
    )?;
    let plans = &checked.facts.flow.terminal_boundary_scalar_returns;
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        coordinate,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        ..
    } = &plan.boundary_call
    else {
        return unsupported("result-bearing boundary plan does not contain a boundary call");
    };
    if coordinate.statement_index != 0
        || coordinate.call_ordinal != 0
        || plan.return_statement_ordinal != 1
    {
        return unsupported("result-bearing boundary call coordinates are not canonical");
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
