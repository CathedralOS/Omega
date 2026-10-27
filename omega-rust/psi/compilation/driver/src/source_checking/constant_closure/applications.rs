//! Target applications of already checked bodies. Proof conditions follow
//! invocation edges, including cleanup and selected callbacks, rather than
//! inheriting the target of the file that defined a portable helper.
//!
//! This traverses retained call facts, not source expressions. Static argument
//! substitution is invocation-local: a machine reached with another binding
//! is another application even when its declaration symbol is unchanged.

use arena::HandleSpan;
use checked_trees::CheckedTrees;
use flow_effects::{OperationalPlan, StaticMachineCallBinding};
use symbols::SymbolHandle;

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

/// Invocation-owned source demands. Provider choices arrive only as exact edges.
pub(super) struct ProofApplications<'source> {
    source: &'source CheckedTrees,
    declarations: Vec<SymbolHandle>,
    owners: Vec<SymbolHandle>,
    has_obligations: bool,
    unscoped_recipe: bool,
}

impl<'source> ProofApplications<'source> {
    pub(super) fn new(source: &'source CheckedTrees) -> Self {
        let facts = &source.facts;
        let owners = facts
            .constant_predicate_obligations
            .iter()
            .map(|row| row.caller())
            .chain(
                facts
                    .integer_range_obligations
                    .iter()
                    .map(|row| row.owner()),
            )
            .chain(
                facts
                    .constant_recipe_obligations
                    .iter()
                    .map(|row| row.owner())
                    .filter(|owner| owner.is_valid()),
            )
            .collect();
        Self {
            source,
            declarations: declaration_formation_owners(source),
            owners,
            has_obligations: !facts.constant_predicate_obligations.is_empty()
                || !facts.integer_range_obligations.is_empty()
                || !facts.constant_recipe_obligations.is_empty(),
            unscoped_recipe: facts
                .constant_recipe_obligations
                .iter()
                .any(|row| !row.owner().is_valid()),
        }
    }

    /// Preserve selected-first and authored declaration order. The target owner
    /// validates every returned identity before any provider request is made.
    pub(super) fn scopes<'request>(&'request self, primary: &'request str) -> Vec<&'request str> {
        let mut scopes = vec![primary];
        if self.has_obligations {
            for machine in self.source.typed.machines() {
                if let Some(scope) = self.source.typed.machine_authored_target(machine)
                    && !scopes.contains(&scope)
                {
                    scopes.push(scope);
                }
            }
        }
        scopes
    }

    pub(super) fn needs_scope(&self, scope: &str, primary: &str) -> bool {
        scope == primary
            || self.unscoped_recipe
            || target_has_proof_demand(self.source, scope, &self.owners, &self.declarations)
    }

    pub(super) fn membership(
        &self,
        scope: &str,
        selected_product: bool,
        providers: &[(SymbolHandle, SymbolHandle)],
    ) -> (Vec<SymbolHandle>, Vec<SymbolHandle>) {
        let machines = application_machines(self.source, scope, selected_product, providers);
        let declarations =
            proof_application_declaration_owners(self.source, &machines, &self.declarations);
        (machines, declarations)
    }

    pub(super) fn declarations(&self) -> &[SymbolHandle] {
        &self.declarations
    }
}

