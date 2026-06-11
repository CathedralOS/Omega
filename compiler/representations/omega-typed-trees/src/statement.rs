use crate::name::Identifier;
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;

pub type StatementHandle = Handle<StatementNode>;
pub type TransitionTargetHandle = Handle<TransitionTargetNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementTable {
    statements: Arena<StatementNode>,
    expression_handles: Arena<crate::expression::ExpressionHandle>,
    name_path_members: Arena<Identifier>,
    transition_targets: Arena<TransitionTargetNode>,
}

impl StatementTable {
    pub fn new() -> Self {
        Self {
            statements: Arena::new(),
            expression_handles: Arena::new(),
            name_path_members: Arena::new(),
            transition_targets: Arena::new(),
        }
    }

    pub fn insert(&mut self, statement: StatementNode) -> StatementHandle {
        self.statements.append(statement)
    }

    pub fn push_statement(
        &mut self,
        span: &mut HandleSpan<StatementNode>,
        statement: StatementNode,
    ) -> StatementHandle {
        self.statements.append_to_span(span, statement)
    }

    pub fn push_expression_handle(
        &mut self,
        span: &mut HandleSpan<crate::expression::ExpressionHandle>,
        expression: crate::expression::ExpressionHandle,
    ) {
        self.expression_handles.append_to_span(span, expression);
    }

    pub fn insert_expression_handles(
        &mut self,
        expressions: impl IntoIterator<Item = crate::expression::ExpressionHandle>,
    ) -> HandleSpan<crate::expression::ExpressionHandle> {
        self.expression_handles.insert_many(expressions)
    }

    pub fn reserve_expression_handles(
        &mut self,
        count: u32,
    ) -> HandleSpan<crate::expression::ExpressionHandle> {
        self.expression_handles.insert_many(
            std::iter::repeat_with(crate::expression::ExpressionHandle::invalid)
                .take(usize::try_from(count).expect("expression handle span count overflow")),
        )
    }

    pub fn set_expression_handle_at_offset(
        &mut self,
        expressions: HandleSpan<crate::expression::ExpressionHandle>,
        offset: u32,
        expression: crate::expression::ExpressionHandle,
    ) {
        *self.expression_handles.get_mut(Handle::from_parts(
            expressions
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("expression handle index overflow"),
            expressions.start().generation(),
        )) = expression;
    }

    pub fn push_name_path_member(&mut self, span: &mut HandleSpan<Identifier>, member: Identifier) {
        self.name_path_members.append_to_span(span, member);
    }

    pub fn insert_transition_target(
        &mut self,
        target: TransitionTargetNode,
    ) -> TransitionTargetHandle {
        self.transition_targets.insert(target)
    }

    pub fn copy_statement_nodes_from(
        &mut self,
        source: &StatementTable,
        statements: HandleSpan<StatementNode>,
    ) -> HandleSpan<StatementNode> {
        let mut copied = HandleSpan::empty();

        for statement in source.statements(statements) {
            self.push_statement(&mut copied, statement.clone());
        }

        copied
    }

    pub fn statement(&self, handle: StatementHandle) -> &StatementNode {
        self.statements.get(handle)
    }

    pub fn statements(&self, span: HandleSpan<StatementNode>) -> &[StatementNode] {
        self.statements.span_or_empty(span)
    }

    pub fn expression_handles(
        &self,
        span: HandleSpan<crate::expression::ExpressionHandle>,
    ) -> &[crate::expression::ExpressionHandle] {
        self.expression_handles.span_or_empty(span)
    }

    pub fn name_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
        self.name_path_members.span_or_empty(span)
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
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub receiver: HandleSpan<Identifier>,
    pub target: Identifier,
    pub arguments: HandleSpan<crate::expression::ExpressionHandle>,
    /// `_ = call();` -- the caller explicitly discards a non-unit result.
    pub discards_result: bool,
}

impl Default for TableCall {
    fn default() -> Self {
        Self {
            receiver_symbol: SymbolHandle::invalid(),
            target_symbol: SymbolHandle::invalid(),
            receiver: HandleSpan::empty(),
            target: Identifier::default(),
            arguments: HandleSpan::empty(),
            discards_result: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableLocalData {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub initial_value: crate::expression::ExpressionHandle,
}

impl Default for TableLocalData {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
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
        path: TableNamePath,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableNamePath {
    pub members: HandleSpan<Identifier>,
    pub head_symbol: SymbolHandle,
    pub symbol: SymbolHandle,
}

#[cfg(test)]
mod tests {
    use super::{StatementNode, StatementTable, TransitionTargetNode};
    use crate::expression::ExpressionTable;
    use crate::name::Identifier;
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn statement_table_appends_handle_native_payloads_directly() {
        let target_symbol = SymbolHandle::from_arena_index(11);
        let mut statements = StatementTable::new();
        let mut expressions = ExpressionTable::new();
        let argument = expressions.insert(crate::expression::ExpressionNode::Integer(99));

        let mut arguments = omega_core::arena::HandleSpan::empty();
        statements.push_expression_handle(&mut arguments, argument);

        let mut path = omega_core::arena::HandleSpan::empty();
        statements.push_name_path_member(&mut path, Identifier::generated("next"));

        let target = statements.insert_transition_target(TransitionTargetNode::Named {
            path: super::TableNamePath {
                members: path,
                head_symbol: target_symbol,
                symbol: target_symbol,
            },
            arguments,
        });

        let mut state_statements = omega_core::arena::HandleSpan::empty();
        let statement = statements.push_statement(
            &mut state_statements,
            StatementNode::Transition(super::TableTransition {
                target,
                continuation: super::TransitionTargetHandle::invalid(),
                guard: super::TransitionGuardNode::Always,
            }),
        );

        assert_eq!(state_statements.count(), 1);
        assert_eq!(statements.statement_count(), 1);
        assert_eq!(statements.transition_target_count(), 1);

        let StatementNode::Transition(transition) = statements.statement(statement) else {
            panic!("statement should be transition");
        };
        let TransitionTargetNode::Named { path, arguments } =
            statements.transition_target(transition.target)
        else {
            panic!("transition target should be named");
        };

        assert_eq!(path.symbol, target_symbol);
        assert_eq!(arguments.count(), 1);
        assert_eq!(statements.expression_handles(*arguments), &[argument]);
    }
}
