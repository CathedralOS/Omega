//! Scalar callees of a Unit closure: the scalar graph machines that Unit bodies
//! call for a scalar result. A callee without a scalar graph is an ordinary
//! Unit body with a scalar completion and lowers with the closure itself.
//!
//! They belong to the same module as their Unit callers. Their catalog roots
//! join the closure's shared namespaces (`retain_catalog_roots`), each is
//! prepared against those namespaces (`prepare`), and each is emitted after
//! the Unit bodies that call it (`emit`).

use super::super::{
    ServiceId, ServiceReachId, StructuralParameterDeclaration, StructuralTypeDeclaration,
    TerminalMachine,
};
use super::operation_frame::BoundaryParameters;
use super::{
    CheckedScalarCallee, CheckedTrees, ClosureOccurrences, LoweringError, MachineId,
    PreparedScalarCallee, ServiceReachSummary, StructuralTypeId, TERMINAL_MACHINE_IDENTITY_STRIDE,
    collect_installation_machine_contract_services, lookup_machine_id,
    lower_declared_service_reach, lower_installation_machine_service_ceiling,
    lower_unit_parameters, unsupported,
};
use crate::expression_preparation::qualifications::PreparedScalarQualifications;
use crate::scalar_graph::scalar_graph_lowering::ScalarContractMode;
use crate::terminal_identities::StructuralDomainBinding;
use terminal_psi::{ScalarBlockInvariant, ScalarQualificationCatalog};
pub(super) fn retain_catalog_roots<'checked>(
    checked: &'checked CheckedTrees,
    callees: &[CheckedScalarCallee<'checked>],
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
            type_roots.extend(
                crate::scalar_graph::scalar_computations::operand_types::type_roots(
                    checked,
                    graph.machine,
                )?,
            );
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
        }
    }
    Ok(())
}

/// Scalar callees prepared against the closure's shared catalogs.
pub(super) struct PreparedScalarCallees {
    pub(super) machines: Vec<PreparedScalarCallee>,
    qualifications: ScalarQualificationCatalog,
    /// The closure's canonical scalar qualification namespace: a call's
    /// emitted result republishes the callee's qualified result through it.
    pub(super) prepared_qualifications:
        crate::expression_preparation::qualifications::PreparedScalarQualifications,
    /// Each callee's structural parameters by source machine, keyed by the
    /// parameter's authored position; only graph callees allocate any here.
    graph_parameters: Vec<(
        symbols::SymbolHandle,
        Vec<(u32, StructuralParameterDeclaration)>,
    )>,
}

/// Allocate each graph callee's structural parameters, then its primitive
/// locals, from the closure's place namespace, and prepare every callee
/// against them and the closure's one qualification namespace.
pub(super) fn prepare<'checked>(
    checked: &'checked CheckedTrees,
    direct_result: symbols::SymbolHandle,
    qualifications: PreparedScalarQualifications,
    callees: Vec<CheckedScalarCallee<'checked>>,
    type_ids: &[(String, StructuralTypeId)],
    domain_ids: &[StructuralDomainBinding],
    structural_types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<PreparedScalarCallees, LoweringError> {
    let allocated = callees
        .iter()
        .map(|callee| {
            let parameters = if let CheckedScalarCallee::Graph(_) = callee {
                lower_unit_parameters(
                    callee.structural_parameters(),
                    type_ids,
                    structural_types,
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
                &qualifications,
                if source == direct_result {
                    ScalarContractMode::RootDirectResult
                } else {
                    ScalarContractMode::ClosedRuntimeValue
                },
                parameters,
                locals,
                structural_types,
                next_place,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok(PreparedScalarCallees {
        machines,
        qualifications: qualifications.catalog().clone(),
        prepared_qualifications: qualifications,
        graph_parameters: allocated
            .into_iter()
            .map(|(source, _, _, view_roster)| (source, view_roster))
            .collect(),
    })
}

/// The closure namespaces and identities scalar callee emission reads.
pub(super) struct EmissionCatalog<'a> {
    pub(super) cleanup_receivers: &'a [(symbols::SymbolHandle, semantic_vocabulary::PlaceId)],
    pub(super) machine_ids: &'a [(symbols::SymbolHandle, MachineId)],
    pub(super) requirement_counts: &'a [(symbols::SymbolHandle, usize)],
    pub(super) boundary_parameters: &'a [BoundaryParameters],
    pub(super) structural_types: &'a [StructuralTypeDeclaration],
    pub(super) domain_ids: &'a [StructuralDomainBinding],
    pub(super) service_ids: &'a [(ServiceReachId, ServiceId)],
    /// Roster index of the first scalar callee; each callee's identity range
    /// follows from its own index.
    pub(super) first_machine_index: usize,
}

/// What scalar callee emission publishes beside its machines and occurrence
/// rows.
pub(super) struct EmittedScalarCallees {
    pub(super) scalar_block_invariants: Vec<ScalarBlockInvariant>,
    /// Domains and sets share one prepared namespace. Coercions and entry
    /// ranges additionally retain their emitted machine-local identities.
    pub(super) qualifications: ScalarQualificationCatalog,
}

/// Emit each prepared scalar graph callee in its own identity range.
pub(super) fn emit(
    checked: &CheckedTrees,
    prepared: PreparedScalarCallees,
    catalog: EmissionCatalog<'_>,
    next_place: &mut u64,
    machines: &mut Vec<TerminalMachine>,
    occurrences: &mut ClosureOccurrences,
) -> Result<EmittedScalarCallees, LoweringError> {
    let mut output = EmittedScalarCallees {
        scalar_block_invariants: Vec::new(),
        qualifications: prepared.qualifications,
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
        let PreparedScalarCallee::Graph(machine) = machine;
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
            result: None,
            parameters: &graph_parameters.1,
            structural_types: catalog.structural_types,
            element_views: &element_views,
        };
        if machine.scalar_qualifications.domains != output.qualifications.domains
            || machine.scalar_qualifications.sets != output.qualifications.sets
        {
            return unsupported("scalar graph changed the shared qualification namespace");
        }
        let mut lowered =
            crate::scalar_graph::scalar_graph_module::build_scalar_graph_module_in_namespace(
                &machine.states,
                &machine.state_symbols,
                &machine.state_requires,
                machine.result_type,
                &machine.scalar_qualifications,
                machine.contract,
                machine.crash_routes,
                machine.identity_reshuffles,
                machine.partition_compositions,
                terminal_machine,
                identity_base,
                std::sync::Arc::new(crate::unit::dynamic_dispatch::DynamicDispatchIndex::new(
                    checked,
                )),
                catalog.machine_ids,
                catalog.requirement_counts,
                catalog.cleanup_receivers,
                catalog.boundary_parameters,
                catalog.domain_ids,
                &graph_parameters.1,
                &views,
                &machine.literal_places,
                machine.loop_plan.as_ref(),
                next_place,
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
        output
            .scalar_block_invariants
            .append(&mut lowered.semantic_module.scalar_block_invariants);
        output
            .qualifications
            .coercions
            .append(&mut lowered.semantic_module.scalar_qualifications.coercions);
        output.qualifications.float_entry_ranges.append(
            &mut lowered
                .semantic_module
                .scalar_qualifications
                .float_entry_ranges,
        );
        output.qualifications.integer_entry_ranges.append(
            &mut lowered
                .semantic_module
                .scalar_qualifications
                .integer_entry_ranges,
        );
        occurrences.retain_lowered(&mut lowered);
    }
    Ok(output)
}
