use psi_arena::{Arena, HandleSpan};
use psi_language_semantics::{
    ServiceReachId, ServiceReachInterface, ServiceReachRowId, ServiceReachRowTable,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;

use crate::OperationalPlan;

/// The symbol-resolved recursive service summary for one machine. All sets
/// are interned in the plan's shared row table; child state/call summaries are
/// grouped in arenas instead of allocating one small `Vec` per parent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineServiceReachInference {
    pub machine: SymbolHandle,
    /// Exact public/private contract axis. A published empty ceiling remains
    /// distinct from an internal empty inference.
    pub interface: ServiceReachInterface,
    pub published: ServiceReachRowId,
    pub inferred_direct: ServiceReachRowId,
    pub inferred_transitive: ServiceReachRowId,
    /// Reach not contributed solely by an installation-selected upper bound.
    /// Final composition unions selected rows into this base; it never tries
    /// to subtract upper bounds from the flattened conservative set.
    pub concrete_effective: ServiceReachRowId,
    /// Exact installation-selected requirement rows reachable from this
    /// machine. Their upper bounds remain in the ordinary service rows for
    /// conservative preselection auditing; composition must later substitute
    /// one selected provider row for every entry here.
    pub unresolved_installation_reaches: Vec<InstallationReachRequirement>,
    /// The modular summary callers consume: published for a pinned/authored
    /// interface, inferred for a private checked body.
    pub effective: ServiceReachRowId,
    pub states: HandleSpan<StateServiceReachInference>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateServiceReachInference {
    pub state: SymbolHandle,
    pub inferred_direct: ServiceReachRowId,
    pub inferred_transitive: ServiceReachRowId,
    pub concrete_direct: ServiceReachRowId,
    pub concrete_transitive: ServiceReachRowId,
    pub unresolved_installation_reaches: Vec<InstallationReachRequirement>,
    pub calls: HandleSpan<CallServiceReachInference>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallServiceReachInference {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target_state: SymbolHandle,
    pub target_machine: SymbolHandle,
    pub inferred_direct: ServiceReachRowId,
    pub inferred_transitive: ServiceReachRowId,
    pub concrete_direct: ServiceReachRowId,
    pub concrete_transitive: ServiceReachRowId,
    pub unresolved_installation_reaches: Vec<InstallationReachRequirement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallationReachRequirement {
    /// Exact normalized boundary-trait requirement identity.
    pub requirement: SymbolHandle,
    /// Conservative upper bound published by `reaches <= Bound`.
    pub upper_bound: ServiceReachRowId,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceReachInferencePlan {
    pub rows: ServiceReachRowTable,
    pub root_machines: HandleSpan<MachineServiceReachInference>,
    pub machines: Arena<MachineServiceReachInference>,
    pub states: Arena<StateServiceReachInference>,
    pub calls: Arena<CallServiceReachInference>,
}

impl ServiceReachInferencePlan {
    pub fn machines(&self) -> &[MachineServiceReachInference] {
        self.machines.span_or_empty(self.root_machines)
    }

    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineServiceReachInference> {
        self.machines()
            .iter()
            .find(|summary| summary.machine == machine)
    }

    pub fn states_for(
        &self,
        machine: &MachineServiceReachInference,
    ) -> &[StateServiceReachInference] {
        self.states.span_or_empty(machine.states)
    }

    pub fn for_state(&self, state: SymbolHandle) -> Option<&StateServiceReachInference> {
        self.states
            .iter()
            .map(|(_, summary)| summary)
            .find(|summary| summary.state == state)
    }

    pub fn calls_for(&self, state: &StateServiceReachInference) -> &[CallServiceReachInference] {
        self.calls.span_or_empty(state.calls)
    }

    pub fn services(&self, row: ServiceReachRowId) -> &[ServiceReachId] {
        self.rows.services(row)
    }
}

#[derive(Debug, Clone)]
struct MachineReachWork {
    symbol: SymbolHandle,
    published: Vec<ServiceReachId>,
    uses_published: bool,
    installation_bound: bool,
    direct: Vec<ServiceReachId>,
    transitive: Vec<ServiceReachId>,
    concrete_direct: Vec<ServiceReachId>,
    concrete_transitive: Vec<ServiceReachId>,
    unresolved_installation_reaches: Vec<InstallationReachRequirement>,
    calls: Vec<(SymbolHandle, DirectServiceReach)>,
}

#[derive(Debug, Clone, Default)]
struct DirectServiceReach {
    services: Vec<ServiceReachId>,
    concrete_services: Vec<ServiceReachId>,
    unresolved_installation_reaches: Vec<InstallationReachRequirement>,
}

pub fn infer_service_reaches(
    program: &TypedTrees,
    operational: &OperationalPlan,
) -> ServiceReachInferencePlan {
    let mut work = Vec::new();
    for machine in program.machines() {
        let published = program
            .service_reach_rows
            .services(machine.service_reach_row)
            .to_vec();
        let mut calls = Vec::new();
        let mut direct = Vec::new();
        let mut concrete_direct = Vec::new();
        if let Some(summary) = operational
            .machines()
            .iter()
            .find(|summary| summary.symbol == machine.symbol)
        {
            for state in operational.states.span_or_empty(summary.states) {
                for call in operational.calls.span_or_empty(state.calls) {
                    let call_reach =
                        direct_service_reach_for_call(program, call.target_state_symbol);
                    extend_service_set(&mut direct, &call_reach.services);
                    extend_service_set(&mut concrete_direct, &call_reach.concrete_services);
                    calls.push((call.target_machine_symbol, call_reach));
                }
            }
        }
        work.push(MachineReachWork {
            symbol: machine.symbol,
            published: published.clone(),
            uses_published: machine.supply_mode
                != psi_language_semantics::MachineSupplyMode::CheckedBody
                || !program
                    .service_reach_rows
                    .services(machine.service_reach_row)
                    .is_empty(),
            installation_bound: machine.service_reach_is_installation_bound,
            direct: direct.clone(),
            transitive: direct,
            concrete_direct: concrete_direct.clone(),
            concrete_transitive: concrete_direct,
            unresolved_installation_reaches: machine
                .service_reach_is_installation_bound
                .then_some(InstallationReachRequirement {
                    requirement: machine.symbol,
                    upper_bound: machine.service_reach_row,
                })
                .into_iter()
                .chain(
                    calls.iter().flat_map(|(_, reach)| {
                        reach.unresolved_installation_reaches.iter().copied()
                    }),
                )
                .collect(),
            calls,
        });
        normalize_installation_reaches(
            &mut work
                .last_mut()
                .expect("machine work")
                .unresolved_installation_reaches,
        );
    }

    loop {
        let previous = work
            .iter()
            .map(|machine| {
                (
                    machine.transitive.clone(),
                    machine.concrete_transitive.clone(),
                    machine.unresolved_installation_reaches.clone(),
                )
            })
            .collect::<Vec<_>>();
        for machine_index in 0..work.len() {
            let mut transitive = work[machine_index].direct.clone();
            let mut concrete_transitive = work[machine_index].concrete_direct.clone();
            let mut unresolved = work[machine_index].unresolved_installation_reaches.clone();
            for (target, direct) in work[machine_index].calls.clone() {
                extend_service_set(&mut transitive, &direct.services);
                extend_service_set(&mut concrete_transitive, &direct.concrete_services);
                if let Some(target) = work.iter().find(|machine| machine.symbol == target) {
                    extend_service_set(&mut transitive, effective_services(target));
                    extend_service_set(
                        &mut concrete_transitive,
                        concrete_effective_services(target),
                    );
                    extend_installation_reaches(
                        &mut unresolved,
                        &target.unresolved_installation_reaches,
                    );
                }
            }
            work[machine_index].transitive = transitive;
            work[machine_index].concrete_transitive = concrete_transitive;
            work[machine_index].unresolved_installation_reaches = unresolved;
        }
        if work.iter().zip(&previous).all(|(machine, previous)| {
            machine.transitive == previous.0
                && machine.concrete_transitive == previous.1
                && machine.unresolved_installation_reaches == previous.2
        }) {
            break;
        }
    }

    let mut plan = ServiceReachInferencePlan {
        rows: program.service_reach_rows.clone(),
        ..Default::default()
    };
    for machine in program.machines() {
        let Some(machine_work) = work.iter().find(|summary| summary.symbol == machine.symbol)
        else {
            continue;
        };
        let mut states = HandleSpan::empty();
        if let Some(machine_summary) = operational
            .machines()
            .iter()
            .find(|summary| summary.symbol == machine.symbol)
        {
            for state_summary in operational.states.span_or_empty(machine_summary.states) {
                let mut calls = HandleSpan::empty();
                let mut state_direct = Vec::new();
                let mut state_transitive = Vec::new();
                let mut state_concrete_direct = Vec::new();
                let mut state_concrete_transitive = Vec::new();
                let mut state_unresolved = Vec::new();
                for call_summary in operational.calls.span_or_empty(state_summary.calls) {
                    let call_direct =
                        direct_service_reach_for_call(program, call_summary.target_state_symbol);
                    let mut call_transitive = call_direct.services.clone();
                    let mut call_concrete_transitive = call_direct.concrete_services.clone();
                    let mut call_unresolved = call_direct.unresolved_installation_reaches;
                    if let Some(target) = work
                        .iter()
                        .find(|summary| summary.symbol == call_summary.target_machine_symbol)
                    {
                        extend_service_set(&mut call_transitive, effective_services(target));
                        extend_service_set(
                            &mut call_concrete_transitive,
                            concrete_effective_services(target),
                        );
                        extend_installation_reaches(
                            &mut call_unresolved,
                            &target.unresolved_installation_reaches,
                        );
                    }
                    extend_service_set(&mut state_direct, &call_direct.services);
                    extend_service_set(&mut state_transitive, &call_transitive);
                    extend_service_set(&mut state_concrete_direct, &call_direct.concrete_services);
                    extend_service_set(&mut state_concrete_transitive, &call_concrete_transitive);
                    extend_installation_reaches(&mut state_unresolved, &call_unresolved);
                    let inferred_direct = plan.rows.intern(call_direct.services);
                    let inferred_transitive = plan.rows.intern(call_transitive);
                    let concrete_direct = plan.rows.intern(call_direct.concrete_services);
                    let concrete_transitive = plan.rows.intern(call_concrete_transitive);
                    plan.calls.append_to_span(
                        &mut calls,
                        CallServiceReachInference {
                            statement_index: call_summary.statement_index,
                            call_ordinal: call_summary.call_ordinal,
                            target_state: call_summary.target_state_symbol,
                            target_machine: call_summary.target_machine_symbol,
                            inferred_direct,
                            inferred_transitive,
                            concrete_direct,
                            concrete_transitive,
                            unresolved_installation_reaches: call_unresolved,
                        },
                    );
                }
                let inferred_direct = plan.rows.intern(state_direct);
                let inferred_transitive = plan.rows.intern(state_transitive);
                let concrete_direct = plan.rows.intern(state_concrete_direct);
                let concrete_transitive = plan.rows.intern(state_concrete_transitive);
                plan.states.append_to_span(
                    &mut states,
                    StateServiceReachInference {
                        state: state_summary.symbol,
                        inferred_direct,
                        inferred_transitive,
                        concrete_direct,
                        concrete_transitive,
                        unresolved_installation_reaches: state_unresolved,
                        calls,
                    },
                );
            }
        }

        let published = plan.rows.intern(machine_work.published.clone());
        let inferred_direct = plan.rows.intern(machine_work.direct.clone());
        let inferred_transitive = plan.rows.intern(machine_work.transitive.clone());
        let effective = plan.rows.intern(effective_services(machine_work).to_vec());
        let concrete_effective = plan
            .rows
            .intern(concrete_effective_services(machine_work).to_vec());
        plan.machines.append_to_span(
            &mut plan.root_machines,
            MachineServiceReachInference {
                machine: machine.symbol,
                interface: if machine_work.uses_published {
                    ServiceReachInterface::PublishedCeiling(published)
                } else {
                    ServiceReachInterface::InternalInferred
                },
                published,
                inferred_direct,
                inferred_transitive,
                concrete_effective,
                unresolved_installation_reaches: machine_work
                    .unresolved_installation_reaches
                    .clone(),
                effective,
                states,
            },
        );
    }
    plan
}

fn effective_services(machine: &MachineReachWork) -> &[ServiceReachId] {
    if machine.uses_published {
        &machine.published
    } else {
        &machine.transitive
    }
}

fn concrete_effective_services(machine: &MachineReachWork) -> &[ServiceReachId] {
    if machine.uses_published && !machine.installation_bound {
        &machine.published
    } else {
        &machine.concrete_transitive
    }
}

fn direct_service_reach_for_call(program: &TypedTrees, target: SymbolHandle) -> DirectServiceReach {
    let mut reach = DirectServiceReach::default();
    if !target.is_valid() {
        return reach;
    }

    // Inline assembly is an unnameable builtin call whose instruction
    // contract reaches a canonical boundary-service identity. Resolve that
    // identity through the same symbol-backed table as authored `reaches`
    // rows; never consult the lowercase/u64 compatibility catalog here.
    for function in psi_symbols::BuiltinFunction::asm_intrinsics() {
        if program.symbols.builtin_function_symbol(function) != Some(target) {
            continue;
        }
        if let Some(service_name) = function.asm_intrinsic_service_name()
            && let Some(service) = program.service_reaches.id_for_name(service_name)
        {
            program
                .service_reaches
                .extend_closure(service, &mut reach.services);
            reach.services.sort_by_key(|service| service.0);
            reach.services.dedup();
            reach.concrete_services = reach.services.clone();
        }
        return reach;
    }

    if let Some((_, signature)) = program.machine_parameter_signature(target) {
        extend_service_set(
            &mut reach.services,
            program
                .service_reach_rows
                .services(signature.service_reach_row),
        );
        if !signature.service_reach_is_installation_bound {
            extend_service_set(
                &mut reach.concrete_services,
                program
                    .service_reach_rows
                    .services(signature.service_reach_row),
            );
        }
        record_installation_reach(signature, &mut reach);
        extend_invoked_binding_services(program, signature, &mut reach.services);
        extend_invoked_binding_services(program, signature, &mut reach.concrete_services);
        return reach;
    }

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            if signature.symbol != target {
                continue;
            }
            extend_service_set(
                &mut reach.services,
                program
                    .service_reach_rows
                    .services(signature.service_reach_row),
            );
            if !signature.service_reach_is_installation_bound {
                extend_service_set(
                    &mut reach.concrete_services,
                    program
                        .service_reach_rows
                        .services(signature.service_reach_row),
                );
            }
            if trait_definition.is_boundary
                && let Some(service) = program
                    .service_reaches
                    .id_for_symbol(trait_definition.symbol)
            {
                program
                    .service_reaches
                    .extend_closure(service, &mut reach.services);
                program
                    .service_reaches
                    .extend_closure(service, &mut reach.concrete_services);
                reach.services.sort_by_key(|service| service.0);
                reach.services.dedup();
                reach.concrete_services.sort_by_key(|service| service.0);
                reach.concrete_services.dedup();
            }
            record_installation_reach(signature, &mut reach);
            extend_invoked_binding_services(program, signature, &mut reach.services);
            extend_invoked_binding_services(program, signature, &mut reach.concrete_services);
            return reach;
        }
    }
    reach
}

fn record_installation_reach(
    signature: &psi_typed_trees::signature::StateSignature,
    reach: &mut DirectServiceReach,
) {
    if signature.service_reach_is_installation_bound {
        reach
            .unresolved_installation_reaches
            .push(InstallationReachRequirement {
                requirement: signature.symbol,
                upper_bound: signature.service_reach_row,
            });
    }
}

fn extend_installation_reaches(
    destination: &mut Vec<InstallationReachRequirement>,
    source: &[InstallationReachRequirement],
) {
    destination.extend_from_slice(source);
    normalize_installation_reaches(destination);
}

fn normalize_installation_reaches(rows: &mut Vec<InstallationReachRequirement>) {
    rows.sort_by_key(|row| {
        (
            row.requirement.arena_index(),
            row.requirement.generation(),
            row.upper_bound.0,
        )
    });
    rows.dedup();
}

fn extend_invoked_binding_services(
    program: &TypedTrees,
    signature: &psi_typed_trees::signature::StateSignature,
    services: &mut Vec<ServiceReachId>,
) {
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    for target in crate::declared_signature_invocations(program, signature) {
        let symbol = match target {
            crate::InvocationTarget::Parameter(index) => parameters
                .get(index as usize)
                .map(|parameter| {
                    program
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                        .type_symbol(&program.type_reference_table)
                })
                .unwrap_or_else(SymbolHandle::invalid),
            crate::InvocationTarget::Service(symbol) => symbol,
        };
        let Some(service) = program.service_reaches.id_for_symbol(symbol) else {
            continue;
        };
        program.service_reaches.extend_closure(service, services);
    }
    services.sort_by_key(|service| service.0);
    services.dedup();
}

fn extend_service_set(destination: &mut Vec<ServiceReachId>, source: &[ServiceReachId]) {
    destination.extend_from_slice(source);
    destination.sort_by_key(|service| service.0);
    destination.dedup();
}
