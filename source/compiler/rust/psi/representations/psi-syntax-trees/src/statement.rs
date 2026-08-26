use crate::identifier::Identifier;
use psi_arena::{Arena, Handle, HandleSpan};
use psi_source::SourceSpan;

pub type StatementHandle = Handle<StatementNode>;
pub type TransitionTargetHandle = Handle<TransitionTargetNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementTable {
    statements: Arena<StatementNode>,
    expression_handles: Arena<crate::expression::ExpressionHandle>,
    identifier_path_members: Arena<Identifier>,
    transition_targets: Arena<TransitionTargetNode>,
    outcome_proof_selectors: Arena<TableOutcomeProofSelector>,
}

impl StatementTable {
    pub fn new() -> Self {
        Self {
            statements: Arena::new(),
            expression_handles: Arena::new(),
            identifier_path_members: Arena::new(),
            transition_targets: Arena::new(),
            outcome_proof_selectors: Arena::new(),
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

    pub fn insert_expression_handles(
        &mut self,
        expressions: impl IntoIterator<Item = crate::expression::ExpressionHandle>,
    ) -> HandleSpan<crate::expression::ExpressionHandle> {
        self.expression_handles.insert_many(expressions)
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

    pub fn insert_outcome_proof_selectors(
        &mut self,
        selectors: impl IntoIterator<Item = TableOutcomeProofSelector>,
    ) -> HandleSpan<TableOutcomeProofSelector> {
        self.outcome_proof_selectors.insert_many(selectors)
    }

    pub fn outcome_proof_selectors(
        &self,
        span: HandleSpan<TableOutcomeProofSelector>,
    ) -> &[TableOutcomeProofSelector] {
        self.outcome_proof_selectors.span_or_empty(span)
    }

    /// Replace a node in place. Reserved for PARSE-phase desugars (the
    /// bool-tuple exhaustiveness rewrite) -- downstream stages treat the
    /// table as immutable, mirroring `ExpressionTable::replace_expression`.
    pub fn replace_statement(&mut self, handle: StatementHandle, statement: StatementNode) {
        *self.statements.get_mut(handle) = statement;
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
    AssemblyFact(TableAssemblyFact),
    Assignment(TableAssignment),
    Call(TableCall),
    ProofOutputBindingStatement(TableProofOutputBindingStatement),
    Expression(crate::expression::ExpressionHandle),
    LocalData(TableLocalData),
    Transition(TableTransition),
}

/// Immediate selective binding of named proof outputs from one call. This is
/// deliberately distinct from record destructuring: the receiver is evaluated
/// once and no proof output is represented by a runtime member read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableProofOutputBindingStatement {
    pub bindings: Box<[TableProofOutputSelector]>,
    pub call: crate::expression::ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableProofOutputSelector {
    pub output_field: Identifier,
    pub binding: Identifier,
}

/// One erased caller-local selection from an outcome-specific guarantee lane.
/// This is attached to an exact transition arm and never becomes a runtime
/// payload field or statement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableOutcomeProofSelector {
    pub output_field: Identifier,
    pub binding: Identifier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableAssemblyFact {
    pub kind: AssemblyFactKind,
    pub expression: crate::expression::ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssemblyFactKind {
    Requires,
    Ensures,
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
    pub receiver_starts_at_self: bool,
    pub target: Identifier,
    pub machine_arguments: Box<[crate::expression::StaticMachineArgument]>,
    pub arguments: HandleSpan<crate::expression::ExpressionHandle>,
    /// Explicit erased evidence-term arguments after the `;` call lane.
    pub evidence_arguments: Box<[Identifier]>,
    pub operational_acknowledgement: psi_language_core::CallOperationalAcknowledgement,
    /// `_ = call();` -- the caller explicitly discards a non-unit result.
    pub discards_result: bool,
}

impl Default for TableCall {
    fn default() -> Self {
        Self {
            receiver: HandleSpan::empty(),
            receiver_starts_at_self: false,
            target: Identifier::default(),
            machine_arguments: Box::default(),
            arguments: HandleSpan::empty(),
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
            discards_result: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableLocalData {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub initial_value: crate::expression::ExpressionHandle,
    /// `let mut` -- the local admits reassignment (ch3/ch14 spelling); a
    /// plain `let` is immutable and reassignment refuses.
    pub is_mutable: bool,
}

impl Default for TableLocalData {
    fn default() -> Self {
        Self {
            name: Identifier::default(),
            type_reference: crate::types::TypeReferenceHandle::invalid(),
            initial_value: crate::expression::ExpressionHandle::invalid(),
            is_mutable: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableTransition {
    pub target: TransitionTargetHandle,
    pub continuation: TransitionTargetHandle,
    pub guard: TransitionGuardNode,
    pub proof_selectors: HandleSpan<TableOutcomeProofSelector>,
    pub exit: TransitionExit,
    pub source_span: SourceSpan,
}

impl Default for TableTransition {
    fn default() -> Self {
        Self {
            target: TransitionTargetHandle::invalid(),
            continuation: TransitionTargetHandle::invalid(),
            guard: TransitionGuardNode::Always,
            proof_selectors: HandleSpan::empty(),
            exit: Default::default(),
            source_span: SourceSpan::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TransitionExit {
    #[default]
    Ordinary,
    Crash(crate::item::CrashCause),
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
        path_starts_at_self: bool,
        arguments: HandleSpan<crate::expression::ExpressionHandle>,
        evidence_arguments: Box<[Identifier]>,
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
    use psi_arena::HandleSpan;

    #[test]
    fn statement_table_stores_transition_payloads_as_handles() {
        let mut statements = StatementTable::new();
        let mut expressions = ExpressionTable::new();
        let path_start = statements.append_identifier_path_member(Identifier::generated("next"));
        let path = HandleSpan::from_parts(path_start, 1);
        let argument_one = expressions.insert(ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::from_value(1),
        ));
        let argument_one = statements.append_expression_handle(argument_one);
        let argument_two = expressions.insert(ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::from_value(2),
        ));
        let _argument_two = statements.append_expression_handle(argument_two);
        let arguments = HandleSpan::from_parts(argument_one, 2);
        let target = statements.insert_transition_target(TransitionTargetNode::Named {
            path,
            path_starts_at_self: false,
            arguments,
            evidence_arguments: Box::default(),
        });
        let guard = expressions.insert(ExpressionNode::Boolean(true));
        let statement = statements.insert(StatementNode::Transition(TableTransition {
            target,
            continuation: super::TransitionTargetHandle::invalid(),
            guard: TransitionGuardNode::When(guard),
            proof_selectors: HandleSpan::empty(),
            exit: Default::default(),
            source_span: Default::default(),
        }));

        let StatementNode::Transition(transition) = statements.statement(statement) else {
            panic!("statement should lower to a table transition");
        };
        let TransitionTargetNode::Named {
            path, arguments, ..
        } = statements.transition_target(transition.target)
        else {
            panic!("transition target should be named");
        };

        assert_eq!(path.count(), 1);
        assert_eq!(arguments.count(), 2);
        assert_eq!(expressions.expression_count(), 3);
    }
}
