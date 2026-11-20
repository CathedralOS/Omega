//! Check which local and nested machine names cross a preparation boundary.

use diagnostics::Diagnostic;
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

/// Refuses the two uses nothing carries across a preparation: a local of an
/// earlier stage used in a later one, and a nested machine of the entry
/// named before the last stage.
pub(super) fn refuse_uncarried_names(
    syntax: &SyntaxTrees,
    machine: &Machine,
    statements: &[StatementHandle],
    ends: &[usize],
) -> Result<(), Vec<Diagnostic>> {
    let nested = machine
        .nested_machines
        .handles()
        .filter_map(|handle| syntax.items.item(handle).as_machine())
        .map(|machine| machine.name.as_str(syntax))
        .collect::<Vec<_>>();
    let last_end = *ends.last().expect("a divided entry has a preparation");
    // Locals bound by stages already passed, and by the stage being walked.
    let mut earlier: Vec<&str> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut refusals = Vec::new();
    for (position, statement) in statements.iter().enumerate() {
        let node = syntax.statements.statement(*statement);
        let mut uses = Uses {
            syntax,
            names: Vec::new(),
            machines: Vec::new(),
        };
        uses.statement(node);
        for name in uses.names {
            let text = name.as_str(syntax);
            if !current.contains(&text) && earlier.contains(&text) {
                refusals.push(
                    Diagnostic::error(format!(
                        "not supported yet: a Build statement after `Build::prepare_dependency` that uses `{text}`, a local bound before that preparation; only the entry's parameters are carried across a preparation, so nothing is claimed about this program"
                    ))
                    .with_source_span(name.source_span()),
                );
            }
        }
        if position <= last_end {
            for name in uses.machines {
                let text = name.as_str(syntax);
                if nested.contains(&text) {
                    refusals.push(
                        Diagnostic::error(format!(
                            "not supported yet: a Build statement before `Build::prepare_dependency` that names `{text}`, a nested machine of the Build entry; the entry's nested machines are checked with the statements after its last preparation, so nothing is claimed about this program"
                        ))
                        .with_source_span(name.source_span()),
                    );
                }
            }
        }
        bound(syntax, node, &mut current);
        if ends.contains(&position) {
            earlier.append(&mut current);
        }
    }
    if refusals.is_empty() {
        Ok(())
    } else {
        Err(refusals)
    }
}

/// The locals `statement` binds for the statements after it.
fn bound<'syntax>(
    syntax: &'syntax SyntaxTrees,
    statement: &'syntax StatementNode,
    names: &mut Vec<&'syntax str>,
) {
    match statement {
        StatementNode::LocalData(local) => {
            names.push(local.name.as_str(syntax));
            for field in syntax.expressions.match_fields(local.pattern_fields) {
                if let MatchFieldAction::Bind(binding) = &field.action {
                    names.push(binding.as_str(syntax));
                }
            }
        }
        StatementNode::ProofOutputBindingStatement(binding) => {
            for selector in binding.bindings.iter() {
                names.push(selector.binding.as_str(syntax));
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
                if syntax
                    .statements
                    .identifier_path_members(call.receiver)
                    .is_empty()
                {
                    self.machines.push(&call.target);
                }
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
        self.expression(pattern.head);
        self.fields(self.syntax.expressions.match_fields(pattern.fields));
        self.expression(pattern.guard);
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
