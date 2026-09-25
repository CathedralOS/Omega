//! Boundary machines a Unit closure calls: retaining each exact checked
//! target once, and declaring the retained roster in the closure's shared
//! type, domain, and service namespaces.
//!
//! Both the shared closure and a standalone composed root declare their
//! boundaries here, so a boundary's parameters, requirements, crash routes,
//! and service ceilings have one lowering.

use super::super::{ServiceId, ServiceReachId};
use super::operation_frame::BoundaryParameters;
use super::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryStructuralResultDeclaration,
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan, CheckedTrees, LoweringError,
    Multiplicity, SemanticDomainId, ServiceReachSummary, StructuralDomainId,
    StructuralDomainRequirement, StructuralMultiplicity, StructuralTypeId, UnitPlans,
    boundary_machine_id, checked_unit_boundary_identity, checked_unit_target_reach_matches,
    dense_identity, lookup_domain_id, lookup_type_id, lower_boundary_content_guarantees,
    lower_boundary_crash_routes, lower_boundary_parameter_order,
    lower_fixed_boundary_service_reach, lower_published_service_ceiling, lower_unit_parameters,
    terminal_scalar_type, unique_unit_boundary, unsupported,
};
use crate::terminal_identities::value_id;
use crate::unit::attached_unit::catalog::lower_program_local_root_introductions;
use crate::unit::{Proposition, ScalarType, ValueDeclaration};

/// Declare each retained boundary, in roster order, with the parameter roster
/// its callers bind against. Structural parameters take places from
/// `next_place`.
pub(super) fn lower_declarations(
    checked: &CheckedTrees,
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    type_ids: &[(String, StructuralTypeId)],
    domain_ids: &[(SemanticDomainId, StructuralDomainId)],
    service_ids: &[(ServiceReachId, ServiceId)],
    next_place: &mut u64,
) -> Result<(Vec<BoundaryMachineDeclaration>, Vec<BoundaryParameters>), LoweringError> {
    let mut declarations = Vec::with_capacity(boundaries.len());
    let mut parameter_rosters = Vec::with_capacity(boundaries.len());
    for (index, (plan, identity)) in boundaries.iter().enumerate() {
        let parameters = lower_unit_parameters(
            &plan.structural_parameters,
            type_ids,
            domain_ids,
            next_place,
        )?;
        let scalar_parameters = plan
            .scalar_parameters
            .iter()
            .map(|parameter| terminal_scalar_type(parameter.primitive_type))
            .collect::<Result<Vec<_>, _>>()?;
        let requires = lower_requirements(plan, parameters.len(), domain_ids)?;
        let scalar_requires = lower_scalar_requires(plan, &scalar_parameters)?;
        let requirement_count = scalar_requires.len();
        let id = boundary_machine_id(dense_identity(index)?);
        declarations.push(BoundaryMachineDeclaration {
            scalar_requires,
            parameter_order: lower_boundary_parameter_order(
                &plan.scalar_parameters,
                &plan.structural_parameters,
            )?,
            id,
            identity: identity.clone(),
            attachment: plan
                .attachment_type_identity
                .as_ref()
                .map(|identity| lookup_type_id(type_ids, identity))
                .transpose()?,
            scalar_parameters: scalar_parameters.clone(),
            crash_routes: lower_boundary_crash_routes(checked, plan, &scalar_parameters)?,
            structural_parameters: parameters.clone(),
            result: lower_boundary_result(&plan.result, type_ids, domain_ids)?,
            requires,
            program_local_root_introductions: lower_program_local_root_introductions(
                checked,
                plan,
                identity,
                &parameters,
                domain_ids,
            )?,
            content_guarantees: lower_boundary_content_guarantees(
                &checked.facts.qualifications.content.conservation_plans,
                plan.state,
            )?,
            fixed_service_reach: lower_fixed_boundary_service_reach(checked, plan, service_ids)?,
            published_service_ceiling: lower_published_service_ceiling(
                &checked.facts.service_reaches.rows,
                plan.contract_service_reach,
                plan.service_reach,
                service_ids,
            )?,
        });
        parameter_rosters.push(BoundaryParameters {
            source: plan.machine,
            id,
            structural: parameters,
            scalar: scalar_parameters,
            requirement_count,
        });
    }
    Ok((declarations, parameter_rosters))
}

