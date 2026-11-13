/*
We validate and canonicalize proof applications, checking formal-to-actual
argument bindings, scope isolation, and premise satisfaction across mathematical
and scalar proof terms.
*/

use arena::{Arena, HandleSpan};

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
    pub declaration: Declaration,
    pub dependencies: HandleSpan<Declaration>,
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
    pub fn retain_product_machines(&mut self, mut selected: impl FnMut(Declaration) -> bool) {
        self.machines.for_each_mut(|_, machine| {
            machine.product_selected &= selected(machine.machine);
        });
    }

    pub fn callable_for_target(&self, symbol: Declaration, target: &str) -> Option<Declaration> {
        self.target_machine(self.called_machine(symbol)?, target)
            .map(|row| row.machine)
    }

    /// No target is selected. Only an untagged, non-family declaration may
    /// supply code; this does not choose a host or a default family member.
    pub fn callable_without_target(&self, symbol: Declaration) -> Option<Declaration> {
        let machine = self.called_machine(symbol)?;
        (machine.authored_target.is_none()
            && machine.source_target.is_none()
            && !machine.family_interface)
            .then_some(machine.machine)
    }

    pub fn declared_callable_for_target(
        &self,
        symbol: Declaration,
        target: &str,
    ) -> Option<Declaration> {
        self.target_machine_matching(self.matching_machine(symbol, |_| true)?, target, |_| true)
            .map(|row| row.machine)
    }

    fn called_machine(&self, symbol: Declaration) -> Option<&ProofApplicationMachine<Declaration>> {
        self.matching_machine(symbol, |row| row.product_selected)
    }

    fn matching_machine(
        &self,
        symbol: Declaration,
        accepts: fn(&ProofApplicationMachine<Declaration>) -> bool,
    ) -> Option<&ProofApplicationMachine<Declaration>> {
        self.machines
            .iter()
            .map(|(_, row)| row)
            .find(|row| accepts(row) && row.machine == symbol)
            .or_else(|| {
                self.machines.iter().map(|(_, row)| row).find(|row| {
                    accepts(row) && self.symbols.span_or_empty(row.states).contains(&symbol)
                })
            })
    }

    fn target_machine<'a>(
        &'a self,
        machine: &'a ProofApplicationMachine<Declaration>,
        target: &str,
    ) -> Option<&'a ProofApplicationMachine<Declaration>> {
        self.target_machine_matching(machine, target, |row| row.product_selected)
    }

    fn target_machine_matching<'a>(
        &'a self,
        machine: &'a ProofApplicationMachine<Declaration>,
        target: &str,
        accepts: fn(&ProofApplicationMachine<Declaration>) -> bool,
    ) -> Option<&'a ProofApplicationMachine<Declaration>> {
        if machine.family_interface {
            let member = self
                .target_members
                .span_or_empty(machine.family_members)
                .iter()
                .find(|member| member.target == target)?;
            return self.matching_machine(member.machine, accepts);
        }
        machine
            .authored_target
            .as_deref()
            .is_none_or(|declared| declared == target)
            .then_some(machine)
    }

    pub fn declaration_closure(&self, declarations: &[Declaration]) -> Vec<Declaration> {
        let mut roots = self.declaration_dependencies(declarations);
        self.retain_declaration_signatures(&mut roots);
        roots
    }

    fn declaration_membership(&self, machines: &[Declaration]) -> Vec<Declaration> {
        let mut roots = self.declaration_dependencies(machines);
        roots.retain(|root| !machines.contains(root));
        self.retain_declaration_signatures(&mut roots);
        roots
    }

    fn declaration_dependencies(&self, declarations: &[Declaration]) -> Vec<Declaration> {
        let mut roots = declarations.to_vec();
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
        roots
    }

    fn retain_declaration_signatures(&self, roots: &mut Vec<Declaration>) {
        for (_, row) in self.declaration_dependencies.iter() {
            if roots.contains(&row.declaration) {
                roots.extend(self.symbols.span_or_empty(row.signatures));
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Binding<Declaration> {
    parameter: Declaration,
    selected: Declaration,
    arguments: Vec<Binding<Declaration>>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Application<Declaration> {
    machine: Declaration,
    bindings: Vec<Binding<Declaration>>,
}

impl<Declaration: Copy> Application<Declaration> {
    pub fn machine(&self) -> Declaration {
        self.machine
    }
}

pub struct ProofInvocationEdge<Declaration> {
    pub source: Application<Declaration>,
    pub target: Application<Declaration>,
    pub call_ordinal: Option<usize>,
    pub provider: bool,
    pub static_delegation: bool,
}

pub struct ProofApplications<'a, Declaration: 'static, Calls> {
    plan: &'a ProofApplicationPlan<Declaration>,
    operational: &'a Calls,
    rows: std::sync::OnceLock<MachineRows<'a, Declaration>>,
}

struct MachineRows<'a, Declaration: 'static> {
    rows: Vec<&'a ProofApplicationMachine<Declaration>>,
    by_machine: Vec<(Declaration, usize)>,
    by_state: Vec<(Declaration, usize)>,
}

pub struct ProofApplicationReachability<Declaration> {
    pub machines: Vec<Declaration>,
    pub calls: Vec<Declaration>,
    pub invocations: Vec<ProofInvocationEdge<Declaration>>,
}

impl<'a, Declaration: Copy + Eq + Default + 'static, Calls: ProofApplicationCalls<Declaration>>
    ProofApplications<'a, Declaration, Calls>
{
    pub fn new(plan: &'a ProofApplicationPlan<Declaration>, operational: &'a Calls) -> Self {
        Self {
            plan,
            operational,
            rows: std::sync::OnceLock::new(),
        }
    }

    fn rows(&self) -> &MachineRows<'a, Declaration> {
        self.rows.get_or_init(|| {
            let rows = self
                .plan
                .machines
                .iter()
                .map(|(_, row)| row)
                .collect::<Vec<_>>();
            let mut by_machine = rows
                .iter()
                .enumerate()
                .map(|(position, row)| (row.machine, position))
                .collect::<Vec<_>>();
            by_machine
                .sort_by_key(|(symbol, position)| (Calls::declaration_order(*symbol), *position));
            let mut by_state = rows
                .iter()
                .enumerate()
                .flat_map(|(position, row)| {
                    self.plan
                        .symbols
                        .span_or_empty(row.states)
                        .iter()
                        .map(move |state| (*state, position))
                })
                .collect::<Vec<_>>();
            by_state
                .sort_by_key(|(symbol, position)| (Calls::declaration_order(*symbol), *position));
            MachineRows {
                rows,
                by_machine,
                by_state,
            }
        })
    }

    fn matching_machine(
        &self,
        symbol: Declaration,
        accepts: fn(&ProofApplicationMachine<Declaration>) -> bool,
    ) -> Option<&'a ProofApplicationMachine<Declaration>> {
        let rows = self.rows();
        let find = |sorted: &[(Declaration, usize)]| {
            let key = Calls::declaration_order(symbol);
            let start = sorted.partition_point(|(row, _)| Calls::declaration_order(*row) < key);
            sorted[start..]
                .iter()
                .take_while(|(row, _)| Calls::declaration_order(*row) == key)
                .filter(|(row, _)| *row == symbol)
                .map(|(_, position)| rows.rows[*position])
                .find(|row| accepts(row))
        };
        find(&rows.by_machine).or_else(|| find(&rows.by_state))
    }

    fn called_machine(
        &self,
        symbol: Declaration,
    ) -> Option<&'a ProofApplicationMachine<Declaration>> {
        self.matching_machine(symbol, |row| row.product_selected)
    }

    fn target_machine(
        &self,
        machine: &'a ProofApplicationMachine<Declaration>,
        target: Option<&str>,
    ) -> Option<&'a ProofApplicationMachine<Declaration>> {
        if target.is_none() && machine.source_target.is_some() {
            return None;
        }
        if machine.family_interface {
            let target = target?;
            let member = self
                .plan
                .target_members
                .span_or_empty(machine.family_members)
                .iter()
                .find(|member| member.target == target)?;
            return self.matching_machine(member.machine, |row| row.product_selected);
        }
        machine
            .authored_target
            .as_deref()
            .is_none_or(|declared| Some(declared) == target)
            .then_some(machine)
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
        self.walk_applications(Some(target), provider_edges, pending)
    }

    pub fn invocation_reachability(
        &self,
        target: &str,
        provider_edges: &[(Declaration, Declaration)],
        roots: &[Declaration],
    ) -> ProofApplicationReachability<Declaration> {
        self.invocation_reachability_for(Some(target), provider_edges, roots)
    }

    /// Reachability for an invocation with an explicitly optional target.
    /// Untargeted traversal never selects a tagged body or a family member.
    pub fn invocation_reachability_for(
        &self,
        target: Option<&str>,
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
        target: Option<&str>,
        provider_edges: &[(Declaration, Declaration)],
        mut pending: Vec<Application<Declaration>>,
    ) -> ProofApplicationReachability<Declaration> {
        let mut visited =
            std::collections::BTreeMap::<Calls::Order, Vec<Application<Declaration>>>::new();
        let mut machines = Vec::new();
        let mut seen_machines = std::collections::BTreeSet::new();
        let mut calls = Vec::new();
        let mut seen_calls = std::collections::BTreeSet::new();
        let mut invocations = Vec::new();
        while let Some(mut application) = pending.pop() {
            if application.machine != Declaration::default()
                && seen_calls.insert(Calls::declaration_order(application.machine))
            {
                calls.push(application.machine);
            }
            let Some(declaration) = self.called_machine(application.machine) else {
                continue;
            };
            let Some(machine) = self.target_machine(declaration, target) else {
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
            let same_machine = visited
                .entry(Calls::declaration_order(application.machine))
                .or_default();
            if same_machine.contains(&application) {
                continue;
            }
            same_machine.push(application.clone());
            if seen_machines.insert(Calls::declaration_order(machine.machine)) {
                machines.push(machine.machine);
            }
            let mut call_ordinal = 0;
            let has_summary = self.operational.visit_calls(machine.machine, |call| {
                for coordinate in [call.state, call.operator] {
                    if coordinate != Declaration::default()
                        && seen_calls.insert(Calls::declaration_order(coordinate))
                    {
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
                let invoked = Application {
                    machine: callee,
                    bindings,
                };
                invocations.push(ProofInvocationEdge {
                    source: application.clone(),
                    target: invoked.clone(),
                    call_ordinal: Some(call_ordinal),
                    provider: false,
                    static_delegation: selected.is_some(),
                });
                pending.push(invoked);
                let requirement_machine = self.called_machine(callee).map(|row| row.machine);
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
                        invocations.push(ProofInvocationEdge {
                            source: application.clone(),
                            target: Application {
                                machine: *realization,
                                bindings: Vec::new(),
                            },
                            call_ordinal: Some(call_ordinal),
                            provider: true,
                            static_delegation: false,
                        });
                    }
                }
                call_ordinal += 1;
            });
            if !has_summary {
                continue;
            }
            self.operational.visit_cleanup(machine.machine, |target| {
                invocations.push(ProofInvocationEdge {
                    source: application.clone(),
                    target: Application {
                        machine: target,
                        bindings: Vec::new(),
                    },
                    call_ordinal: None,
                    provider: false,
                    static_delegation: false,
                });
                pending.push(Application {
                    machine: target,
                    bindings: Vec::new(),
                });
            });
            for realization in self
                .plan
                .symbols
                .span_or_empty(machine.dynamic_realizations)
            {
                invocations.push(ProofInvocationEdge {
                    source: application.clone(),
                    target: Application {
                        machine: *realization,
                        bindings: Vec::new(),
                    },
                    call_ordinal: None,
                    provider: false,
                    static_delegation: false,
                });
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
        for edge in &mut invocations {
            if let Some(declaration) = self.called_machine(edge.target.machine)
                && let Some(machine) = self.target_machine(declaration, target)
            {
                if declaration.machine != machine.machine {
                    for binding in &mut edge.target.bindings {
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
                edge.target.machine = machine.machine;
            }
        }
        ProofApplicationReachability {
            machines,
            calls,
            invocations,
        }
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofApplicationCall<Declaration, Bindings> {
    pub binder: Declaration,
    pub callee: Declaration,
    pub state: Declaration,
    pub operator: Declaration,
    pub arguments: Bindings,
}

pub trait ProofApplicationCalls<Declaration> {
    type Bindings: Copy;
    type Order: Ord;

    fn declaration_order(declaration: Declaration) -> Self::Order;

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

#[derive(Debug, Clone, Default)]
pub struct ProofCallGraph<Declaration: 'static> {
    machines: Arena<ProofCallMachine<Declaration>>,
    pub calls:
        Arena<ProofApplicationCall<Declaration, HandleSpan<ProofStaticBinding<Declaration>>>>,
    pub bindings: Arena<ProofStaticBinding<Declaration>>,
    pub cleanup: Arena<Declaration>,
    machine_index: Vec<(Declaration, arena::Handle<ProofCallMachine<Declaration>>)>,
    indexed_at: Option<(u64, usize)>,
}

impl<Declaration: PartialEq> PartialEq for ProofCallGraph<Declaration> {
    fn eq(&self, other: &Self) -> bool {
        self.machines == other.machines
            && self.calls == other.calls
            && self.bindings == other.bindings
            && self.cleanup == other.cleanup
    }
}

impl<Declaration: Eq> Eq for ProofCallGraph<Declaration> {}

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
        let Some(row) = self.machine_row(machine) else {
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
        if let Some(row) = self.machine_row(machine) {
            for target in self.cleanup.span_or_empty(row.cleanup) {
                visit(*target);
            }
        }
    }
}

impl<Declaration: Default + 'static> ProofCallGraph<Declaration> {
    pub fn machines(&self) -> &Arena<ProofCallMachine<Declaration>> {
        &self.machines
    }

    pub fn insert_machine(
        &mut self,
        row: ProofCallMachine<Declaration>,
    ) -> arena::Handle<ProofCallMachine<Declaration>> {
        self.machines.insert(row)
    }

    pub fn update_machine(
        &mut self,
        row: arena::Handle<ProofCallMachine<Declaration>>,
        edit: impl FnOnce(&mut ProofCallMachine<Declaration>),
    ) {
        edit(self.machines.get_mut(row));
    }
}

impl<Declaration: Copy + Ord + Default + 'static> ProofCallGraph<Declaration> {
    pub fn index_machines(&mut self) {
        let mut index = self
            .machines
            .iter()
            .enumerate()
            .map(|(position, (handle, row))| (row.machine, position, handle))
            .collect::<Vec<_>>();
        index.sort_by_key(|left| (left.0, left.1));
        self.machine_index = index
            .into_iter()
            .map(|(machine, _, handle)| (machine, handle))
            .collect();
        self.indexed_at = Some((self.machines.revision(), self.machines.len()));
    }

    fn machine_row(&self, machine: Declaration) -> Option<&ProofCallMachine<Declaration>> {
        if self.indexed_at != Some((self.machines.revision(), self.machines.len())) {
            return self
                .machines
                .iter()
                .map(|(_, row)| row)
                .find(|row| row.machine == machine);
        }
        let start = self
            .machine_index
            .partition_point(|(row, _)| *row < machine);
        self.machine_index[start..]
            .iter()
            .take_while(|(row, _)| *row == machine)
            .map(|(_, handle)| self.machines.get(*handle))
            .find(|row| row.machine == machine)
    }
}

impl<Declaration: Copy + Eq + Default + 'static> ProofApplicationPlan<Declaration> {
    pub fn try_map<Mapped: Copy + Eq + Default + 'static, Error>(
        &self,
        mut map: impl FnMut(Declaration) -> Result<Mapped, Error>,
    ) -> Result<ProofApplicationPlan<Mapped>, Error> {
        let mut result = ProofApplicationPlan {
            has_obligations: self.has_obligations,
            unscoped_recipe: self.unscoped_recipe,
            ..Default::default()
        };
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
            result.insert_machine(ProofCallMachine {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_untargeted_invocation_retains_but_does_not_select_a_tagged_callee() {
        let mut plan = ProofApplicationPlan::<u32>::default();
        machine(&mut plan, 1, None);
        machine(&mut plan, 2, Some("linux_x86_64"));
        machine(&mut plan, 3, None);
        let mut graph = ProofCallGraph::<u32>::default();
        let calls = graph.calls.insert_many([
            ProofApplicationCall { callee: 2, ..Default::default() },
            ProofApplicationCall { callee: 3, ..Default::default() },
        ]);
        graph.insert_machine(ProofCallMachine { machine: 1, calls, ..Default::default() });
        graph.insert_machine(ProofCallMachine { machine: 2, ..Default::default() });
        graph.insert_machine(ProofCallMachine { machine: 3, ..Default::default() });
        let applications = ProofApplications::new(&plan, &graph);
        let absent = applications.invocation_reachability_for(None, &[], &[1]);
        assert!(absent.calls.contains(&2), "admission must still see the demanded tagged call");
        assert!(!absent.machines.contains(&2));
        assert!(absent.machines.contains(&1) && absent.machines.contains(&3));
        let named = applications.invocation_reachability("linux_x86_64", &[], &[1]);
        assert!(named.machines.contains(&2));
        assert_eq!(plan.callable_without_target(2), None);
        assert_eq!(plan.callable_without_target(1), Some(1));
    }

    #[test]
    fn declared_target_membership_is_not_product_membership() {
        let mut plan = ProofApplicationPlan::<u32>::default();
        let members = plan
            .target_members
            .insert_many([ProofApplicationTargetMember {
                target: "macos_arm64".into(),
                machine: 2,
            }]);
        let head = plan.machines.insert(ProofApplicationMachine {
            machine: 1,
            family_interface: true,
            family_members: members,
            ..Default::default()
        });
        let body = plan.machines.insert(ProofApplicationMachine {
            machine: 2,
            authored_target: Some("macos_arm64".into()),
            ..Default::default()
        });
        assert_eq!(plan.declared_callable_for_target(1, "macos_arm64"), Some(2));
        assert_eq!(plan.declared_callable_for_target(1, "linux_arm64"), None);
        assert_eq!(plan.callable_for_target(1, "macos_arm64"), None);
        plan.machines.get_mut(head).product_selected = true;
        assert_eq!(plan.callable_for_target(1, "macos_arm64"), None);
        plan.machines.get_mut(body).product_selected = true;
        assert_eq!(plan.callable_for_target(1, "macos_arm64"), Some(2));
        assert_eq!(plan.callable_for_target(1, "linux_arm64"), None);
    }

    #[test]
    fn family_binder_rebasing_keeps_distinct_static_applications() {
        let mut plan = ProofApplicationPlan::<u32>::default();
        machine(&mut plan, 1, Some("macos_arm64"));
        let interface_binders = plan.symbols.insert_many([10]);
        let member_binders = plan.symbols.insert_many([11]);
        let members = plan
            .target_members
            .insert_many([ProofApplicationTargetMember {
                target: "macos_arm64".into(),
                machine: 3,
            }]);
        plan.machines.insert(ProofApplicationMachine {
            machine: 2,
            binders: interface_binders,
            family_interface: true,
            family_members: members,
            product_selected: true,
            ..Default::default()
        });
        plan.machines.insert(ProofApplicationMachine {
            machine: 3,
            binders: member_binders,
            authored_target: Some("macos_arm64".into()),
            product_selected: true,
            ..Default::default()
        });
        machine(&mut plan, 4, None);
        machine(&mut plan, 5, None);
        let mut graph = ProofCallGraph::<u32>::default();
        let first = graph.bindings.insert_many([ProofStaticBinding {
            parameter: 10,
            selected: 4,
            ..Default::default()
        }]);
        let second = graph.bindings.insert_many([ProofStaticBinding {
            parameter: 10,
            selected: 5,
            ..Default::default()
        }]);
        let calls = graph.calls.insert_many([
            ProofApplicationCall {
                callee: 2,
                arguments: first,
                ..Default::default()
            },
            ProofApplicationCall {
                callee: 2,
                arguments: second,
                ..Default::default()
            },
        ]);
        graph.insert_machine(ProofCallMachine {
            machine: 1,
            calls,
            ..Default::default()
        });
        let calls = graph.calls.insert_many([ProofApplicationCall {
            binder: 11,
            ..Default::default()
        }]);
        graph.insert_machine(ProofCallMachine {
            machine: 3,
            calls,
            ..Default::default()
        });
        let mut reached = ProofApplications::new(&plan, &graph)
            .membership("macos_arm64", false, &[], None)
            .0;
        reached.sort_unstable();
        assert_eq!(reached, [1, 3, 4, 5]);
        assert!(
            ProofApplications::new(&plan, &graph)
                .membership("windows_x86_64", false, &[], None)
                .0
                .is_empty()
        );
    }

    fn symbol(ordinal: u32) -> u32 {
        ordinal
    }

    fn machine(plan: &mut ProofApplicationPlan<u32>, ordinal: u32, scope: Option<&str>) {
        plan.machines.insert(ProofApplicationMachine {
            machine: symbol(ordinal),
            authored_target: scope.map(str::to_owned),
            source_target: scope.map(str::to_owned),
            product_selected: true,
            ..Default::default()
        });
    }

    #[test]
    fn family_lookup_keeps_exact_members_and_missing_targets() {
        let mut plan = ProofApplicationPlan::default();
        let members = plan.target_members.insert_many([
            ProofApplicationTargetMember {
                target: "macos_arm64".into(),
                machine: symbol(2),
            },
            ProofApplicationTargetMember {
                target: "windows_x86_64".into(),
                machine: symbol(3),
            },
        ]);
        plan.machines.insert(ProofApplicationMachine {
            machine: symbol(1),
            family_interface: true,
            family_members: members,
            product_selected: true,
            ..Default::default()
        });
        machine(&mut plan, 2, Some("macos_arm64"));
        machine(&mut plan, 3, Some("windows_x86_64"));
        let head = plan.called_machine(symbol(1)).expect("family head");
        let member = plan.called_machine(symbol(2)).expect("exact member");
        assert_eq!(
            plan.target_machine(head, "windows_x86_64")
                .map(|body| body.machine),
            Some(symbol(3))
        );
        assert!(plan.target_machine(head, "linux_arm64").is_none());
        assert!(plan.target_machine(member, "windows_x86_64").is_none());
        assert_eq!(
            plan.target_machine(member, "macos_arm64")
                .map(|body| body.machine),
            Some(symbol(2))
        );
    }

    #[test]
    fn repeated_declaration_payloads_contribute_all_formation_dependencies() {
        let mut plan = ProofApplicationPlan::default();
        for (owner, dependencies) in [(1, vec![2]), (2, vec![3]), (2, vec![4]), (4, vec![2, 5])] {
            let dependencies = plan
                .symbols
                .insert_many(dependencies.into_iter().map(symbol));
            plan.declaration_dependencies
                .insert(ProofDeclarationDependencies {
                    declaration: symbol(owner),
                    dependencies,
                    ..Default::default()
                });
        }
        assert_eq!(
            plan.declaration_membership(&[symbol(1)]),
            [2, 3, 4, 5].map(symbol)
        );
    }

    #[test]
    fn invocation_roots_do_not_seed_uninvoked_scoped_branches() {
        let mut plan = ProofApplicationPlan::<u32>::default();
        machine(&mut plan, 1, None);
        machine(&mut plan, 2, Some("macos_arm64"));
        machine(&mut plan, 3, Some("macos_arm64"));
        machine(&mut plan, 4, Some("windows_x86_64"));
        let mut graph = ProofCallGraph::<u32>::default();
        let calls = graph.calls.insert_many([ProofApplicationCall {
            callee: 10,
            ..Default::default()
        }]);
        graph.insert_machine(ProofCallMachine {
            machine: 1,
            calls,
            ..Default::default()
        });
        let applications = ProofApplications::new(&plan, &graph);
        let reached = applications.invocation_reachability("macos_arm64", &[(10, 2)], &[1]);
        assert_eq!(reached.machines, [1, 2]);
        assert!(reached.calls.contains(&10));
        let language = applications.reachability("macos_arm64", false, &[(10, 2)], Some(&[1]));
        assert!(language.machines.contains(&3));
        assert!(!language.machines.contains(&4));
    }
}
