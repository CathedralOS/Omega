//! Boundary-wrapper catalog roots belong to the same module as their Unit callers.

use super::*;

pub(super) fn retain_catalog_roots<'checked>(
    checked: &'checked CheckedTrees,
    callees: &[CheckedScalarCallee<'checked>],
    boundaries: &mut Vec<(&'checked CheckedBoundaryMachinePlan, String)>,
    type_roots: &mut Vec<String>,
    service_roots: &mut Vec<ServiceReachId>,
) -> Result<(), LoweringError> {
    for callee in callees {
        if let CheckedScalarCallee::Graph(graph) = callee {
            let reach = checked
                .facts
                .service_reaches
                .for_machine(graph.machine)
                .ok_or(LoweringError::Unsupported(
                    "scalar graph lost its checked service contract",
                ))?;
            let contract = checked
                .facts
                .service_reaches
                .plan_for_machine(graph.machine)
                .ok_or(LoweringError::Unsupported(
                    "scalar graph lost its checked service plan",
                ))?;
            collect_installation_machine_contract_services(
                checked,
                graph.machine,
                contract,
                ServiceReachSummary {
                    direct: reach.inferred_direct,
                    transitive: reach.inferred_transitive,
                },
                service_roots,
            )?;
            type_roots.extend(crate::scalar_computations::cases::type_roots(
                checked,
                graph.machine,
            )?);
            type_roots.extend(
                graph
                    .states
                    .iter()
                    .flat_map(|state| &state.primitive_locals)
                    .map(|local| local.type_identity.clone()),
            );
            type_roots.extend(
                graph
                    .states
                    .iter()
                    .flat_map(|state| &state.structural_parameters)
                    .map(|parameter| parameter.type_identity.clone()),
            );
            continue;
        }
        if let CheckedScalarCallee::Structural(plan) = callee {
            crate::structural_scalar_return::validate_scalar_callee(checked, plan)?;
            type_roots.extend(plan.attachment_type_identity.iter().cloned());
            type_roots.extend(
                plan.structural_parameters
                    .iter()
                    .map(|parameter| parameter.type_identity.clone()),
            );
            continue;
        }
        let CheckedScalarCallee::Boundary(plan) = callee else {
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
        type_roots.extend(
            plan.structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity.clone()),
        );
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
