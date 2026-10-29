//! Application scope of conditional proof requirements, independent of source.
//!
//! Checking freezes declaration relationships; product selection narrows seed
//! eligibility without discarding those descriptions. Call and static-binding
//! facts remain owned by OperationalPlan. Provider candidates discover possible
//! work only; actual membership consumes the caller's settled provider edges.

use arena::{Arena, HandleSpan};
use flow_effects::{OperationalPlan, StaticMachineCallBinding};
use symbols::SymbolHandle;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProofApplicationMachine {
    pub machine: SymbolHandle,
    pub states: HandleSpan<SymbolHandle>,
    pub binders: HandleSpan<SymbolHandle>,
    pub authored_target: Option<String>,
    pub source_target: Option<String>,
    pub family_interface: bool,
    pub family_members: HandleSpan<ProofApplicationTargetMember>,
    pub dynamic_realizations: HandleSpan<SymbolHandle>,
    pub product_selected: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProofApplicationTargetMember {
    pub target: String,
    pub machine: SymbolHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProofDeclarationDependencies {
    /// Several retained application payloads may contribute to the same owner.
    pub declaration: SymbolHandle,
    pub dependencies: HandleSpan<SymbolHandle>,
    /// Trait formation includes its exact own/refining signature declarations.
    pub signatures: HandleSpan<SymbolHandle>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProofApplicationPlan {
    pub machines: Arena<ProofApplicationMachine>,
    pub target_members: Arena<ProofApplicationTargetMember>,
    pub symbols: Arena<SymbolHandle>,
    pub declaration_dependencies: Arena<ProofDeclarationDependencies>,
    pub declarations: HandleSpan<SymbolHandle>,
    pub obligation_owners: HandleSpan<SymbolHandle>,
    pub provider_candidates: Arena<(SymbolHandle, SymbolHandle)>,
    pub has_obligations: bool,
    pub unscoped_recipe: bool,
}

impl ProofApplicationPlan {
    /// Narrow the selected-product view; retained metadata is not a seed.
    pub fn retain_product_machines(&mut self, mut selected: impl FnMut(SymbolHandle) -> bool) {
        self.machines.for_each_mut(|_, machine| {
            machine.product_selected &= selected(machine.machine);
        });
    }

    fn called_machine(&self, symbol: SymbolHandle) -> Option<&ProofApplicationMachine> {
        self.machines
            .iter()
            .map(|(_, row)| row)
            .find(|row| row.product_selected && row.machine == symbol)
            .or_else(|| {
                self.machines.iter().map(|(_, row)| row).find(|row| {
                    row.product_selected && self.symbols.span_or_empty(row.states).contains(&symbol)
                })
            })
    }

    fn target_machine<'a>(
        &'a self,
        machine: &'a ProofApplicationMachine,
        target: &str,
    ) -> Option<&'a ProofApplicationMachine> {
        if machine.family_interface {
            let member = self
                .target_members
                .span_or_empty(machine.family_members)
                .iter()
                .find(|member| member.target == target)?;
            return self.called_machine(member.machine);
        }
        machine
            .authored_target
            .as_deref()
            .is_none_or(|declared| declared == target)
            .then_some(machine)
    }

    fn declaration_membership(&self, machines: &[SymbolHandle]) -> Vec<SymbolHandle> {
        let mut roots = machines.to_vec();
        let mut next = 0;
        while let Some(owner) = roots.get(next).copied() {
            next += 1;
            for (_, row) in self
                .declaration_dependencies
                .iter()
                .filter(|(_, row)| row.declaration == owner)
            {
                for dependency in self.symbols.span_or_empty(row.dependencies) {
                    if !roots.contains(dependency) {
                        roots.push(*dependency);
                    }
                }
            }
        }
        roots.retain(|root| !machines.contains(root));
        for (_, row) in self.declaration_dependencies.iter() {
            if roots.contains(&row.declaration) {
                roots.extend(self.symbols.span_or_empty(row.signatures));
            }
        }
        roots.sort_unstable_by_key(|root| (root.arena_index(), root.generation()));
        roots.dedup();
        roots
    }
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

/// Borrowed application queries have no source tree or source-expression access.
pub struct ProofApplications<'a> {
    plan: &'a ProofApplicationPlan,
    operational: &'a OperationalPlan,
}

impl<'a> ProofApplications<'a> {
    pub fn new(plan: &'a ProofApplicationPlan, operational: &'a OperationalPlan) -> Self {
        Self { plan, operational }
    }

    pub fn scopes<'request>(&'request self, primary: &'request str) -> Vec<&'request str> {
        let mut scopes = vec![primary];
        if self.plan.has_obligations {
            for (_, machine) in self.plan.machines.iter() {
                if machine.product_selected
                    && let Some(scope) = machine.authored_target.as_deref()
                    && !scopes.contains(&scope)
                {
                    scopes.push(scope);
                }
            }
        }
        scopes
    }

    pub fn needs_scope(&self, scope: &str, primary: &str) -> bool {
        if scope == primary || self.plan.unscoped_recipe {
            return true;
        }
        let owners = self.plan.symbols.span_or_empty(self.plan.obligation_owners);
        let (machines, declarations) =
            self.membership(scope, false, self.plan.provider_candidates.storage_slice());
        machines
            .iter()
            .chain(&declarations)
            .any(|owner| owners.contains(owner))
    }

    pub fn declarations(&self) -> &[SymbolHandle] {
        self.plan.symbols.span_or_empty(self.plan.declarations)
    }

    pub fn membership(
        &self,
        scope: &str,
        selected_product: bool,
        providers: &[(SymbolHandle, SymbolHandle)],
    ) -> (Vec<SymbolHandle>, Vec<SymbolHandle>) {
        let machines = self.application_machines(scope, selected_product, providers);
        let declarations = self.plan.declaration_membership(&machines);
        (machines, declarations)
    }

    fn application_machines(
        &self,
        target: &str,
        selected_product: bool,
        provider_edges: &[(SymbolHandle, SymbolHandle)],
    ) -> Vec<SymbolHandle> {
        let mut pending = Vec::new();
        for (_, machine) in self.plan.machines.iter() {
            let authored = machine.source_target.as_deref();
            if machine.product_selected
                && (authored.is_some_and(|name| name == target)
                    || (selected_product && authored.is_none()))
            {
                pending.push(Application {
                    machine: machine.machine,
                    bindings: Vec::new(),
                });
            }
        }
        let mut visited = Vec::new();
        let mut machines = Vec::new();
        while let Some(mut application) = pending.pop() {
            let Some(declaration) = self.plan.called_machine(application.machine) else {
                continue;
            };
            let Some(machine) = self.plan.target_machine(declaration, target) else {
                continue;
            };
            if declaration.machine != machine.machine {
                for binding in &mut application.bindings {
                    if let Some((_, selected)) = self
                        .plan
                        .symbols
                        .span_or_empty(declaration.binders)
                        .iter()
                        .zip(self.plan.symbols.span_or_empty(machine.binders))
                        .find(|(declared, _)| **declared == binding.parameter)
                    {
                        binding.parameter = *selected;
                    }
                }
            }
            application.machine = machine.machine;
            if visited.contains(&application) {
                continue;
            }
            visited.push(application.clone());
            if !machines.contains(&machine.machine) {
                machines.push(machine.machine);
            }
            let Some(summary) = self.operational.machine_by_symbol(machine.machine) else {
                continue;
            };
            for state in self.operational.states.span_or_empty(summary.states) {
                for call in self.operational.calls.span_or_empty(state.calls) {
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
                            resolve_bindings(
                                self.operational,
                                call.static_machine_bindings,
                                &application.bindings,
                            ),
                        ),
                    };
                    pending.push(Application {
                        machine: callee,
                        bindings,
                    });
                    let requirement_machine = self
                        .plan
                        .called_machine(callee)
                        .map(|machine| machine.machine);
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
            for (_, cleanup) in self.operational.cleanup_dependencies.iter() {
                if cleanup.caller_machine == machine.machine {
                    pending.push(Application {
                        machine: cleanup.target_machine,
                        bindings: Vec::new(),
                    });
                }
            }
            pending.extend(
                self.plan
                    .symbols
                    .span_or_empty(machine.dynamic_realizations)
                    .iter()
                    .map(|realization| Application {
                        machine: *realization,
                        bindings: Vec::new(),
                    }),
            );
        }
        machines
    }
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
