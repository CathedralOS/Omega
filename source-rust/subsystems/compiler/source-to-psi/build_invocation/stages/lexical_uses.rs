//! Check which local and nested machine names cross a preparation boundary.

use diagnostics::Diagnostic;
use std::collections::HashMap;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchFieldAction, MatchPattern, TableDestructurePattern,
    TableMatchField,
};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::Machine;
use syntax_trees::statement::{
    StatementHandle, StatementNode, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};

/// Keep each local's definition and all its uses in one checked machine. A
/// preparation inside that lifetime parks the interpreter with its frame intact.
/// Once the preparation has returned, the next program can start at the first
/// statement boundary where no such local is still needed.
pub(super) fn program_ends(
    syntax: &SyntaxTrees,
    machine: &Machine,
    statements: &[StatementHandle],
    preparations: &[usize],
) -> Result<Vec<usize>, Vec<Diagnostic>> {
    let nested = machine
        .nested_machines
        .handles()
        .filter_map(|handle| syntax.items.item(handle).as_machine())
        .map(|machine| machine.name.as_str(syntax))
        .collect::<Vec<_>>();
    let mut locals = HashMap::new();
    let mut last_use = (0..statements.len()).collect::<Vec<_>>();
    let mut calls_to_nested = Vec::new();
    let mut refusals = Vec::new();
    let mut phase = 0;
    for (position, statement) in statements.iter().enumerate() {
        let node = syntax.statements.statement(*statement);
        let mut uses = Uses {
            syntax,
            names: Vec::new(),
            machines: Vec::new(),
            bindings: Vec::new(),
        };
        uses.statement(node);
        for name in uses.names {
            if let Some(&(definition, _)) = locals.get(name.as_str(syntax)) {
                last_use[definition] = position;
            }
        }
        calls_to_nested.extend(
            uses.machines
                .into_iter()
                .filter(|name| nested.contains(&name.as_str(syntax)))
                .map(|name| (position, name)),
        );
        let first_local = uses.bindings.len();
        local_bindings(syntax, node, &mut uses.bindings);
        for binding in &uses.bindings {
            let name = binding.as_str(syntax);
            if locals
                .get(name)
                .is_some_and(|&(_, declared)| declared < phase)
            {
                refusals.push(
                    Diagnostic::error(format!(
                        "binding `{name}` conflicts with a local still visible across Build::prepare_dependency"
                    ))
                    .with_source_span(binding.source_span()),
                );
            }
        }
        for binding in &uses.bindings[first_local..] {
            locals.insert(binding.as_str(syntax), (position, phase));
        }
        if preparations.get(phase) == Some(&position) {
            phase += 1;
        }
    }
    let mut live_through = 0;
    let mut pending_preparation = false;
    let mut ends = Vec::new();
    let mut waits = preparations.iter().copied().peekable();
    for (position, last) in last_use.into_iter().enumerate() {
        // Ending a program after an authored control transfer would turn a
        // normal return from another path into permission to run its suffix.
        // The ordinary checker and interpreter own the rest of that control flow.
        if matches!(
            syntax.statements.statement(statements[position]),
            StatementNode::Transition(_)
                | StatementNode::Dispatch(_)
                | StatementNode::Assembly(_)
                | StatementNode::AuthoredAssembly(_)
        ) {
            break;
        }
        live_through = live_through.max(last);
        let preparation = waits.next_if_eq(&position).is_some();
        pending_preparation |= preparation;
        if pending_preparation
            && live_through == position
            && (preparation || position + 1 < statements.len())
        {
            ends.push(position);
            pending_preparation = false;
        }
    }
    if let Some(&last_end) = ends.last() {
        for (position, name) in calls_to_nested {
            if position <= last_end {
                let text = name.as_str(syntax);
                refusals.push(Diagnostic::error(format!(
                    "not supported yet: a Build statement before `Build::prepare_dependency` that names `{text}`, a nested machine checked with the final Build stage"
                )).with_source_span(name.source_span()));
            }
        }
    }
    if refusals.is_empty() {
        Ok(ends)
    } else {
        Err(refusals)
    }
}

