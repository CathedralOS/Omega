//! Read the checked invocation graph without introducing executable roots.
//! Provider edges are borrowed from the consumer's exact selection or conservative
//! candidate domain; metadata references never donate another machine body.

use crate::CheckedTrees;
use arena::HandleSpan;
use flow_effects::{OperationalPlan, StaticMachineCallBinding};
use symbols::SymbolHandle;

/// Compiler-supplied invocation roots for one checked Build target application.
#[derive(Clone)]
pub struct TargetApplicationRoots {
    pub target: String,
    pub machines: Vec<SymbolHandle>,
}

#[derive(Clone, PartialEq, Eq)]
struct Binding {
    parameter: SymbolHandle,
    selected: SymbolHandle,
    arguments: Vec<Binding>,
}

#[derive(Clone, PartialEq, Eq)]
struct Application {
    machine: SymbolHandle,
    bindings: Vec<Binding>,
}

pub fn application_machines(
    checked: &CheckedTrees,
    target: &str,
    roots: &[SymbolHandle],
    provider_edges: &[(SymbolHandle, SymbolHandle)],
) -> Vec<SymbolHandle> {
    let mut pending: Vec<_> = roots
        .iter()
        .copied()
        .map(|machine| Application {
            machine,
            bindings: Vec::new(),
        })
        .collect();
    let typed = &checked.typed;
    let plan = &checked.facts.operational;
    let mut visited = Vec::new();
    let mut machines = Vec::new();
    while let Some(mut application) = pending.pop() {
        let Some(declaration) = called_machine(typed, application.machine) else {
            continue;
        };
        let Some(machine) = typed.machine_for_target(declaration, target) else {
            continue;
        };
        if declaration.symbol != machine.symbol {
            // Family formation established declaration-order correspondence;
            // symbols of the member's binders are nevertheless its own.
            for binding in &mut application.bindings {
                if let Some((_, selected)) = typed
                    .machine_kind_type_parameters(declaration)
                    .zip(typed.machine_kind_type_parameters(machine))
                    .find(|(declared, _)| declared.symbol == binding.parameter)
                {
                    binding.parameter = selected.symbol;
                }
            }
        }
        application.machine = machine.symbol;
        if visited.contains(&application) {
            continue;
        }
        visited.push(application.clone());
        if !machines.contains(&machine.symbol) {
            machines.push(machine.symbol);
        }
        let Some(summary) = plan.machine_by_symbol(machine.symbol) else {
            continue;
        };
        for state in plan.states.span_or_empty(summary.states) {
            for call in plan.calls.span_or_empty(state.calls) {
                // An unspecialized binder call names the binder as its target
                // state; a specialized call retains that identity separately.
                let binder = if call.static_machine_parameter.is_valid() {
                    call.static_machine_parameter
                } else {
                    call.target_state_symbol
                };
                let selected = application
                    .bindings
                    .iter()
                    .find(|binding| binding.parameter == binder);
                let (callee, bindings) = match selected {
                    Some(selected) => (selected.selected, selected.arguments.clone()),
                    None => (
                        call.target_machine_symbol,
                        resolve_bindings(plan, call.static_machine_bindings, &application.bindings),
                    ),
                };
                pending.push(Application {
                    machine: callee,
                    bindings,
                });
                let requirement_machine =
                    called_machine(typed, callee).map(|machine| machine.symbol);
                for (requirement, realization) in provider_edges {
                    if *requirement == callee
                        || Some(*requirement) == requirement_machine
                        || *requirement == call.target_state_symbol
                        || *requirement == call.target_operator_symbol
                    {
                        pending.push(Application {
                            machine: *realization,
                            bindings: Vec::new(),
                        });
                    }
                }
            }
        }
        for (_, cleanup) in plan.cleanup_dependencies.iter() {
            if cleanup.caller_machine == machine.symbol {
                pending.push(Application {
                    machine: cleanup.target_machine,
                    bindings: Vec::new(),
                });
            }
        }
        // Constructing a dynamic selection makes each of its checked rows a
        // possible runtime invocation in this capsule; runtime data decides
        // which row is exercised, not whether its body must form correctly.
        for selection in &checked.facts.dynamic_conformances.selections {
            if selection.machine == machine.symbol {
                pending.extend(selection.rows.iter().map(|row| Application {
                    machine: row.realization_machine,
                    bindings: Vec::new(),
                }));
            }
        }
    }
    machines
}

fn resolve_bindings(
    plan: &OperationalPlan,
    arguments: HandleSpan<StaticMachineCallBinding>,
    enclosing: &[Binding],
) -> Vec<Binding> {
    plan.static_machine_bindings
        .span_or_empty(arguments)
        .iter()
        .map(|argument| {
            if let Some(selected) = enclosing
                .iter()
                .find(|binding| binding.parameter == argument.selected)
            {
                Binding {
                    parameter: argument.parameter,
                    selected: selected.selected,
                    arguments: selected.arguments.clone(),
                }
            } else {
                Binding {
                    parameter: argument.parameter,
                    selected: argument.selected,
                    arguments: resolve_bindings(plan, argument.arguments, enclosing),
                }
            }
        })
        .collect()
}

fn called_machine(
    typed: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::machine::Machine> {
    typed
        .machine_by_symbol(symbol)
        .or_else(|| typed.machine_holding_state(symbol))
}
