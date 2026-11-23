//! A pending import is relevant only at a use whose ordinary lookup found no declaration.
//! Walk the selected entry and the helpers it actually calls before choosing a preparation.

use super::{EntryPlan, PendingBuildImport};
use crate::source_symbols::UnresolvedName;
use diagnostics::Diagnostic;
use source_text::{SourceSpan, Span};
use symbol_resolved_trees::source_resolved::SymbolResolvedTrees;
use symbol_resolved_trees::source_resolved::expression::{ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::source_resolved::machine::Machine;
use symbol_resolved_trees::source_resolved::name::DiagnosticName;
use symbol_resolved_trees::source_resolved::statement::{
    StatementNode, TransitionGuardNode, TransitionTargetNode,
};
use symbol_resolved_trees::source_resolved::{
    domain::ProofFact,
    state::State,
    types::{FixedArrayLength, TypeReference},
};
use symbols::{SymbolHandle, SymbolKind};

#[derive(Default)]
struct Uses {
    pending: bool,
    calls: Vec<SymbolHandle>,
    locals: Vec<SymbolHandle>,
    call_site: Option<(Span, SymbolHandle)>,
}

struct Lookup<'a> {
    trees: &'a SymbolResolvedTrees,
    imports: &'a [PendingBuildImport],
}

impl<'a> Lookup<'a> {
    fn new(trees: &'a SymbolResolvedTrees, imports: &'a [PendingBuildImport]) -> Self {
        Self { trees, imports }
    }

