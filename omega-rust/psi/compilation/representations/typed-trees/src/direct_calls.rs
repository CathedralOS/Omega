//! Record structural direct calls once, without target choice or executable reach.

use crate::TypedTrees;
use crate::typed_trees::inspection::callees::ExplicitStateCallees;
use language_semantics::direct_calls::{
    DirectCall, DirectCallDeclarations, DirectCallFamily, DirectCallFamilyMember,
    DirectCallMachine, DirectCallState,
};
use symbols::SymbolHandle;

pub fn collect(
    typed: &TypedTrees,
    scope: source::DependencyScope,
) -> DirectCallDeclarations<SymbolHandle> {
    let mut machine_positions = Vec::new();
    for (position, machine) in typed.machines().iter().enumerate() {
        if machine.symbol.is_valid() {
            machine_positions.push((machine.symbol, position));
        }
    }
    for machine in typed.machines() {
        for state in typed.machine_states(machine) {
            if let Some(owner) = called_machine_position(typed, state.symbol) {
                machine_positions.push((state.symbol, owner));
            }
        }
    }
    let mut callees = ExplicitStateCallees::default();
    let machines = typed
        .machines()
        .iter()
        .map(|machine| {
            let states = typed
                .machine_states(machine)
                .iter()
                .map(|state| {
                    let calls = callees
                        .collect(typed, state)
                        .iter()
                        .map(|called| DirectCall {
                            declaration: *called,
                            machine: called_machine_position(typed, *called),
                        })
                        .collect();
                    DirectCallState {
                        declaration: state.symbol,
                        name: state.name.as_str().to_owned(),
                        calls,
                    }
                })
                .collect();
            DirectCallMachine {
                declaration: machine.symbol,
                name: machine.name.as_str().to_owned(),
                in_scope: typed
                    .symbols
                    .symbol_is_in_dependency_scope(machine.symbol, scope),
                family_interface: machine.target_family_interface,
                authored_target: typed.machine_authored_target(machine).map(str::to_owned),
                source_target_context: typed.source_target_context(machine).map(str::to_owned),
                target: machine
                    .target
                    .as_ref()
                    .map(|target| target.as_str().to_owned()),
                states,
            }
        })
        .collect();
    let families = typed
        .target_machine_families
        .iter()
        .map(|family| DirectCallFamily {
            canonical: typed.machine_position_by_symbol(family.canonical),
            members: family
                .members
                .iter()
                .map(|member| DirectCallFamilyMember {
                    target: member.target.as_str().to_owned(),
                    machine: typed.machine_position_by_symbol(member.machine),
                })
                .collect(),
        })
        .collect();
    DirectCallDeclarations::new(scope, machines, families, machine_positions)
}

fn called_machine_position(typed: &TypedTrees, called: SymbolHandle) -> Option<usize> {
    typed
        .machine_by_symbol(called)
        .or_else(|| typed.machine_holding_state(called))
        .and_then(|machine| typed.machine_position_by_symbol(machine.symbol))
}
