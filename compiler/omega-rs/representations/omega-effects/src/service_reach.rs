use omega_core::semantics::ServiceReachId;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;

use crate::EffectPlan;

/// The symbol-resolved recursive service summary for one machine. Sets are
/// sorted and deduplicated `ServiceReachId`s owned by the typed tree's
/// `ServiceReachTable`; no global spelling catalog or numeric effect bit is
/// consulted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineServiceReachInference {
    pub machine: SymbolHandle,
    pub published: Vec<ServiceReachId>,
    pub inferred_direct: Vec<ServiceReachId>,
    pub inferred_transitive: Vec<ServiceReachId>,
    /// The modular summary callers consume: published for a pinned/authored
    /// interface, inferred for a private checked body.
    pub effective: Vec<ServiceReachId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceReachInferencePlan {
    pub machines: Vec<MachineServiceReachInference>,
}

impl ServiceReachInferencePlan {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineServiceReachInference> {
        self.machines
            .iter()
            .find(|summary| summary.machine == machine)
    }
}

pub fn infer_service_reaches(
    program: &TypedTrees,
    effects: &EffectPlan,
) -> ServiceReachInferencePlan {
    #[derive(Clone)]
    struct Work {
        symbol: SymbolHandle,
        published: Vec<ServiceReachId>,
        uses_published: bool,
        direct: Vec<ServiceReachId>,
        transitive: Vec<ServiceReachId>,
        calls: Vec<(SymbolHandle, Vec<ServiceReachId>)>,
    }

    let mut work = Vec::new();
    for machine in program.machines() {
        let published = program
            .service_reach_rows
            .services(machine.service_reach_row)
            .to_vec();
        let mut calls = Vec::new();
        let mut direct = Vec::new();
        if let Some(summary) = effects
            .machines()
            .iter()
            .find(|summary| summary.symbol == machine.symbol)
        {
            for state in effects.states.span_or_empty(summary.states) {
                for call in effects.calls.span_or_empty(state.calls) {
                    let call_services =
                        direct_service_reach_for_call(program, call.target_state_symbol);
                    extend_service_set(&mut direct, &call_services);
                    calls.push((call.target_machine_symbol, call_services));
                }
            }
        }
        work.push(Work {
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
                    if target.uses_published {
                        extend_service_set(&mut transitive, &target.published);
                    } else {
                        extend_service_set(&mut transitive, &target.transitive);
                    }
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

    ServiceReachInferencePlan {
        machines: work
            .into_iter()
            .map(|machine| MachineServiceReachInference {
                machine: machine.symbol,
                published: machine.published.clone(),
                inferred_direct: machine.direct,
                inferred_transitive: machine.transitive.clone(),
                effective: if machine.uses_published {
                    machine.published
                } else {
                    machine.transitive
                },
            })
            .collect(),
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
