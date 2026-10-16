use crate::name::Identifier;
use crate::typed_trees::remapped;
use arena::{Arena, Handle, HandleSpan};
use source::SourceSpan;
use symbols::SymbolHandle;
mod assembly;
mod control_graph;
mod payloads;
pub use assembly::{AssemblyBlock, AssemblyControlFlow, AssemblyControlPoint, AssemblyExit};
pub use control_graph::{
    StatementControlBlock, StatementControlExit, StatementControlGraph, StatementExecutionIter,
    StatementExecutionOrder,
};

pub type StatementHandle = Handle<StatementNode>;

/// The executable prefix of one ordered transition tail. The first
/// unconditional arm selects its destination, so later alternatives cannot
/// execute. This is structural reachability, not a proof that a conditional
/// guard is true. Preserve the authored statements and their ordinals for
/// checking; consumers borrow this prefix only when planning/replaying exits.
/// Statements before that departure stay in order, including mixed prefixes.
/// Later source statements remain in the authored table for type, effect and
/// loan admission; executable planning must not manufacture blocks for them.
pub fn reachable_transition_prefix(statements: &[StatementNode]) -> &[StatementNode] {
    match statements.iter().position(|statement| {
        matches!(statement, StatementNode::Transition(transition)
            if transition.guard == TransitionGuardNode::Always)
    }) {
        Some(ordinal) => &statements[..=ordinal],
        None => statements,
    }
}

/// A state's ordinary body followed by its executable transition tail. Keep
/// authored ordinals unchanged and trim only statements after an explicit
/// unconditional departure. Mixed statements before it are preserved, so this
/// query never disguises unsupported reachable sequencing.
pub fn reachable_state_statements(statements: &[StatementNode]) -> &[StatementNode] {
    let Some(start) = statements
        .iter()
        .position(|statement| matches!(statement, StatementNode::Transition(_)))
    else {
        return statements;
    };
    let tail = reachable_transition_prefix(&statements[start..]);
    &statements[..start + tail.len()]
}

/// A Build declaration with product-context operands, never a runtime call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBinding {
    pub receiver: crate::expression::ExpressionHandle,
    pub slot: Box<[Identifier]>,
    pub implementation: Box<[Identifier]>,
    /// Present only when the implementation operand is a delegated
    /// product-description place: the bare name resolved to a local,
    /// parameter, or field. Static product declaration paths keep this
    /// invalid and are rejoined lexically from `implementation`.
    pub implementation_operand: crate::expression::ExpressionHandle,
    pub source_span: SourceSpan,
}
pub type TransitionTargetHandle = Handle<TransitionTargetNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementTable {
    assembly_graphs: Arena<AssemblyControlFlow>,
    assembly_blocks: Arena<AssemblyBlock>,
    statements: Arena<StatementNode>,
    expression_handles: Arena<crate::expression::ExpressionHandle>,
    name_path_members: Arena<Identifier>,
    transition_targets: Arena<TransitionTargetNode>,
    outcome_proof_selectors: Arena<OutcomeProofSelector>,
}