    fn machine(&self, owner: SymbolHandle) -> Option<&'a Machine> {
        self.trees.machines.unique_by_symbol(owner)
    }

    fn local(&self, symbol: SymbolHandle, uses: &mut Uses) {
        if symbol.is_valid()
            && matches!(
                self.trees.symbols.get(symbol).kind,
                SymbolKind::Local | SymbolKind::Parameter
            )
        {
            uses.locals.push(symbol);
        }
    }

    fn missing(
        &self,
        outcome: UnresolvedName,
        members: &[DiagnosticName],
        target: Option<&DiagnosticName>,
    ) -> bool {
        if outcome != UnresolvedName::NoVisibleDeclaration {
            return false;
        }
        let Some(first) = members.first().or(target) else {
            return false;
        };
        let source = first.source_span().source_id;
        let mut path = members.iter().chain(target).map(|name| name.as_str());
        let first = path.next().unwrap();
        self.imports.iter().any(|import| {
            import.source == source && import_matches(&import.path, first, path.clone())
        })
    }

    fn observe(&self, symbol: SymbolHandle, uses: &mut Uses) {
        if !symbol.is_valid() {
            return;
        }
        let declaration = self.trees.symbols.get(symbol);
        match declaration.kind {
            SymbolKind::Local | SymbolKind::Parameter => uses.locals.push(symbol),
            SymbolKind::Machine => uses.calls.push(symbol),
            SymbolKind::State => uses.calls.push(declaration.parent),
            _ => {}
        }
    }

    fn missing_type(&self, symbol: SymbolHandle, name: &DiagnosticName) -> bool {
        !symbol.is_valid()
            && self.imports.iter().any(|import| {
                import.source == name.source_span().source_id
                    && import_matches(&import.path, name.as_str(), std::iter::empty())
            })
            && matches!(
                self.trees
                    .symbols
                    .lookup_source_type(name.as_str(), name.source_span()),
                crate::source_symbols::SymbolLookup::NotFound
            )
    }

    fn type_uses(
        &self,
        root: &TypeReference,
        uses: &mut Uses,
        expressions: &mut Vec<ExpressionHandle>,
    ) {
        let trees = self.trees;
        let mut children = Vec::new();
        let mut current = Some(root);
        while let Some(ty) = current.take().or_else(|| children.pop()) {
            match ty {
                TypeReference::Named { symbol, name, .. } => {
                    uses.pending |= self.missing_type(*symbol, name)
                }
                TypeReference::Generic(generic) => {
                    uses.pending |= self.missing_type(generic.base_symbol, &generic.base_name);
                    children.extend(trees.child_type_references(generic.arguments));
                }
                TypeReference::Reference(reference) => {
                    current = Some(trees.child_type_reference(reference.referee))
                }
                TypeReference::Slice(slice) => {
                    current = Some(trees.child_type_reference(slice.element_type))
                }
                TypeReference::Constrained(constrained) => {
                    current = Some(trees.child_type_reference(constrained.base_type))
                }
                TypeReference::FixedArray(array) => {
                    current = Some(trees.child_type_reference(array.element_type));
                    if let FixedArrayLength::Expression(expr) = array.length {
                        children.push(trees.child_type_reference(expr));
                    }
                }
                TypeReference::ConstExpression(value) => expressions.push(value.expression),
                TypeReference::AliasUse {
                    expansion,
                    application,
                    ..
                } => {
                    current = Some(expansion);
                    children.push(application);
                }
                TypeReference::Callable { .. } | TypeReference::Unit => {}
            }
        }
    }

    fn header(&self, machine: &Machine, state: &State) -> Uses {
        let mut uses = Uses::default();
        let mut expressions = Vec::new();
        for ty in self
            .trees
            .state_parameters(state.parameters)
            .iter()
            .map(|parameter| &parameter.type_reference)
            .chain(state.return_type.iter())
        {
            self.type_uses(ty, &mut uses, &mut expressions);
        }
        for clause in self
            .trees
            .machine_contracts(machine)
            .iter()
            .chain(self.trees.signature_contracts(state.contracts))
        {
            for fact in self.trees.proof_facts(clause.facts) {
                match fact {
                    ProofFact::Expression(expression) => expressions.push(*expression),
                    ProofFact::Membership(fact) => {
                        expressions.push(fact.value);
                        for ty in self.trees.child_type_references(fact.domain_arguments) {
                            self.type_uses(ty, &mut uses, &mut expressions);
                        }
                    }
                }
            }
        }
        self.expression_uses(expressions, &mut uses);
        uses
    }

    fn statement(&self, state: &State, statement: &StatementNode) -> Uses {
        let rows = &self.trees.tables.bodies.statements;
        let table = &self.trees.tables.bodies.expressions;
        let mut uses = Uses::default();
        let mut pending = Vec::<ExpressionHandle>::new();
        match statement {
            StatementNode::LocalData(local) => {
                if let Some((_, ty)) = self.trees.state_local(state, local.symbol) {
                    self.type_uses(ty, &mut uses, &mut pending);
                }
                pending.push(local.initial_value);
                if local.initial_value.is_valid()
                    && let ExpressionNode::Call(call) = table.expression(local.initial_value)
                {
                    uses.call_site = Some((call.target.source_span().span, call.target_symbol));
                }
            }
            StatementNode::Expression(expression) | StatementNode::Evaluate { expression, .. } => {
                pending.push(*expression);
                if expression.is_valid()
                    && let ExpressionNode::Call(call) = table.expression(*expression)
                {
                    uses.call_site = Some((call.target.source_span().span, call.target_symbol));
                }
            }
            StatementNode::Assignment(write) => pending.extend([write.target, write.value]),
            StatementNode::Call(call) => {
                uses.call_site = Some((call.target.source_span().span, call.target_symbol));
                self.observe(call.target_symbol, &mut uses);
                self.local(call.receiver_root_symbol, &mut uses);
                if !call.target_symbol.is_valid() && !call.receiver_root_symbol.is_valid() {
                    uses.pending |= self.missing(
                        call.unresolved_name,
                        rows.name_path_members(call.receiver),
                        Some(&call.target),
                    );
                }
                pending.push(call.callee);
                pending.extend(rows.expression_handles(call.arguments));
            }
            StatementNode::RootBinding(binding) => {
                pending.extend([binding.receiver, binding.implementation_operand])
            }
            StatementNode::ProofOutputBindingStatement(binding) => pending.push(binding.call),
            StatementNode::AssemblyFact(fact) => pending.push(fact.expression),
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(value) = transition.guard {
                    pending.push(value);
                }
                for pattern in rows.transition_patterns(transition.patterns) {
                    pending.extend([
                        pattern.subject,
                        pattern.pattern.head,
                        pattern.pattern.condition,
                    ]);
                }
                for target in [transition.target, transition.continuation]
                    .into_iter()
                    .filter(|target| target.is_valid())
                {
                    match rows.transition_target(target) {
                        TransitionTargetNode::Named {
                            path,
                            arguments,
                            operational_acknowledgement,
                            ..
                        } => {
                            if !transition.continuation.is_valid()
                                && operational_acknowledgement.acknowledges_suspend
                                && let Some(last) = rows.name_path_members(path.members).last()
                            {
                                uses.call_site = Some((last.source_span().span, path.symbol));
                            }
                            self.observe(path.symbol, &mut uses);
                            self.local(path.head_symbol, &mut uses);
                            if !path.symbol.is_valid() && !path.head_symbol.is_valid() {
                                uses.pending |= self.missing(
                                    path.unresolved_name,
                                    rows.name_path_members(path.members),
                                    None,
                                );
                            }
                            pending.extend(rows.expression_handles(*arguments));
                        }
                        TransitionTargetNode::Value(value) => {
                            if !transition.continuation.is_valid()
                                && let ExpressionNode::Call(call) = table.expression(*value)
                                && call.operational_acknowledgement.acknowledges_suspend
                            {
                                uses.call_site =
                                    Some((call.target.source_span().span, call.target_symbol));
                            }
                            pending.push(*value);
                        }
                        TransitionTargetNode::Terminal => {}
                    }
                }
            }
            // Authored assembly is not a Build evaluation operation. Its ordinary
            // resolver and checker diagnose it if the selected entry reaches it.
            StatementNode::Assembly(_) => {}
        }
        self.expression_uses(pending, &mut uses);
        uses
    }

    fn expression_uses(&self, mut pending: Vec<ExpressionHandle>, uses: &mut Uses) {
        let table = &self.trees.tables.bodies.expressions;
        while let Some(handle) = pending.pop() {
            if !handle.is_valid() {
                continue;
            }
            let node = table.expression(handle);
            match node {
                ExpressionNode::Name(path) => {
                    self.local(path.head_symbol, uses);
                    self.local(path.symbol, uses);
                    if !path.head_symbol.is_valid() && !path.symbol.is_valid() {
                        uses.pending |= self.missing(
                            table.unresolved_name(handle),
                            table.name_path_members(path.members),
                            None,
                        );
                    }
                }
                ExpressionNode::StructLiteral(literal) => {
                    uses.pending |= self.missing_type(literal.type_symbol, &literal.type_name);
                    if literal.application_type.is_valid() {
                        self.type_uses(
                            self.trees.child_type_reference(literal.application_type),
                            uses,
                            &mut pending,
                        );
                    }
                }
                ExpressionNode::Cast(cast) => {
                    if cast.target_type.is_valid() {
                        self.type_uses(
                            self.trees.child_type_reference(cast.target_type),
                            uses,
                            &mut pending,
                        );
                    }
                    for ty in self
                        .trees
                        .child_type_references(cast.semantic_domain_arguments)
                    {
                        self.type_uses(ty, uses, &mut pending);
                    }
                }
                ExpressionNode::TypeExpression(ty) | ExpressionNode::ZeroValue(ty)
                    if ty.is_valid() =>
                {
                    self.type_uses(self.trees.child_type_reference(*ty), uses, &mut pending);
                }
                ExpressionNode::Call(call) => {
                    self.observe(call.target_symbol, uses);
                    if call.callee.is_valid() && !call.target_symbol.is_valid() {
                        pending.push(call.callee);
                    }
                    if !call.target_symbol.is_valid() {
                        let receiver = call
                            .receiver
                            .is_valid()
                            .then(|| table.name_path_node(call.receiver))
                            .flatten();
                        if receiver.is_none_or(|path| {
                            !path.head_symbol.is_valid() && !path.symbol.is_valid()
                        }) {
                            let names = receiver
                                .map_or(&[][..], |path| table.name_path_members(path.members));
                            uses.pending |= self.missing(
                                table.unresolved_name(handle),
                                names,
                                Some(&call.target),
                            );
                        }
                    }
                }
                _ => {}
            }
            table.visit_children(node, |child| pending.push(child));
        }
    }

    fn called_body_demands(&self, uses: &Uses, visited: &mut Vec<SymbolHandle>) -> bool {
        let mut calls = uses.calls.clone();
        while let Some(symbol) = calls.pop() {
            if visited.contains(&symbol) {
                continue;
            }
            visited.push(symbol);
            let Some(machine) = self.machine(symbol) else {
                continue;
            };
            if self.machine_uses(machine, |uses| {
                calls.extend(uses.calls);
                uses.pending
            }) {
                return true;
            }
        }
        false
    }
    fn machine_uses(&self, machine: &Machine, mut visit: impl FnMut(Uses) -> bool) -> bool {
        for state in self.trees.machine_state_handles(machine.states) {
            let state = self.trees.machine_state(*state);
            if visit(self.header(machine, state))
                || self
                    .trees
                    .state_body(state)
                    .iter()
                    .any(|row| visit(self.statement(state, row)))
            {
                return true;
            }
        }
        false
    }

    /// Whole-package checking also roots helpers independently of their callers.
    /// Defer exactly those suffix calls whose own checking needs the pending import.
    fn dependent_subjects(&self, mut calls: Vec<SymbolHandle>) -> Vec<SourceSpan> {
        let mut seen = Vec::new();
        let mut callers = Vec::new();
        let mut dependent = Vec::new();
        while let Some(symbol) = calls.pop() {
            if seen.contains(&symbol) {
                continue;
            }
            seen.push(symbol);
            let Some(machine) = self.machine(symbol) else {
                continue;
            };
            let mut needs_import = false;
            self.machine_uses(machine, |uses| {
                needs_import |= uses.pending;
                for callee in uses.calls {
                    callers.push((callee, symbol));
                    calls.push(callee);
                }
                false
            });
            if needs_import {
                dependent.push(symbol);
            }
        }
        callers.sort_unstable_by_key(|(callee, _)| (callee.arena_index(), callee.generation()));
        let mut next = 0;
        while next < dependent.len() {
            let symbol = dependent[next];
            next += 1;
            let key = (symbol.arena_index(), symbol.generation());
            let index = callers
                .partition_point(|(callee, _)| (callee.arena_index(), callee.generation()) < key);
            for (_, caller) in callers[index..]
                .iter()
                .take_while(|(callee, _)| *callee == symbol)
            {
                if !dependent.contains(caller) {
                    dependent.push(*caller);
                }
            }
        }
        let mut spans = dependent
            .into_iter()
            .filter_map(|symbol| self.trees.symbols.symbol_source_span(symbol))
            .collect::<Vec<_>>();
        spans.sort_unstable_by_key(|span| (span.source_id.0, span.span.start, span.span.end));
        spans.dedup();
        spans
    }
}

