//! A local value binding must have a fresh name among the value bindings
//! visible at its declaration. Each machine starts with its own parameters
//! and visible static values. A state gains locals in statement order.
//! Match and transition arms extend that scope with their subject, field,
//! and proof bindings. Ending an arm removes those names before its sibling.
//! A block value adds its own locals in statement order and removes them at
//! its end, so disjoint blocks may reuse a name (state_contracts.md,
//! "Disjoint arms or blocks may reuse names").
//! Arm uses have already become references to the saved dispatch subject;
//! the patterns retain the authored names and their diagnostic spans.

use super::scopes::DeclarationTypeScopes;
use crate::source_resolved::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, MatchFieldAction, MatchPattern,
    TableDestructurePattern,
};
use crate::source_resolved::name::DiagnosticName;
use crate::source_resolved::statement::{Statement, TransitionGuard, TransitionTarget};
use crate::source_resolved::{GenericBindingOverlap, SymbolResolvedTrees};
use diagnostics::Diagnostic;

pub(super) fn check_bindings(
    program: &SymbolResolvedTrees,
) -> Result<Vec<GenericBindingOverlap>, Vec<Diagnostic>> {
    let symbols = &program.symbols;
    let declarations = &program.tables.declarations;
    let expressions = &program.tables.bodies.expressions;
    let mut diagnostics = Vec::new();
    let mut overlaps = Vec::new();
    let inherited = DeclarationTypeScopes::of(program)?;
    for definition in program.machines.iter() {
        if !definition.symbol.is_valid() {
            continue;
        }
        let machine = symbols.name(definition.symbol);
        let mut visible = super::scopes::declaration_type_scope(
            program.machine_type_parameters(definition),
            inherited.parameters(definition.symbol),
        );
        if definition.attached_data_symbol.is_valid()
            && let Some(data) = program
                .data_definitions
                .iter()
                .find(|data| data.symbol == definition.attached_data_symbol)
        {
            visible = super::scopes::declaration_type_scope(
                &visible,
                program.data_type_parameters(data.type_parameters),
            );
        }
        let mut record_overlap = |name: &DiagnosticName| {
            if let Some(parameter) = visible
                .iter()
                .find(|binder| binder.name.as_str() == name.as_str())
            {
                overlaps.push(GenericBindingOverlap {
                    machine: definition.symbol,
                    parameter: parameter.symbol,
                    binding: name.source_span(),
                });
            }
        };
        for &state_handle in program.machine_state_handles(definition.states) {
            let state = program.machine_state(state_handle);
            if !state.symbol.is_valid() {
                continue;
            }
            let state_name = symbols.name(state.symbol);
            let mut declared = Vec::new();
            for parameter in declarations
                .state_parameters
                .span_or_empty(state.parameters)
            {
                if !parameter.symbol.is_valid() {
                    continue;
                }
                let name = symbols.name(parameter.symbol);
                record_overlap(&parameter.name);
                // A use of the name could only ever reach the first of two
                // parameters that share it, so the second is refused where
                // it is declared, like a local that repeats a visible name.
                if declared.iter().any(|earlier| earlier == name) {
                    diagnostics.push(
                        Diagnostic::error(format!(
                            "machine `{machine}` state `{state_name}` has duplicate parameter \
                             `{name}`: an earlier parameter with this name is still visible"
                        ))
                        .with_source_span(parameter.name.source_span()),
                    );
                }
                declared.push(name.to_owned());
            }
            let parameter_count = declared.len();
            let mut arms = ArmBindings {
                expressions,
                statements: &declarations.state_statements,
                symbols,
                record_overlap: &mut record_overlap,
                declared,
                parameter_count,
                machine,
                state_name,
                diagnostics: &mut diagnostics,
            };
            arms.sequence(
                declarations
                    .state_statements
                    .span_or_empty(state.construction_statement_span()),
            );
        }
    }
    if diagnostics.is_empty() {
        Ok(overlaps)
    } else {
        Err(diagnostics)
    }
}

/// One state's walk over its arm bindings. `declared` holds the names in
/// scope: parameters, earlier locals, then the bindings of each enclosing
/// arm, pushed on entry to the arm and popped on exit.
struct ArmBindings<'walk> {
    expressions: &'walk ExpressionTable,
    statements: &'walk arena::Arena<Statement>,
    symbols: &'walk crate::source_symbols::SymbolTable,
    record_overlap: &'walk mut dyn FnMut(&DiagnosticName),
    declared: Vec<String>,
    parameter_count: usize,
    machine: &'walk str,
    state_name: &'walk str,
    diagnostics: &'walk mut Vec<Diagnostic>,
}

