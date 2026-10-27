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

pub(super) fn closed_arrivals(
    program: &dyn crate::CheckingInput,
    root: &Machine,
    callee: &Machine,
) -> Result<Vec<Arrival>, &'static str> {
    let members: Vec<_> = program
        .machines()
        .iter()
        .filter(|machine| {
            !is_root(program, machine, root) && belongs_to(program, machine.symbol, root.symbol)
        })
        .collect();
    let destination = members
        .iter()
        .position(|member| member.symbol == callee.symbol)
        .ok_or("the callable is not a private member of the enclosing declaration")?;
    let mut expected = Vec::new();
    let mut edges = vec![Vec::new(); members.len()];
    let mut outside = Vec::new();
    for caller in program.machines() {
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
                    let Some(destination) = members
                        .iter()
                        .position(|member| member.symbol == target.symbol)
                    else {
                        continue;
                    };
                    if !belongs_to(program, caller.symbol, root.symbol) {
                        outside.push(destination);
                    }
                    if let Some(source) = members
                        .iter()
                        .position(|member| member.symbol == caller.symbol)
                        && !edges[source].contains(&destination)
                    {
                        edges[source].push(destination);
                    }
                    if target.symbol == callee.symbol {
                        let expression = match site {
                            crate::CallSite::Expression { expression, .. } => Some(expression),
                            _ => None,
                        };
                        let arrival = Arrival {
                            caller: state.symbol,
                            statement: index,
                            expression,
                        };
                        if !expected.contains(&arrival) {
                            expected.push(arrival);
                        }
                    }
                }
            }
        }
    }
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
    let mut uses = PrivateUses {
        program,
        root: root.symbol,
        members: members
            .iter()
            .enumerate()
            .filter_map(|(index, member)| needed[index].then_some(member.symbol))
            .collect(),
        scopes: Vec::new(),
        pending: Vec::new(),
        expressions: Vec::new(),
        types: Vec::new(),
        closed: true,
    };
    typed_trees::references::visit_program(program.typed_program(), &mut uses);
    if !uses.closed || !uses.pending.is_empty() {
        return Err("a private callable has a non-direct or out-of-scope use");
    }
    // Cycles avoiding the declared enclosing entry need their own established
    // invariant. The first observed arrival cannot justify later iterations.
    let mut incoming = vec![0usize; members.len()];
    for (_, successors) in edges
        .iter()
        .enumerate()
        .filter(|(source, _)| needed[*source])
    {
        for &successor in successors {
            if needed[successor] {
                incoming[successor] += 1;
            }
        }
    }
    let mut ready: Vec<_> = incoming
        .iter()
        .enumerate()
        .filter_map(|(index, &count)| (needed[index] && count == 0).then_some(index))
        .collect();
    let mut visited = 0;
    while let Some(index) = ready.pop() {
        visited += 1;
        for &successor in &edges[index] {
            if !needed[successor] {
                continue;
            }
            incoming[successor] -= 1;
            if incoming[successor] == 0 {
                ready.push(successor);
            }
        }
    }
    (visited == needed.iter().filter(|&&needed| needed).count())
        .then_some(expected)
        .ok_or("a private call cycle avoids the enclosing declaration entry")
}

struct PrivateUses<'program> {
    program: &'program dyn crate::CheckingInput,
    root: SymbolHandle,
    members: Vec<SymbolHandle>,
    scopes: Vec<SymbolHandle>,
    pending: Vec<SymbolHandle>,
    expressions: Vec<(Option<SymbolHandle>, ExpressionHandle)>,
    types: Vec<(
        Option<SymbolHandle>,
        typed_trees::types::TypeReferenceHandle,
    )>,
    closed: bool,
}

impl PrivateUses<'_> {
    fn owner(&self) -> Option<SymbolHandle> {
        self.scopes.iter().rev().find_map(|symbol| {
            self.program
                .machine_by_symbol(*symbol)
                .map(|machine| machine.symbol)
        })
    }
    fn private_target(&self, symbol: SymbolHandle) -> bool {
        self.program
            .machine_by_own_or_state_symbol(symbol)
            .is_some_and(|machine| self.members.contains(&machine.symbol))
    }
    fn direct(&mut self, symbol: SymbolHandle) {
        if self.private_target(symbol) {
            self.pending.push(symbol);
        }
    }
}

impl Extend<SymbolHandle> for PrivateUses<'_> {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _symbols: T) {}
}

impl TypedReferences for PrivateUses<'_> {
    fn enter_lexical_scope(&mut self, symbol: SymbolHandle) {
        self.scopes.push(symbol);
    }
    fn leave_lexical_scope(&mut self) {
        self.scopes.pop();
    }
    fn callable_use(&mut self, symbol: SymbolHandle) {
        if !self.private_target(symbol) {
            return;
        }
        if !self
            .owner()
            .is_some_and(|owner| belongs_to(self.program, owner, self.root))
        {
            self.closed = false;
        }
        if let Some(position) = self.pending.iter().rposition(|pending| *pending == symbol) {
            self.pending.remove(position);
        } else {
            self.closed = false;
        }
    }
    fn statement(&mut self, statement: &typed_trees::statement::StatementNode) -> bool {
        if let typed_trees::statement::StatementNode::Call(call) = statement {
            self.direct(call.target_symbol);
        }
        true
    }
    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        let key = (self.owner(), expression);
        if self.expressions.contains(&key) {
            return false;
        }
        self.expressions.push(key);
        match self.program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Call(call) => self.direct(call.target_symbol),
            typed_trees::expression::ExpressionNode::Name(path)
                if self.private_target(path.symbol) =>
            {
                self.closed = false
            }
            _ => {}
        }
        true
    }
    fn type_reference(&mut self, reference: typed_trees::types::TypeReferenceHandle) -> bool {
        let key = (self.owner(), reference);
        if self.types.contains(&key) {
            return false;
        }
        self.types.push(key);
        true
    }
}