/// The locals `statement` binds for the statements after it.
fn local_bindings<'syntax>(
    syntax: &'syntax SyntaxTrees,
    statement: &'syntax StatementNode,
    names: &mut Vec<&'syntax Identifier>,
) {
    match statement {
        StatementNode::LocalData(local) => {
            names.push(&local.name);
            for field in syntax.expressions.match_fields(local.pattern_fields) {
                if let MatchFieldAction::Bind(binding) = &field.action {
                    names.push(binding);
                }
            }
        }
        StatementNode::ProofOutputBindingStatement(binding) => {
            for selector in binding.bindings.iter() {
                names.push(&selector.binding);
            }
        }
        _ => {}
    }
}

/// The value names and callee names one statement reads. A name is recorded
/// by its first path member: `registered`, or `request` of `request.alias`.
struct Uses<'syntax> {
    syntax: &'syntax SyntaxTrees,
    names: Vec<&'syntax Identifier>,
    machines: Vec<&'syntax Identifier>,
    bindings: Vec<&'syntax Identifier>,
}

impl<'syntax> Uses<'syntax> {
    fn statement(&mut self, statement: &'syntax StatementNode) {
        let syntax = self.syntax;
        match statement {
            // Assembly belongs to target code, which a Build entry is not;
            // its operands are still expressions of this statement.
            StatementNode::AuthoredAssembly(handle) => {
                syntax
                    .statements
                    .authored_assembly(*handle)
                    .visit_operands(syntax, |operand| self.expression(operand));
            }
            StatementNode::Assembly(handle) => {
                let graph = syntax.statements.assembly(*handle);
                for block in syntax.statements.assembly_blocks(graph.blocks) {
                    self.all(syntax.statements.assembly_expressions(block.requires));
                    self.all(syntax.statements.assembly_expressions(block.ensures));
                    for instruction in syntax.statements.assembly_instructions(block.instructions) {
                        self.statement(syntax.statements.statement(*instruction));
                    }
                }
                self.all(syntax.statements.assembly_expressions(graph.primitives));
                self.all(&graph.scratch);
            }
            StatementNode::RootBinding(binding) => {
                self.expression(binding.receiver);
                self.expression(binding.implementation_operand);
            }
            StatementNode::AssemblyFact(fact) => self.expression(fact.expression),
            StatementNode::Assignment(assignment) => {
                self.expression(assignment.target);
                self.expression(assignment.value);
            }
            StatementNode::Call(call) => {
                self.expression(call.callee);
                if let Some(receiver) = syntax
                    .statements
                    .identifier_path_members(call.receiver)
                    .first()
                {
                    self.names.push(receiver);
                } else {
                    self.machines.push(&call.target);
                }
                self.names.extend(call.evidence_arguments.iter());
                self.all(syntax.statements.expression_handles(call.arguments));
            }
            StatementNode::ProofOutputBindingStatement(binding) => self.expression(binding.call),
            StatementNode::Expression(value) => self.expression(*value),
            StatementNode::LocalData(local) => {
                self.expression(local.initial_value);
                self.fields(syntax.expressions.match_fields(local.pattern_fields));
            }
            StatementNode::Dispatch(handle) => {
                let dispatch = syntax.statements.dispatch(*handle);
                self.all(syntax.statements.expression_handles(dispatch.subjects));
                for arm in &dispatch.arms {
                    for slot in &arm.patterns {
                        match slot.pattern {
                            MatchPattern::Wildcard => {}
                            MatchPattern::Value(value) => self.expression(value),
                            MatchPattern::Destructure(pattern) => self.pattern(&pattern),
                        }
                    }
                    if let TransitionGuardNode::When(guard) = arm.guard {
                        self.expression(guard);
                    }
                    self.target(arm.target);
                }
            }
            StatementNode::Transition(transition) => {
                for pattern in syntax.statements.transition_patterns(transition.patterns) {
                    self.expression(pattern.subject);
                    self.capture(pattern.capture);
                    self.pattern(&pattern.pattern);
                }
                self.proof_bindings(transition.proof_selectors);
                self.target(transition.target);
                self.target(transition.continuation);
                if let TransitionGuardNode::When(guard) = transition.guard {
                    self.expression(guard);
                }
            }
        }
    }

