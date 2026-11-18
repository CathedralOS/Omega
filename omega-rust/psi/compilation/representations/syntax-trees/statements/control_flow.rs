use crate::assembly::AssemblyTable;
pub use crate::assembly::{
    AssemblyBlock, AssemblyControlFlow, AssemblyExit, AssemblyInstruction, AssemblyOperands,
    AuthoredAssembly, AuthoredAssemblyBlock,
};
use crate::expression::ExpressionHandle;
use crate::identifier::Identifier;
use crate::paged_roster::paged_roster;
use arena::{Handle, HandleSpan};
use source::SourceSpan;

pub type StatementHandle = Handle<StatementNode>;

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct RootBinding {
    pub receiver: ExpressionHandle,
    pub slot: Box<[Identifier]>,
    pub implementation: Box<[Identifier]>,
    pub implementation_operand: ExpressionHandle,
    pub source_span: SourceSpan,
}
pub type TransitionTargetHandle = Handle<TransitionTargetNode>;

paged_roster! {
    pub struct StatementTable => pub(crate) StatementTableAppendCheckpoint {
        arena statements: StatementNode,
        arena expression_handles: ExpressionHandle,
        arena identifier_path_members: Identifier,
        arena transition_targets: TransitionTargetNode,
        arena transition_patterns: TransitionPattern,
        arena dispatches: TableDispatch,
        arena outcome_proof_selectors: TableOutcomeProofSelector,
        roster assembly: AssemblyTable => crate::assembly::AssemblyTableAppendCheckpoint,
    }
}

impl StatementTable {
    pub fn insert_dispatch(&mut self, dispatch: TableDispatch) -> Handle<TableDispatch> {
        self.dispatches.insert(dispatch)
    }

    pub fn dispatch(&self, handle: Handle<TableDispatch>) -> &TableDispatch {
        self.dispatches.get(handle)
    }

    pub fn has_dispatches(&self) -> bool {
        !self.dispatches.is_empty()
    }

    pub fn insert_transition_patterns(
        &mut self,
        patterns: impl IntoIterator<Item = TransitionPattern>,
    ) -> HandleSpan<TransitionPattern> {
        self.transition_patterns.insert_many(patterns)
    }

    pub fn transition_patterns(
        &self,
        patterns: HandleSpan<TransitionPattern>,
    ) -> &[TransitionPattern] {
        self.transition_patterns.span_or_empty(patterns)
    }

    pub fn transition_pattern_mut(
        &mut self,
        handle: Handle<TransitionPattern>,
    ) -> &mut TransitionPattern {
        self.transition_patterns.get_mut(handle)
    }

    pub fn new() -> Self {
        Self::new_with_generation(1)
    }

    pub fn insert(&mut self, statement: StatementNode) -> StatementHandle {
        self.statements.append(statement)
    }

    pub fn insert_authored_assembly(
        &mut self,
        graph: AuthoredAssembly,
    ) -> Handle<AuthoredAssembly> {
        self.assembly.insert_authored(graph)
    }
    pub fn authored_assembly(&self, handle: Handle<AuthoredAssembly>) -> &AuthoredAssembly {
        self.assembly.authored(handle)
    }
    pub fn has_authored_assemblies(&self) -> bool {
        self.assembly.has_authored()
    }

    pub fn insert_assembly(&mut self, graph: AssemblyControlFlow) -> Handle<AssemblyControlFlow> {
        self.assembly.insert_graph(graph)
    }

    pub fn assembly(&self, handle: Handle<AssemblyControlFlow>) -> &AssemblyControlFlow {
        self.assembly.graph(handle)
    }

    pub fn insert_assembly_blocks(
        &mut self,
        blocks: impl IntoIterator<Item = AssemblyBlock>,
    ) -> HandleSpan<AssemblyBlock> {
        self.assembly.insert_blocks(blocks)
    }

    pub fn assembly_blocks(&self, span: HandleSpan<AssemblyBlock>) -> &[AssemblyBlock] {
        self.assembly.blocks(span)
    }

    pub fn insert_assembly_instructions(
        &mut self,
        instructions: impl IntoIterator<Item = StatementHandle>,
    ) -> HandleSpan<StatementHandle> {
        self.assembly.insert_instructions(instructions)
    }

