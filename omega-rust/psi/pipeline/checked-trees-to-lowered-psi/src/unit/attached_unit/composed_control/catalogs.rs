//! Selected type, boundary, and service catalogs for composed Unit control.
use super::super::super::{
    BoundaryMachineId, MachineId, ServiceDeclaration, ServiceId, ServiceReachId, ServiceReachPlan,
    StructuralDomainDeclaration, StructuralTypeDeclaration,
};
use super::super::{
    BoundaryMachineDeclaration, BoundaryMachineResult, CheckedBoundaryMachinePlan,
    CheckedBoundaryMachineResultPlan, CheckedUnitEffectOperationPlan, LoweredPsi, ScalarType,
    SemanticDomainId, ServiceReachSummary, StructuralDomainId, StructuralDomainRequirement,
    StructuralPlaceDeclaration, StructuralTypeId, ValueDeclaration, boundary_machine_id,
    dense_identity, lookup_domain_id, lookup_type_id, lower_boundary_content_guarantees,
    lower_boundary_crash_routes, lower_boundary_result, lower_fixed_boundary_service_reach,
    lower_published_service_ceiling, lower_root_service_reach, lower_unit_parameters,
    terminal_scalar_type, unsupported,
};
use super::{CheckedTrees, LoweringError, internal_calls, scalar_calls};
use crate::unit::attached_unit::bodies::{UnitBody, UnitPlans};
use crate::unit::attached_unit::catalog::{
    collect_installation_machine_contract_services, collect_published_contract_services,
    collect_service_summary, lower_program_local_root_introductions, lower_selected_unit_services,
    lower_unit_structural_domains_including, lower_unit_structural_type_roots,
};
use semantic_vocabulary::Proposition;
use std::borrow::Cow;

/// Standalone roots own their publication tables; shared callees borrow the
/// already allocated closure. Only owned type tables admit generated carriers.
pub(crate) struct ComposedCatalogs<'a> {
    pub(crate) structural_types: Cow<'a, [StructuralTypeDeclaration]>,
    pub(crate) type_ids: Cow<'a, [(String, StructuralTypeId)]>,
    pub(crate) structural_domains: Cow<'a, [StructuralDomainDeclaration]>,
    pub(crate) domain_ids: Cow<'a, [(SemanticDomainId, StructuralDomainId)]>,
    pub(crate) services: Cow<'a, [ServiceDeclaration]>,
    pub(crate) root_service_reach: Cow<'a, terminal_psi::TerminalRootServiceReach>,
    pub(crate) boundary_machines: Cow<'a, [BoundaryMachineDeclaration]>,
    pub(crate) lowered_boundaries: Vec<LoweredComposedBoundary>,
    pub(crate) internal_targets: Vec<LoweredComposedInternalTarget>,
    pub(crate) service_ids: Cow<'a, [(ServiceReachId, ServiceId)]>,
    pub(crate) next_place: u64,
    /// Private constructor, join, and literal places; not authored result ordinals.
    pub(crate) temporary_places: Vec<StructuralPlaceDeclaration>,
    pub(crate) result_places: Vec<StructuralPlaceDeclaration>,
    pub(crate) scalar_calls: scalar_calls::ComposedScalarCalls,
    pub(crate) root_crash_routes: Vec<checked_trees::CrashRouteBucket>,
    pub(crate) shared_units: Option<LoweredPsi>,
    pub(crate) next_value: u64,
    pub(crate) next_block: u64,
    pub(crate) next_operation: u64,
    pub(crate) next_edge: u64,
}