impl StatementTable {
    pub(crate) fn retains_exact_prefix(&self, base: &Self) -> bool {
        let Self {
            assembly_graphs,
            assembly_blocks,
            statements,
            expression_handles,
            name_path_members,
            transition_targets,
            outcome_proof_selectors,
        } = base;
        crate::typed_trees::retained_prefix::arena_is_exact_prefix(
            assembly_graphs,
            &self.assembly_graphs,
        ) && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
            assembly_blocks,
            &self.assembly_blocks,
        ) && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
            statements,
            &self.statements,
        ) && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
            expression_handles,
            &self.expression_handles,
        ) && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
            name_path_members,
            &self.name_path_members,
        ) && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
            transition_targets,
            &self.transition_targets,
        ) && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
            outcome_proof_selectors,
            &self.outcome_proof_selectors,
        )
    }

    pub fn new() -> Self {
        Self {
            assembly_graphs: Arena::new(),
            assembly_blocks: Arena::new(),
            statements: Arena::new(),
            expression_handles: Arena::new(),
            name_path_members: Arena::new(),
            transition_targets: Arena::new(),
            outcome_proof_selectors: Arena::new(),
        }
    }

    pub fn insert(&mut self, statement: StatementNode) -> StatementHandle {
        self.statements.append(statement)
    }

    pub fn insert_statements(
        &mut self,
        statements: impl IntoIterator<Item = StatementNode>,
    ) -> HandleSpan<StatementNode> {
        self.statements.insert_many(statements)
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
        *self
            .expression_handles
            .get_mut(expressions.handle_at(offset)) = expression;
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

    pub fn insert_outcome_proof_selectors(
        &mut self,
        selectors: impl IntoIterator<Item = OutcomeProofSelector>,
    ) -> HandleSpan<OutcomeProofSelector> {
        self.outcome_proof_selectors.insert_many(selectors)
    }

    pub fn outcome_proof_selectors(
        &self,
        span: HandleSpan<OutcomeProofSelector>,
    ) -> &[OutcomeProofSelector] {
        self.outcome_proof_selectors.span_or_empty(span)
    }

    pub(super) fn copy_transition_target_deep_from(
        &mut self,
        source: &StatementTable,
        source_expressions: &crate::expression::ExpressionTable,
        target_expressions: &mut crate::expression::ExpressionTable,
        target: TransitionTargetHandle,
    ) -> TransitionTargetHandle {
        if !target.is_valid() {
            return TransitionTargetHandle::invalid();
        }
        let target = match source.transition_target(target) {
            TransitionTargetNode::Named {
                path,
                static_machine_parameter,
                arguments,
                evidence_arguments,
                source_span,
                authored_call_selection,
                operational_acknowledgement,
            } => {
                let members = self
                    .name_path_members
                    .insert_many(source.name_path_members(path.members).iter().cloned());
                let arguments = self.expression_handles.insert_many(
                    source
                        .expression_handles(*arguments)
                        .iter()
                        .map(|argument| {
                            target_expressions.copy_from(source_expressions, *argument)
                        }),
                );
                TransitionTargetNode::Named {
                    static_machine_parameter: *static_machine_parameter,
                    path: TableNamePath {
                        members,
                        head_symbol: path.head_symbol,
                        symbol: path.symbol,
                    },
                    arguments,
                    evidence_arguments: evidence_arguments.clone(),
                    source_span: *source_span,
                    authored_call_selection: *authored_call_selection,
                    operational_acknowledgement: *operational_acknowledgement,
                }
            }
            TransitionTargetNode::Value(value) => TransitionTargetNode::Value(
                target_expressions.copy_from(source_expressions, *value),
            ),
            TransitionTargetNode::Terminal => TransitionTargetNode::Terminal,
        };
        self.insert_transition_target(target)
    }

    pub(super) fn remap_transition_target_symbols(
        &mut self,
        target: TransitionTargetHandle,
        expressions: &mut crate::expression::ExpressionTable,
        symbols: &[(SymbolHandle, SymbolHandle)],
    ) {
        if !target.is_valid() {
            return;
        }
        let snapshot = self.transition_target(target).clone();
        match snapshot {
            TransitionTargetNode::Named {
                path, arguments, ..
            } => {
                for argument in self.expression_handles(arguments).to_vec() {
                    expressions.remap_symbols_in(argument, symbols);
                }
                let TransitionTargetNode::Named { path: current, .. } =
                    self.transition_targets.get_mut(target)
                else {
                    unreachable!();
                };
                current.head_symbol = remapped(path.head_symbol, symbols);
                current.symbol = remapped(path.symbol, symbols);
            }
            TransitionTargetNode::Value(value) => {
                expressions.remap_symbols_in(value, symbols);
            }
            TransitionTargetNode::Terminal => {}
        }
    }

    pub fn statement(&self, handle: StatementHandle) -> &StatementNode {
        self.statements.get(handle)
    }

    pub fn statement_mut(&mut self, handle: StatementHandle) -> &mut StatementNode {
        self.statements.get_mut(handle)
    }

    #[inline]
    pub fn statements(&self, span: HandleSpan<StatementNode>) -> &[StatementNode] {
        self.statements.span_or_empty(span)
    }

    /// The state's statements paired with their index inside the state's
    /// node span — the shape every per-statement walk consumes.
    pub fn indexed_statements(
        &self,
        span: HandleSpan<StatementNode>,
    ) -> impl Iterator<Item = (usize, &StatementNode)> {
        self.statements(span).iter().enumerate()
    }

    /// The `index`th statement inside `state`'s statement list, if present.
    pub fn statement_at(
        &self,
        state: &crate::state::State,
        index: usize,
    ) -> Option<&StatementNode> {
        self.statements(state.statement_nodes).get(index)
    }

    pub fn iter_statements(
        &self,
        span: HandleSpan<StatementNode>,
    ) -> impl Iterator<Item = (StatementHandle, &StatementNode)> {
        span.handles()
            .map(|handle| (handle, self.statements.get(handle)))
    }

    /// Mutable span access for deterministic pre-check normalization passes.
    /// Handles and statement order remain unchanged.
    pub fn statements_mut(&mut self, span: HandleSpan<StatementNode>) -> &mut [StatementNode] {
        self.statements.span_mut_or_empty(span)
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

    /// The named-target path and argument span, or `None` for value and
    /// terminal targets.
    pub fn named_transition_target(
        &self,
        handle: TransitionTargetHandle,
    ) -> Option<(
        &TableNamePath,
        HandleSpan<crate::expression::ExpressionHandle>,
    )> {
        match self.transition_target(handle) {
            TransitionTargetNode::Named {
                path, arguments, ..
            } => Some((path, *arguments)),
            _ => None,
        }
    }

    pub fn transition_target_mut(
        &mut self,
        handle: TransitionTargetHandle,
    ) -> &mut TransitionTargetNode {
        self.transition_targets.get_mut(handle)
    }

    /// Whether the exact generational handle names a live transition target.
    /// A source terminal must not be confused with the arena's dummy terminal.
    pub fn transition_target_is_valid(&self, handle: TransitionTargetHandle) -> bool {
        self.transition_targets.is_valid(handle)
    }

    pub fn statement_count(&self) -> usize {
        self.statements.len()
    }

    /// The owned freshness counter over every arena the table owns. Each
    /// arena's revision only ever moves forward, so their sum moves on any
    /// mutation — including same-length rewrites through `remap_symbols_in` —
    /// and a cache keyed on it can never be stale.
    pub fn revision(&self) -> u64 {
        self.statements
            .revision()
            .wrapping_add(self.expression_handles.revision())
            .wrapping_add(self.name_path_members.revision())
            .wrapping_add(self.transition_targets.revision())
            .wrapping_add(self.outcome_proof_selectors.revision())
    }

    /// Every transition target reachable from a statement span.
    pub fn transition_targets(
        &self,
        span: HandleSpan<StatementNode>,
    ) -> impl Iterator<Item = TransitionTargetHandle> + '_ {
        self.statements(span)
            .iter()
            .filter_map(StatementNode::as_transition)
            .flat_map(TableTransition::targets)
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

/// The part of a statement an expression root fills
/// ([`StatementTable::expression_roots`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementRootRole {
    BindingReceiver,
    BindingImplementation,
    AssemblyFact,
    AssignmentTarget,
    AssignmentValue,
    CallArgument,
    Expression,
    LocalInitializer,
    TransitionGuard,
    TransitionArgument,
    TransitionValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementNode {
    Assembly(AssemblyControlPoint),
    RootBinding(RootBinding),
    AssemblyFact(TableAssemblyFact),
    Assignment(TableAssignment),
    Call(TableCall),
    Expression(crate::expression::ExpressionHandle),
    LocalData(TableLocalData),
    Transition(TableTransition),
}

impl StatementNode {
    /// The `RootBinding` record, if this statement is one.
    pub fn as_root_binding(&self) -> Option<&RootBinding> {
        let StatementNode::RootBinding(binding) = self else {
            return None;
        };
        Some(binding)
    }

    /// The `AssemblyFact` record, if this statement is one.
    pub fn as_assembly_fact(&self) -> Option<&TableAssemblyFact> {
        let StatementNode::AssemblyFact(fact) = self else {
            return None;
        };
        Some(fact)
    }

    /// The `Assignment` record, if this statement is one.
    pub fn as_assignment(&self) -> Option<&TableAssignment> {
        let StatementNode::Assignment(assignment) = self else {
            return None;
        };
        Some(assignment)
    }

    /// The `Call` record, if this statement is one.
    pub fn as_call(&self) -> Option<&TableCall> {
        let StatementNode::Call(call) = self else {
            return None;
        };
        Some(call)
    }

    /// The expression handle, if this statement is a bare expression.
    pub fn as_expression(&self) -> Option<&crate::expression::ExpressionHandle> {
        let StatementNode::Expression(expression) = self else {
            return None;
        };
        Some(expression)
    }

    /// The `LocalData` record, if this statement is one.
    pub fn as_local_data(&self) -> Option<&TableLocalData> {
        let StatementNode::LocalData(local) = self else {
            return None;
        };
        Some(local)
    }

    /// The `Transition` record, if this statement is one.
    pub fn as_transition(&self) -> Option<&TableTransition> {
        let StatementNode::Transition(transition) = self else {
            return None;
        };
        Some(transition)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableAssemblyFact {
    pub kind: AssemblyFactKind,
    pub fact: Handle<crate::domain::ProofFact>,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableAssignment {
    pub target: crate::expression::ExpressionHandle,
    pub value: crate::expression::ExpressionHandle,
    /// Complete receiving value type, prepared after specialization. A range
    /// receives an array of its selected width, not its backing array or a
    /// borrowed view. This is a construction destination, never evidence that
    /// the source value satisfies it. Copied bodies prepare their own instance.
    /// Rewriting either operand requires preparation again: the RHS determines
    /// whether a reference assignment replaces its descriptor or its referent.
    pub receiving_type: crate::types::TypeReferenceHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableCall {
    /// Zero until specialization erases this call's authored static operands.
    pub authored_machine_application: crate::static_applications::StaticMachineApplicationHandle,
    /// Exact current-state storage or namespace root; `receiver_symbol` is
    /// the final projected member and need not descend from this declaration.
    pub receiver_root_symbol: SymbolHandle,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    /// Original static binder contract, independent of the selected target.
    /// Zero denotes an ordinary call without a specialization contract.
    pub static_machine_parameter: SymbolHandle,
    pub receiver: HandleSpan<Identifier>,
    pub target: Identifier,
    /// Public requirement identity plus private closed realization retained
    /// when static conformance dispatch rewrites `target_symbol`.
    pub static_requirement_dispatch: Option<crate::typed_trees::StaticRequirementDispatch>,
    pub machine_arguments: Box<[crate::expression::StaticMachineArgument]>,
    pub arguments: HandleSpan<crate::expression::ExpressionHandle>,
    pub evidence_arguments: Box<[Identifier]>,
    pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
    /// `_ = call();` -- the caller explicitly discards a non-unit result.
    pub discards_result: bool,
    /// Exact authored call-target span. Compiler-generated calls retain the
    /// default span rather than inventing source provenance.
    pub source_span: SourceSpan,
    pub authored_call_selection:
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableLocalData {
    pub role: language_core::LocalBindingRole,
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub initial_value: crate::expression::ExpressionHandle,
    /// `let mut` -- see the syntax-tree twin.
    pub is_mutable: bool,
    /// This binding has no authored annotation; its type is compiler-derived.
    /// Retained even while an untyped generated binding awaits inference.
    /// Specialization may refresh it; an authored annotation remains fixed.
    pub type_is_inferred: bool,
    /// Normalization captured an operand. Materialization joins its exact symbol
    /// use to the live consumer before choosing value or reference storage;
    /// this is still an ordinary value binding for checking and execution.
    pub is_operand_capture: bool,
    /// Authored `[erased]` on the binding occurrence -- see the parameter twin.
    pub relevance: language_core::BindingRelevance,
}

impl Default for TableLocalData {
    fn default() -> Self {
        Self {
            role: language_core::LocalBindingRole::Value,
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            type_reference: crate::types::TypeReferenceHandle::invalid(),
            initial_value: crate::expression::ExpressionHandle::invalid(),
            is_mutable: false,
            type_is_inferred: false,
            is_operand_capture: false,
            relevance: language_core::BindingRelevance::Relevant,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableTransition {
    pub target: TransitionTargetHandle,
    pub continuation: TransitionTargetHandle,
    pub guard: TransitionGuardNode,
    pub proof_selectors: HandleSpan<OutcomeProofSelector>,
    pub exit: TransitionExit,
    pub source_span: SourceSpan,
}

impl TableTransition {
    /// Both named target handles, skipping absent (zero) slots.
    pub fn targets(&self) -> impl Iterator<Item = TransitionTargetHandle> + use<> {
        [self.target, self.continuation]
            .into_iter()
            .filter(|handle| handle.is_valid())
    }
}

impl Default for TableTransition {
    fn default() -> Self {
        Self {
            target: TransitionTargetHandle::invalid(),
            continuation: TransitionTargetHandle::invalid(),
            guard: TransitionGuardNode::Always,
            proof_selectors: HandleSpan::empty(),
            exit: TransitionExit::Ordinary,
            source_span: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutcomeProofSelector {
    pub output_field: Identifier,
    pub binding: Identifier,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TransitionExit {
    #[default]
    Ordinary,
    Crash(crate::signature::CrashCause),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionGuardNode {
    Always,
    When(crate::expression::ExpressionHandle),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TransitionTargetNode {
    Named {
        /// Call admission metadata; target identity determines invocation versus jump.
        operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
        path: TableNamePath,
        static_machine_parameter: SymbolHandle,
        arguments: HandleSpan<crate::expression::ExpressionHandle>,
        evidence_arguments: Box<[Identifier]>,
        /// Exact authored target-name span. Generated targets retain the
        /// default span rather than inventing source provenance.
        source_span: SourceSpan,
        authored_call_selection: Option<
            language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        >,
    },
    Value(crate::expression::ExpressionHandle),
    #[default]
    Terminal,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableNamePath {
    pub members: HandleSpan<Identifier>,
    pub head_symbol: SymbolHandle,
    pub symbol: SymbolHandle,
}
