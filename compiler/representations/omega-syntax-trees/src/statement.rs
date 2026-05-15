use crate::identifier::Identifier;
use omega_core::arena::{Arena, Handle, HandleSpan};

pub type StatementHandle = Handle<StatementNode>;
pub type TransitionTargetHandle = Handle<TransitionTargetNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementTable {
    statements: Arena<StatementNode>,
    expression_handles: Arena<crate::expression::ExpressionHandle>,
    identifier_path_members: Arena<Identifier>,
    transition_targets: Arena<TransitionTargetNode>,
}

impl StatementTable {
    pub fn new() -> Self {
        Self {
            statements: Arena::new(),
            expression_handles: Arena::new(),
            identifier_path_members: Arena::new(),
            transition_targets: Arena::new(),
        }
    }

    pub fn insert(&mut self, statement: StatementNode) -> StatementHandle {
        self.statements.append(statement)
    }

    pub fn append_expression_handle(
        &mut self,
        expression: crate::expression::ExpressionHandle,
    ) -> Handle<crate::expression::ExpressionHandle> {
        self.expression_handles.append(expression)
    }

    pub fn append_identifier_path_member(&mut self, member: Identifier) -> Handle<Identifier> {
        self.identifier_path_members.append(member)
    }

    pub fn insert_transition_target(
        &mut self,
        target: TransitionTargetNode,
    ) -> TransitionTargetHandle {
        self.transition_targets.insert(target)
    }

    pub fn statement(&self, handle: StatementHandle) -> &StatementNode {
        self.statements.get(handle)
    }

    pub fn expression_handles(
        &self,
        span: HandleSpan<crate::expression::ExpressionHandle>,
    ) -> &[crate::expression::ExpressionHandle] {
        self.expression_handles.span_or_empty(span)
    }

    pub fn identifier_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
        self.identifier_path_members.span_or_empty(span)
    }

    pub fn transition_target(&self, handle: TransitionTargetHandle) -> &TransitionTargetNode {
        self.transition_targets.get(handle)
    }

    pub fn statement_count(&self) -> usize {
        self.statements.len()
    }

    pub fn transition_target_count(&self) -> usize {
        self.transition_targets.len()
    }
}

impl Default for StatementTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementNode {
    Assignment(TableAssignment),
    Call(TableCall),
    Expression(crate::expression::ExpressionHandle),
    LocalData(TableLocalData),
    Transition(TableTransition),
}

impl Default for StatementNode {
    fn default() -> Self {
        Self::Expression(crate::expression::ExpressionHandle::invalid())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableAssignment {
    pub target: crate::expression::ExpressionHandle,
    pub value: crate::expression::ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCall {
    pub receiver: HandleSpan<Identifier>,
    pub target: Identifier,
    pub arguments: HandleSpan<crate::expression::ExpressionHandle>,
}

impl Default for TableCall {
    fn default() -> Self {
        Self {
            receiver: HandleSpan::empty(),
            target: Identifier::default(),
            arguments: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableLocalData {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub initial_value: crate::expression::ExpressionHandle,
}

impl Default for TableLocalData {
    fn default() -> Self {
        Self {
            name: Identifier::default(),
            type_reference: crate::types::TypeReferenceHandle::invalid(),
            initial_value: crate::expression::ExpressionHandle::invalid(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableTransition {
    pub target: TransitionTargetHandle,
    pub continuation: TransitionTargetHandle,
    pub guard: TransitionGuardNode,
}

impl Default for TableTransition {
    fn default() -> Self {
        Self {
            target: TransitionTargetHandle::invalid(),
            continuation: TransitionTargetHandle::invalid(),
            guard: TransitionGuardNode::Always,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionGuardNode {
    Always,
    When(crate::expression::ExpressionHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionTargetNode {
    Named {
        path: HandleSpan<Identifier>,
        arguments: HandleSpan<crate::expression::ExpressionHandle>,
    },
    Value(crate::expression::ExpressionHandle),
    SelfTarget,
    Terminal,
}

impl Default for TransitionTargetNode {
    fn default() -> Self {
        Self::Terminal
    }
}

#[cfg(test)]
mod tests {
    use super::{
        StatementNode, StatementTable, TableTransition, TransitionGuardNode, TransitionTargetNode,
    };
    use crate::expression::{ExpressionNode, ExpressionTable};
    use crate::identifier::Identifier;
    use omega_core::arena::HandleSpan;

    #[test]
    fn statement_table_stores_transition_payloads_as_handles() {
        let mut statements = StatementTable::new();
        let mut expressions = ExpressionTable::new();
        let path_start = statements.append_identifier_path_member(Identifier::generated("next"));
        let path = HandleSpan::from_parts(path_start, 1);
        let argument_one = expressions.insert(ExpressionNode::Integer(1));
        let argument_one = statements.append_expression_handle(argument_one);
        let argument_two = expressions.insert(ExpressionNode::Integer(2));
        let _argument_two = statements.append_expression_handle(argument_two);
        let arguments = HandleSpan::from_parts(argument_one, 2);
        let target =
            statements.insert_transition_target(TransitionTargetNode::Named { path, arguments });
        let guard = expressions.insert(ExpressionNode::Boolean(true));
        let statement = statements.insert(StatementNode::Transition(TableTransition {
            target,
            continuation: super::TransitionTargetHandle::invalid(),
            guard: TransitionGuardNode::When(guard),
        }));

        let StatementNode::Transition(transition) = statements.statement(statement) else {
            panic!("statement should lower to a table transition");
        };
        let TransitionTargetNode::Named { path, arguments } =
            statements.transition_target(transition.target)
        else {
            panic!("transition target should be named");
        };

        assert_eq!(path.count(), 1);
        assert_eq!(arguments.count(), 2);
        assert_eq!(expressions.expression_count(), 3);
    }
}