/// A qualified name can be retained as one joined token or several path members.
/// Compare complete components so that importing "kit" cannot defer "kitchen".
fn import_matches<'a>(import: &str, first: &'a str, rest: impl Iterator<Item = &'a str>) -> bool {
    let mut occurrence = std::iter::once(first)
        .chain(rest)
        .flat_map(|member| member.split("::"))
        .peekable();
    // A file can use the selected module by its leaf, as in
    // use support\\configure followed by configure\\run().
    import
        .rsplit_once("::")
        .is_some_and(|(_, leaf)| occurrence.peek() == Some(&leaf))
        || import
            .split("::")
            .all(|wanted| occurrence.next() == Some(wanted))
}

/// One frame contributes a source projection only when its original suffix depends
/// on an unavailable import. A helper whose suffix is already resolved keeps it,
/// including its genuine return value and any effects before that return.
pub(super) struct SelectedFrame {
    pub(super) authored: usize,
    pub(super) owner: SymbolHandle,
    pub(super) state: arena::Handle<symbol_resolved_trees::source_resolved::state::State>,
    pub(super) declarations: Vec<Span>,
    pub(super) deferred_subjects: Vec<SourceSpan>,
    pub(super) needs_projection: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Frame {
    owner: SymbolHandle,
    state: arena::Handle<symbol_resolved_trees::source_resolved::state::State>,
    position: usize,
    authored: usize,
}

struct Search<'a> {
    lookup: Lookup<'a>,
    plan: &'a EntryPlan,
    incoming: Vec<usize>,
    past_incoming: bool,
    stack: Vec<Frame>,
    selected: Vec<Frame>,
    paths: Vec<Vec<Frame>>,
    uncovered: bool,
}

