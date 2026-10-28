use crate::name::DiagnosticName;
use arena::{Arena, Handle, HandleSpan};
use source::SourceSpan;
use std::ops::{Deref, DerefMut};
use symbols::SymbolHandle;

pub type StatementHandle = Handle<StatementNode>;

/// A Build declaration with product-context operands, never a runtime call.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct RootBinding {
    pub receiver: crate::expression::ExpressionHandle,
    pub slot: Box<[DiagnosticName]>,
    pub implementation: Box<[DiagnosticName]>,
    /// Present only while the implementation operand remains a delegated
    /// product-description candidate: symbol routing keeps it when the bare
    /// name resolves to a place (local, parameter, or field) and clears it
    /// when the name resolves to a product declaration path.
    pub implementation_operand: crate::expression::ExpressionHandle,
    pub source_span: SourceSpan,
}
pub type TransitionTargetHandle = Handle<TransitionTargetNode>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuthoredSelectionStatementStore {
    Statements,
    TransitionTargets,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum Statement {
    Assembly(Handle<AssemblyControlFlow>),
    RootBinding(RootBinding),
    AssemblyFact(AssemblyFact),
    Assignment(Assignment),
    Call(Call),
    ProofOutputBindingStatement(ProofOutputBindingStatement),
    Expression(crate::expression::ExpressionHandle),
    LocalData(LocalData),
    Transition(Transition),
}

/// One source assembly scope, retained until typed elaboration can close its
/// captured value frontier. Local edges bind block handles, never names.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct AssemblyControlFlow {
    /// The intersection of instruction and register-view catalog demands.
    pub target_requirement: language_core::inline_assembly::AsmTargetApplicability,
    /// Catalog-generated arithmetic and ZF operations: their selected meaning
    /// must remain builtin, independently of authored token overloads.
    pub primitives: HandleSpan<crate::expression::ExpressionHandle>,
    /// Explicit body-local register/flag identities; empty cells are absent.
    pub scratch: [crate::expression::ExpressionHandle;
        language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT],
    pub blocks: HandleSpan<AssemblyBlock>,
}
impl Default for AssemblyControlFlow {
    fn default() -> Self {
        Self {
            target_requirement: Default::default(),
            primitives: Default::default(),
            scratch: [crate::expression::ExpressionHandle::invalid();
                language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT],
            blocks: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct AssemblyBlock {
    pub label: DiagnosticName,
    pub requires: HandleSpan<crate::expression::ExpressionHandle>,
    pub ensures: HandleSpan<crate::expression::ExpressionHandle>,
    pub instructions: HandleSpan<Statement>,
    /// Normalized together with the enclosing state's ordinary statements.
    pub statement_nodes: HandleSpan<StatementNode>,
    pub exit: AssemblyExit,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum AssemblyExit {
    #[default]
    Fallthrough,
    Jump(Handle<AssemblyBlock>),
    /// Local destination and required ZF polarity (true for set, false for clear).
    ZeroFlag(Handle<AssemblyBlock>, bool),
    EnclosingTransition,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ProofOutputBindingStatement {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub bindings: Box<[ProofOutputSelector]>,
    pub call: crate::expression::ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ProofOutputSelector {
    pub output_field: DiagnosticName,
    pub binding: DiagnosticName,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct OutcomeProofSelector {
    pub output_field: DiagnosticName,
    pub binding: DiagnosticName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct AssemblyFact {
    pub kind: AssemblyFactKind,
    pub expression: crate::expression::ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum AssemblyFactKind {
    Requires,
    Ensures,
}

impl Default for Statement {
    fn default() -> Self {
        Self::Expression(Handle::invalid())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct Assignment {
    pub target: crate::expression::ExpressionHandle,
    pub value: crate::expression::ExpressionHandle,
}

/// An erased named-`ensures` assignment classified out of the runtime
/// statement stream. Names remain diagnostic at this phase; owner symbols and
/// an explicitly named producer implementation are bound after
/// ordinary symbol assignment.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
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
    /// resolves to a named implementation. Incoming evidence forwarding
    /// leaves this empty and is bound by the checked proof pass.
    pub source_conformance: Option<SymbolHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct LocalData {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub storage: LocalDataStorage,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
pub struct LocalDataStorage {
    pub role: language_core::LocalBindingRole,
    pub type_reference: crate::types::TypeReference,
    pub initial_value: crate::expression::ExpressionHandle,
    /// `let mut` -- see the syntax-tree twin.
    pub is_mutable: bool,
    /// No authored type annotation; typing must infer this generated binding.
    pub type_is_inferred: bool,
    /// A generated operand snapshot; its consumer determines value versus reference capture.
    pub is_operand_capture: bool,
    /// Authored `[erased]` on the binding occurrence -- see the syntax twin.
    pub relevance: language_core::BindingRelevance,
}

deref_to_storage!(LocalData => LocalDataStorage);

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct Call {
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub target: DiagnosticName,
    pub storage: CallStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CallStorage {
    /// Exact storage/namespace root, distinct from the final receiver member.
    pub receiver_root_symbol: SymbolHandle,
    pub receiver: HandleSpan<DiagnosticName>,
    pub machine_arguments: Box<[crate::expression::StaticMachineArgument]>,
    pub arguments: HandleSpan<crate::expression::ExpressionHandle>,
    pub evidence_arguments: Box<[DiagnosticName]>,
    pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
    /// `_ = call();` -- the caller explicitly discards a non-unit result.
    pub discards_result: bool,
    /// Exact ledger occurrence for this authored statement call. Generated
    /// calls retain `None`.
    pub authored_call_selection:
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId>,
}

deref_to_storage!(Call => CallStorage);

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct Transition {
    pub target: TransitionTarget,
    pub continuation: Option<TransitionTarget>,
    pub guard: TransitionGuard,
    pub proof_selectors: Box<[OutcomeProofSelector]>,
    pub exit: TransitionExit,
    /// Final arm of one authored dispatch; standalone transitions also end a dispatch.
    /// Coverage must not borrow an outcome from a physically adjacent source block.
    pub ends_dispatch: bool,
    pub source_span: SourceSpan,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum TransitionExit {
    #[default]
    Ordinary,
    Crash(crate::signature::CrashCause),
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum TransitionGuard {
    Always,
    When(crate::expression::ExpressionHandle),
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum TransitionTarget {
    Named(NamedTransitionTarget),
    Value(crate::expression::ExpressionHandle),
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct NamedTransitionTarget {
    pub receiver_symbol: SymbolHandle,
    pub head_symbol: SymbolHandle,
    pub symbol: SymbolHandle,
    pub storage: NamedTransitionTargetStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct NamedTransitionTargetStorage {
    /// Authored call markers survive identity-based recursive tail normalization.
    pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
    pub path: HandleSpan<DiagnosticName>,
    pub arguments: HandleSpan<crate::expression::ExpressionHandle>,
    pub evidence_arguments: Box<[DiagnosticName]>,
    pub source_span: SourceSpan,
    pub authored_call_selection:
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId>,
}

deref_to_storage!(NamedTransitionTarget => NamedTransitionTargetStorage);

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct StatementTable {
    nodes: StatementNodeStorage,
    paths: StatementPathStorage,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
struct StatementNodeStorage {
    statements: Arena<StatementNode>,
    transition_targets: Arena<TransitionTargetNode>,
    outcome_proof_selectors: Arena<TableOutcomeProofSelector>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
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
                outcome_proof_selectors: Arena::new(),
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
        *self
            .paths
            .expression_handles
            .get_mut(expressions.handle_at(offset)) = expression;
    }

    pub fn name_path_members(&self, span: HandleSpan<DiagnosticName>) -> &[DiagnosticName] {
        self.paths.name_path_members.span_or_empty(span)
    }

    pub fn transition_target(&self, handle: TransitionTargetHandle) -> &TransitionTargetNode {
        self.nodes.transition_targets.get(handle)
    }

    pub fn insert_transition_target(
        &mut self,
        target: TransitionTargetNode,
    ) -> TransitionTargetHandle {
        self.nodes.transition_targets.insert(target)
    }

    pub fn transition_target_mut(
        &mut self,
        handle: TransitionTargetHandle,
    ) -> &mut TransitionTargetNode {
        self.nodes.transition_targets.get_mut(handle)
    }

    pub fn outcome_proof_selectors(
        &self,
        span: HandleSpan<TableOutcomeProofSelector>,
    ) -> &[TableOutcomeProofSelector] {
        self.nodes.outcome_proof_selectors.span_or_empty(span)
    }

    pub fn statement_count(&self) -> usize {
        self.nodes.statements.len()
    }

    pub fn transition_target_count(&self) -> usize {
        self.nodes.transition_targets.len()
    }

    pub(crate) fn validate_authored_selection_rebase(
        &self,
        statement_frontier: usize,
        transition_target_frontier: usize,
        rebase: language_semantics::declaration_selection::AuthoredDeclarationSelectionSuffixRebase,
    ) -> Result<(), AuthoredSelectionStatementStore> {
        if statement_frontier > self.statement_count() {
            return Err(AuthoredSelectionStatementStore::Statements);
        }
        if transition_target_frontier > self.transition_target_count() {
            return Err(AuthoredSelectionStatementStore::TransitionTargets);
        }

        for (index, (_, statement)) in self.nodes.statements.iter().enumerate() {
            let StatementNode::Call(call) = statement else {
                continue;
            };
            let Some(occurrence) = call.authored_call_selection else {
                continue;
            };
            let valid = if index < statement_frontier {
                rebase.retain_base(occurrence)
            } else {
                rebase.rebase_appended(occurrence)
            };
            if valid.is_none() {
                return Err(AuthoredSelectionStatementStore::Statements);
            }
        }
        for (index, (_, target)) in self.nodes.transition_targets.iter().enumerate() {
            let TransitionTargetNode::Named {
                authored_call_selection: Some(occurrence),
                ..
            } = target
            else {
                continue;
            };
            let valid = if index < transition_target_frontier {
                rebase.retain_base(*occurrence)
            } else {
                rebase.rebase_appended(*occurrence)
            };
            if valid.is_none() {
                return Err(AuthoredSelectionStatementStore::TransitionTargets);
            }
        }

        Ok(())
    }

    /// The enclosing tree transaction validated both stores before mutation.
    pub(crate) fn apply_authored_selection_rebase(
        &mut self,
        statement_frontier: usize,
        transition_target_frontier: usize,
        rebase: language_semantics::declaration_selection::AuthoredDeclarationSelectionSuffixRebase,
    ) {
        self.nodes.statements.for_each_mut(|handle, statement| {
            let StatementNode::Call(call) = statement else {
                return;
            };
            let Some(occurrence) = call.authored_call_selection else {
                return;
            };
            if usize::try_from(handle.arena_index()).expect("statement index overflow")
                > statement_frontier
            {
                call.authored_call_selection = Some(
                    rebase
                        .rebase_appended(occurrence)
                        .expect("statement occurrence was validated before mutation"),
                );
            }
        });
        self.nodes
            .transition_targets
            .for_each_mut(|handle, target| {
                let TransitionTargetNode::Named {
                    authored_call_selection: Some(occurrence),
                    ..
                } = target
                else {
                    return;
                };
                if usize::try_from(handle.arena_index()).expect("transition target index overflow")
                    > transition_target_frontier
                {
                    *occurrence = rebase
                        .rebase_appended(*occurrence)
                        .expect("extension occurrences were validated before mutation");
                }
            });
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
        self.insert_tree_from_source(
            statement,
            &StatementTransplant {
                source_expressions: crate::expression::ExpressionSource::Table(source_expressions),
                source_expression_table: crate::expression::ExpressionSource::Table(
                    source_expression_table,
                ),
                source_child_type_references,
                source_constraints,
                source_statement_path_members,
                copy_expression_handles,
            },
            expressions,
            type_references,
        )
    }

    pub(crate) fn insert_tree_from_source(
        &mut self,
        statement: &Statement,
        transplant: &StatementTransplant<'_>,
        expressions: &mut crate::expression::ExpressionTable,
        type_references: &mut crate::types::TypeReferenceTable,
    ) -> StatementHandle {
        match statement {
            Statement::Assembly(handle) => self.insert(StatementNode::Assembly(*handle)),
            Statement::RootBinding(binding) => {
                let receiver = transplant.expression(expressions, binding.receiver);
                let implementation_operand = if binding.implementation_operand.is_valid() {
                    transplant.expression(expressions, binding.implementation_operand)
                } else {
                    crate::expression::ExpressionHandle::invalid()
                };
                self.insert(StatementNode::RootBinding(RootBinding {
                    receiver,
                    implementation_operand,
                    ..binding.clone()
                }))
            }
            Statement::AssemblyFact(fact) => {
                let expression = transplant.expression(expressions, fact.expression);
                self.insert(StatementNode::AssemblyFact(TableAssemblyFact {
                    kind: fact.kind,
                    expression,
                }))
            }
            Statement::Assignment(assignment) => {
                let target = transplant.expression(expressions, assignment.target);
                let value = transplant.expression(expressions, assignment.value);
                self.insert(StatementNode::Assignment(TableAssignment { target, value }))
            }
            Statement::Call(call) => {
                let arguments =
                    self.transplant_expression_span(transplant, call.arguments, expressions);
                let receiver = self.insert_name_path_members(
                    transplant
                        .source_statement_path_members
                        .span_or_empty(call.receiver),
                );
                self.insert(StatementNode::Call(TableCall {
                    receiver_root_symbol: call.receiver_root_symbol,
                    receiver_symbol: call.receiver_symbol,
                    target_symbol: call.target_symbol,
                    receiver,
                    target: call.target.clone(),
                    machine_arguments: call.machine_arguments.clone(),
                    arguments,
                    evidence_arguments: call.evidence_arguments.clone(),
                    operational_acknowledgement: call.operational_acknowledgement,
                    discards_result: call.discards_result,
                    authored_call_selection: call.authored_call_selection,
                }))
            }
            Statement::ProofOutputBindingStatement(package) => {
                let call = transplant.expression(expressions, package.call);
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
                let expression = transplant.expression(expressions, *expression);
                self.insert(StatementNode::Expression(expression))
            }
            Statement::LocalData(local_data) => {
                let type_reference = transplant.type_reference(
                    type_references,
                    expressions,
                    &local_data.type_reference,
                );
                let initial_value = if local_data.initial_value.is_valid() {
                    transplant.expression(expressions, local_data.initial_value)
                } else {
                    crate::expression::ExpressionHandle::invalid()
                };
                self.insert(StatementNode::LocalData(TableLocalData {
                    role: local_data.role,
                    symbol: local_data.symbol,
                    name: local_data.name.clone(),
                    type_reference,
                    initial_value,
                    is_mutable: local_data.is_mutable,
                    type_is_inferred: local_data.type_is_inferred,
                    is_operand_capture: local_data.is_operand_capture,
                    relevance: local_data.relevance,
                }))
            }
            Statement::Transition(transition) => {
                let target =
                    self.insert_transition_target_tree(&transition.target, transplant, expressions);
                let continuation = transition
                    .continuation
                    .as_ref()
                    .map(|target| {
                        self.insert_transition_target_tree(target, transplant, expressions)
                    })
                    .unwrap_or_else(TransitionTargetHandle::invalid);
                let guard = match &transition.guard {
                    TransitionGuard::Always => TransitionGuardNode::Always,
                    TransitionGuard::When(expression) => {
                        TransitionGuardNode::When(transplant.expression(expressions, *expression))
                    }
                };
                let proof_selectors = self.nodes.outcome_proof_selectors.insert_many(
                    transition
                        .proof_selectors
                        .iter()
                        .map(|selector| TableOutcomeProofSelector {
                            output_field: selector.output_field.clone(),
                            binding: selector.binding.clone(),
                        }),
                );
                self.insert(StatementNode::Transition(TableTransition {
                    target,
                    continuation,
                    guard,
                    proof_selectors,
                    exit: transition.exit,
                    ends_dispatch: transition.ends_dispatch,
                    source_span: transition.source_span,
                }))
            }
        }
    }

    fn transplant_expression_span(
        &mut self,
        transplant: &StatementTransplant<'_>,
        arguments: HandleSpan<crate::expression::ExpressionHandle>,
        expressions: &mut crate::expression::ExpressionTable,
    ) -> HandleSpan<crate::expression::ExpressionHandle> {
        let count = transplant
            .source_expressions
            .table(expressions)
            .expression_handles(arguments)
            .len();
        let span = self.reserve_expression_handles(
            count
                .try_into()
                .expect("expression handle span count overflow"),
        );

        for offset in 0..count {
            let argument = transplant
                .source_expressions
                .table(expressions)
                .expression_handles(arguments)[offset];
            let argument = transplant.expression(expressions, argument);
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

    pub fn insert_name_path_members(
        &mut self,
        path: &[DiagnosticName],
    ) -> HandleSpan<DiagnosticName> {
        self.paths
            .name_path_members
            .insert_many(path.iter().cloned())
    }

    fn insert_transition_target_tree(
        &mut self,
        target: &TransitionTarget,
        transplant: &StatementTransplant<'_>,
        expressions: &mut crate::expression::ExpressionTable,
    ) -> TransitionTargetHandle {
        let target = match target {
            TransitionTarget::Named(named) => TransitionTargetNode::Named {
                operational_acknowledgement: named.operational_acknowledgement,
                path: TableNamePath {
                    members: self.insert_name_path_members(
                        transplant
                            .source_statement_path_members
                            .span_or_empty(named.path),
                    ),
                    receiver_symbol: named.receiver_symbol,
                    head_symbol: named.head_symbol,
                    symbol: named.symbol,
                },
                arguments: self.transplant_expression_span(
                    transplant,
                    named.arguments,
                    expressions,
                ),
                evidence_arguments: named.evidence_arguments.clone(),
                source_span: named.source_span,
                authored_call_selection: named.authored_call_selection,
            },
            TransitionTarget::Value(expression) => {
                TransitionTargetNode::Value(transplant.expression(expressions, *expression))
            }
            TransitionTarget::Terminal => TransitionTargetNode::Terminal,
        };

        self.insert_transition_target(target)
    }
}

/// The source coordinates and destination policy of one tree-to-table statement
/// transplant: which table a copied expression reads, which expression table
/// type payloads copy from, the source type-reference and constraint rows, the
/// source path-member roster, and whether expression handles are copied at all.
/// The walker used to thread these as separate parameters through every arm and
/// every nested call.
pub(crate) struct StatementTransplant<'tree> {
    pub(crate) source_expressions: crate::expression::ExpressionSource<'tree>,
    pub(crate) source_expression_table: crate::expression::ExpressionSource<'tree>,
    pub(crate) source_child_type_references: &'tree Arena<crate::types::TypeReference>,
    pub(crate) source_constraints: &'tree Arena<crate::types::TypeConstraint>,
    pub(crate) source_statement_path_members: &'tree Arena<DiagnosticName>,
    pub(crate) copy_expression_handles: bool,
}

impl StatementTransplant<'_> {
    /// One expression handle: copy its subtree beside the original, or keep the
    /// handle as the caller already owns the rows.
    fn expression(
        &self,
        expressions: &mut crate::expression::ExpressionTable,
        expression: crate::expression::ExpressionHandle,
    ) -> crate::expression::ExpressionHandle {
        if self.copy_expression_handles {
            self.source_expressions.copy_into(expressions, expression)
        } else {
            expression
        }
    }

    fn type_reference(
        &self,
        type_references: &mut crate::types::TypeReferenceTable,
        expressions: &mut crate::expression::ExpressionTable,
        type_reference: &crate::types::TypeReference,
    ) -> crate::types::TypeReferenceHandle {
        type_references.insert_tree_from_source(
            type_reference,
            expressions,
            self.source_child_type_references,
            self.source_constraints,
            self.source_expression_table,
        )
    }
}

impl Default for StatementTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum StatementNode {
    Assembly(Handle<AssemblyControlFlow>),
    RootBinding(RootBinding),
    AssemblyFact(TableAssemblyFact),
    Assignment(TableAssignment),
    Call(TableCall),
    ProofOutputBindingStatement(ProofOutputBindingStatement),
    Expression(crate::expression::ExpressionHandle),
    LocalData(TableLocalData),
    Transition(TableTransition),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct TableAssemblyFact {
    pub kind: AssemblyFactKind,
    pub expression: crate::expression::ExpressionHandle,
}

impl Default for StatementNode {
    fn default() -> Self {
        Self::Expression(crate::expression::ExpressionHandle::invalid())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct TableAssignment {
    pub target: crate::expression::ExpressionHandle,
    pub value: crate::expression::ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
pub struct TableCall {
    pub receiver_root_symbol: SymbolHandle,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub receiver: HandleSpan<DiagnosticName>,
    pub target: DiagnosticName,
    pub machine_arguments: Box<[crate::expression::StaticMachineArgument]>,
    pub arguments: HandleSpan<crate::expression::ExpressionHandle>,
    pub evidence_arguments: Box<[DiagnosticName]>,
    pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
    /// `_ = call();` -- the caller explicitly discards a non-unit result.
    pub discards_result: bool,
    pub authored_call_selection:
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
pub struct TableLocalData {
    pub role: language_core::LocalBindingRole,
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub initial_value: crate::expression::ExpressionHandle,
    /// `let mut` -- see the syntax-tree twin.
    pub is_mutable: bool,
    /// Retained inference origin, independent of the binding's name.
    pub type_is_inferred: bool,
    /// A generated operand snapshot; its consumer determines value versus reference capture.
    pub is_operand_capture: bool,
    /// Authored `[erased]` on the binding occurrence -- see the syntax twin.
    pub relevance: language_core::BindingRelevance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct TableTransition {
    pub target: TransitionTargetHandle,
    pub continuation: TransitionTargetHandle,
    pub guard: TransitionGuardNode,
    pub proof_selectors: HandleSpan<TableOutcomeProofSelector>,
    pub exit: TransitionExit,
    /// Final arm of one authored dispatch; standalone transitions also end a dispatch.
    /// Coverage must not borrow an outcome from a physically adjacent source block.
    pub ends_dispatch: bool,
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
            ends_dispatch: true,
            source_span: SourceSpan::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TableOutcomeProofSelector {
    pub output_field: DiagnosticName,
    pub binding: DiagnosticName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum TransitionGuardNode {
    Always,
    When(crate::expression::ExpressionHandle),
}

#[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
pub enum TransitionTargetNode {
    Named {
        operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
        path: TableNamePath,
        arguments: HandleSpan<crate::expression::ExpressionHandle>,
        evidence_arguments: Box<[DiagnosticName]>,
        source_span: SourceSpan,
        authored_call_selection: Option<
            language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        >,
    },
    Value(crate::expression::ExpressionHandle),
    #[default]
    Terminal,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TableNamePath {
    /// Exact endpoint of the qualified prefix; zero for an unqualified target.
    pub receiver_symbol: SymbolHandle,
    pub members: HandleSpan<DiagnosticName>,
    pub head_symbol: SymbolHandle,
    pub symbol: SymbolHandle,
}

#[cfg(test)]
mod tests {
    use super::{
        NamedTransitionTarget, NamedTransitionTargetStorage, Statement, StatementNode,
        StatementTable, TableCall, Transition, TransitionGuard, TransitionTarget,
        TransitionTargetNode,
    };
    use crate::expression::{ExpressionNode, ExpressionTable};
    use crate::name::DiagnosticName;
    use crate::types::TypeReferenceTable;
    use arena::{Arena, Handle};
    use symbols::SymbolHandle;

    fn selection_ledger(
        start: usize,
        symbol: u32,
    ) -> language_semantics::declaration_selection::AuthoredDeclarationSelections {
        let mut ledger =
            language_semantics::declaration_selection::AuthoredDeclarationSelections::default();
        ledger
            .record_resolved(
                source::SourceSpan::new(
                    source::SourceId(1),
                    source::Span::new(start, start + 1),
                ),
                language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
                language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call,
                SymbolHandle::from_arena_index(symbol),
            )
            .expect("selection row");
        ledger
    }

    #[test]
    fn authored_selection_rebase_covers_folded_calls_and_named_transition_targets() {
        let mut combined = selection_ledger(1, 2);
        let extension = combined
            .record_resolved(
                source::SourceSpan::new(
                    source::SourceId(1),
                    source::Span::new(3, 4),
                ),
                language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
                language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call,
                SymbolHandle::from_arena_index(3),
            )
            .expect("extension selection");
        let mut destination = selection_ledger(1, 2);
        destination
            .record_resolved(
                source::SourceSpan::new(
                    source::SourceId(1),
                    source::Span::new(2, 3),
                ),
                language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
                language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::MemberAccess,
                SymbolHandle::from_arena_index(4),
            )
            .expect("later-phase row");
        let (_, rebase) = combined
            .replace_prefix_and_rebase_suffix(1, &destination)
            .expect("compatible prefix");
        let shifted = rebase
            .rebase_extension(extension)
            .expect("extension mapping");

        let mut table = StatementTable::new();
        table
            .nodes
            .statements
            .insert(StatementNode::Call(TableCall {
                authored_call_selection: Some(extension),
                ..Default::default()
            }));
        let target = table
            .nodes
            .transition_targets
            .insert(TransitionTargetNode::Named {
                operational_acknowledgement: Default::default(),
                path: Default::default(),
                arguments: Default::default(),
                evidence_arguments: Default::default(),
                source_span: Default::default(),
                authored_call_selection: Some(extension),
            });

        table
            .validate_authored_selection_rebase(0, 0, rebase)
            .expect("extension-owned folded stores");
        table.apply_authored_selection_rebase(0, 0, rebase);

        let StatementNode::Call(call) = table.statement(Handle::from_arena_index(1)) else {
            panic!("folded call")
        };
        assert_eq!(call.authored_call_selection, Some(shifted));
        let TransitionTargetNode::Named {
            authored_call_selection,
            ..
        } = table.transition_target(target)
        else {
            panic!("folded named transition target")
        };
        assert_eq!(*authored_call_selection, Some(shifted));
    }

    #[test]
    fn statement_table_stores_transition_payloads_as_handles() {
        let target_symbol = SymbolHandle::from_arena_index(7);
        let mut source_expressions = ExpressionTable::new();
        let mut arguments = arena::HandleSpan::empty();
        let first_argument = source_expressions.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(1),
        ));
        source_expressions.push_expression_handle(&mut arguments, first_argument);
        let second_argument = source_expressions.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(2),
        ));
        source_expressions.push_expression_handle(&mut arguments, second_argument);
        let guard = source_expressions.insert(ExpressionNode::Boolean(true));
        let mut source_statement_path_members = Arena::new();
        let path = source_statement_path_members.insert_many([DiagnosticName::generated("next")]);
        let statement = Statement::Transition(Transition {
            target: TransitionTarget::Named(NamedTransitionTarget {
                receiver_symbol: Default::default(),
                head_symbol: target_symbol,
                symbol: target_symbol,
                storage: NamedTransitionTargetStorage {
                    operational_acknowledgement: Default::default(),
                    path,
                    arguments,
                    evidence_arguments: Box::default(),
                    source_span: Default::default(),
                    authored_call_selection: None,
                },
            }),
            continuation: None,
            guard: TransitionGuard::When(guard),
            proof_selectors: Box::default(),
            exit: Default::default(),
            ends_dispatch: true,
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
