use super::assembly::AssemblyTable;
pub use super::assembly::{AssemblyBlock, AssemblyControlFlow, AssemblyExit};
use crate::identifier::Identifier;
use arena::{Handle, HandleSpan, PagedArenaSet as Arena};
use source::SourceSpan;

pub type StatementHandle = Handle<StatementNode>;

/// A Build declaration with product-context operands, never a runtime call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBinding {
    pub receiver: crate::expression::ExpressionHandle,
    pub slot: Box<[Identifier]>,
    pub implementation: Box<[Identifier]>,
    /// Present only when the implementation operand is one bare name: a
    /// delegated product-description place candidate. Product declaration
    /// paths (every multi-member spelling) keep this invalid and stay purely
    /// lexical; name resolution decides which form the operand is.
    pub implementation_operand: crate::expression::ExpressionHandle,
    pub source_span: SourceSpan,
}
pub type TransitionTargetHandle = Handle<TransitionTargetNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementTable {
    assembly: AssemblyTable,
    statements: Arena<StatementNode>,
    expression_handles: Arena<crate::expression::ExpressionHandle>,
    identifier_path_members: Arena<Identifier>,
    transition_targets: Arena<TransitionTargetNode>,
    outcome_proof_selectors: Arena<TableOutcomeProofSelector>,
}

impl StatementTable {
    pub fn new() -> Self {
        Self::new_with_generation(1)
    }

    pub fn new_with_generation(generation: u32) -> Self {
        Self {
            assembly: AssemblyTable::new(generation),
            statements: Arena::with_generation(generation),
            expression_handles: Arena::with_generation(generation),
            identifier_path_members: Arena::with_generation(generation),
            transition_targets: Arena::with_generation(generation),
            outcome_proof_selectors: Arena::with_generation(generation),
        }
    }

    pub(crate) fn adopt(&mut self, other: Self) {
        self.assembly.adopt(other.assembly);
        self.statements.adopt(other.statements);
        self.expression_handles.adopt(other.expression_handles);
        self.identifier_path_members
            .adopt(other.identifier_path_members);
        self.transition_targets.adopt(other.transition_targets);
        self.outcome_proof_selectors
            .adopt(other.outcome_proof_selectors);
    }

    pub(crate) fn begin_page(&mut self, generation: u32) {
        self.assembly.begin_page(generation);
        self.statements.begin_page(generation);
        self.expression_handles.begin_page(generation);
        self.identifier_path_members.begin_page(generation);
        self.transition_targets.begin_page(generation);
        self.outcome_proof_selectors.begin_page(generation);
    }

    pub fn insert(&mut self, statement: StatementNode) -> StatementHandle {
        self.statements.append(statement)
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
        expressions: impl IntoIterator<Item = crate::expression::ExpressionHandle>,
    ) -> HandleSpan<crate::expression::ExpressionHandle> {
        self.assembly.insert_expressions(expressions)
    }

    pub fn assembly_expressions(
        &self,
        span: HandleSpan<crate::expression::ExpressionHandle>,
    ) -> &[crate::expression::ExpressionHandle] {
        self.assembly.expressions(span)
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
    Assembly(Handle<AssemblyControlFlow>),
    RootBinding(RootBinding),
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableCall {
    pub receiver: HandleSpan<Identifier>,
    pub target: Identifier,
    /// Preserves static `::` selection through expression-to-statement conversion.
    pub target_is_static: bool,
    pub machine_arguments: Box<[crate::expression::StaticMachineArgument]>,
    pub arguments: HandleSpan<crate::expression::ExpressionHandle>,
    /// Explicit erased evidence-term arguments after the `;` call lane.
    pub evidence_arguments: Box<[Identifier]>,
    pub operational_acknowledgement: language_core::CallOperationalAcknowledgement,
    /// `_ = call();` -- the caller explicitly discards a non-unit result.
    pub discards_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableLocalData {
    pub role: language_core::LocalBindingRole,
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub initial_value: crate::expression::ExpressionHandle,
    /// `let mut` -- the local admits reassignment (ch3/ch14 spelling); a
    /// plain `let` is immutable and reassignment refuses.
    pub is_mutable: bool,
    /// `let name [erased]: T` -- a proof-side local with no runtime storage;
    /// the same binding-occurrence marker as an erased data field.
    pub relevance: language_core::BindingRelevance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl TableTransition {
    /// The transition's valid destination handles in authored order —
    /// `target` then `continuation`.
    pub fn targets(&self) -> impl Iterator<Item = TransitionTargetHandle> + use<> {
        [self.target, self.continuation]
            .into_iter()
            .filter(|target| target.is_valid())
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TransitionTargetNode {
    Named {
        path: HandleSpan<Identifier>,
        arguments: HandleSpan<crate::expression::ExpressionHandle>,
        evidence_arguments: Box<[Identifier]>,
        /// Exact authored target-name span retained even when the parser
        /// rewrites a terminal tail call into a named transition.
        source_span: SourceSpan,
    },
    Value(crate::expression::ExpressionHandle),
    #[default]
    Terminal,
}

#[cfg(test)]
mod tests {
    use super::{
        StatementNode, StatementTable, TableTransition, TransitionGuardNode, TransitionTargetNode,
    };
    use crate::expression::{ExpressionNode, ExpressionTable};
    use crate::identifier::Identifier;
    use arena::HandleSpan;

    #[test]
    fn statement_table_stores_transition_payloads_as_handles() {
        let mut statements = StatementTable::new();
        let mut expressions = ExpressionTable::new();
        let path_start = statements.append_identifier_path_member(Identifier::generated("next"));
        let path = HandleSpan::from_parts(path_start, 1);
        let argument_one = expressions.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(1),
        ));
        let argument_one = statements.append_expression_handle(argument_one);
        let argument_two = expressions.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(2),
        ));
        let _argument_two = statements.append_expression_handle(argument_two);
        let arguments = HandleSpan::from_parts(argument_one, 2);
        let target = statements.insert_transition_target(TransitionTargetNode::Named {
            path,
            arguments,
            evidence_arguments: Box::default(),
            source_span: Default::default(),
        });
        let guard = expressions.insert(ExpressionNode::Boolean(true));
        let statement = statements.insert(StatementNode::Transition(TableTransition {
            target,
            continuation: super::TransitionTargetHandle::invalid(),
            guard: TransitionGuardNode::When(guard),
            proof_selectors: HandleSpan::empty(),
            exit: Default::default(),
            ends_dispatch: true,
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