/// A boundary's scalar `requires` rows in its declaration-local formal
/// telescope (scalar lane position `k` is formal `ValueId` `k + 1`, the crash
/// routes' namespace), normalized as a callee's published requires are: no
/// `true` rows, conjunctions split into rows, canonical order, no repeats.
fn lower_scalar_requires(
    plan: &CheckedBoundaryMachinePlan,
    scalar_types: &[ScalarType],
) -> Result<Vec<Proposition>, LoweringError> {
    if plan.scalar_requires.is_empty() {
        return Ok(Vec::new());
    }
    let formals = scalar_types
        .iter()
        .enumerate()
        .map(|(position, scalar_type)| {
            Ok(ValueDeclaration {
                id: value_id(dense_identity(position)?),
                scalar_type: *scalar_type,
                qualifications: Default::default(),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut pending = plan
        .scalar_requires
        .iter()
        .map(|row| {
            crate::scalar_graph::scalar_contracts::clauses(
                std::slice::from_ref(&Some(row.clone())),
                &formals,
                &[],
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .rev()
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    while let Some(row) = pending.pop() {
        match row {
            Proposition::Truth => {}
            Proposition::Conjunction(conjuncts) => pending.extend(conjuncts.into_iter().rev()),
            other => rows.push(other),
        }
    }
    crate::scalar_graph::scalar_contracts::canonicalize_requires(&mut rows)?;
    Ok(rows)
}

/// A boundary's structural domain requirements, each naming one of its
/// structural parameters, in canonical order and without repetition.
fn lower_requirements(
    plan: &CheckedBoundaryMachinePlan,
    parameter_count: usize,
    domain_ids: &[(SemanticDomainId, StructuralDomainId)],
) -> Result<Vec<StructuralDomainRequirement>, LoweringError> {
    let mut requires = plan
        .domain_requirements
        .iter()
        .map(|requirement| {
            if usize::try_from(requirement.argument_index)
                .ok()
                .is_none_or(|index| index >= parameter_count)
            {
                return Err(LoweringError::Unsupported(
                    "boundary structural requirement has an invalid argument index",
                ));
            }
            Ok(StructuralDomainRequirement {
                argument_index: requirement.argument_index,
                domain: lookup_domain_id(domain_ids, requirement.domain)?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    requires.sort();
    let original_requirement_count = requires.len();
    requires.dedup();
    if requires.len() != original_requirement_count {
        return unsupported("boundary structural requirements contain duplicates");
    }
    Ok(requires)
}

pub(super) fn lower_boundary_result(
    result: &CheckedBoundaryMachineResultPlan,
    type_ids: &[(String, StructuralTypeId)],
    domain_ids: &[(SemanticDomainId, StructuralDomainId)],
) -> Result<BoundaryMachineResult, LoweringError> {
    Ok(match result {
        CheckedBoundaryMachineResultPlan::Unit => BoundaryMachineResult::Unit,
        CheckedBoundaryMachineResultPlan::Scalar(scalar) => {
            BoundaryMachineResult::Scalar(terminal_scalar_type(*scalar)?)
        }
        CheckedBoundaryMachineResultPlan::Structural {
            type_identity,
            multiplicity,
            qualifications,
        } => BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
            structural_type: lookup_type_id(type_ids, type_identity)?,
            multiplicity: match multiplicity {
                Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                Multiplicity::Affine => StructuralMultiplicity::Affine,
                Multiplicity::Linear => StructuralMultiplicity::Linear,
            },
            qualifications: qualifications
                .iter()
                .map(|domain| lookup_domain_id(domain_ids, *domain))
                .collect::<Result<Vec<_>, _>>()?,
        }),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn retain_exact_unit_boundary<'plans>(
    checked: &CheckedTrees,
    plans: UnitPlans<'plans>,
    boundaries: &mut Vec<(&'plans CheckedBoundaryMachinePlan, String)>,
    target_machine: symbols::SymbolHandle,
    target_state: symbols::SymbolHandle,
    target_contract_report_fingerprint: u64,
    service_reach: ServiceReachSummary,
    expected_result: CheckedBoundaryMachineResultPlan,
) -> Result<(), LoweringError> {
    let target = unique_unit_boundary(plans, target_machine)?;
    if target.contract_report_fingerprint == 0 {
        return unsupported("Unit boundary target has a null checked contract fingerprint");
    }
    if target.state != target_state
        || target.contract_report_fingerprint != target_contract_report_fingerprint
        || target.result != expected_result
        || !checked_unit_target_reach_matches(service_reach, target.contract_service_reach)
    {
        return unsupported(
            "Unit boundary call does not match the exact checked target state, result, contract, and reach",
        );
    }
    let exact_identity = checked
        .facts
        .contract_plans
        .for_machine(target.contract_owner)
        .map(|contract| (contract.report_fingerprint, contract.commitment))
        .or_else(|| {
            checked
                .facts
                .contract_plans
                .crash_capsule(target.contract_owner, target.state)
                .map(|capsule| {
                    (
                        capsule.target_contract_report_fingerprint(),
                        capsule.target_contract_commitment(),
                    )
                })
        })
        .ok_or(LoweringError::Unsupported(
            "Unit boundary target is missing its canonical checked contract identity",
        ))?;
    if (
        target.contract_report_fingerprint,
        target.contract_commitment,
    ) != exact_identity
    {
        return unsupported(
            "Unit boundary target contract compatibility coordinate or strong commitment drifted",
        );
    }
    if !boundaries
        .iter()
        .any(|(candidate, _)| candidate.machine == target.machine)
    {
        boundaries.push((
            target,
            checked_unit_boundary_identity(checked, target.machine)?,
        ));
    }
    Ok(())
}
