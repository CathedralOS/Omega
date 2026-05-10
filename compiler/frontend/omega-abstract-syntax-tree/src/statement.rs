use crate::identifier::{Identifier, IdentifierPath};
use omega_core::arena::{Arena, Handle, HandleSpan};

pub type StatementHandle = Handle<StatementNode>;
pub type TransitionTargetHandle = Handle<TransitionTargetNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Assignment(Assignment),
    Call(Call),
    Expression(crate::expression::Expression),
    LocalData(LocalData),
    Transition(Transition),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub target: crate::expression::Expression,
    pub value: crate::expression::Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalData {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub receiver: Option<IdentifierPath>,
    pub target: Identifier,
    pub arguments: Vec<crate::expression::Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub target: TransitionTarget,
    pub continuation: Option<TransitionTarget>,
    pub guard: TransitionGuard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionGuard {
    Always,
    When(crate::expression::Expression),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionTarget {
    Named {
        path: IdentifierPath,
        arguments: Vec<crate::expression::Expression>,
    },
    SelfTarget,
    Terminal,
}

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

    pub fn insert_tree(
        &mut self,
        statement: &Statement,
        expressions: &mut crate::expression::ExpressionTable,
        type_references: &mut crate::types::TypeReferenceTable,
    ) -> StatementHandle {
        match statement {
            Statement::Assignment(assignment) => {
                let target = expressions.insert_tree(&assignment.target);
                let value = expressions.insert_tree(&assignment.value);
                self.insert(StatementNode::Assignment(TableAssignment { target, value }))
            }
            Statement::Call(call) => {
                let arguments =
                    self.insert_expression_handle_span_from_trees(&call.arguments, expressions);
                let receiver = call
                    .receiver
                    .as_ref()
                    .map(|path| self.insert_identifier_path_members(path))
                    .unwrap_or_else(HandleSpan::empty);
                self.insert(StatementNode::Call(TableCall {
                    receiver,
                    target: call.target.clone(),
                    arguments,
                }))
            }
            Statement::Expression(expression) => {
                let expression = expressions.insert_tree(expression);
                self.insert(StatementNode::Expression(expression))
            }
            Statement::LocalData(local_data) => {
                let type_reference =
                    type_references.insert_tree(&local_data.type_reference, expressions);
                self.insert(StatementNode::LocalData(TableLocalData {
                    name: local_data.name.clone(),
                    type_reference,
                }))
            }
            Statement::Transition(transition) => {
                let target = self.insert_transition_target_tree(&transition.target, expressions);
                let continuation = transition
                    .continuation
                    .as_ref()
                    .map(|target| self.insert_transition_target_tree(target, expressions))
                    .unwrap_or_else(TransitionTargetHandle::invalid);
                let guard = match &transition.guard {
                    TransitionGuard::Always => TransitionGuardNode::Always,
                    TransitionGuard::When(expression) => {
                        TransitionGuardNode::When(expressions.insert_tree(expression))
                    }
                };
                self.insert(StatementNode::Transition(TableTransition {
                    target,
                    continuation,
                    guard,
                }))
            }
        }
    }

    fn insert_expression_handle_span_from_trees(
        &mut self,
        arguments: &[crate::expression::Expression],
        expressions: &mut crate::expression::ExpressionTable,
    ) -> HandleSpan<crate::expression::ExpressionHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for argument in arguments {
            let argument = expressions.insert_tree(argument);
            let handle = self.expression_handles.append(argument);
            if count == 0 {
                start = handle;
            }
            count = count
                .checked_add(1)
                .expect("statement expression span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn insert_identifier_path_members(&mut self, path: &IdentifierPath) -> HandleSpan<Identifier> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for member in path.iter() {
            let handle = self.identifier_path_members.append(member.clone());
            if count == 0 {
                start = handle;
            }
            count = count
                .checked_add(1)
                .expect("transition target path member span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn insert_transition_target_tree(
        &mut self,
        target: &TransitionTarget,
        expressions: &mut crate::expression::ExpressionTable,
    ) -> TransitionTargetHandle {
        let target = match target {
            TransitionTarget::Named { path, arguments } => TransitionTargetNode::Named {
                path: self.insert_identifier_path_members(path),
                arguments: self.insert_expression_handle_span_from_trees(arguments, expressions),
            },
            TransitionTarget::SelfTarget => TransitionTargetNode::SelfTarget,
            TransitionTarget::Terminal => TransitionTargetNode::Terminal,
        };

        self.transition_targets.insert(target)
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
}

impl Default for TableLocalData {
    fn default() -> Self {
        Self {
            name: Identifier::default(),
            type_reference: crate::types::TypeReferenceHandle::invalid(),
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
        Statement, StatementNode, StatementTable, Transition, TransitionGuard, TransitionTarget,
        TransitionTargetNode,
    };
    use crate::expression::{Expression, ExpressionTable};
    use crate::identifier::{Identifier, IdentifierPath};
    use crate::types::TypeReferenceTable;

    #[test]
    fn statement_table_stores_transition_payloads_as_handles() {
        let statement = Statement::Transition(Transition {
            target: TransitionTarget::Named {
                path: IdentifierPath::from(vec![Identifier::generated("next")]),
                arguments: vec![Expression::Integer(1), Expression::Integer(2)],
            },
            continuation: None,
            guard: TransitionGuard::When(Expression::Boolean(true)),
        });

        let mut statements = StatementTable::new();
        let mut expressions = ExpressionTable::new();
        let mut type_references = TypeReferenceTable::new();
        let statement = statements.insert_tree(&statement, &mut expressions, &mut type_references);

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
