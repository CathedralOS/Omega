//! Scalar callees of a Unit closure: graph, structural-return, and
//! boundary-wrapper machines that Unit bodies call for a scalar result.
//!
//! They belong to the same module as their Unit callers. Their catalog roots
//! join the closure's shared namespaces (`retain_catalog_roots`,
//! `wrapper_domains`), each is prepared against those namespaces (`prepare`),
//! and each is emitted after the Unit bodies that call it (`emit`).

use super::super::{
    ServiceId, ServiceReachId, StructuralParameterDeclaration, StructuralTypeDeclaration,
    TerminalMachine,
};
use super::operation_frame::BoundaryParameters;
use super::{
    CallEmissionContext, CheckedBoundaryMachinePlan, CheckedScalarCallee, CheckedTrees,
    CheckedUnitEffectOperationPlan, ClosureOccurrences, LoweringError, MachineId,
    PreparedScalarCallee, SemanticDomainId, ServiceReachSummary, StructuralDomainId,
    StructuralTypeId, TERMINAL_MACHINE_IDENTITY_STRIDE, checked_unit_boundary_identity,
    collect_installation_machine_contract_services, collect_service_summary, contract_id,
    lookup_machine_id, lower_declared_service_reach, lower_installation_machine_service_ceiling,
    lower_unit_parameters, unsupported,
};
use terminal_psi::{ObligationEvidence, ScalarFloatRange, ScalarIntegerRange};
pub(super) fn retain_catalog_roots<'checked>(
    checked: &'checked CheckedTrees,
    callees: &[CheckedScalarCallee<'checked>],
    boundaries: &mut Vec<(&'checked CheckedBoundaryMachinePlan, String)>,
    type_roots: &mut Vec<String>,
    service_roots: &mut Vec<ServiceReachId>,
) -> Result<(), LoweringError> {
    for callee in callees {
        if let CheckedScalarCallee::Graph(graph) = callee {
            type_roots.extend(graph.states.iter().flat_map(|state| &state.unit_operations).filter_map(|operation| match operation {
                checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } => Some(result.type_identity.clone()),
                _ => None,
            }));
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
            type_roots.extend(crate::scalar_graph::scalar_computations::cases::type_roots(
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
            crate::returns::structural_scalar_return::validate_scalar_callee(checked, plan)?;
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
            crate::returns::boundary_scalar_return::validate_boundary_scalar_return(checked, plan)?;
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

/// The semantic domains boundary-wrapper callees qualify their structural
/// parameters with; the closure's domain catalog must include them.
pub(super) fn wrapper_domains(callees: &[CheckedScalarCallee<'_>]) -> Vec<SemanticDomainId> {
    callees
        .iter()
        .filter_map(|callee| match callee {
            CheckedScalarCallee::Boundary(plan) => Some(plan),
            _ => None,
        })
        .flat_map(|plan| &plan.structural_parameters)
        .flat_map(|parameter| {
            parameter.qualifications.iter().chain(
                parameter
                    .projected_qualifications
                    .iter()
                    .map(|row| &row.domain),
            )
        })
        .copied()
        .collect()
}

/// Scalar callees prepared against the closure's shared catalogs.
pub(super) struct PreparedScalarCallees<'checked> {
    pub(super) machines: Vec<PreparedScalarCallee<'checked>>,
    /// Each callee's structural parameters by source machine, keyed by the
    /// parameter's authored position; only graph callees allocate any here.
    graph_parameters: Vec<(
        symbols::SymbolHandle,
        Vec<(u32, StructuralParameterDeclaration)>,
    )>,
}

/// Allocate each graph callee's structural parameters, then its primitive
/// locals, from the closure's place namespace, and prepare every callee
/// against them.
pub(super) fn prepare<'checked>(
    checked: &'checked CheckedTrees,
    callees: Vec<CheckedScalarCallee<'checked>>,
    type_ids: &[(String, StructuralTypeId)],
    domain_ids: &[(SemanticDomainId, StructuralDomainId)],
    structural_types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<PreparedScalarCallees<'checked>, LoweringError> {
    let allocated = callees
        .iter()
        .map(|callee| {
            let parameters = if let CheckedScalarCallee::Graph(_) = callee {
                lower_unit_parameters(
                    callee.structural_parameters(),
                    type_ids,
                    domain_ids,
                    next_place,
                )?
            } else {
                Vec::new()
            };
            let view_roster: Vec<(u32, StructuralParameterDeclaration)> = callee
                .structural_parameters()
                .iter()
                .zip(parameters.iter())
                .map(|(source, parameter)| (source.position, parameter.clone()))
                .collect();
            let locals = if let CheckedScalarCallee::Graph(graph) = callee {
                crate::scalar_graph::scalar_graph_lowering::primitive_locals::allocate(
                    checked, graph, type_ids, next_place,
                )?
            } else {
                Vec::new()
            };
            Ok((callee.source_machine(), parameters, locals, view_roster))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let machines = callees
        .into_iter()
        .map(|callee| {
            let source = callee.source_machine();
            let (_, parameters, locals, _) = allocated
                .iter()
                .find(|(symbol, _, _, _)| *symbol == source)
                .ok_or(LoweringError::Unsupported(
                    "scalar graph has no allocated parameter namespace",
                ))?;
            callee.prepare(
                checked,
                source,
                parameters,
                locals,
                structural_types,
                next_place,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok(PreparedScalarCallees {
        machines,
        graph_parameters: allocated
            .into_iter()
            .map(|(source, _, _, view_roster)| (source, view_roster))
            .collect(),
    })
}

/// The closure namespaces and identities scalar callee emission reads.
pub(super) struct EmissionCatalog<'a> {
    pub(super) machine_ids: &'a [(symbols::SymbolHandle, MachineId)],
    pub(super) requirement_counts: &'a [(symbols::SymbolHandle, usize)],
    pub(super) boundary_parameters: &'a [BoundaryParameters],
    pub(super) structural_types: &'a [StructuralTypeDeclaration],
    pub(super) type_ids: &'a [(String, StructuralTypeId)],
    pub(super) domain_ids: &'a [(SemanticDomainId, StructuralDomainId)],
    pub(super) service_ids: &'a [(ServiceReachId, ServiceId)],
    /// Roster index of the first scalar callee; each callee's identity range
    /// follows from its own index.
    pub(super) first_machine_index: usize,
}

/// What scalar callee emission publishes beside its machines and occurrence
/// rows.
pub(super) struct EmittedScalarCallees {
    pub(super) evidence: Vec<ObligationEvidence>,
    /// Floating and integer entry ranges are machine-local rows keyed by each
    /// emitted helper's own identities; they merge into the assembled catalog
    /// without sharing the qualification namespace this path keeps empty.
    pub(super) float_entry_ranges: Vec<ScalarFloatRange>,
    pub(super) integer_entry_ranges: Vec<ScalarIntegerRange>,
}

/// Emit each prepared scalar callee in its own identity range: a structural
/// return, a boundary wrapper, or a scalar graph.
pub(super) fn emit(
    checked: &CheckedTrees,
    prepared: PreparedScalarCallees<'_>,
    catalog: EmissionCatalog<'_>,
    next_place: &mut u64,
    next_call_obligation: &mut u64,
    machines: &mut Vec<TerminalMachine>,
    occurrences: &mut ClosureOccurrences,
) -> Result<EmittedScalarCallees, LoweringError> {
    let mut output = EmittedScalarCallees {
        evidence: Vec::new(),
        float_entry_ranges: Vec::new(),
        integer_entry_ranges: Vec::new(),
    };
    for (index, machine) in prepared.machines.into_iter().enumerate() {
        let terminal_machine = lookup_machine_id(catalog.machine_ids, machine.source_machine())?;
        let machine_index =
            catalog
                .first_machine_index
                .checked_add(index)
                .ok_or(LoweringError::Unsupported(
                    "selected scalar closure machine count overflows usize",
                ))?;
        let identity_base = u64::try_from(machine_index)
            .map_err(|_| {
                LoweringError::Unsupported("selected scalar closure machine count exceeds u64")
            })?
            .checked_mul(TERMINAL_MACHINE_IDENTITY_STRIDE)
            .ok_or(LoweringError::Unsupported(
                "selected scalar closure identity range overflows",
            ))?;
        if let PreparedScalarCallee::Structural { plan, .. } = &machine {
            let mut lowered = crate::returns::structural_scalar_return::lower_structural_scalar_return_machine_in_namespace(
                checked, plan, terminal_machine, identity_base, Some(catalog.structural_types),
            )?;
            machines.append(&mut lowered.semantic_module.machines);
            output.evidence.append(&mut lowered.proof_bundle.evidence);
            occurrences.retain_lowered(&mut lowered);
            continue;
        }
        let PreparedScalarCallee::Graph(machine) = machine else {
            let PreparedScalarCallee::Boundary { plan, .. } = machine else {
                unreachable!("scalar callee has exactly one checked body owner")
            };
            let CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. } =
                &plan.boundary_call
            else {
                return unsupported("scalar wrapper lost its boundary operation");
            };
            let boundary = catalog
                .boundary_parameters
                .iter()
                .find_map(|boundary| (boundary.source == *target_machine).then_some(boundary.id))
                .ok_or(LoweringError::Unsupported(
                    "scalar wrapper boundary is absent from the shared catalog",
                ))?;
            let mut context = CallEmissionContext {
                machine_ids: catalog.machine_ids,
                requirement_counts: catalog.requirement_counts,
                next_obligation_identity: *next_call_obligation,
                obligation_limit: u64::MAX,
            };
            let emitted = crate::returns::boundary_scalar_return::emit_boundary_scalar_return(
                checked,
                plan,
                lower_unit_parameters(
                    &plan.structural_parameters,
                    catalog.type_ids,
                    catalog.domain_ids,
                    next_place,
                )?,
                crate::returns::boundary_scalar_return::BoundaryScalarReturnCatalogs {
                    structural_types: catalog.structural_types,
                    type_ids: catalog.type_ids,
                    service_ids: catalog.service_ids,
                },
                crate::returns::boundary_scalar_return::BoundaryScalarReturnIdentities {
                    machine: terminal_machine,
                    contract: contract_id(terminal_machine.get()),
                    boundary,
                    identity_base,
                },
                &mut context,
            )?;
            *next_call_obligation = context.next_obligation_identity;
            machines.push(emitted.machine);
            occurrences.retain(
                emitted.source_call_occurrences,
                emitted.selected_ieee_float_comparison_occurrences,
                emitted.selected_integer_comparison_occurrences,
            );
            continue;
        };
        let graph_parameters = prepared
            .graph_parameters
            .iter()
            .find(|(source, _)| *source == machine.source_machine)
            .ok_or(LoweringError::Unsupported(
                "scalar graph emission lost its allocated parameters",
            ))?;
        let element_views = crate::expression_preparation::bindings::element_views(
            &graph_parameters.1,
            catalog.structural_types,
        );
        let views = crate::scalar_graph::scalar_contracts::ContractViewNamespace {
            parameters: &graph_parameters.1,
            element_views: &element_views,
        };
        if !machine.scalar_qualifications.domains.is_empty() {
            return unsupported(
                "attached scalar graph requires the enclosing qualification catalog namespace",
            );
        }
        let mut lowered =
            crate::scalar_graph::scalar_graph_module::build_scalar_graph_module_in_namespace(
                &machine.states,
                &machine.state_symbols,
                machine.result_type,
                &machine.scalar_qualifications,
                machine.contract,
                machine.crash_routes,
                machine.identity_reshuffles,
                machine.partition_compositions,
                terminal_machine,
                identity_base,
                catalog.machine_ids,
                catalog.requirement_counts,
                &graph_parameters.1,
                &views,
                machine.loop_plan.as_ref(),
            )?;
        let [terminal_machine] = lowered.semantic_module.machines.as_mut_slice() else {
            unreachable!("one prepared selected scalar graph emits one terminal machine")
        };
        let reach = checked
            .facts
            .service_reaches
            .for_machine(machine.source_machine)
            .ok_or(LoweringError::Unsupported(
                "scalar graph lost its checked service contract",
            ))?;
        let contract = checked
            .facts
            .service_reaches
            .plan_for_machine(machine.source_machine)
            .ok_or(LoweringError::Unsupported(
                "scalar graph lost its checked service plan",
            ))?;
        terminal_machine.published_service_ceiling = lower_installation_machine_service_ceiling(
            checked,
            machine.source_machine,
            contract,
            ServiceReachSummary {
                direct: reach.inferred_direct,
                transitive: reach.inferred_transitive,
            },
            catalog.service_ids,
        )?;
        terminal_machine.declared_service_reach =
            lower_declared_service_reach(checked, machine.source_machine, catalog.service_ids)?;
        machines.push(terminal_machine.clone());
        output.float_entry_ranges.append(
            &mut lowered
                .semantic_module
                .scalar_qualifications
                .float_entry_ranges,
        );
        output.integer_entry_ranges.append(
            &mut lowered
                .semantic_module
                .scalar_qualifications
                .integer_entry_ranges,
        );
        output.evidence.append(&mut lowered.proof_bundle.evidence);
        occurrences.retain_lowered(&mut lowered);
    }
    Ok(output)
}