fn lower_composed_services(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    contract_service_reach: ServiceReachPlan,
    service_reach: ServiceReachSummary,
    states: &[checked_trees::CheckedComposedUnitControlStatePlan],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    internal_targets: &[(UnitBody<'_>, String)],
) -> Result<(Vec<ServiceDeclaration>, Vec<(ServiceReachId, ServiceId)>), LoweringError> {
    let facts = &checked.facts.service_reaches;
    let mut selected = Vec::new();
    collect_installation_machine_contract_services(
        checked,
        machine,
        contract_service_reach,
        service_reach,
        &mut selected,
    )?;
    for operation in states
        .iter()
        .flat_map(|state| state.operation_dependencies())
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
    {
        let service_reach = match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { service_reach, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { service_reach, .. }
            | CheckedUnitEffectOperationPlan::ScalarCall { service_reach, .. }
            | CheckedUnitEffectOperationPlan::CallUnit { service_reach, .. } => *service_reach,
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
            | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
            | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
            | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
            | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
            | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
            | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
            | CheckedUnitEffectOperationPlan::WriteOnlyIndexedPrimitiveStore { .. } => continue,
            _ => return unsupported("composed Unit control contains a non-call operation"),
        };
        collect_service_summary(&facts.rows, service_reach, &mut selected)?;
    }
    for (boundary, _) in boundaries {
        collect_published_contract_services(
            &facts.rows,
            boundary.contract_service_reach,
            boundary.service_reach,
            &mut selected,
        )?;
    }
    for (target, _) in internal_targets {
        let entry = target.entry()?;
        collect_installation_machine_contract_services(
            checked,
            entry.machine,
            entry.contract_service_reach,
            target.service_reach(),
            &mut selected,
        )?;
    }
    lower_selected_unit_services(checked, selected)
}

pub(crate) struct LoweredComposedInternalTarget {
    pub(super) result: checked_trees::CheckedControlResultPlan,
    pub(super) source: symbols::SymbolHandle,
    pub(super) id: MachineId,
    pub(super) scalar_parameters: Vec<ScalarType>,
    /// The callee's published erased-formal roster; a call carries one
    /// proof-only erased argument per row.
    pub(super) erased_scalar_formals: Vec<ValueDeclaration>,
    /// The callee's published erased-proof roster in contract order; a call
    /// carries one erased proof argument per row.
    pub(super) erased_proof_formals: Vec<terminal_psi::ErasedProofFormal>,
    /// The callee's published `requires` clauses in contract order; the call
    /// allocates one obligation per row.
    pub(super) requires: Vec<Proposition>,
    pub(super) structural_parameters: Vec<checked_trees::CheckedUnitStructuralParameterPlan>,
    pub(super) parameter_relative_crash_routes: Vec<checked_trees::CrashRouteBucket>,
}

pub(crate) struct LoweredComposedBoundary {
    pub(crate) source: symbols::SymbolHandle,
    pub(crate) id: BoundaryMachineId,
    pub(crate) checked_structural_parameters:
        Vec<checked_trees::CheckedUnitStructuralParameterPlan>,
    pub(crate) scalar_parameters: Vec<ScalarType>,
    pub(crate) result: BoundaryMachineResult,
    /// The checked boundary's declared result domains in semantic form, kept so
    /// emission can replay the caller-side establishment mint for each one.
    pub(crate) result_domains: Vec<SemanticDomainId>,
}

pub(crate) fn lower_dynamic_catalogs(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedDynamicScalarCallPlan,
    continuation: &checked_trees::CheckedDynamicUnitContinuationPlan,
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    internal_targets: &[(UnitBody<'_>, String)],
) -> Result<ComposedCatalogs<'static>, LoweringError> {
    let contract_service_reach = checked
        .facts
        .service_reaches
        .plan_for_machine(plan.caller_machine)
        .ok_or(LoweringError::Unsupported(
            "direct dynamic continuation has no checked service contract",
        ))?;
    lower_catalogs(
        checked,
        plan.caller_machine,
        Some(&plan.caller_attachment_type_identity),
        contract_service_reach,
        plan.caller_service_reach,
        &continuation.leaves,
        boundaries,
        internal_targets,
    )
}

#[allow(clippy::too_many_arguments)]
fn lower_catalogs(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    attachment_type_identity: Option<&str>,
    contract_service_reach: ServiceReachPlan,
    service_reach: ServiceReachSummary,
    states: &[checked_trees::CheckedComposedUnitControlStatePlan],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    admitted_internal_targets: &[(UnitBody<'_>, String)],
) -> Result<ComposedCatalogs<'static>, LoweringError> {
    if !admitted_internal_targets.is_empty() {
        return internal_calls::catalogs::lower(
            checked,
            machine,
            attachment_type_identity,
            contract_service_reach,
            service_reach,
            states,
            boundaries,
            admitted_internal_targets,
        );
    }
    let mut type_roots = attachment_type_identity
        .map(str::to_owned)
        .into_iter()
        .collect::<Vec<_>>();
    type_roots.extend(crate::scalar_graph::scalar_computations::cases::type_roots(
        checked, machine,
    )?);
    type_roots.extend(
        states
            .iter()
            .flat_map(|state| &state.structural_parameters)
            .map(|parameter| parameter.type_identity.clone()),
    );
    type_roots.extend(
        states
            .iter()
            .flat_map(|state| state.operation_dependencies())
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } => {
                    Some(result.type_identity.clone())
                }
                _ => None,
            }),
    );
    for (boundary, _) in boundaries {
        type_roots.extend(boundary.attachment_type_identity.iter().cloned());
        type_roots.extend(
            boundary
                .structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity.clone()),
        );
        if let Some(identity) = boundary.result.structural_identity() {
            type_roots.push(identity.to_owned());
        }
    }
    type_roots.extend(
        admitted_internal_targets
            .iter()
            .filter_map(|(target, _)| target.attachment().map(str::to_owned)),
    );
    let plans = UnitPlans::published(&checked.facts.flow.terminal_unit_effects);
    let (structural_types, type_ids) =
        lower_unit_structural_type_roots(checked, plans, &type_roots)?;
    let mut domain_roots = plans
        .for_machine(machine)
        .into_iter()
        .flat_map(|plan| {
            plan.structural_parameters
                .iter()
                .flat_map(|parameter| {
                    parameter.qualifications.iter().chain(
                        parameter
                            .projected_qualifications
                            .iter()
                            .map(|row| &row.domain),
                    )
                })
                .chain(plan.body_qualifications.iter())
        })
        .chain(
            plans
                .composed_for_machine(machine)
                .into_iter()
                .flat_map(|plan| plan.body_qualifications.iter()),
        )
        .chain(states.iter().flat_map(|state| {
            state.structural_parameters.iter().flat_map(|parameter| {
                parameter.qualifications.iter().chain(
                    parameter
                        .projected_qualifications
                        .iter()
                        .map(|row| &row.domain),
                )
            })
        }))
        .chain(boundaries.iter().flat_map(|(boundary, _)| {
            boundary
                .structural_parameters
                .iter()
                .flat_map(|parameter| parameter.qualifications.iter())
                .chain(match &boundary.result {
                    CheckedBoundaryMachineResultPlan::Structural { qualifications, .. } => {
                        qualifications.as_slice()
                    }
                    CheckedBoundaryMachineResultPlan::Unit
                    | CheckedBoundaryMachineResultPlan::Scalar(_) => &[],
                })
        }))
        .copied()
        .collect::<Vec<SemanticDomainId>>();
    domain_roots.sort_by_key(|domain| domain.0);
    domain_roots.dedup();
    let (structural_domains, domain_ids) = lower_unit_structural_domains_including(
        checked,
        plans,
        &[],
        boundaries,
        &type_ids,
        &domain_roots,
    )?;
    let (services, service_ids) = lower_composed_services(
        checked,
        machine,
        contract_service_reach,
        service_reach,
        states,
        boundaries,
        admitted_internal_targets,
    )?;
    let root_service_reach = lower_root_service_reach(checked, machine, &service_ids)?;
    let mut boundary_machines = Vec::with_capacity(boundaries.len());
    let mut lowered_boundaries = Vec::with_capacity(boundaries.len());
    let internal_targets = Vec::new();
    let mut next_place = 1_u64;
    for (index, (boundary, identity)) in boundaries.iter().enumerate() {
        let scalar_parameters = boundary
            .scalar_parameters
            .iter()
            .map(|parameter| terminal_scalar_type(parameter.primitive_type))
            .collect::<Result<Vec<_>, _>>()?;
        let id = boundary_machine_id(dense_identity(index)?);
        let structural_parameters = lower_unit_parameters(
            &boundary.structural_parameters,
            &type_ids,
            &domain_ids,
            &mut next_place,
        )?;
        let result = lower_boundary_result(&boundary.result, &type_ids, &domain_ids)?;
        let mut requires = boundary
            .domain_requirements
            .iter()
            .map(|requirement| {
                if usize::try_from(requirement.argument_index)
                    .ok()
                    .is_none_or(|index| index >= structural_parameters.len())
                {
                    return Err(LoweringError::Unsupported(
                        "boundary structural requirement has an invalid argument index",
                    ));
                }
                Ok(StructuralDomainRequirement {
                    argument_index: requirement.argument_index,
                    domain: lookup_domain_id(&domain_ids, requirement.domain)?,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        requires.sort();
        let original_requirement_count = requires.len();
        requires.dedup();
        if requires.len() != original_requirement_count {
            return unsupported("boundary structural requirements contain duplicates");
        }
        boundary_machines.push(BoundaryMachineDeclaration {
            parameter_order: crate::unit::attached_unit::lower_boundary_parameter_order(
                &boundary.scalar_parameters,
                &boundary.structural_parameters,
            )?,
            id,
            identity: identity.clone(),
            attachment: boundary
                .attachment_type_identity
                .as_ref()
                .map(|identity| lookup_type_id(&type_ids, identity))
                .transpose()?,
            scalar_parameters: scalar_parameters.clone(),
            crash_routes: lower_boundary_crash_routes(checked, boundary, &scalar_parameters)?,
            structural_parameters: structural_parameters.clone(),
            result: result.clone(),
            requires,
            program_local_root_introductions: lower_program_local_root_introductions(
                checked,
                boundary,
                identity,
                &structural_parameters,
                &domain_ids,
            )?,
            content_guarantees: lower_boundary_content_guarantees(
                &checked.facts.qualifications.content.conservation_plans,
                boundary.state,
            )?,
            fixed_service_reach: lower_fixed_boundary_service_reach(
                checked,
                boundary,
                &service_ids,
            )?,
            published_service_ceiling: lower_published_service_ceiling(
                &checked.facts.service_reaches.rows,
                boundary.contract_service_reach,
                boundary.service_reach,
                &service_ids,
            )?,
        });
        lowered_boundaries.push(LoweredComposedBoundary {
            source: boundary.machine,
            id,
            checked_structural_parameters: boundary.structural_parameters.clone(),
            scalar_parameters,
            result_domains: match &boundary.result {
                CheckedBoundaryMachineResultPlan::Structural { qualifications, .. } => {
                    qualifications.clone()
                }
                CheckedBoundaryMachineResultPlan::Unit
                | CheckedBoundaryMachineResultPlan::Scalar(_) => Vec::new(),
            },
            result,
        });
    }
    let scalar_calls = scalar_calls::prepare(checked, machine, states, &internal_targets)?;
    let root_crash_routes = crate::unit::effective_crash_routes(checked, machine)?;
    Ok(ComposedCatalogs {
        structural_types: structural_types.into(),
        type_ids: type_ids.into(),
        structural_domains: structural_domains.into(),
        domain_ids: domain_ids.into(),
        services: services.into(),
        root_service_reach: Cow::Owned(root_service_reach),
        boundary_machines: boundary_machines.into(),
        lowered_boundaries,
        internal_targets,
        service_ids: service_ids.into(),
        next_place,
        temporary_places: Vec::new(),
        result_places: Vec::new(),
        scalar_calls,
        root_crash_routes,
        shared_units: None,
        next_value: 1,
        next_block: 1,
        next_operation: 1,
        next_edge: 1,
    })
}
