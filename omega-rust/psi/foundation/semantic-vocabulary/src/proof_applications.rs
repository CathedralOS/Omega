//! Application scope of conditional proof requirements, independent of source.
//!
//! Checking freezes declaration relationships; product selection narrows seed
//! eligibility without discarding those descriptions. Call and static-binding
//! facts remain owned by the producing graph. Provider candidates discover possible
//! work only; actual membership consumes the caller's settled provider edges.

use arena::{Arena, HandleSpan};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProofApplicationMachine<Declaration: 'static> {
    pub machine: Declaration,
    pub states: HandleSpan<Declaration>,
    pub binders: HandleSpan<Declaration>,
    pub dynamic_realizations: HandleSpan<Declaration>,
    pub product_selected: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProofDeclarationDependencies<Declaration: 'static> {
    /// Several retained application payloads may contribute to the same owner.
    pub declaration: Declaration,
    pub dependencies: HandleSpan<Declaration>,
    /// Trait formation includes its exact own/refining signature declarations.
    pub signatures: HandleSpan<Declaration>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProofApplicationPlan<Declaration: 'static> {
    pub machines: Arena<ProofApplicationMachine<Declaration>>,
    pub symbols: Arena<Declaration>,
    pub declaration_dependencies: Arena<ProofDeclarationDependencies<Declaration>>,
    pub declarations: HandleSpan<Declaration>,
    pub obligation_owners: HandleSpan<Declaration>,
    pub provider_candidates: Arena<(Declaration, Declaration)>,
    pub has_obligations: bool,
    pub unscoped_recipe: bool,
}