    fn target(&mut self, target: TransitionTargetHandle) {
        if !target.is_valid() {
            return;
        }
        let syntax = self.syntax;
        match syntax.statements.transition_target(target) {
            TransitionTargetNode::Named {
                path, arguments, ..
            } => {
                if let [machine] = syntax.statements.identifier_path_members(*path) {
                    self.machines.push(machine);
                }
                self.all(syntax.statements.expression_handles(*arguments));
            }
            TransitionTargetNode::Value(value) => self.expression(*value),
            TransitionTargetNode::Terminal => {}
        }
    }

    fn all(&mut self, expressions: &[ExpressionHandle]) {
        for expression in expressions {
            self.expression(*expression);
        }
    }

    fn capture(&mut self, capture: arena::Handle<syntax_trees::expression::MatchSubjectCapture>) {
        if capture.is_valid() {
            self.expression(self.syntax.expressions.match_subject(capture).subject);
        }
    }

    fn fields(&mut self, fields: &[TableMatchField]) {
        for field in fields {
            if let MatchFieldAction::Equal(value) = &field.action {
                self.expression(*value);
            }
        }
    }

    fn pattern(&mut self, pattern: &TableDestructurePattern) {
        let syntax = self.syntax;
        if pattern.subject_binding.is_valid()
            && let ExpressionNode::Name { members, .. } =
                syntax.expressions.expression(pattern.subject_binding)
            && let [name] = syntax.expressions.identifier_path_members(*members)
        {
            self.bindings.push(name);
        }
        for field in syntax.expressions.match_fields(pattern.fields) {
            if let MatchFieldAction::Bind(binding) = &field.action {
                self.bindings.push(binding);
            }
        }
        self.proof_bindings(pattern.proof_selectors);
        self.expression(pattern.head);
        self.fields(self.syntax.expressions.match_fields(pattern.fields));
        self.expression(pattern.guard);
    }

    fn proof_bindings(
        &mut self,
        span: arena::HandleSpan<syntax_trees::statement::TableOutcomeProofSelector>,
    ) {
        self.bindings.extend(
            self.syntax
                .statements
                .outcome_proof_selectors(span)
                .iter()
                .map(|selector| &selector.binding),
        );
    }

    fn expression(&mut self, handle: ExpressionHandle) {
        if !handle.is_valid() {
            return;
        }
        let syntax = self.syntax;
        match syntax.expressions.expression(handle) {
            ExpressionNode::Match(dispatch) => {
                self.expression(dispatch.subject);
                self.capture(dispatch.capture);
                for arm in syntax.expressions.match_arms(dispatch.arms) {
                    match &arm.pattern {
                        MatchPattern::Value(value) => self.expression(*value),
                        MatchPattern::Wildcard => {}
                        MatchPattern::Destructure(pattern) => self.pattern(pattern),
                    }
                    self.expression(arm.value);
                }
            }
            ExpressionNode::SavedSubject(saved) => {
                self.capture(saved.capture);
                self.expression(saved.pattern_head);
            }
            ExpressionNode::ArrayLiteral(values) => {
                self.all(syntax.expressions.expression_handles(*values));
            }
            ExpressionNode::Atomic(atomic) => {
                self.expression(atomic.value);
                self.expression(atomic.result);
            }
            ExpressionNode::Binary(binary) => {
                self.expression(binary.left);
                self.expression(binary.right);
            }
            ExpressionNode::Cast(cast) => self.expression(cast.value),
            ExpressionNode::Call(call) => {
                self.expression(call.callee);
                self.names.extend(call.evidence_arguments.iter());
                if call.receiver.is_valid() {
                    self.expression(call.receiver);
                } else {
                    self.machines.push(&call.target);
                }
                self.all(syntax.expressions.expression_handles(call.arguments));
            }
            ExpressionNode::Indexed(indexed) => {
                self.expression(indexed.collection);
                self.expression(indexed.index);
            }
            ExpressionNode::Membership(membership) => self.expression(membership.value),
            ExpressionNode::Member(member) => self.expression(member.receiver),
            ExpressionNode::Borrow(borrow) => self.expression(borrow.target),
            ExpressionNode::Name { members, .. } => {
                if let Some(first) = syntax.expressions.identifier_path_members(*members).first() {
                    self.names.push(first);
                }
            }
            ExpressionNode::Range(range) => {
                self.expression(range.start);
                self.expression(range.end);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in syntax.expressions.struct_fields(literal.fields) {
                    self.expression(field.value);
                }
            }
            ExpressionNode::Unary(unary) => self.expression(unary.operand),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
}
