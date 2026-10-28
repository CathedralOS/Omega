//! Close the private invocation roster before using caller-side premises.
//! Callable selections are uses independently of direct calls. The shared
//! typed reference walk exposes callbacks and type-held selections; a private
//! helper supplied through such a lane needs its declared contract instead.

use super::{Arrival, ExpressionHandle, Machine, SymbolHandle};
use typed_trees::references::TypedReferences;

/// Specialization custody retains declaration ancestry after executable roots
/// are flattened. It selects a scope, never imports a template's runtime facts.
fn authored_machine(
    program: &dyn crate::CheckingInput,
    mut symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    let mut seen = Vec::new();
    while symbol.is_valid() && !seen.contains(&symbol) {
        seen.push(symbol);
        let Some(template) = program
            .typed_program()
            .executable_application_template(symbol)
            .ok()?
        else {
            return Some(symbol);
        };
        if template == symbol {
            return Some(symbol);
        }
        symbol = template;
    }
    None
}

pub(super) fn is_root(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    root: &Machine,
) -> bool {
    authored_machine(program, machine.symbol) == Some(root.symbol)
}

pub(super) fn lexical_root<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
) -> Option<&'program Machine> {
    let declaration = authored_machine(program, machine.symbol)?;
    let mut current = program.symbols.get(declaration).parent;
    let mut root = None;
    let mut seen = Vec::new();
    while current.is_valid() {
        if seen.contains(&current) {
            return None;
        }
        seen.push(current);
        if let Some(owner) = program.machine_by_symbol(current) {
            root = Some(owner);
        }
        current = program.symbols.get(current).parent;
    }
    root
}

fn belongs_to(
    program: &dyn crate::CheckingInput,
    mut symbol: SymbolHandle,
    root: SymbolHandle,
) -> bool {
    let mut seen = Vec::new();
    while symbol.is_valid() {
        if program.symbols.get(symbol).kind == symbols::SymbolKind::Machine {
            let Some(declaration) = authored_machine(program, symbol) else {
                return false;
            };
            symbol = declaration;
        }
        if seen.contains(&symbol) {
            return false;
        }
        seen.push(symbol);
        if symbol == root {
            return true;
        }
        symbol = program.symbols.get(symbol).parent;
    }
    false
}

fn enclosing_machine_roots(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
) -> Vec<SymbolHandle> {
    let mut symbol = machine.symbol;
    let mut seen = Vec::new();
    let mut roots = Vec::new();
    while symbol.is_valid() {
        if program.symbols.get(symbol).kind == symbols::SymbolKind::Machine {
            let Some(declaration) = authored_machine(program, symbol) else {
                break;
            };
            symbol = declaration;
        }
        if seen.contains(&symbol) {
            break;
        }
        seen.push(symbol);
        if let Some(root) = program.machine_by_symbol(symbol)
            && !is_root(program, machine, root)
        {
            roots.push(symbol);
        }
        symbol = program.symbols.get(symbol).parent;
    }
    roots
}

#[derive(Clone, Copy)]
struct SubordinateCallSite {
    caller_machine_index: usize,
    caller_state: SymbolHandle,
    statement: usize,
    expression: Option<ExpressionHandle>,
    target_machine: SymbolHandle,
}

type SymbolSet = std::collections::HashSet<SymbolHandle, symbols::BuildSymbolHasher>;

pub(crate) struct ProgramPrivateArrivalIndex {
    members_by_root: symbols::SymbolKeyMap<SymbolHandle, Vec<usize>>,
    call_sites_by_root: symbols::SymbolKeyMap<SymbolHandle, Vec<SubordinateCallSite>>,
    unclosed_symbols: SymbolSet,
    callable_use_owners: symbols::SymbolKeyMap<SymbolHandle, Vec<SymbolHandle>>,
}

