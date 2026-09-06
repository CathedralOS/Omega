//! Boundary-wrapper catalog roots belong to the same module as their Unit callers.

use super::*;

pub(super) fn retain_catalog_roots<'checked>(
    checked: &'checked CheckedTrees,
    callees: &[PreparedScalarCallee<'checked>],
    boundaries: &mut Vec<(&'checked CheckedBoundaryMachinePlan, String)>,
    type_roots: &mut Vec<String>,
    service_roots: &mut Vec<ServiceReachId>,
) -> Result<(), LoweringError> {
    for callee in callees {
        let PreparedScalarCallee::Boundary { plan, .. } = callee else {
            continue;
        };
        let boundary =
            crate::boundary_scalar_return::validate_boundary_scalar_return(checked, plan)?;
        if let Some((existing, _)) = boundaries
            .iter()
            .find(|(candidate, _)| candidate.machine == boundary.machine)
        {
            if *existing != boundary {
                return unsupported("scalar wrapper and Unit boundary declarations disagree");
            }
        } else {
            boundaries.push((
                boundary,
                checked_unit_boundary_identity(checked, boundary.machine)?,
            ));
        }
        type_roots.push(plan.attachment_type_identity.clone());
        collect_installation_machine_contract_services(
            checked,
            plan.machine,
            plan.contract_service_reach,
            plan.service_reach,
            service_roots,
        )?;
        let CheckedUnitEffectOperationPlan::BoundaryCall { service_reach, .. } =
            &plan.boundary_call
        else {
            return unsupported("scalar wrapper lost its boundary operation");
        };
        collect_service_summary(
            &checked.facts.service_reaches.rows,
            *service_reach,
            service_roots,
        )?;
    }
    Ok(())
}