/// Declaration formation follows the exact typed descriptions consumed by
/// these already selected machine applications. Metadata references may add
/// declarations and their types/contracts, never another executable body.
/// This is the ordinary retention visitor, not a second source checker or a
/// nearest-enclosing-machine approximation.
fn proof_application_declaration_owners(
    checked: &CheckedTrees,
    machines: &[SymbolHandle],
    declarations: &[SymbolHandle],
) -> Vec<SymbolHandle> {
    let typed = &checked.typed;
    let mut roots = machines.to_vec();
    loop {
        let selection = checked_trees::typed_trees::references::DeclarationSelection::from_symbols(
            roots.iter().copied(),
        );
        let references =
            checked_trees::typed_trees::references::collect_selected_program_references(
                typed, &selection,
            );
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
fn declaration_formation_owners(checked: &CheckedTrees) -> Vec<SymbolHandle> {
    let mut declarations =
        checked_trees::typed_trees::references::collect_program_declarations(&checked.typed);
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
fn target_has_proof_demand(
    checked: &CheckedTrees,
    target: &str,
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
    target: &str,
    selected_product: bool,
    provider_edges: &[(SymbolHandle, SymbolHandle)],
) -> Vec<SymbolHandle> {
    let typed = &checked.typed;
    let plan = &checked.facts.operational;
    let mut pending = Vec::new();
    for machine in typed.machines() {
        let authored = typed.machine_authored_target(machine);
        if authored.is_some_and(|name| name == target) || (selected_product && authored.is_none()) {
            pending.push(Application {
                machine: machine.symbol,
                bindings: Vec::new(),
            });
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees::{CheckFacts, ConstantRecipeObligation, ConstantRecipeRequirement};
    use flow_effects::{CallOperational, CleanupDependency, MachineOperational, StateOperational};
    use typed_trees::TypedTrees;
    use typed_trees::machine::{Machine, TargetMachineFamily, TargetMachineFamilyMember};

    fn symbol(ordinal: u32) -> SymbolHandle {
        SymbolHandle::from_parts(ordinal, 1)
    }

    fn machine(program: &mut TypedTrees, ordinal: u32, scope: Option<&str>) {
        program.push_machine(Machine {
            symbol: symbol(ordinal),
            authored_target: scope.map(Into::into),
            ..Default::default()
        });
    }

    #[test]
    fn family_lookup_never_redirects_an_exact_member_or_fills_a_missing_target() {
        let mut program = TypedTrees::default();
        program.push_machine(Machine {
            symbol: symbol(1),
            target_family_interface: true,
            ..Default::default()
        });
        machine(&mut program, 2, Some("macos_arm64"));
        machine(&mut program, 3, Some("windows_x86_64"));
        program.target_machine_families.push(TargetMachineFamily {
            name: Default::default(),
            canonical: symbol(1),
            purpose: Default::default(),
            members: vec![
                TargetMachineFamilyMember {
                    target: "macos_arm64".into(),
                    machine: symbol(2),
                },
                TargetMachineFamilyMember {
                    target: "windows_x86_64".into(),
                    machine: symbol(3),
                },
            ],
        });
        let head = program.machine_by_symbol(symbol(1)).expect("family head");
        let member = program.machine_by_symbol(symbol(2)).expect("exact member");
        assert_eq!(
            program
                .machine_for_target(head, "windows_x86_64")
                .map(|body| body.symbol),
            Some(symbol(3))
        );
        assert!(program.machine_for_target(head, "linux_arm64").is_none());
        assert!(
            program
                .machine_for_target(member, "windows_x86_64")
                .is_none()
        );
        assert_eq!(
            program
                .machine_for_target(member, "macos_arm64")
                .map(|body| body.symbol),
            Some(symbol(2))
        );
    }

    fn recorded_calls() -> CheckedTrees {
        let mut program = TypedTrees::default();
        machine(&mut program, 1, Some("windows_x86_64"));
        for ordinal in 2..=8 {
            machine(&mut program, ordinal, None);
        }
        machine(&mut program, 9, Some("macos_arm64"));
        let mut plan = OperationalPlan::default();
        let bindings = plan
            .static_machine_bindings
            .insert_many([StaticMachineCallBinding {
                parameter: symbol(10),
                selected: symbol(3),
                ..Default::default()
            }]);
        let root_calls = plan.calls.insert_many([
            CallOperational {
                target_machine_symbol: symbol(2),
                static_machine_bindings: bindings,
                ..Default::default()
            },
            CallOperational {
                target_machine_symbol: symbol(6),
                ..Default::default()
            },
        ]);
        let callback = plan.calls.insert_many([CallOperational {
            static_machine_parameter: symbol(10),
            ..Default::default()
        }]);
        let root_states = plan.states.insert_many([StateOperational {
            calls: root_calls,
            ..Default::default()
        }]);
        let helper_states = plan.states.insert_many([StateOperational {
            calls: callback,
            ..Default::default()
        }]);
        plan.root_machines = plan.machines.insert_many([
            MachineOperational {
                symbol: symbol(1),
                states: root_states,
                ..Default::default()
            },
            MachineOperational {
                symbol: symbol(2),
                states: helper_states,
                ..Default::default()
            },
        ]);
        plan.cleanup_dependencies.insert(CleanupDependency {
            caller_machine: symbol(1),
            target_machine: symbol(5),
            ..Default::default()
        });
        CheckedTrees::with_roots(
            program,
            CheckFacts {
                operational: plan,
                ..Default::default()
            },
        )
    }

    #[test]
    fn supplemental_membership_follows_static_bindings_cleanup_and_only_selected_providers() {
        let checked = recorded_calls();
        let mut members =
            application_machines(&checked, "windows_x86_64", false, &[(symbol(6), symbol(7))]);
        members.sort_unstable_by_key(|member| member.arena_index());
        assert_eq!(members, [1, 2, 3, 5, 6, 7].map(symbol));
        assert_eq!(
            application_machines(&checked, "macos_arm64", false, &[]),
            vec![symbol(9)]
        );
    }

    #[test]
    fn demand_discovery_keeps_selected_first_and_skips_unrelated_scopes() {
        let mut checked = recorded_calls();
        let empty = ProofApplications::new(&checked);
        assert_eq!(empty.scopes("macos_arm64"), ["macos_arm64"]);
        assert!(!empty.needs_scope("windows_x86_64", "macos_arm64"));
        checked
            .facts
            .constant_recipe_obligations
            .push(ConstantRecipeObligation::retain(
                symbol(3),
                Default::default(),
                ConstantRecipeRequirement::EqualRecipe(Default::default()),
                Default::default(),
                "callback recipe".to_owned(),
            ));
        let demands = ProofApplications::new(&checked);
        assert_eq!(
            demands.scopes("macos_arm64"),
            ["macos_arm64", "windows_x86_64"]
        );
        assert!(demands.needs_scope("windows_x86_64", "macos_arm64"));
        assert!(demands.needs_scope("macos_arm64", "macos_arm64"));
        assert!(!demands.needs_scope("linux_arm64", "macos_arm64"));
    }
}
