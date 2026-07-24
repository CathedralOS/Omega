use omega_core::arena::{Arena, HandleSpan};
use omega_core::semantics::{ServiceReachId, ServiceReachRowId, ServiceReachRowTable};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;

use crate::OperationalPlan;

/// The symbol-resolved recursive service summary for one machine. All sets
/// are interned in the plan's shared row table; child state/call summaries are
/// grouped in arenas instead of allocating one small `Vec` per parent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineServiceReachInference {
    pub machine: SymbolHandle,
    pub published: ServiceReachRowId,
    pub inferred_direct: ServiceReachRowId,
    pub inferred_transitive: ServiceReachRowId,
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
    direct: Vec<ServiceReachId>,
    transitive: Vec<ServiceReachId>,
    calls: Vec<(SymbolHandle, Vec<ServiceReachId>)>,
}

pub fn infer_service_reaches(
    program: &TypedTrees,
    operations: &OperationalPlan,
) -> ServiceReachInferencePlan {
    let mut work = Vec::new();
    for machine in program.machines() {
        let published = program
            .service_reach_rows
            .services(machine.service_reach_row)
            .to_vec();
        let mut calls = Vec::new();
        let mut direct = Vec::new();
        if let Some(summary) = operations
            .machines()
            .iter()
            .find(|summary| summary.symbol == machine.symbol)
        {
            for state in operations.states.span_or_empty(summary.states) {
                for call in operations.calls.span_or_empty(state.calls) {
                    let call_services =
                        direct_service_reach_for_call(program, call.target_state_symbol);
                    extend_service_set(&mut direct, &call_services);
                    calls.push((call.target_machine_symbol, call_services));
                }
            }
        }
        work.push(MachineReachWork {
            symbol: machine.symbol,
            published: published.clone(),
            uses_published: machine.supply_mode
                != omega_core::semantics::MachineSupplyMode::CheckedBody
                || !program
                    .service_reach_rows
                    .services(machine.service_reach_row)
                    .is_empty(),
            direct: direct.clone(),
            transitive: direct,
            calls,
        });
    }

    loop {
        let previous = work
            .iter()
            .map(|machine| machine.transitive.clone())
            .collect::<Vec<_>>();
        for machine_index in 0..work.len() {
            let mut transitive = work[machine_index].direct.clone();
            for (target, direct) in work[machine_index].calls.clone() {
                extend_service_set(&mut transitive, &direct);
                if let Some(target) = work.iter().find(|machine| machine.symbol == target) {
                    extend_service_set(&mut transitive, effective_services(target));
                }
            }
            work[machine_index].transitive = transitive;
        }
        if work
            .iter()
            .map(|machine| &machine.transitive)
            .eq(previous.iter())
        {
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
        if let Some(machine_operations) = operations
            .machines()
            .iter()
            .find(|summary| summary.symbol == machine.symbol)
        {
            for state_operations in operations.states.span_or_empty(machine_operations.states) {
                let mut calls = HandleSpan::empty();
                let mut state_direct = Vec::new();
                let mut state_transitive = Vec::new();
                for call_operations in operations.calls.span_or_empty(state_operations.calls) {
                    let call_direct =
                        direct_service_reach_for_call(program, call_operations.target_state_symbol);
                    let mut call_transitive = call_direct.clone();
                    if let Some(target) = work
                        .iter()
                        .find(|summary| summary.symbol == call_operations.target_machine_symbol)
                    {
                        extend_service_set(&mut call_transitive, effective_services(target));
                    }
                    extend_service_set(&mut state_direct, &call_direct);
                    extend_service_set(&mut state_transitive, &call_transitive);
                    let inferred_direct = plan.rows.intern(call_direct);
                    let inferred_transitive = plan.rows.intern(call_transitive);
                    plan.calls.append_to_span(
                        &mut calls,
                        CallServiceReachInference {
                            statement_index: call_operations.statement_index,
                            call_ordinal: call_operations.call_ordinal,
                            target_state: call_operations.target_state_symbol,
                            target_machine: call_operations.target_machine_symbol,
                            inferred_direct,
                            inferred_transitive,
                        },
                    );
                }
                let inferred_direct = plan.rows.intern(state_direct);
                let inferred_transitive = plan.rows.intern(state_transitive);
                plan.states.append_to_span(
                    &mut states,
                    StateServiceReachInference {
                        state: state_operations.symbol,
                        inferred_direct,
                        inferred_transitive,
                        calls,
                    },
                );
            }
        }

        let published = plan.rows.intern(machine_work.published.clone());
        let inferred_direct = plan.rows.intern(machine_work.direct.clone());
        let inferred_transitive = plan.rows.intern(machine_work.transitive.clone());
        let effective = plan.rows.intern(effective_services(machine_work).to_vec());
        plan.machines.append_to_span(
            &mut plan.root_machines,
            MachineServiceReachInference {
                machine: machine.symbol,
                published,
                inferred_direct,
                inferred_transitive,
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

fn direct_service_reach_for_call(
    program: &TypedTrees,
    target: SymbolHandle,
) -> Vec<ServiceReachId> {
    let mut services = Vec::new();
    if !target.is_valid() {
        return services;
    }

    // Inline assembly is an unnameable builtin call whose instruction
    // contract reaches a canonical boundary-service identity. Resolve that
    // identity through the same symbol-backed table as authored `effects`
    // rows; never consult the lowercase/u64 compatibility catalog here.
    for function in omega_core::symbols::BuiltinFunction::asm_intrinsics() {
        if program.symbols.builtin_function_symbol(function) != Some(target) {
            continue;
        }
        if let Some(service_name) = function.asm_intrinsic_service_name()
            && let Some(service) = program.service_reaches.id_for_name(service_name)
        {
            program
                .service_reaches
                .extend_closure(service, &mut services);
            services.sort_by_key(|service| service.0);
            services.dedup();
        }
        return services;
    }

    if let Some((_, signature)) = program.machine_parameter_signature(target) {
        extend_service_set(
            &mut services,
            program
                .service_reach_rows
                .services(signature.service_reach_row),
        );
        return services;
    }

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            if signature.symbol != target {
                continue;
            }
            extend_service_set(
                &mut services,
                program
                    .service_reach_rows
                    .services(signature.service_reach_row),
            );
            if trait_definition.is_boundary
                && let Some(service) = program
                    .service_reaches
                    .id_for_symbol(trait_definition.symbol)
            {
                program
                    .service_reaches
                    .extend_closure(service, &mut services);
                services.sort_by_key(|service| service.0);
                services.dedup();
            }
            return services;
        }
    }
    services
}

fn extend_service_set(destination: &mut Vec<ServiceReachId>, source: &[ServiceReachId]) {
    destination.extend_from_slice(source);
    destination.sort_by_key(|service| service.0);
    destination.dedup();
}