impl Search<'_> {
    fn preparation(&mut self) {
        if self.past_incoming {
            self.selected.clone_from(&self.stack);
        } else if self
            .stack
            .iter()
            .map(|frame| frame.authored)
            .eq(self.incoming.iter().copied())
        {
            self.past_incoming = true;
        }
    }

    fn stop_at_demand(&mut self) {
        if self.selected.is_empty() {
            self.uncovered = true;
        } else if !self.paths.contains(&self.selected) {
            self.paths.push(self.selected.clone());
        }
    }

    // A distinct terminal arm cannot have executed the incoming preparation.
    // It may still need its own checked prefix in the same published dispatch.
    fn sibling_of_incoming(&self, authored: usize) -> bool {
        let depth = self.stack.len();
        let Some(&incoming) = self.incoming.get(depth) else {
            return false;
        };
        let current = &self.plan.calls[authored];
        let committed = &self.plan.calls[incoming];
        incoming != authored
            && current.terminal_arm
            && committed.terminal_arm
            && current.state == committed.state
            && current.through == committed.through
            && self
                .stack
                .iter()
                .map(|frame| frame.authored)
                .eq(self.incoming[..depth].iter().copied())
    }

    /// True only when every possible normal return is stopped before a missing use.
    /// The caller still checks its suffix if any arm can return ordinarily.
    fn body(
        &mut self,
        owner: SymbolHandle,
        state: arena::Handle<symbol_resolved_trees::source_resolved::state::State>,
    ) -> bool {
        let trees = self.lookup.trees;
        let Some(source) = trees.symbols.symbol_source_span(owner) else {
            return false;
        };
        let mut visited = vec![owner];
        if let Some(machine) = self.lookup.machine(owner) {
            let header = self.lookup.header(machine, trees.machine_state(state));
            if header.pending || self.lookup.called_body_demands(&header, &mut visited) {
                self.stop_at_demand();
                return true;
            }
        }
        let body = trees.machine_state(state);
        let mut straight_line = true;
        let mut all_arms_stopped = None;
        for (position, statement) in trees.state_body(body).iter().enumerate() {
            let mut uses = self.lookup.statement(body, statement);
            if uses.pending {
                self.stop_at_demand();
                return true;
            }
            let direct = uses.call_site.and_then(|(site, target)| {
                self.plan.call_at(source, site).map(|index| (index, target))
            });
            let stopped = if let Some((authored, target)) = direct
                && (straight_line || self.plan.calls[authored].terminal_arm)
            {
                let target_machine = target.is_valid().then(|| {
                    let row = trees.symbols.get(target);
                    if row.kind == SymbolKind::State {
                        row.parent
                    } else {
                        target
                    }
                });
                if let Some(target) = target_machine {
                    uses.calls.retain(|symbol| *symbol != target);
                }
                // An argument may itself require the import before invocation.
                if self.lookup.called_body_demands(&uses, &mut visited) {
                    self.stop_at_demand();
                    return true;
                }
                let frame = Frame {
                    owner,
                    state,
                    position,
                    authored,
                };
                // A wait reached inside this terminal arm cannot establish scope
                // for a later sibling arm. Only its own descendant may select it.
                let prior = self.plan.calls[authored]
                    .terminal_arm
                    .then(|| (self.selected.clone(), self.past_incoming));
                if !self.past_incoming && self.sibling_of_incoming(authored) {
                    self.selected.clear();
                    self.past_incoming = true;
                }
                self.stack.push(frame);
                let stopped = if self.plan.calls[authored].prepares {
                    self.preparation();
                    false
                } else if let Some(callee) = target_machine
                    && !self.stack.iter().any(|frame| frame.owner == callee)
                    && let Some(machine) = self.lookup.machine(callee)
                    && let Some(&state) =
                        trees
                            .machine_state_handles(machine.states)
                            .iter()
                            .find(|state| {
                                let body = trees.machine_state(**state);
                                target == callee || target == body.symbol
                            })
                {
                    self.body(callee, state)
                } else {
                    false
                };
                self.stack.pop();
                if let Some((selected, past_incoming)) = prior {
                    self.selected = selected;
                    self.past_incoming = past_incoming;
                }
                stopped
            } else if self.lookup.called_body_demands(&uses, &mut visited) {
                self.stop_at_demand();
                true
            } else {
                false
            };
            if matches!(statement, StatementNode::Transition(_)) {
                *all_arms_stopped.get_or_insert(true) &= stopped;
            } else if stopped {
                return true;
            }
            if matches!(
                statement,
                StatementNode::Transition(_) | StatementNode::Assembly(_)
            ) {
                straight_line = false;
            }
        }
        all_arms_stopped.unwrap_or(false)
    }

    fn projection(&self, frame: Frame) -> SelectedFrame {
        let trees = self.lookup.trees;
        let mut declarations = Vec::new();
        let mut needs_projection = false;
        let mut calls = Vec::new();
        let call = &self.plan.calls[frame.authored];
        // A terminal transition returns its callee's result. Its other arms
        // are alternatives, never a suffix of the selected arm.
        if call.terminal_arm {
            return SelectedFrame {
                authored: frame.authored,
                owner: frame.owner,
                state: frame.state,
                deferred_subjects: Vec::new(),
                declarations,
                needs_projection,
            };
        }
        let body = trees.machine_state(frame.state);
        for statement in &trees.state_body(body)[frame.position + 1..] {
            let uses = self.lookup.statement(body, statement);
            needs_projection |= uses.pending;
            calls.extend(uses.calls);
            declarations.extend(
                uses.locals
                    .into_iter()
                    .filter_map(|symbol| trees.symbols.symbol_source_span(symbol))
                    .filter(|local| {
                        local.source_id == call.owner.source_id && local.span.end <= call.call.start
                    })
                    .map(|local| local.span),
            );
        }
        let deferred_subjects = self.lookup.dependent_subjects(calls);
        needs_projection |= !deferred_subjects.is_empty();
        declarations.sort_unstable_by_key(|span| (span.start, span.end));
        declarations.dedup();
        SelectedFrame {
            authored: frame.authored,
            owner: frame.owner,
            state: frame.state,
            deferred_subjects,
            declarations,
            needs_projection,
        }
    }
}