    pub fn assembly_instructions(&self, span: HandleSpan<StatementHandle>) -> &[StatementHandle] {
        self.assembly.instructions(span)
    }

    pub fn insert_assembly_expressions(
        &mut self,
        expressions: impl IntoIterator<Item = ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        self.assembly.insert_expressions(expressions)
    }

    pub fn assembly_expressions(&self, span: HandleSpan<ExpressionHandle>) -> &[ExpressionHandle] {
        self.assembly.expressions(span)
    }

    pub fn append_expression_handle(
        &mut self,
        expression: ExpressionHandle,
    ) -> Handle<ExpressionHandle> {
        self.expression_handles.append(expression)
    }

    pub fn insert_expression_handles(
        &mut self,
        expressions: impl IntoIterator<Item = ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
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

    pub fn replace_statement(&mut self, handle: StatementHandle, statement: StatementNode) {
        *self.statements.get_mut(handle) = statement;
    }

    pub fn statement(&self, handle: StatementHandle) -> &StatementNode {
        self.statements.get(handle)
    }

    pub fn expression_handles(&self, span: HandleSpan<ExpressionHandle>) -> &[ExpressionHandle] {
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

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum StatementNode {
    Assembly(Handle<AssemblyControlFlow>),
    RootBinding(RootBinding),
    AssemblyFact(TableAssemblyFact),
    Assignment(TableAssignment),
    Call(TableCall),
    ProofOutputBindingStatement(TableProofOutputBindingStatement),
    Expression(ExpressionHandle),
    LocalData(TableLocalData),
    Transition(TableTransition),
    Dispatch(Handle<TableDispatch>),
    AuthoredAssembly(Handle<AuthoredAssembly>),
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct TableProofOutputBindingStatement {
    pub bindings: Box<[TableProofOutputSelector]>,
    pub call: ExpressionHandle,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct TableProofOutputSelector {
    pub output_field: Identifier,
    pub binding: Identifier,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct TableOutcomeProofSelector {
    pub output_field: Identifier,
    pub binding: Identifier,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct TableAssemblyFact {
    pub kind: AssemblyFactKind,
    pub expression: ExpressionHandle,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record, syntax_trees_derive::SyntaxRelocate,
)]
pub enum AssemblyFactKind {
    Requires,
    Ensures,
}

impl Default for StatementNode {
    fn default() -> Self {
        Self::Expression(ExpressionHandle::invalid())
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct TableAssignment {
    pub target: ExpressionHandle,
    pub value: ExpressionHandle,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct TableCall {
    pub callee: ExpressionHandle,
    pub receiver: HandleSpan<Identifier>,
    pub target: Identifier,
    pub target_is_static: bool,
    pub machine_arguments: Box<[crate::expression::StaticMachineArgument]>,
    pub arguments: HandleSpan<ExpressionHandle>,
    pub evidence_arguments: Box<[Identifier]>,
    pub operational_acknowledgement: language_core::CallOperationalAcknowledgement,
    pub discards_result: bool,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct TableLocalData {
    pub role: language_core::LocalBindingRole,
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub initial_value: ExpressionHandle,
    pub is_mutable: bool,
    pub relevance: language_core::BindingRelevance,
    /// The fields a record pattern names, in authored order, each
    /// bound or waived. Empty on every local that is not a
    /// `LocalBindingRole::RecordPattern` marker.
    pub pattern_fields: HandleSpan<crate::expression::TableMatchField>,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct TransitionPattern {
    pub is_structural: bool,
    pub subject: ExpressionHandle,
    pub capture: Handle<crate::expression::MatchSubjectCapture>,
    pub pattern: crate::expression::TableDestructurePattern,
}

/// A single authored dispatch. Each patterned arm has one slot per subject;
/// wildcards keep their position. A bare `_` or subjectless guard has no slots.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct TableDispatch {
    pub subjects_span: Option<SourceSpan>,
    pub subjects: HandleSpan<ExpressionHandle>,
    pub arms: Box<[DispatchArm]>,
    pub source_span: SourceSpan,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct DispatchPattern {
    pub pattern: crate::expression::MatchPattern,
    /// A destructuring binder may refer to this subject from its guard or target.
    pub capture: Handle<crate::expression::MatchSubjectCapture>,
}

impl TableDispatch {
    /// Visit each source expression of the dispatch once at its authored position.
    pub fn visit_operands(
        &self,
        trees: &crate::SyntaxTrees,
        mut visit: impl FnMut(ExpressionHandle),
    ) {
        for &subject in trees.statements.expression_handles(self.subjects) {
            visit(subject);
        }
        for arm in &self.arms {
            for slot in &arm.patterns {
                match slot.pattern {
                    crate::expression::MatchPattern::Wildcard => {}
                    crate::expression::MatchPattern::Value(value) => visit(value),
                    crate::expression::MatchPattern::Destructure(pattern) => {
                        visit(pattern.head);
                        for field in trees.expressions.match_fields(pattern.fields) {
                            if let crate::expression::MatchFieldAction::Equal(value) = field.action
                            {
                                visit(value);
                            }
                        }
                        if pattern.guard.is_valid() {
                            visit(pattern.guard);
                        }
                    }
                }
            }
            if let TransitionGuardNode::When(guard) = arm.guard {
                visit(guard);
            }
            match trees.statements.transition_target(arm.target) {
                TransitionTargetNode::Named { arguments, .. } => {
                    for &argument in trees.statements.expression_handles(*arguments) {
                        visit(argument);
                    }
                }
                TransitionTargetNode::Value(value) => visit(*value),
                TransitionTargetNode::Terminal => {}
            }
        }
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, syntax_trees_derive::SyntaxRelocate, compiler_storage::Record,
)]
pub struct DispatchArm {
    pub patterns: Box<[DispatchPattern]>,
    pub guard: TransitionGuardNode,
    pub target: TransitionTargetHandle,
    pub source_span: SourceSpan,
}

impl DispatchPattern {
    pub fn proof_selectors(&self) -> HandleSpan<TableOutcomeProofSelector> {
        match self.pattern {
            crate::expression::MatchPattern::Destructure(pattern) => pattern.proof_selectors,
            _ => HandleSpan::empty(),
        }
    }
}

impl DispatchArm {
    /// Only a single exact result-case pattern can carry proof selectors.
    pub fn proof_selectors(&self) -> HandleSpan<TableOutcomeProofSelector> {
        match self.patterns.as_ref() {
            [pattern] => pattern.proof_selectors(),
            _ => HandleSpan::empty(),
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct TableTransition {
    pub patterns: HandleSpan<TransitionPattern>,
    pub target: TransitionTargetHandle,
    pub continuation: TransitionTargetHandle,
    pub guard: TransitionGuardNode,
    pub proof_selectors: HandleSpan<TableOutcomeProofSelector>,
    pub exit: TransitionExit,
    pub ends_dispatch: bool,
    pub source_span: SourceSpan,
}

impl TableTransition {
    pub fn targets(&self) -> impl Iterator<Item = TransitionTargetHandle> + use<> {
        [self.target, self.continuation]
            .into_iter()
            .filter(|target| target.is_valid())
    }
}

impl Default for TableTransition {
    fn default() -> Self {
        Self {
            patterns: HandleSpan::empty(),
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

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    compiler_storage::Record,
    syntax_trees_derive::SyntaxRelocate,
)]
pub enum TransitionExit {
    #[default]
    Ordinary,
    Crash(crate::item::CrashCause),
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record, syntax_trees_derive::SyntaxRelocate,
)]
pub enum TransitionGuardNode {
    Always,
    When(ExpressionHandle),
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum TransitionTargetNode {
    Named {
        operational_acknowledgement: language_core::CallOperationalAcknowledgement,
        path: HandleSpan<Identifier>,
        arguments: HandleSpan<ExpressionHandle>,
        evidence_arguments: Box<[Identifier]>,
        source_span: SourceSpan,
    },
    Value(ExpressionHandle),
    #[default]
    Terminal,
}

#[cfg(test)]
#[path = "../tests/unit/statement_tests.rs"]
mod tests;