impl ProgramPrivateArrivalIndex {
    fn build(program: &dyn crate::CheckingInput) -> Self {
        let machines = program.machines();
        let mut members_by_root: symbols::SymbolKeyMap<SymbolHandle, Vec<usize>> =
            Default::default();
        let mut target_roots: symbols::SymbolKeyMap<SymbolHandle, Vec<SymbolHandle>> =
            Default::default();
        let mut all_private_targets = SymbolSet::default();
        for (machine_index, machine) in machines.iter().enumerate() {
            let roots = enclosing_machine_roots(program, machine);
            if roots.is_empty() {
                continue;
            }
            all_private_targets.insert(machine.symbol);
            for state in program.machine_states(machine) {
                all_private_targets.insert(state.symbol);
            }
            for &root_symbol in &roots {
                members_by_root
                    .entry(root_symbol)
                    .or_default()
                    .push(machine_index);
            }
            target_roots.insert(machine.symbol, roots);
        }

        let mut call_sites_by_root: symbols::SymbolKeyMap<SymbolHandle, Vec<SubordinateCallSite>> =
            Default::default();
        if !target_roots.is_empty() {
            for (caller_machine_index, caller) in machines.iter().enumerate() {
                for state in program.machine_states(caller) {
                    for (index, _) in program
                        .statement_table
                        .indexed_statements(state.statement_nodes)
                    {
                        for site in crate::machine_calls::calls::collect_call_sites_in_statement(
                            program, caller, state, index,
                        ) {
                            let Some(target) =
                                program.machine_by_own_or_state_symbol(site.target_symbol(program))
                            else {
                                continue;
                            };
                            let Some(roots) = target_roots.get(&target.symbol) else {
                                continue;
                            };
                            let expression = match site {
                                crate::CallSite::Expression { expression, .. } => Some(expression),
                                _ => None,
                            };
                            let call_site = SubordinateCallSite {
                                caller_machine_index,
                                caller_state: state.symbol,
                                statement: index,
                                expression,
                                target_machine: target.symbol,
                            };
                            for &root_symbol in roots {
                                call_sites_by_root
                                    .entry(root_symbol)
                                    .or_default()
                                    .push(call_site);
                            }
                        }
                    }
                }
            }
        }

        let mut uses = ProgramPrivateUses {
            program,
            target_symbols: all_private_targets,
            owner_scopes: Vec::new(),
            pending_counts: Default::default(),
            seen_expressions: Vec::new(),
            seen_types: Vec::new(),
            unclosed_symbols: Default::default(),
            callable_use_owners: Default::default(),
        };
        if !uses.target_symbols.is_empty() {
            typed_trees::references::visit_program(program.typed_program(), &mut uses);
            for (symbol, count) in uses.pending_counts {
                if count > 0 {
                    uses.unclosed_symbols.insert(symbol);
                }
            }
        }

        Self {
            members_by_root,
            call_sites_by_root,
            unclosed_symbols: uses.unclosed_symbols,
            callable_use_owners: uses.callable_use_owners,
        }
    }
}

pub(crate) struct ClosedArrivals {
    pub(super) expected: Vec<Arrival>,
    /// Successive complete cuts of the retained caller graph, nearest first.
    /// The original lexical/cycle frontier remains the final authority.
    pub(super) caller_frontiers: Vec<Vec<usize>>,
    pub(super) cycle_entries: Vec<SymbolHandle>,
    pub(super) bodies: Vec<SymbolHandle>,
}

pub(super) fn closed_arrivals(
    program: &dyn crate::CheckingInput,
    root: &Machine,
    callee: &Machine,
) -> Result<std::sync::Arc<ClosedArrivals>, &'static str> {
    if let Some(memos) = crate::frozen_program::frozen_program_memos(program) {
        if let Ok(guard) = memos.private_closed_arrivals.lock()
            && let Some(cached) = guard.get(&(root.symbol, callee.symbol))
        {
            return cached.clone();
        }
    }
    // This summary depends only on the frozen call/use graph. It contains
    // neither a runtime observation nor a predicate's proof judgment.
    let result = compute_closed_arrivals(program, root, callee).map(std::sync::Arc::new);
    if let Some(memos) = crate::frozen_program::frozen_program_memos(program)
        && let Ok(mut guard) = memos.private_closed_arrivals.lock()
    {
        guard.insert((root.symbol, callee.symbol), result.clone());
    }
    result
}