pub(super) struct Selection {
    pub(super) paths: Vec<Vec<SelectedFrame>>,
    pub(super) can_return: bool,
}

pub(super) fn select(
    trees: &SymbolResolvedTrees,
    plan: &EntryPlan,
    imports: &[PendingBuildImport],
) -> Result<Selection, Vec<Diagnostic>> {
    let machine = trees
        .machines
        .iter()
        .find(|machine| {
            machine.name.source_span() == plan.entry && !machine.lexical_parent.is_valid()
        })
        .ok_or_else(|| {
            vec![
                Diagnostic::error("Build dependency lookup lost the selected declaration")
                    .with_source_span(plan.entry),
            ]
        })?;
    let &state = trees
        .machine_state_handles(machine.states)
        .first()
        .ok_or_else(|| {
            vec![
                Diagnostic::error("Build dependency lookup lost its entry state")
                    .with_source_span(plan.entry),
            ]
        })?;
    let mut candidates = crate::source_symbols::SourceSpanCandidates::default();
    let mut incoming = Vec::with_capacity(plan.stage.incoming.len());
    for site in &plan.stage.incoming {
        let owner = crate::source_symbols::rejoin_declaration(
            &site.owner,
            &trees.symbols,
            candidates.at_span(&trees.symbols, site.owner.span()),
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let index = trees
            .symbols
            .symbol_source_span(owner)
            .and_then(|owner| plan.call_at(owner, site.call))
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "the resumed Build lost a selected suspension call",
                )]
            })?;
        incoming.push(index);
    }
    let mut search = Search {
        lookup: Lookup::new(trees, imports),
        plan,
        past_incoming: incoming.is_empty(),
        incoming,
        stack: Vec::new(),
        selected: Vec::new(),
        paths: Vec::new(),
        uncovered: false,
    };
    let all_paths_stop = search.body(machine.symbol, state);
    let paths = if search.uncovered {
        Vec::new()
    } else {
        search
            .paths
            .iter()
            .map(|path| path.iter().map(|frame| search.projection(*frame)).collect())
            .collect()
    };
    Ok(Selection {
        can_return: !paths.is_empty() && !all_paths_stop,
        paths,
    })
}
