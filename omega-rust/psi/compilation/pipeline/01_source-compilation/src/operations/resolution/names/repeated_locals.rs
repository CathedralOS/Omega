/*
    Refuse a `let` or a pattern binding that reuses a name its body already
    declares.

    A name is declared at most once among a machine's parameters and the
    locals of its own body, and pattern bindings within one arm follow the
    same rule (`wiki/spec/language/state_contracts.md`, "Bindings").
    This is where each binder gets its identity, so the rule is applied
    here, once, for both checkers: a body that breaks it never reaches
    typing. Parameters and `let` locals are compared by the name their
    declaration gave their symbol.

    Match and transition arm bindings get no symbol: the parser rewrites
    each use into a projection of the subject and checks only repeats
    inside one arm. Their names survive in the pattern fields and outcome
    proof selectors, so this pass walks each state's statements and
    expressions and compares every arm binding with the parameters, the
    locals declared before that statement, and the bindings of the arms
    enclosing it. Different arms of one match may reuse a name.
*/

use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, MatchFieldAction, MatchPattern,
    TableDestructurePattern,
};
use crate::source_resolved::statement::{Statement, TransitionGuard, TransitionTarget};
use diagnostics::Diagnostic;
use source_text::SourceSpan;
use symbols::SymbolHandle;

pub(super) fn reject_repeated_local_names(
    program: &SymbolResolvedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let symbols = &program.symbols;
    let declarations = &program.tables.declarations;
    let expressions = &program.tables.bodies.expressions;
    let mut diagnostics = Vec::new();
    for (_, state) in declarations.machine_states.iter() {
        if !state.symbol.is_valid() {
            continue;
        }
        let machine = symbols.name(symbols.get(state.symbol).parent);
        let state_name = symbols.name(state.symbol);
        let parameters: Vec<SymbolHandle> = declarations
            .state_parameters
            .span_or_empty(state.parameters)
            .iter()
            .map(|parameter| parameter.symbol)
            .filter(|symbol| symbol.is_valid())
            .collect();
        let mut arms = ArmBindings {
            expressions,
            declared: parameters
                .iter()
                .map(|parameter| symbols.name(*parameter).to_owned())
                .collect(),
            machine,
            state_name,
            diagnostics: &mut diagnostics,
        };
        let mut locals: Vec<SymbolHandle> = Vec::new();
        for statement in declarations
            .state_statements
            .span_or_empty(state.statements)
        {
            arms.statement(statement);
            let Statement::LocalData(local) = statement else {
                continue;
            };
            if !local.role.is_runtime_binding() || !local.symbol.is_valid() {
                continue;
            }
            let name = symbols.name(local.symbol);
            let rule = if parameters
                .iter()
                .any(|parameter| symbols.name(*parameter) == name)
            {
                Some("conflicts with an existing name")
            } else if locals.iter().any(|earlier| symbols.name(*earlier) == name) {
                Some("duplicate")
            } else {
                None
            };
            locals.push(local.symbol);
            arms.declared.push(name.to_owned());
            let Some(rule) = rule else {
                continue;
            };
            let message = if rule == "duplicate" {
                format!(
                    "machine `{machine}` state `{state_name}` has duplicate local data `{name}`: \
                         a local name is declared at most once in a machine body"
                )
            } else {
                format!(
                    "machine `{machine}` state `{state_name}` local data `{name}` conflicts \
                         with an existing name: parameters and locals share one namespace"
                )
            };
            let diagnostic = Diagnostic::error(message);
            arms.diagnostics
                .push(match symbols.symbol_source_span(local.symbol) {
                    Some(span) => diagnostic.with_source_span(span),
                    None => diagnostic,
                });
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// One state's walk over its arm bindings. `declared` holds the names in
/// scope: parameters, earlier locals, then the bindings of each enclosing
/// arm, pushed on entry to the arm and popped on exit.
struct ArmBindings<'walk> {
    expressions: &'walk ExpressionTable,
    declared: Vec<String>,
    machine: &'walk str,
    state_name: &'walk str,
    diagnostics: &'walk mut Vec<Diagnostic>,
}

impl ArmBindings<'_> {
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
            Statement::Expression(expression) => self.expression(*expression),
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
                        .map(|selector| selector.binding.as_str().to_owned()),
                );
                self.bind(names, transition.source_span);
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
            ExpressionNode::Match(dispatch) => {
                self.expression(dispatch.subject);
                for arm in expressions.match_arms(dispatch.arms) {
                    let scope = self.declared.len();
                    match arm.pattern {
                        MatchPattern::Value(value) => self.expression(value),
                        MatchPattern::Destructure(pattern) => {
                            let mut names = self.field_bindings(&pattern);
                            names.extend(
                                expressions
                                    .match_proof_selectors(pattern.proof_selectors)
                                    .iter()
                                    .map(|selector| selector.binding.as_str().to_owned()),
                            );
                            self.bind(names, arm.source_span);
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

    fn field_bindings(&self, pattern: &TableDestructurePattern) -> Vec<String> {
        self.expressions
            .match_fields(pattern.fields)
            .iter()
            .filter_map(|field| match &field.action {
                MatchFieldAction::Bind(binding) => Some(binding.as_str().to_owned()),
                MatchFieldAction::Equal(_) | MatchFieldAction::Waive => None,
            })
            .collect()
    }

    /// Refuse each of one arm's bindings whose name is already in scope,
    /// then bring them into scope for the arm's body. Repeats inside one
    /// arm were refused by the parser.
    fn bind(&mut self, names: Vec<String>, arm: SourceSpan) {
        for name in names {
            if self.declared.contains(&name) {
                let (machine, state_name) = (self.machine, self.state_name);
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "machine `{machine}` state `{state_name}` pattern binding `{name}` \
                             conflicts with an existing name: parameters, locals and the \
                             bindings of one arm share one namespace"
                    ))
                    .with_source_span(arm),
                );
            }
            self.declared.push(name);
        }
    }
}
