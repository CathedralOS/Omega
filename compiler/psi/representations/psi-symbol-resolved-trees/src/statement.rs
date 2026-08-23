use crate::name::DiagnosticName;
use psi_arena::{Arena, Handle, HandleSpan};
use psi_source::SourceSpan;
use psi_symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

pub type StatementHandle = Handle<StatementNode>;
pub type TransitionTargetHandle = Handle<TransitionTargetNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    AssemblyFact(AssemblyFact),
    Assignment(Assignment),
    Call(Call),
    ProofOutputBindingStatement(ProofOutputBindingStatement),
    Expression(crate::expression::ExpressionHandle),
    LocalData(LocalData),
    Transition(Transition),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofOutputBindingStatement {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub bindings: Box<[ProofOutputSelector]>,
    pub call: crate::expression::ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofOutputSelector {
    pub output_field: DiagnosticName,
    pub binding: DiagnosticName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssemblyFact {
    pub kind: AssemblyFactKind,
    pub expression: crate::expression::ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssemblyFactKind {
    Requires,
    Ensures,
}

impl Default for Statement {
    fn default() -> Self {
        Self::Expression(Handle::invalid())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub target: crate::expression::ExpressionHandle,
    pub value: crate::expression::ExpressionHandle,
}

/// An erased named-`ensures` assignment classified out of the runtime
/// statement stream. Names remain diagnostic at this phase; owner symbols and
/// an explicitly named subjectless producer conformance are bound after
/// ordinary symbol assignment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvidenceForwarding {
    /// Stable lowering-time coordinate used to bind the owning machine after
    /// symbols are assigned. Short machine names are not unique for attached
    /// machines, so owner binding must not key only on diagnostic spelling.
    pub machine_root_index: usize,
    pub machine_name: DiagnosticName,
    pub state_name: DiagnosticName,
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    /// Runtime-statement insertion point in the owning state. Evidence erases,
    /// so several forwardings may share one coordinate.
    pub statement_index: usize,
    pub target: DiagnosticName,
    pub source: DiagnosticName,
    /// Exact producer selected by `target = ConformanceName`, when `source`
    /// resolves to a subjectless conformance. Incoming evidence forwarding
    /// leaves this empty and is bound by the checked proof pass.
    pub source_conformance: Option<SymbolHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalData {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub storage: LocalDataStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDataStorage {
    pub type_reference: crate::types::TypeReference,
    pub initial_value: crate::expression::ExpressionHandle,
    /// `let mut` -- see the syntax-tree twin.
    pub is_mutable: bool,
}

impl Default for LocalDataStorage {
    fn default() -> Self {
        Self {
            type_reference: crate::types::TypeReference::Unit,
            initial_value: crate::expression::ExpressionHandle::invalid(),
            is_mutable: false,
        }
    }
}

impl Deref for LocalData {
    type Target = LocalDataStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for LocalData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub target: DiagnosticName,
    pub storage: CallStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallStorage {
    pub receiver: HandleSpan<DiagnosticName>,
    pub receiver_starts_at_self: bool,
    pub machine_arguments: Box<[crate::expression::StaticMachineArgument]>,
    pub arguments: HandleSpan<crate::expression::ExpressionHandle>,
    pub evidence_arguments: Box<[DiagnosticName]>,
    pub operational_acknowledgement: psi_language_semantics::CallOperationalAcknowledgement,
    /// `_ = call();` -- the caller explicitly discards a non-unit result.
    pub discards_result: bool,
}

impl Deref for Call {
    type Target = CallStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for Call {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub target: TransitionTarget,
    pub continuation: Option<TransitionTarget>,
    pub guard: TransitionGuard,
    pub exit: TransitionExit,
    pub source_span: SourceSpan,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TransitionExit {
    #[default]
    Ordinary,
    Crash(crate::signature::CrashCause),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionGuard {
    Always,
    When(crate::expression::ExpressionHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionTarget {
    Named(NamedTransitionTarget),
    Value(crate::expression::ExpressionHandle),
    SelfTarget,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedTransitionTarget {
    pub head_symbol: SymbolHandle,
    pub symbol: SymbolHandle,
    pub storage: NamedTransitionTargetStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamedTransitionTargetStorage {
    pub path: HandleSpan<DiagnosticName>,
    pub path_starts_at_self: bool,
    pub arguments: HandleSpan<crate::expression::ExpressionHandle>,
    pub evidence_arguments: Box<[DiagnosticName]>,
}

impl Deref for NamedTransitionTarget {
    type Target = NamedTransitionTargetStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for NamedTransitionTarget {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementTable {
    nodes: StatementNodeStorage,
    paths: StatementPathStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StatementNodeStorage {
    statements: Arena<StatementNode>,
    transition_targets: Arena<TransitionTargetNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StatementPathStorage {
    expression_handles: Arena<crate::expression::ExpressionHandle>,
    name_path_members: Arena<DiagnosticName>,
}

impl StatementTable {
    pub fn new() -> Self {
        Self {
            nodes: StatementNodeStorage {
                statements: Arena::new(),
                transition_targets: Arena::new(),
            },
            paths: StatementPathStorage {
                expression_handles: Arena::new(),
                name_path_members: Arena::new(),
            },
        }
    }

    pub fn insert(&mut self, statement: StatementNode) -> StatementHandle {
        self.nodes.statements.append(statement)
    }

    pub fn statement(&self, handle: StatementHandle) -> &StatementNode {
        self.nodes.statements.get(handle)
    }

    pub fn statement_mut(&mut self, handle: StatementHandle) -> &mut StatementNode {
        self.nodes.statements.get_mut(handle)
    }

    pub fn statements(&self, span: HandleSpan<StatementNode>) -> &[StatementNode] {
        self.nodes.statements.span_or_empty(span)
    }

    pub fn expression_handles(
        &self,
        span: HandleSpan<crate::expression::ExpressionHandle>,
    ) -> &[crate::expression::ExpressionHandle] {
        self.paths.expression_handles.span_or_empty(span)
    }

    pub fn reserve_expression_handles(
        &mut self,
        count: u32,
    ) -> HandleSpan<crate::expression::ExpressionHandle> {
        self.paths.expression_handles.insert_many(
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
        *self.paths.expression_handles.get_mut(Handle::from_parts(
            expressions
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("expression handle index overflow"),
            expressions.start().generation(),
        )) = expression;
    }

    pub fn name_path_members(&self, span: HandleSpan<DiagnosticName>) -> &[DiagnosticName] {
        self.paths.name_path_members.span_or_empty(span)
    }

    pub fn transition_target(&self, handle: TransitionTargetHandle) -> &TransitionTargetNode {
        self.nodes.transition_targets.get(handle)
    }

    pub fn statement_count(&self) -> usize {
        self.nodes.statements.len()
    }

    pub fn transition_target_count(&self) -> usize {
        self.nodes.transition_targets.len()
    }

    pub fn insert_tree(
        &mut self,
        statement: &Statement,
        source_expressions: &crate::expression::ExpressionTable,
        expressions: &mut crate::expression::ExpressionTable,
        type_references: &mut crate::types::TypeReferenceTable,
        source_child_type_references: &Arena<crate::types::TypeReference>,
        source_constraints: &Arena<crate::types::TypeConstraint>,
        source_expression_table: &crate::expression::ExpressionTable,
        source_statement_path_members: &Arena<DiagnosticName>,
        copy_expression_handles: bool,
    ) -> StatementHandle {
        match statement {
            Statement::AssemblyFact(fact) => {
                let expression = expression_handle_from_tree(
                    source_expressions,
                    expressions,
                    fact.expression,
                    copy_expression_handles,
                );
                self.insert(StatementNode::AssemblyFact(TableAssemblyFact {
                    kind: fact.kind,
                    expression,
                }))
            }
            Statement::Assignment(assignment) => {
                let target = expression_handle_from_tree(
                    source_expressions,
                    expressions,
                    assignment.target,
                    copy_expression_handles,
                );
                let value = expression_handle_from_tree(
                    source_expressions,
                    expressions,
                    assignment.value,
                    copy_expression_handles,
                );
                self.insert(StatementNode::Assignment(TableAssignment { target, value }))
            }
            Statement::Call(call) => {
                let arguments = self.insert_expression_handle_span_from_trees(
                    source_expressions,
                    call.arguments,
                    expressions,
                    copy_expression_handles,
                );
                let receiver = self.insert_name_path_members(
                    source_statement_path_members.span_or_empty(call.receiver),
                );
                self.insert(StatementNode::Call(TableCall {
                    receiver_symbol: call.receiver_symbol,
                    target_symbol: call.target_symbol,
                    receiver,
                    receiver_starts_at_self: call.receiver_starts_at_self,
                    target: call.target.clone(),
                    machine_arguments: call.machine_arguments.clone(),
                    arguments,
                    evidence_arguments: call.evidence_arguments.clone(),
                    operational_acknowledgement: call.operational_acknowledgement,
                    discards_result: call.discards_result,
                }))
            }
            Statement::ProofOutputBindingStatement(package) => {
                let call = expression_handle_from_tree(
                    source_expressions,
                    expressions,
                    package.call,
                    copy_expression_handles,
                );
                self.insert(StatementNode::ProofOutputBindingStatement(
                    ProofOutputBindingStatement {
                        machine_symbol: package.machine_symbol,
                        state_symbol: package.state_symbol,
                        statement_index: package.statement_index,
                        bindings: package.bindings.clone(),
                        call,
                    },
                ))
            }
            Statement::Expression(expression) => {
                let expression = expression_handle_from_tree(
                    source_expressions,
                    expressions,
                    *expression,
                    copy_expression_handles,
                );
                self.insert(StatementNode::Expression(expression))
            }
            Statement::LocalData(local_data) => {
                let type_reference = type_references.insert_tree(
                    &local_data.type_reference,
                    expressions,
                    source_child_type_references,
                    source_constraints,
                    source_expression_table,
                );
                let initial_value = local_data
                    .initial_value
                    .is_valid()
                    .then(|| {
                        expression_handle_from_tree(
                            source_expressions,
                            expressions,
                            local_data.initial_value,
                            copy_expression_handles,
                        )
                    })
                    .unwrap_or_else(crate::expression::ExpressionHandle::invalid);
                self.insert(StatementNode::LocalData(TableLocalData {
                    symbol: local_data.symbol,
                    name: local_data.name.clone(),
                    type_reference,
                    initial_value,
                    is_mutable: local_data.is_mutable,
                }))
            }
            Statement::Transition(transition) => {
                let target = self.insert_transition_target_tree(
                    &transition.target,
                    source_expressions,
                    expressions,
                    source_statement_path_members,
                    copy_expression_handles,
                );
                let continuation = transition
                    .continuation
                    .as_ref()
                    .map(|target| {
                        self.insert_transition_target_tree(
                            target,
                            source_expressions,
                            expressions,
                            source_statement_path_members,
                            copy_expression_handles,
                        )
                    })
                    .unwrap_or_else(TransitionTargetHandle::invalid);
                let guard = match &transition.guard {
                    TransitionGuard::Always => TransitionGuardNode::Always,
                    TransitionGuard::When(expression) => {
                        TransitionGuardNode::When(expression_handle_from_tree(
                            source_expressions,
                            expressions,
                            *expression,
                            copy_expression_handles,
                        ))
                    }
                };
                self.insert(StatementNode::Transition(TableTransition {
                    target,
                    continuation,
                    guard,
                    exit: transition.exit,
                    source_span: transition.source_span,
                }))
            }
        }
    }

    fn insert_expression_handle_span_from_trees(
        &mut self,
        source_expressions: &crate::expression::ExpressionTable,
        arguments: HandleSpan<crate::expression::ExpressionHandle>,
        expressions: &mut crate::expression::ExpressionTable,
        copy_expression_handles: bool,
    ) -> HandleSpan<crate::expression::ExpressionHandle> {
        let source = source_expressions.expression_handles(arguments);
        let span = self.reserve_expression_handles(
            source
                .len()
                .try_into()
                .expect("expression handle span count overflow"),
        );

        for (offset, argument) in source.iter().enumerate() {
            let argument = expression_handle_from_tree(
                source_expressions,
                expressions,
                *argument,
                copy_expression_handles,
            );
            self.set_expression_handle_at_offset(
                span,
                offset
                    .try_into()
                    .expect("expression handle span count overflow"),
                argument,
            );
        }

        span
    }

    fn insert_name_path_members(&mut self, path: &[DiagnosticName]) -> HandleSpan<DiagnosticName> {
        self.paths
            .name_path_members
            .insert_many(path.iter().cloned())
    }

    fn insert_transition_target_tree(
        &mut self,
        target: &TransitionTarget,
        source_expressions: &crate::expression::ExpressionTable,
        expressions: &mut crate::expression::ExpressionTable,
        source_statement_path_members: &Arena<DiagnosticName>,
        copy_expression_handles: bool,
    ) -> TransitionTargetHandle {
        let target = match target {
            TransitionTarget::Named(named) => TransitionTargetNode::Named {
                path: TableNamePath {
                    members: self.insert_name_path_members(
                        source_statement_path_members.span_or_empty(named.path),
                    ),
                    starts_at_self: named.path_starts_at_self,
                    head_symbol: named.head_symbol,
                    symbol: named.symbol,
                },
                arguments: self.insert_expression_handle_span_from_trees(
                    source_expressions,
                    named.arguments,
                    expressions,
                    copy_expression_handles,
                ),
                evidence_arguments: named.evidence_arguments.clone(),
            },
            TransitionTarget::Value(expression) => {
                TransitionTargetNode::Value(expression_handle_from_tree(
                    source_expressions,
                    expressions,
                    *expression,
                    copy_expression_handles,
                ))
            }
            TransitionTarget::SelfTarget => TransitionTargetNode::SelfTarget,
            TransitionTarget::Terminal => TransitionTargetNode::Terminal,
        };

        self.nodes.transition_targets.insert(target)
    }
}

fn expression_handle_from_tree(
    source_expressions: &crate::expression::ExpressionTable,
    expressions: &mut crate::expression::ExpressionTable,
    expression: crate::expression::ExpressionHandle,
    copy_expression_handles: bool,
) -> crate::expression::ExpressionHandle {
    if copy_expression_handles {
        expressions.copy_from(source_expressions, expression)
    } else {
        expression
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
    ProofOutputBindingStatement(ProofOutputBindingStatement),
    Expression(crate::expression::ExpressionHandle),
    LocalData(TableLocalData),
    Transition(TableTransition),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableAssemblyFact {
    pub kind: AssemblyFactKind,
    pub expression: crate::expression::ExpressionHandle,
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
    pub receiver: HandleSpan<DiagnosticName>,
    pub receiver_starts_at_self: bool,
    pub target: DiagnosticName,
    pub machine_arguments: Box<[crate::expression::StaticMachineArgument]>,
    pub arguments: HandleSpan<crate::expression::ExpressionHandle>,
    pub evidence_arguments: Box<[DiagnosticName]>,
    pub operational_acknowledgement: psi_language_semantics::CallOperationalAcknowledgement,
    /// `_ = call();` -- the caller explicitly discards a non-unit result.
    pub discards_result: bool,
}

impl Default for TableCall {
    fn default() -> Self {
        Self {
            receiver_symbol: SymbolHandle::invalid(),
            target_symbol: SymbolHandle::invalid(),
            receiver: HandleSpan::empty(),
            receiver_starts_at_self: false,
            target: DiagnosticName::default(),
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
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub initial_value: crate::expression::ExpressionHandle,
    /// `let mut` -- see the syntax-tree twin.
    pub is_mutable: bool,
}

impl Default for TableLocalData {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::default(),
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
    pub exit: TransitionExit,
    pub source_span: SourceSpan,
}

impl Default for TableTransition {
    fn default() -> Self {
        Self {
            target: TransitionTargetHandle::invalid(),
            continuation: TransitionTargetHandle::invalid(),
            guard: TransitionGuardNode::Always,
            exit: Default::default(),
            source_span: SourceSpan::default(),
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
        evidence_arguments: Box<[DiagnosticName]>,
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
    pub members: HandleSpan<DiagnosticName>,
    pub starts_at_self: bool,
    pub head_symbol: SymbolHandle,
    pub symbol: SymbolHandle,
}

#[cfg(test)]
mod tests {
    use super::{
        NamedTransitionTarget, NamedTransitionTargetStorage, Statement, StatementNode,
        StatementTable, Transition, TransitionGuard, TransitionTarget, TransitionTargetNode,
    };
    use crate::expression::{ExpressionNode, ExpressionTable};
    use crate::name::DiagnosticName;
    use crate::types::TypeReferenceTable;
    use psi_arena::Arena;
    use psi_symbols::SymbolHandle;

    #[test]
    fn statement_table_stores_transition_payloads_as_handles() {
        let target_symbol = SymbolHandle::from_arena_index(7);
        let mut source_expressions = ExpressionTable::new();
        let mut arguments = psi_arena::HandleSpan::empty();
        let first_argument = source_expressions.insert(ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::from_value(1),
        ));
        source_expressions.push_expression_handle(&mut arguments, first_argument);
        let second_argument = source_expressions.insert(ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::from_value(2),
        ));
        source_expressions.push_expression_handle(&mut arguments, second_argument);
        let guard = source_expressions.insert(ExpressionNode::Boolean(true));
        let mut source_statement_path_members = Arena::new();
        let path = source_statement_path_members.insert_many([DiagnosticName::generated("next")]);
        let statement = Statement::Transition(Transition {
            target: TransitionTarget::Named(NamedTransitionTarget {
                head_symbol: target_symbol,
                symbol: target_symbol,
                storage: NamedTransitionTargetStorage {
                    path,
                    path_starts_at_self: false,
                    arguments,
                    evidence_arguments: Box::default(),
                },
            }),
            continuation: None,
            guard: TransitionGuard::When(guard),
            exit: Default::default(),
            source_span: Default::default(),
        });

        let mut statements = StatementTable::new();
        let mut expressions = ExpressionTable::new();
        let mut type_references = TypeReferenceTable::new();
        let child_type_references = Arena::new();
        let type_constraints = Arena::new();
        let statement = statements.insert_tree(
            &statement,
            &source_expressions,
            &mut expressions,
            &mut type_references,
            &child_type_references,
            &type_constraints,
            &source_expressions,
            &source_statement_path_members,
            true,
        );

        let StatementNode::Transition(transition) = statements.statement(statement) else {
            panic!("statement should lower to a table transition");
        };
        let TransitionTargetNode::Named {
            path, arguments, ..
        } = statements.transition_target(transition.target)
        else {
            panic!("transition target should be named");
        };

        assert_eq!(path.members.count(), 1);
        assert_eq!(path.symbol, target_symbol);
        assert_eq!(arguments.count(), 2);
        assert_eq!(expressions.expression_count(), 3);
    }
}
