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
    pub authored_target: Option<String>,
    pub source_target: Option<String>,
    pub family_interface: bool,
    pub family_members: HandleSpan<ProofApplicationTargetMember<Declaration>>,
    pub dynamic_realizations: HandleSpan<Declaration>,
    pub product_selected: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProofApplicationTargetMember<Declaration> {
    pub target: String,
    pub machine: Declaration,
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
    pub target_members: Arena<ProofApplicationTargetMember<Declaration>>,
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

    fn target_machine<'a>(
        &'a self,
        machine: &'a ProofApplicationMachine<Declaration>,
        target: &str,
    ) -> Option<&'a ProofApplicationMachine<Declaration>> {
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

/// One walk retains executable owners and invoked coordinates separately.
/// A boundary requirement can be invoked without having an executable body.
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
        let (machines, declarations) = self.membership(
            scope,
            false,
            self.plan.provider_candidates.storage_slice(),
            None,
        );
        machines
            .iter()
            .chain(&declarations)
            .any(|owner| owners.contains(owner))
    }

    pub fn declarations(&self) -> &[Declaration] {
        self.plan.symbols.span_or_empty(self.plan.declarations)
    }

    /// Discover required authored scopes from the actual condition owners.
    /// Portable consumers supply those owners directly instead of trusting a
    /// separately serialized obligation-presence or owner-list summary.
    pub fn required_scopes<'request>(
        &'request self,
        primary: &'request str,
        owners: &[Declaration],
        unscoped: bool,
    ) -> Vec<&'request str> {
        let mut scopes = vec![primary];
        if owners.is_empty() && !unscoped {
            return scopes;
        }
        for (_, machine) in self.plan.machines.iter() {
            let Some(scope) = machine.authored_target.as_deref() else {
                continue;
            };
            if !machine.product_selected || scopes.contains(&scope) {
                continue;
            }
            let (machines, declarations) = self.membership(
                scope,
                false,
                self.plan.provider_candidates.storage_slice(),
                None,
            );
            if unscoped
                || machines
                    .iter()
                    .chain(&declarations)
                    .any(|owner| owners.contains(owner))
            {
                scopes.push(scope);
            }
        }
        scopes
    }

    pub fn membership(
        &self,
        scope: &str,
        selected_product: bool,
        providers: &[(Declaration, Declaration)],
        roots: Option<&[Declaration]>,
    ) -> (Vec<Declaration>, Vec<Declaration>) {
        let machines = self
            .reachability(scope, selected_product, providers, roots)
            .machines;
        let mut declarations = self.plan.declaration_membership(&machines);
        declarations.sort_unstable_by_key(|declaration| Calls::declaration_order(*declaration));
        declarations.dedup();
        (machines, declarations)
    }

    pub fn reachability(
        &self,
        target: &str,
        selected_product: bool,
        provider_edges: &[(Declaration, Declaration)],
        roots: Option<&[Declaration]>,
    ) -> ProofApplicationReachability<Declaration> {
        let mut pending = roots
            .into_iter()
            .flatten()
            .map(|root| Application {
                machine: *root,
                bindings: Vec::new(),
            })
            .collect::<Vec<_>>();
        for (_, machine) in self.plan.machines.iter() {
            let authored = machine.source_target.as_deref();
            if machine.product_selected
                && (authored.is_some_and(|name| name == target)
                    || (roots.is_none() && selected_product && authored.is_none()))
            {
                pending.push(Application {
                    machine: machine.machine,
                    bindings: Vec::new(),
                });
            }
        }
        self.walk_applications(target, provider_edges, pending)
    }

    /// Physical application demand begins only at supplied invocations. Merely
    /// declaring a target-scoped body does not invoke its layout policies.
    /// Conditional language checking uses `reachability` to include authored
    /// source scopes as well; both consumers share the same call/substitution walk.
    pub fn invocation_reachability(
        &self,
        target: &str,
        provider_edges: &[(Declaration, Declaration)],
        roots: &[Declaration],
    ) -> ProofApplicationReachability<Declaration> {
        let pending = roots
            .iter()
            .map(|root| Application {
                machine: *root,
                bindings: Vec::new(),
            })
            .collect();
        self.walk_applications(target, provider_edges, pending)
    }

    fn walk_applications(
        &self,
        target: &str,
        provider_edges: &[(Declaration, Declaration)],
        mut pending: Vec<Application<Declaration>>,
    ) -> ProofApplicationReachability<Declaration> {
        let mut visited = Vec::new();
        let mut machines = Vec::new();
        let mut calls = Vec::new();
        while let Some(mut application) = pending.pop() {
            if application.machine != Declaration::default()
                && !calls.contains(&application.machine)
            {
                calls.push(application.machine);
            }
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
            let members = self
                .target_members
                .span_or_empty(machine.family_members)
                .iter()
                .map(|member| {
                    Ok(ProofApplicationTargetMember {
                        target: member.target.clone(),
                        machine: map(member.machine)?,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            let family_members = result.target_members.insert_many(members);
            result.machines.insert(ProofApplicationMachine {
                machine: map(machine.machine)?,
                states,
                binders,
                dynamic_realizations,
                family_members,
                authored_target: machine.authored_target.clone(),
                source_target: machine.source_target.clone(),
                family_interface: machine.family_interface,
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