fn compute_closed_arrivals(
    program: &dyn crate::CheckingInput,
    root: &Machine,
    callee: &Machine,
) -> Result<ClosedArrivals, &'static str> {
    let index = crate::frozen_program::frozen_memo(
        program,
        |memos| &memos.private_arrival_index,
        || ProgramPrivateArrivalIndex::build(program),
    );
    let machines = program.machines();
    let Some(member_indices) = index.members_by_root.get(&root.symbol) else {
        return Err("the callable is not a private member of the enclosing declaration");
    };
    let members: Vec<_> = member_indices.iter().map(|&idx| &machines[idx]).collect();
    let destination = members
        .iter()
        .position(|member| member.symbol == callee.symbol)
        .ok_or("the callable is not a private member of the enclosing declaration")?;
    if callee.is_public
        || callee.target_family_interface
        || !program.machine_trait_conformances(callee).is_empty()
    {
        return Err("the private declaration group has an exposed callable");
    }
    let mut expected_owners: Vec<(Arrival, usize)> = Vec::new();
    let mut caller_ownership_is_unique = true;
    let mut edges = vec![Vec::new(); members.len()];
    let mut outside = Vec::new();
    let mut declared_entries = Vec::new();
    if let Some(sites) = index.call_sites_by_root.get(&root.symbol) {
        for site in sites {
            let Some(destination) = members
                .iter()
                .position(|member| member.symbol == site.target_machine)
            else {
                continue;
            };
            let caller = &machines[site.caller_machine_index];
            if !belongs_to(program, caller.symbol, root.symbol) {
                outside.push(destination);
            }
            if is_root(program, caller, root) && !declared_entries.contains(&destination) {
                declared_entries.push(destination);
            }
            if let Some(source) = members
                .iter()
                .position(|member| member.symbol == caller.symbol)
                && !edges[source].contains(&destination)
            {
                edges[source].push(destination);
            }
            if site.target_machine == callee.symbol {
                let arrival = Arrival {
                    caller: site.caller_state,
                    statement: site.statement,
                    expression: site.expression,
                };
                if expected_owners.iter().any(|(prior, owner)| {
                    prior.caller == arrival.caller && *owner != site.caller_machine_index
                }) {
                    caller_ownership_is_unique = false;
                }
                if !expected_owners.iter().any(|(prior, _)| *prior == arrival) {
                    expected_owners.push((arrival, site.caller_machine_index));
                }
            }
        }
    }
    let mut expected = Vec::with_capacity(expected_owners.len());
    let mut caller_machine_indices = Vec::new();
    for (arrival, owner) in expected_owners {
        // The graph carries the owner; validate uniqueness once while forming
        // this cached plan rather than rebuilding ownership for each query.
        if !caller_owner_is_unique(program, arrival.caller, owner) {
            caller_ownership_is_unique = false;
        }
        expected.push(arrival);
        if !caller_machine_indices.contains(&owner) {
            caller_machine_indices.push(owner);
        }
    }
    let caller_machine_indices = caller_ownership_is_unique.then_some(caller_machine_indices);
    // Close backward over actual callers of the requested entry. A callback
    // that cannot reach it is independent; an escaping ancestor still matters.
    let mut needed = vec![false; members.len()];
    needed[destination] = true;
    loop {
        let mut changed = false;
        for (source, successors) in edges.iter().enumerate() {
            if !needed[source] && successors.iter().any(|&successor| needed[successor]) {
                needed[source] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    if outside.iter().any(|&destination| needed[destination]) {
        return Err("a private callable has an arrival from outside its lexical group");
    }
    if members.iter().enumerate().any(|(index, member)| {
        needed[index]
            && (member.is_public
                || member.target_family_interface
                || !program.machine_trait_conformances(member).is_empty())
    }) {
        return Err("the private declaration group has an exposed callable");
    }
    let mut target_symbols = Vec::new();
    let mut bodies = Vec::new();
    for (index, member) in members.iter().enumerate() {
        if needed[index] {
            bodies.push(member.symbol);
            target_symbols.push(member.symbol);
            for state in program.machine_states(member) {
                target_symbols.push(state.symbol);
            }
        }
    }
    let closed = target_symbols.iter().all(|symbol| {
        !index.unclosed_symbols.contains(symbol)
            && index.callable_use_owners.get(symbol).is_none_or(|owners| {
                owners
                    .iter()
                    .all(|&owner| belongs_to(program, owner, root.symbol))
            })
    });
    if !closed {
        return Err("a private callable has a non-direct or out-of-scope use");
    }
    // A recurring entry has no inherited runtime values. Its separately
    // observed prefix must establish the requested fact again from arbitrary
    // inputs; roots/initializers and a prior iteration supply no invariant.
    let cycle_entries: Vec<_> = super::cycles::entries(&edges, &needed, &declared_entries)
        .into_iter()
        .map(|index| members[index].symbol)
        .collect();
    let caller_frontiers = progressive_caller_frontiers(
        program,
        root,
        index
            .call_sites_by_root
            .get(&root.symbol)
            .map(Vec::as_slice)
            .unwrap_or(&[]),
        caller_machine_indices,
        &cycle_entries,
    );
    Ok(ClosedArrivals {
        expected,
        caller_frontiers,
        cycle_entries,
        bodies,
    })
}

/// The graph already knows the executable owner. Check ambiguous storage
/// membership once in the cached plan; never resolve an owner by spelling.
fn caller_owner_is_unique(
    program: &dyn crate::CheckingInput,
    state: SymbolHandle,
    owner: usize,
) -> bool {
    let machines = program.machines();
    let Some(caller) = machines.get(owner) else {
        return false;
    };
    program
        .machine_states(caller)
        .iter()
        .any(|entry| entry.symbol == state)
        && !machines.iter().enumerate().any(|(other, machine)| {
            other != owner
                && program
                    .machine_states(machine)
                    .iter()
                    .any(|entry| entry.symbol == state)
        })
}

fn progressive_caller_frontiers(
    program: &dyn crate::CheckingInput,
    root: &Machine,
    sites: &[SubordinateCallSite],
    immediate: Option<Vec<usize>>,
    cycle_entries: &[SymbolHandle],
) -> Vec<Vec<usize>> {
    let Some(mut frontier) = immediate else {
        return Vec::new();
    };
    if frontier.is_empty() {
        return Vec::new();
    }
    let machines = program.machines();
    let mut anchors = vec![false; machines.len()];
    let mut original = Vec::new();
    for (index, machine) in machines.iter().enumerate() {
        let recurrent = cycle_entries.contains(&machine.symbol);
        anchors[index] = recurrent || is_root(program, machine, root);
        if anchors[index] {
            original.push(index);
        }
        // A prior iteration's values never establish a fresh cycle entry.
        if recurrent {
            frontier.push(index);
        }
    }
    frontier.sort_unstable();
    frontier.dedup();
    let mut frontiers = Vec::new();
    loop {
        // The full root set is observed exactly once by the fallback. A
        // repeated cut cannot reveal another independently available premise.
        if frontier == original || frontiers.contains(&frontier) {
            break;
        }
        frontiers.push(frontier.clone());
        let mut expanded = Vec::new();
        for &owner in &frontier {
            let Some(machine) = machines.get(owner) else {
                return frontiers;
            };
            if anchors[owner] {
                expanded.push(owner);
                continue;
            }
            let mut has_incoming = false;
            for site in sites
                .iter()
                .filter(|site| site.target_machine == machine.symbol)
            {
                if !caller_owner_is_unique(program, site.caller_state, site.caller_machine_index) {
                    return frontiers;
                }
                has_incoming = true;
                expanded.push(site.caller_machine_index);
            }
            // An unknown or absent incoming roster cannot be dropped from a
            // cut. Keep the previous cuts and use the original final frontier.
            if !has_incoming {
                return frontiers;
            }
        }
        expanded.sort_unstable();
        expanded.dedup();
        frontier = expanded;
    }
    frontiers
}

fn retain_scoped_handle<Element>(
    seen: &mut Vec<Vec<(Option<SymbolHandle>, arena::Handle<Element>)>>,
    owner: Option<SymbolHandle>,
    handle: arena::Handle<Element>,
) -> bool {
    let slot = handle.arena_index() as usize;
    if seen.len() <= slot {
        seen.resize_with(slot + 1, Vec::new);
    }
    let occurrence = (owner, handle);
    if seen[slot].contains(&occurrence) {
        return false;
    }
    seen[slot].push(occurrence);
    true
}

struct ProgramPrivateUses<'program> {
    program: &'program dyn crate::CheckingInput,
    target_symbols: SymbolSet,
    owner_scopes: Vec<Option<SymbolHandle>>,
    pending_counts: symbols::SymbolKeyMap<SymbolHandle, usize>,
    seen_expressions: Vec<Vec<(Option<SymbolHandle>, ExpressionHandle)>>,
    seen_types: Vec<
        Vec<(
            Option<SymbolHandle>,
            typed_trees::types::TypeReferenceHandle,
        )>,
    >,
    unclosed_symbols: SymbolSet,
    callable_use_owners: symbols::SymbolKeyMap<SymbolHandle, Vec<SymbolHandle>>,
}

impl ProgramPrivateUses<'_> {
    fn owner(&self) -> Option<SymbolHandle> {
        self.owner_scopes.last().copied().flatten()
    }
    fn private_target(&self, symbol: SymbolHandle) -> bool {
        self.target_symbols.contains(&symbol)
    }
    fn direct(&mut self, symbol: SymbolHandle) {
        if self.private_target(symbol) {
            *self.pending_counts.entry(symbol).or_insert(0) += 1;
        }
    }
}

