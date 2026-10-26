//! Target applications of already checked bodies. Proof conditions follow
//! invocation edges, including cleanup and selected callbacks, rather than
//! inheriting the target of the file that defined a portable helper.
//!
//! This traverses retained call facts, not source expressions. Static argument
//! substitution is invocation-local: a machine reached with another binding
//! is another application even when its declaration symbol is unchanged.

use arena::HandleSpan;
use checked_trees::{CheckedTrees, typed_trees};
use flow_effects::{OperationalPlan, StaticMachineCallBinding};
use symbols::SymbolHandle;
use target::TargetProfile;

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

/// Shared judgments apply to the selected product. Supplemental capsules
/// start at their authored bodies and carry that capsule into portable callees.
pub fn proof_application_machines(
    checked: &CheckedTrees,
    target: TargetProfile,
    selected_product: bool,
    providers: &[crate::SelectedProviderReviewProvenance],
) -> Vec<SymbolHandle> {
    let provider_edges: Vec<_> = providers
        .iter()
        .flat_map(|selected| {
            selected
                .provider
                .row_requirements
                .iter()
                .copied()
                .zip(selected.provider.row_realizations.iter().copied())
        })
        .collect();
    application_machines(checked, target, selected_product, &provider_edges)
}

/// Declaration formation follows the exact typed descriptions consumed by
/// these already selected machine applications. Metadata references may add
/// declarations and their types/contracts, never another executable body.
/// This is the ordinary retention visitor, not a second source checker or a
/// nearest-enclosing-machine approximation.
pub fn proof_application_declaration_owners(
    checked: &CheckedTrees,
    machines: &[SymbolHandle],
    declarations: &[SymbolHandle],
) -> Vec<SymbolHandle> {
    let typed = &checked.typed;
    let mut roots = machines.to_vec();
    loop {
        let selection =
            typed_trees::references::DeclarationSelection::from_symbols(roots.iter().copied());
        let references =
            typed_trees::references::collect_selected_program_references(typed, &selection);
        let before = roots.len();
        // Descriptive custody still retains type/contract formation; it
        // grants no executable body application. Both roles may add only
        // non-machine declarations below.
        for symbol in references
            .symbols
            .into_iter()
            .chain(references.descriptor_symbols)
        {
            let declaration = typed
                .trait_machine_signature(symbol)
                .map_or(symbol, |(owner, _)| owner.symbol);
            if typed.machine_by_symbol(declaration).is_none()
                && declarations.contains(&declaration)
                && !roots.contains(&declaration)
            {
                roots.push(declaration);
            }
        }
        if roots.len() == before {
            break;
        }
    }
    roots.retain(|root| typed.machine_by_symbol(*root).is_none());
    for declaration in typed.traits() {
        if roots.contains(&declaration.symbol) {
            roots.extend(
                typed
                    .trait_machine_signatures(declaration)
                    .iter()
                    .map(|signature| signature.symbol),
            );
            roots.extend(
                declaration
                    .refinement_clauses
                    .iter()
                    .map(|row| row.signature.symbol),
            );
        }
    }
    roots.sort_unstable_by_key(|root| (root.arena_index(), root.generation()));
    roots.dedup();
    roots
}

/// The immutable declaration roster, including exact trait-signature owners.
/// Ground model recipes may close these descriptions under the selected copy's
/// capsule without executing them or inventing a machine application.
pub fn declaration_formation_owners(checked: &CheckedTrees) -> Vec<SymbolHandle> {
    let mut declarations = typed_trees::references::collect_program_declarations(&checked.typed);
    declarations.retain(|owner| checked.typed.machine_by_symbol(*owner).is_none());
    for owner in checked.typed.traits() {
        declarations.extend(
            checked
                .typed
                .trait_machine_signatures(owner)
                .iter()
                .map(|signature| signature.symbol),
        );
        declarations.extend(
            owner
                .refinement_clauses
                .iter()
                .map(|row| row.signature.symbol),
        );
    }
    declarations.sort_unstable_by_key(|owner| (owner.arena_index(), owner.generation()));
    declarations.dedup();
    declarations
}

/// Conservative demand discovery may follow every checked candidate. It
/// grants no proof or provider authority: the actual application walk below
/// consumes only settled rows. This avoids demanding unrelated target
/// providers merely because another body has a conditional arithmetic fact.
pub fn target_has_proof_demand(
    checked: &CheckedTrees,
    target: TargetProfile,
    owners: &[SymbolHandle],
    declarations: &[SymbolHandle],
) -> bool {
    let typed = &checked.typed;
    let mut candidates: Vec<_> = typed
        .conformances()
        .iter()
        .flat_map(|conformance| {
            conformance
                .rows
                .iter()
                .map(|row| (row.requirement, row.realization_machine))
        })
        .collect();
    for machine in typed.machines() {
        candidates.extend(
            typed
                .machine_trait_conformances(machine)
                .iter()
                .filter(|clause| clause.requirement_symbol.is_valid())
                .map(|clause| (clause.requirement_symbol, machine.symbol)),
        );
    }
    let machines = application_machines(checked, target, false, &candidates);
    machines.iter().any(|machine| owners.contains(machine))
        || proof_application_declaration_owners(checked, &machines, declarations)
            .iter()
            .any(|declaration| owners.contains(declaration))
}

fn application_machines(
    checked: &CheckedTrees,
    target: TargetProfile,
    selected_product: bool,
    provider_edges: &[(SymbolHandle, SymbolHandle)],
) -> Vec<SymbolHandle> {
    let typed = &checked.typed;
    let plan = &checked.facts.operational;
    let mut pending = Vec::new();
    for machine in typed.machines() {
        let authored = typed.machine_authored_target(machine);
        if authored.is_some_and(|name| name == target.target_name())
            || (selected_product && authored.is_none())
        {
            pending.push(Application {
                machine: machine.symbol,
                bindings: Vec::new(),
            });
        }
    }
    let mut visited = Vec::new();
    let mut machines = Vec::new();
    while let Some(mut application) = pending.pop() {
        let Some(declaration) = super::called_machine(typed, application.machine) else {
            continue;
        };
        let Some(machine) = super::realized_machine(typed, declaration, target) else {
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
                    super::called_machine(typed, callee).map(|machine| machine.symbol);
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