impl ArmBindings<'_> {
    /// A body's or block's statements in order: each local enters the scope
    /// after its own initializer and must not reuse a visible name.
    fn sequence(&mut self, statements: &[Statement]) {
        let (machine, state_name) = (self.machine, self.state_name);
        for statement in statements {
            self.statement(statement);
            let Statement::LocalData(local) = statement else {
                continue;
            };
            if !local.role.is_runtime_binding() || !local.symbol.is_valid() {
                continue;
            }
            let name = self.symbols.name(local.symbol);
            (self.record_overlap)(&local.name);
            let shadowed = self.declared.iter().position(|earlier| earlier == name);
            self.declared.push(name.to_owned());
            if let Some(position) = shadowed {
                let message = if position < self.parameter_count {
                    format!(
                        "machine `{machine}` state `{state_name}` local data `{name}` conflicts \
                         with an existing name: a parameter with this name is still visible"
                    )
                } else {
                    format!(
                        "machine `{machine}` state `{state_name}` has duplicate local data `{name}`: \
                         an earlier local with this name is still visible"
                    )
                };
                self.diagnostics
                    .push(Diagnostic::error(message).with_source_span(local.name.source_span()));
            }
        }
    }

    fn statement(&mut self, statement: &Statement) {
        match statement {
            Statement::Assembly(_) | Statement::AssemblyFact(_) => {}
            Statement::RootBinding(binding) => {
                self.expression(binding.receiver);
                self.expression(binding.implementation_operand);
            }
            Statement::Assignment(assignment) => {
                self.expression(assignment.target);
                self.expression(assignment.value);
            }
            Statement::Call(call) => {
                self.expression(call.callee);
                self.expressions_in(call.arguments);
            }
            Statement::ProofOutputBindingStatement(binding) => self.expression(binding.call),
            Statement::Expression(expression) | Statement::Evaluate { expression, .. } => {
                self.expression(*expression)
            }
            Statement::LocalData(local) => self.expression(local.initial_value),
            Statement::Transition(transition) => {
                let scope = self.declared.len();
                let mut names = Vec::new();
                for pattern in &transition.patterns {
                    self.expression(pattern.subject);
                    names.extend(self.field_bindings(&pattern.pattern));
                }
                names.extend(
                    transition
                        .proof_selectors
                        .iter()
                        .map(|selector| selector.binding.clone()),
                );
                self.bind(names);
                // The resolved guard already joins each pattern's
                // condition with the authored guard.
                if let TransitionGuard::When(guard) = &transition.guard {
                    self.expression(*guard);
                }
                for target in
                    std::iter::once(&transition.target).chain(transition.continuation.as_ref())
                {
                    match target {
                        TransitionTarget::Named(named) => self.expressions_in(named.arguments),
                        TransitionTarget::Value(value) => self.expression(*value),
                        TransitionTarget::Terminal => {}
                    }
                }
                self.declared.truncate(scope);
            }
        }
    }

    fn expressions_in(&mut self, span: arena::HandleSpan<ExpressionHandle>) {
        for expression in self.expressions.expression_handles(span) {
            self.expression(*expression);
        }
    }

    fn expression(&mut self, expression: ExpressionHandle) {
        if !expression.is_valid() {
            return;
        }
        let expressions = self.expressions;
        match expressions.expression(expression) {
            ExpressionNode::Block(block) => {
                let scope = self.declared.len();
                let statements = self.statements;
                self.sequence(statements.span_or_empty(block.statements));
                self.expression(block.tail);
                self.declared.truncate(scope);
            }
            ExpressionNode::Match(dispatch) => {
                self.expression(dispatch.subject);
                for arm in expressions.match_arms(dispatch.arms) {
                    let scope = self.declared.len();
                    match arm.pattern {
                        MatchPattern::Value(value) | MatchPattern::Guard(value) => {
                            self.expression(value)
                        }
                        MatchPattern::Destructure(pattern) => {
                            let mut names = self.field_bindings(&pattern);
                            names.extend(
                                expressions
                                    .match_proof_selectors(pattern.proof_selectors)
                                    .iter()
                                    .map(|selector| selector.binding.clone()),
                            );
                            self.bind(names);
                            self.expression(pattern.condition);
                        }
                        MatchPattern::Wildcard => {}
                    }
                    self.expression(arm.value);
                    self.declared.truncate(scope);
                }
            }
            ExpressionNode::SavedSubject(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::ZeroValue(_) => {}
            ExpressionNode::ArrayLiteral(values) => self.expressions_in(*values),
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
                self.expression(call.receiver);
                self.expressions_in(call.arguments);
            }
            ExpressionNode::Indexed(indexed) => {
                self.expression(indexed.collection);
                self.expression(indexed.index);
            }
            ExpressionNode::Membership(membership) => self.expression(membership.value),
            ExpressionNode::Member(member) => self.expression(member.receiver),
            ExpressionNode::Borrow(borrow) => self.expression(borrow.target),
            ExpressionNode::Range(range) => {
                self.expression(range.start);
                self.expression(range.end);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in expressions.struct_fields(literal.fields) {
                    self.expression(field.value);
                }
            }
            ExpressionNode::Unary(unary) => self.expression(unary.operand),
        }
    }

    fn field_bindings(&self, pattern: &TableDestructurePattern) -> Vec<DiagnosticName> {
        let whole = pattern
            .subject_binding
            .is_valid()
            .then(|| self.expressions.name_path_node(pattern.subject_binding))
            .flatten()
            .and_then(|path| self.expressions.name_path_members(path.members).first())
            .cloned();
        whole
            .into_iter()
            .chain(
                self.expressions
                    .match_fields(pattern.fields)
                    .iter()
                    .filter_map(|field| match &field.action {
                        MatchFieldAction::Bind(binding) => Some(binding.clone()),
                        MatchFieldAction::Equal(_) | MatchFieldAction::Waive => None,
                    }),
            )
            .collect()
    }

    /// Refuse each of one arm's bindings whose name is already in scope,
    /// then bring them into scope for the arm's body. Repeats inside one
    /// arm were refused by the parser.
    fn bind(&mut self, names: Vec<DiagnosticName>) {
        for binding in names {
            (self.record_overlap)(&binding);
            let name = binding.as_str();
            if self.declared.iter().any(|bound| bound == name) {
                let (machine, state_name) = (self.machine, self.state_name);
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "machine `{machine}` state `{state_name}` pattern binding `{name}` \
                             conflicts with an existing name in the visible value scope"
                    ))
                    .with_source_span(binding.source_span()),
                );
            }
            self.declared.push(name.to_owned());
        }
    }
}