impl Extend<SymbolHandle> for ProgramPrivateUses<'_> {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _symbols: T) {}
}

impl TypedReferences for ProgramPrivateUses<'_> {
    fn enter_lexical_scope(&mut self, symbol: SymbolHandle) {
        let next_owner = self
            .program
            .machine_by_symbol(symbol)
            .map(|machine| machine.symbol)
            .or_else(|| self.owner());
        self.owner_scopes.push(next_owner);
    }
    fn leave_lexical_scope(&mut self) {
        self.owner_scopes.pop();
    }
    fn callable_use(&mut self, symbol: SymbolHandle) {
        if !self.private_target(symbol) {
            return;
        }
        if let Some(owner) = self.owner() {
            let owners = self.callable_use_owners.entry(symbol).or_default();
            if !owners.contains(&owner) {
                owners.push(owner);
            }
        } else {
            self.unclosed_symbols.insert(symbol);
        }
        if let Some(count) = self.pending_counts.get_mut(&symbol)
            && *count > 0
        {
            *count -= 1;
        } else {
            self.unclosed_symbols.insert(symbol);
        }
    }
    fn statement(&mut self, statement: &typed_trees::statement::StatementNode) -> bool {
        if let typed_trees::statement::StatementNode::Call(call) = statement {
            self.direct(call.target_symbol);
        }
        true
    }
    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        let owner = self.owner();
        if !retain_scoped_handle(&mut self.seen_expressions, owner, expression) {
            return false;
        }
        match self.program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Call(call) => self.direct(call.target_symbol),
            typed_trees::expression::ExpressionNode::Name(path)
                if self.private_target(path.symbol) =>
            {
                self.unclosed_symbols.insert(path.symbol);
            }
            _ => {}
        }
        true
    }
    fn type_reference(&mut self, reference: typed_trees::types::TypeReferenceHandle) -> bool {
        let owner = self.owner();
        retain_scoped_handle(&mut self.seen_types, owner, reference)
    }
}