impl<Declaration: Copy + Eq + Default + 'static> ProofApplicationPlan<Declaration> {
    /// Narrow the selected-product view; retained metadata is not a seed.
    pub fn retain_product_machines(&mut self, mut selected: impl FnMut(Declaration) -> bool) {
        self.machines.for_each_mut(|_, machine| {
            machine.product_selected &= selected(machine.machine);
        });
    }

    fn called_machine(&self, symbol: Declaration) -> Option<&ProofApplicationMachine<Declaration>> {
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

    fn declaration_membership(&self, machines: &[Declaration]) -> Vec<Declaration> {
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
        roots
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Binding<Declaration> {
    parameter: Declaration,
    selected: Declaration,
    arguments: Vec<Binding<Declaration>>,
}

#[derive(Clone, PartialEq, Eq)]
struct Application<Declaration> {
    machine: Declaration,
    bindings: Vec<Binding<Declaration>>,
}

/// Borrowed application queries have no source tree or source-expression access.
pub struct ProofApplications<'a, Declaration: 'static, Calls> {
    plan: &'a ProofApplicationPlan<Declaration>,
    operational: &'a Calls,
}

/// Executable declaration owners and invoked coordinates from the same walk.
/// A boundary requirement can be called without an executable machine body.
pub struct ProofApplicationReachability<Declaration> {
    pub machines: Vec<Declaration>,
    pub calls: Vec<Declaration>,
}

impl<'a, Declaration: Copy + Eq + Default + 'static, Calls: ProofApplicationCalls<Declaration>>
    ProofApplications<'a, Declaration, Calls>
{
    pub fn new(plan: &'a ProofApplicationPlan<Declaration>, operational: &'a Calls) -> Self {
        Self { plan, operational }
    }

    pub fn declarations(&self) -> &[Declaration] {
        self.plan.symbols.span_or_empty(self.plan.declarations)
    }

    pub fn membership(
        &self,
        providers: &[(Declaration, Declaration)],
    ) -> (Vec<Declaration>, Vec<Declaration>) {
        self.membership_from_roots(providers, None)
    }

    /// None checks the retained product surface; explicit roots describe only
    /// the supplied invocation context without promoting retained candidates.
    pub fn membership_from_roots(
        &self,
        providers: &[(Declaration, Declaration)],
        roots: Option<&[Declaration]>,
    ) -> (Vec<Declaration>, Vec<Declaration>) {
        let machines = self.reachability(providers, roots).machines;
        let mut declarations = self.plan.declaration_membership(&machines);
        declarations.sort_unstable_by_key(|declaration| Calls::declaration_order(*declaration));
        declarations.dedup();
        (machines, declarations)
    }

    pub fn reachability(
        &self,
        provider_edges: &[(Declaration, Declaration)],
        roots: Option<&[Declaration]>,
    ) -> ProofApplicationReachability<Declaration> {
        let mut pending = Vec::new();
        if let Some(roots) = roots {
            pending.extend(roots.iter().map(|root| Application {
                machine: *root,
                bindings: Vec::new(),
            }));
        }
        for (_, machine) in self.plan.machines.iter() {
            if roots.is_none() && machine.product_selected {
                pending.push(Application {
                    machine: machine.machine,
                    bindings: Vec::new(),
                });
            }
        }
        let mut visited = Vec::new();
        let mut machines = Vec::new();
        let mut calls = Vec::new();
        while let Some(mut application) = pending.pop() {
            if application.machine != Declaration::default()
                && !calls.contains(&application.machine)
            {
                calls.push(application.machine);
            }
            let Some(machine) = self.plan.called_machine(application.machine) else {
                continue;
            };
            application.machine = machine.machine;
            if visited.contains(&application) {
                continue;
            }
            visited.push(application.clone());
            if !machines.contains(&machine.machine) {
                machines.push(machine.machine);
            }
            let has_summary = self.operational.visit_calls(machine.machine, |call| {
                for coordinate in [call.state, call.operator] {
                    if coordinate != Declaration::default() && !calls.contains(&coordinate) {
                        calls.push(coordinate);
                    }
                }
                let selected = application
                    .bindings
                    .iter()
                    .find(|binding| binding.parameter == call.binder);
                let (callee, bindings) = match selected {
                    Some(selected) => (selected.selected, selected.arguments.clone()),
                    None => (
                        call.callee,
                        resolve_bindings(self.operational, call.arguments, &application.bindings),
                    ),
                };
                pending.push(Application {
                    machine: callee,
                    bindings,
                });
                let requirement_machine = self.plan.called_machine(callee).map(|row| row.machine);
                for (requirement, realization) in provider_edges {
                    if *requirement == callee
                        || Some(*requirement) == requirement_machine
                        || *requirement == call.state
                        || *requirement == call.operator
                    {
                        pending.push(Application {
                            machine: *realization,
                            bindings: Vec::new(),
                        });
                    }
                }
            });
            // Missing operational facts are not an empty body. Match the
            // checked graph's stopping boundary before cleanup/dynamic edges.
            if !has_summary {
                continue;
            }
            self.operational.visit_cleanup(machine.machine, |target| {
                pending.push(Application {
                    machine: target,
                    bindings: Vec::new(),
                });
            });
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
        ProofApplicationReachability { machines, calls }
    }
}

fn resolve_bindings<Declaration: Copy + Eq, Calls: ProofApplicationCalls<Declaration>>(
    plan: &Calls,
    arguments: Calls::Bindings,
    enclosing: &[Binding<Declaration>],
) -> Vec<Binding<Declaration>> {
    let mut resolved = Vec::new();
    plan.visit_bindings(arguments, |parameter, selected, arguments| {
        let binding = match enclosing
            .iter()
            .find(|binding| binding.parameter == selected)
        {
            Some(enclosing) => Binding {
                parameter,
                selected: enclosing.selected,
                arguments: enclosing.arguments.clone(),
            },
            None => Binding {
                parameter,
                selected,
                arguments: resolve_bindings(plan, arguments, enclosing),
            },
        };
        resolved.push(binding);
    });
    resolved
}

/// Borrowed call coordinates. The producer resolves the static binder fallback
/// once; consumers never infer a binding from a display name or emitted body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofApplicationCall<Declaration, Bindings> {
    pub binder: Declaration,
    pub callee: Declaration,
    pub state: Declaration,
    pub operator: Declaration,
    pub arguments: Bindings,
}

/// The checked graph and portable graph share one application walk. Source
/// borrowers project their existing operational facts; portable consumers read
/// the published rows. Neither reconstructs calls from executable instructions.
pub trait ProofApplicationCalls<Declaration> {
    type Bindings: Copy;
    type Order: Ord;

    fn declaration_order(declaration: Declaration) -> Self::Order;

    /// Return false when this machine has no retained operational summary.
    fn visit_calls(
        &self,
        machine: Declaration,
        visit: impl FnMut(ProofApplicationCall<Declaration, Self::Bindings>),
    ) -> bool;
    fn visit_bindings(
        &self,
        bindings: Self::Bindings,
        visit: impl FnMut(Declaration, Declaration, Self::Bindings),
    );
    fn visit_cleanup(&self, machine: Declaration, visit: impl FnMut(Declaration));
}

/// Source-free operational projection. Only the relationships used by proof
/// scope are published; effect summaries remain with their existing owner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofCallGraph<Declaration: 'static> {
    pub machines: Arena<ProofCallMachine<Declaration>>,
    pub calls:
        Arena<ProofApplicationCall<Declaration, HandleSpan<ProofStaticBinding<Declaration>>>>,
    pub bindings: Arena<ProofStaticBinding<Declaration>>,
    pub cleanup: Arena<Declaration>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofCallMachine<Declaration: 'static> {
    pub machine: Declaration,
    pub calls:
        HandleSpan<ProofApplicationCall<Declaration, HandleSpan<ProofStaticBinding<Declaration>>>>,
    pub cleanup: HandleSpan<Declaration>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofStaticBinding<Declaration: 'static> {
    pub parameter: Declaration,
    pub selected: Declaration,
    pub arguments: HandleSpan<ProofStaticBinding<Declaration>>,
}

impl<Declaration: Copy + Eq + Default + 'static> ProofApplicationCalls<Declaration>
    for ProofCallGraph<Declaration>
where
    Declaration: Ord,
{
    type Bindings = HandleSpan<ProofStaticBinding<Declaration>>;
    type Order = Declaration;

    fn declaration_order(declaration: Declaration) -> Declaration {
        declaration
    }

    fn visit_calls(
        &self,
        machine: Declaration,
        mut visit: impl FnMut(ProofApplicationCall<Declaration, Self::Bindings>),
    ) -> bool {
        let Some(row) = self
            .machines
            .iter()
            .map(|(_, row)| row)
            .find(|row| row.machine == machine)
        else {
            return false;
        };
        for call in self.calls.span_or_empty(row.calls) {
            visit(call.clone());
        }
        true
    }

    fn visit_bindings(
        &self,
        bindings: Self::Bindings,
        mut visit: impl FnMut(Declaration, Declaration, Self::Bindings),
    ) {
        for binding in self.bindings.span_or_empty(bindings) {
            visit(binding.parameter, binding.selected, binding.arguments);
        }
    }

    fn visit_cleanup(&self, machine: Declaration, mut visit: impl FnMut(Declaration)) {
        if let Some(row) = self
            .machines
            .iter()
            .map(|(_, row)| row)
            .find(|row| row.machine == machine)
        {
            for target in self.cleanup.span_or_empty(row.cleanup) {
                visit(*target);
            }
        }
    }
}

impl<Declaration: Copy + Eq + Default + 'static> ProofApplicationPlan<Declaration> {
    /// Rebind retained declaration coordinates without changing scope policy.
    /// The map is supplied by the boundary that owns declaration identity.
    pub fn try_map<Mapped: Copy + Eq + Default + 'static, Error>(
        &self,
        mut map: impl FnMut(Declaration) -> Result<Mapped, Error>,
    ) -> Result<ProofApplicationPlan<Mapped>, Error> {
        let mut result = ProofApplicationPlan::default();
        result.has_obligations = self.has_obligations;
        result.unscoped_recipe = self.unscoped_recipe;
        for (_, machine) in self.machines.iter() {
            let states = map_span(&self.symbols, machine.states, &mut result.symbols, &mut map)?;
            let binders = map_span(
                &self.symbols,
                machine.binders,
                &mut result.symbols,
                &mut map,
            )?;
            let dynamic_realizations = map_span(
                &self.symbols,
                machine.dynamic_realizations,
                &mut result.symbols,
                &mut map,
            )?;
            result.machines.insert(ProofApplicationMachine {
                machine: map(machine.machine)?,
                states,
                binders,
                dynamic_realizations,
                product_selected: machine.product_selected,
            });
        }
        for (_, row) in self.declaration_dependencies.iter() {
            let declaration = map(row.declaration)?;
            let dependencies = map_span(
                &self.symbols,
                row.dependencies,
                &mut result.symbols,
                &mut map,
            )?;
            let signatures =
                map_span(&self.symbols, row.signatures, &mut result.symbols, &mut map)?;
            result
                .declaration_dependencies
                .insert(ProofDeclarationDependencies {
                    declaration,
                    dependencies,
                    signatures,
                });
        }
        result.declarations = map_span(
            &self.symbols,
            self.declarations,
            &mut result.symbols,
            &mut map,
        )?;
        result.obligation_owners = map_span(
            &self.symbols,
            self.obligation_owners,
            &mut result.symbols,
            &mut map,
        )?;
        for (_, (requirement, implementation)) in self.provider_candidates.iter() {
            result
                .provider_candidates
                .insert((map(*requirement)?, map(*implementation)?));
        }
        Ok(result)
    }
}

