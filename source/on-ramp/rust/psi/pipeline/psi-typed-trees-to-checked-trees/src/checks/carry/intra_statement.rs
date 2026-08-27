use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

pub(super) fn place_is_used_after_call(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    statement_index: usize,
    target: &crate::CallSite<'_>,
    symbol: SymbolHandle,
    fallback_name: &str,
) -> bool {
    let Some(statement) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
    else {
        return false;
    };
    let mut traversal = EvaluationTraversal {
        program,
        state_symbol: state.symbol,
        statement_index,
        target,
        target_reached: false,
        place_used_after_target: false,
        symbol,
        fallback_name,
    };
    traversal.visit_statement(statement);
    traversal.place_used_after_target
}

struct EvaluationTraversal<'program, 'target> {
    program: &'program psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    target: &'target crate::CallSite<'program>,
    target_reached: bool,
    place_used_after_target: bool,
    symbol: SymbolHandle,
    fallback_name: &'target str,
}

impl EvaluationTraversal<'_, '_> {
    fn visit_statement(&mut self, statement: &StatementNode) {
        match statement {
            StatementNode::AssemblyFact(fact) => self.visit_expression(fact.expression),
            StatementNode::Assignment(assignment) => {
                // Calls in assignment targets are not semantic call sites today;
                // evaluate the checked value graph in its specified order.
                self.visit_expression(assignment.value);
            }
            StatementNode::Call(call) => {
                for argument in self
                    .program
                    .statement_table
                    .expression_handles(call.arguments)
                {
                    self.visit_expression(*argument);
                }
                if matches!(self.target, crate::CallSite::Statement(target) if std::ptr::eq(*target, call))
                {
                    self.target_reached = true;
                }
            }
            StatementNode::Expression(expression) => self.visit_expression(*expression),
            StatementNode::LocalData(local) => self.visit_expression(local.initial_value),
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(guard) = transition.guard {
                    self.visit_expression(guard);
                }
                self.visit_transition_target(transition.target);
                if transition.continuation.is_valid() {
                    self.visit_transition_target(transition.continuation);
                }
            }
        }
    }

    fn visit_transition_target(
        &mut self,
        target_handle: psi_typed_trees::statement::TransitionTargetHandle,
    ) {
        match self
            .program
            .statement_table
            .transition_target(target_handle)
        {
            TransitionTargetNode::Named { arguments, .. } => {
                for argument in self.program.statement_table.expression_handles(*arguments) {
                    self.visit_expression(*argument);
                }
                if matches!(self.target, crate::CallSite::TransitionNamed { arguments: target, .. } if *target == *arguments)
                {
                    self.target_reached = true;
                }
            }
            TransitionTargetNode::Value(expression) => self.visit_expression(*expression),
            TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
        }
    }

    fn visit_expression(&mut self, expression: ExpressionHandle) {
        if !expression.is_valid() || self.place_used_after_target {
            return;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Atomic(atomic) => self.visit_expression(atomic.value),
            ExpressionNode::ArrayLiteral(values) => {
                for value in self.program.expression_table.expression_handles(*values) {
                    self.visit_expression(*value);
                }
            }
            ExpressionNode::Binary(binary) => {
                self.visit_expression(binary.left);
                self.visit_expression(binary.right);
            }
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    self.visit_expression(call.receiver);
                }
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                {
                    self.visit_expression(*argument);
                }
                if matches!(self.target, crate::CallSite::Expression { call: target, .. } if std::ptr::eq(*target, call))
                {
                    self.target_reached = true;
                }
            }
            ExpressionNode::Cast(cast) => self.visit_expression(cast.value),
            ExpressionNode::Indexed(indexed) => {
                self.visit_expression(indexed.collection);
                self.visit_expression(indexed.index);
                self.observe_place(expression);
            }
            ExpressionNode::Member(member) => {
                self.visit_expression(member.receiver);
                self.observe_place(expression);
            }
            ExpressionNode::Borrow(inner) => self.visit_expression(inner.target),
            ExpressionNode::Name(_) => self.observe_place(expression),
            ExpressionNode::Range(range) => {
                self.visit_expression(range.start);
                self.visit_expression(range.end);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in self.program.expression_table.struct_fields(literal.fields) {
                    self.visit_expression(field.value);
                }
            }
            ExpressionNode::Unary(unary) => self.visit_expression(unary.operand),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }

    fn observe_place(&mut self, expression: ExpressionHandle) {
        if !self.target_reached {
            return;
        }
        let canonical_matches = crate::flow::canonical_place_from_expression_in_state(
            self.program,
            self.state_symbol,
            self.statement_index,
            expression,
        )
        .is_some_and(|place| {
            matches!(place.root, psi_facts::PlaceRoot::Symbol(root) if root == self.symbol)
                || place.segments.iter().any(|segment| {
                    matches!(segment, psi_facts::PlaceSegment::Field { symbol } if *symbol == self.symbol)
                })
        });
        let name_matches = matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Name(path)
                if self.program
                    .expression_table
                    .name_path_members(path.members)
                    .first()
                    .is_some_and(|member| member.as_str() == self.fallback_name)
        );
        self.place_used_after_target = canonical_matches || name_matches;
    }
}
