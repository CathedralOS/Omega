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

    /// Project one retained callable onto its explicit target member. This
    /// describes membership only; no execution or checking authority is added.
    pub fn callable_for_target(&self, symbol: Declaration, target: &str) -> Option<Declaration> {
        self.target_machine(self.called_machine(symbol)?, target)
            .map(|row| row.machine)
    }

    /// Describe authored target membership independently of Product selection
    /// or executable availability. This adds no invocation or seed authority.
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

    /// Complete descriptive dependencies of independently supplied declaration
    /// roots. This traversal never follows execution/provider edges or grants
    /// a machine body membership merely because a type mentions its context.
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

/// Exact instantiated edge of the existing call/substitution walk. Static
/// operands remain part of application equality rather than flattening into
/// declaration membership. Provider substitution is explicit, not a direct
/// authored call to the implementation.
pub struct ProofInvocationEdge<Declaration> {
    pub source: Application<Declaration>,
    pub target: Application<Declaration>,
    pub call_ordinal: Option<usize>,
    pub provider: bool,
    pub static_delegation: bool,
}

/// Borrowed application queries have no source tree or source-expression access.
pub struct ProofApplications<'a, Declaration: 'static, Calls> {
    plan: &'a ProofApplicationPlan<Declaration>,
    operational: &'a Calls,
    rows: std::sync::OnceLock<MachineRows<'a, Declaration>>,
}

/// The plan's machine rows in arena order, with each row's machine symbol
/// and each of its state symbols sorted once by `declaration_order`.
///
/// Every program machine is a walk root, and each step of the walk looked
/// its callee up by scanning every plan row, which made one walk quadratic
/// in the machine count. Ties keep arena order, so the first match of an
/// equal range is the row the scan found first.
struct MachineRows<'a, Declaration: 'static> {
    rows: Vec<&'a ProofApplicationMachine<Declaration>>,
    by_machine: Vec<(Declaration, usize)>,
    by_state: Vec<(Declaration, usize)>,
}

/// One walk retains executable owners and invoked coordinates separately.
/// A boundary requirement can be invoked without having an executable body.
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

    /// `ProofApplicationPlan::matching_machine` through the sorted rows.
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

    /// `ProofApplicationPlan::target_machine` through the sorted rows.
    fn target_machine(
        &self,
        machine: &'a ProofApplicationMachine<Declaration>,
        target: &str,
    ) -> Option<&'a ProofApplicationMachine<Declaration>> {
        if machine.family_interface {
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
            .is_none_or(|declared| declared == target)
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
        // Membership tests over everything reached so far made one walk
        // quadratic in the machine count. These sets answer the same exact
        // questions (`declaration_order` is injective), and each list keeps
        // first-reached order.
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
            // Missing operational facts are not an empty body. Match the
            // checked graph's stopping boundary before cleanup/dynamic edges.
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
        // Resolve state/family coordinates and their binders exactly as the
        // visited application does; no display identity participates.
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
#[derive(Debug, Clone, Default)]
pub struct ProofCallGraph<Declaration: 'static> {
    pub machines: Arena<ProofCallMachine<Declaration>>,
    pub calls:
        Arena<ProofApplicationCall<Declaration, HandleSpan<ProofStaticBinding<Declaration>>>>,
    pub bindings: Arena<ProofStaticBinding<Declaration>>,
    pub cleanup: Arena<Declaration>,
    /// `machines` sorted by machine, published by the producer once the rows
    /// are final (`index_machines`); ties keep arena order. A walk reads one
    /// row per reached machine, and scanning every row for each made a walk
    /// over every program machine quadratic. It is derived from `machines`,
    /// so equality ignores it.
    machine_index: Vec<(Declaration, arena::Handle<ProofCallMachine<Declaration>>)>,
    /// The `machines` revision and length the index was built at; any later
    /// mutation retires it.
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

impl<Declaration: Copy + Ord + Default + 'static> ProofCallGraph<Declaration> {
    /// Publish `machine_index` over the final `machines` rows.
    pub fn index_machines(&mut self) {
        let mut index = self
            .machines
            .iter()
            .enumerate()
            .map(|(position, (handle, row))| (row.machine, position, handle))
            .collect::<Vec<_>>();
        index.sort_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)));
        self.machine_index = index
            .into_iter()
            .map(|(machine, _, handle)| (machine, handle))
            .collect();
        self.indexed_at = Some((self.machines.revision(), self.machines.len()));
    }

    /// The first row for `machine`, through the published index when it
    /// is current.
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
    /// Rebind retained declaration coordinates without changing scope policy.
    /// The map is supplied by the boundary that owns declaration identity.
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