fn map_span<Declaration: Copy + Default, Mapped: Default, Error>(
    source: &Arena<Declaration>,
    span: HandleSpan<Declaration>,
    destination: &mut Arena<Mapped>,
    map: &mut impl FnMut(Declaration) -> Result<Mapped, Error>,
) -> Result<HandleSpan<Mapped>, Error> {
    let mapped = source
        .span_or_empty(span)
        .iter()
        .map(|value| map(*value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(destination.insert_many(mapped))
}

impl<Declaration: Copy + Eq + Default + 'static> ProofCallGraph<Declaration> {
    /// Capture the producing graph through the same view used by membership.
    /// Missing summaries stay missing; no body or provider demand is invented.
    pub fn capture<
        Source: Copy + Eq + Default + 'static,
        Calls: ProofApplicationCalls<Source>,
        Error,
    >(
        source: &Calls,
        machines: impl IntoIterator<Item = Source>,
        map: &mut impl FnMut(Source) -> Result<Declaration, Error>,
    ) -> Result<Self, Error> {
        let mut result = Self::default();
        for machine in machines {
            let mut calls = Vec::new();
            let present = source.visit_calls(machine, |call| {
                calls.push((|| {
                    Ok(ProofApplicationCall {
                        binder: map(call.binder)?,
                        callee: map(call.callee)?,
                        state: map(call.state)?,
                        operator: map(call.operator)?,
                        arguments: result.capture_bindings(source, call.arguments, map)?,
                    })
                })());
            });
            if !present {
                continue;
            }
            let calls = result
                .calls
                .insert_many(calls.into_iter().collect::<Result<Vec<_>, Error>>()?);
            let mut cleanup = Vec::new();
            source.visit_cleanup(machine, |target| cleanup.push(map(target)));
            let cleanup = result
                .cleanup
                .insert_many(cleanup.into_iter().collect::<Result<Vec<_>, Error>>()?);
            result.machines.insert(ProofCallMachine {
                machine: map(machine)?,
                calls,
                cleanup,
            });
        }
        Ok(result)
    }

    fn capture_bindings<Source: Copy, Calls: ProofApplicationCalls<Source>, Error>(
        &mut self,
        source: &Calls,
        arguments: Calls::Bindings,
        map: &mut impl FnMut(Source) -> Result<Declaration, Error>,
    ) -> Result<HandleSpan<ProofStaticBinding<Declaration>>, Error> {
        let mut bindings = Vec::new();
        source.visit_bindings(arguments, |parameter, selected, arguments| {
            bindings.push((|| {
                Ok(ProofStaticBinding {
                    parameter: map(parameter)?,
                    selected: map(selected)?,
                    arguments: self.capture_bindings(source, arguments, map)?,
                })
            })());
        });
        Ok(self
            .bindings
            .insert_many(bindings.into_iter().collect::<Result<Vec<_>, Error>>()?))
    }
}
