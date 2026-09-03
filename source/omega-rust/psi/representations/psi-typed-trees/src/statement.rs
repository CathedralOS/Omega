use crate::name::Identifier;
use psi_arena::{Arena, Handle, HandleSpan};
use psi_source::SourceSpan;
use psi_symbols::SymbolHandle;

pub type StatementHandle = Handle<StatementNode>;
pub type TransitionTargetHandle = Handle<TransitionTargetNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementTable {
    statements: Arena<StatementNode>,
    expression_handles: Arena<crate::expression::ExpressionHandle>,
    name_path_members: Arena<Identifier>,
    transition_targets: Arena<TransitionTargetNode>,
    outcome_proof_selectors: Arena<OutcomeProofSelector>,
}

impl StatementTable {
    pub fn new() -> Self {
        Self {
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

    /// Deep-copy a statement span and every table-owned payload reachable
    /// from it. Symbols are deliberately preserved: callers that clone a
    /// lexical scope mint fresh symbols first, then remap those identities in
    /// the copied graph. Keeping identity remapping out of this table primitive
    /// also makes it useful for source-to-source expansion within one scope.
    pub fn copy_statement_nodes_deep_from(
        &mut self,
        source: &StatementTable,
        source_expressions: &crate::expression::ExpressionTable,
        target_expressions: &mut crate::expression::ExpressionTable,
        source_types: &crate::types::TypeReferenceTable,
        target_types: &mut crate::types::TypeReferenceTable,
        statements: HandleSpan<StatementNode>,
    ) -> HandleSpan<StatementNode> {
        let mut copied = HandleSpan::empty();

        for statement in source.statements(statements) {
            let statement = match statement {
                StatementNode::AssemblyFact(fact) => {
                    StatementNode::AssemblyFact(TableAssemblyFact {
                        kind: fact.kind,
                        expression: target_expressions
                            .copy_from(source_expressions, fact.expression),
                    })
                }
                StatementNode::Assignment(assignment) => {
                    StatementNode::Assignment(TableAssignment {
                        target: target_expressions.copy_from(source_expressions, assignment.target),
                        value: target_expressions.copy_from(source_expressions, assignment.value),
                    })
                }
                StatementNode::Call(call) => {
                    let receiver = self
                        .name_path_members
                        .insert_many(source.name_path_members(call.receiver).iter().cloned());
                    let arguments = call
                        .arguments
                        .is_empty()
                        .then(HandleSpan::empty)
                        .unwrap_or_else(|| {
                            self.expression_handles.insert_many(
                                source
                                    .expression_handles(call.arguments)
                                    .iter()
                                    .map(|argument| {
                                        target_expressions.copy_from(source_expressions, *argument)
                                    }),
                            )
                        });
                    StatementNode::Call(TableCall {
                        receiver_symbol: call.receiver_symbol,
                        target_symbol: call.target_symbol,
                        receiver,
                        target: call.target.clone(),
                        static_requirement_dispatch: call.static_requirement_dispatch.clone(),
                        machine_arguments: call.machine_arguments.clone(),
                        arguments,
                        evidence_arguments: call.evidence_arguments.clone(),
                        operational_acknowledgement: call.operational_acknowledgement,
                        discards_result: call.discards_result,
                        source_span: call.source_span,
                        authored_call_selection: call.authored_call_selection,
                    })
                }
                StatementNode::Expression(expression) => StatementNode::Expression(
                    target_expressions.copy_from(source_expressions, *expression),
                ),
                StatementNode::LocalData(local) => {
                    let type_reference = local.type_reference.is_valid().then(|| {
                        target_types.copy_from(
                            source_types,
                            source_expressions,
                            target_expressions,
                            local.type_reference,
                        )
                    });
                    let initial_value = local.initial_value.is_valid().then(|| {
                        target_expressions.copy_from(source_expressions, local.initial_value)
                    });
                    StatementNode::LocalData(TableLocalData {
                        symbol: local.symbol,
                        name: local.name.clone(),
                        type_reference: type_reference
                            .unwrap_or_else(crate::types::TypeReferenceHandle::invalid),
                        initial_value: initial_value
                            .unwrap_or_else(crate::expression::ExpressionHandle::invalid),
                        is_mutable: local.is_mutable,
                    })
                }
                StatementNode::Transition(transition) => {
                    let target = self.copy_transition_target_deep_from(
                        source,
                        source_expressions,
                        target_expressions,
                        transition.target,
                    );
                    let continuation = self.copy_transition_target_deep_from(
                        source,
                        source_expressions,
                        target_expressions,
                        transition.continuation,
                    );
                    let guard = match transition.guard {
                        TransitionGuardNode::Always => TransitionGuardNode::Always,
                        TransitionGuardNode::When(guard) => TransitionGuardNode::When(
                            target_expressions.copy_from(source_expressions, guard),
                        ),
                    };
                    StatementNode::Transition(TableTransition {
                        target,
                        continuation,
                        guard,
                        proof_selectors: self.outcome_proof_selectors.insert_many(
                            source
                                .outcome_proof_selectors(transition.proof_selectors)
                                .iter()
                                .cloned(),
                        ),
                        exit: transition.exit,
                        source_span: transition.source_span,
                    })
                }
            };
            self.push_statement(&mut copied, statement);
        }

        copied
    }

    fn copy_transition_target_deep_from(
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
                arguments,
                evidence_arguments,
                source_span,
                authored_call_selection,
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
                    path: TableNamePath {
                        members,
                        head_symbol: path.head_symbol,
                        symbol: path.symbol,
                    },
                    arguments,
                    evidence_arguments: evidence_arguments.clone(),
                    source_span: *source_span,
                    authored_call_selection: *authored_call_selection,
                }
            }
            TransitionTargetNode::Value(value) => TransitionTargetNode::Value(
                target_expressions.copy_from(source_expressions, *value),
            ),
            TransitionTargetNode::SelfTarget => TransitionTargetNode::SelfTarget,
            TransitionTargetNode::Terminal => TransitionTargetNode::Terminal,
        };
        self.insert_transition_target(target)
    }

    /// Remap lexical symbols in a copied statement graph. Expression and type
    /// payloads are delegated to their owning tables so no arena internals
    /// escape this representation layer.
    pub fn remap_symbols_in(
        &mut self,
        statements: HandleSpan<StatementNode>,
        expressions: &mut crate::expression::ExpressionTable,
        types: &mut crate::types::TypeReferenceTable,
        symbols: &[(SymbolHandle, SymbolHandle)],
    ) {
        let statement_handles: Vec<_> = (0..statements.count())
            .map(|offset| {
                Handle::from_parts(
                    statements.start().arena_index() + offset,
                    statements.start().generation(),
                )
            })
            .collect();
        for handle in statement_handles {
            let statement = self.statement(handle).clone();
            match statement {
                StatementNode::AssemblyFact(fact) => {
                    expressions.remap_symbols_in(fact.expression, symbols);
                }
                StatementNode::Assignment(assignment) => {
                    expressions.remap_symbols_in(assignment.target, symbols);
                    expressions.remap_symbols_in(assignment.value, symbols);
                }
                StatementNode::Call(call) => {
                    for argument in self.expression_handles(call.arguments).to_vec() {
                        expressions.remap_symbols_in(argument, symbols);
                    }
                    let StatementNode::Call(current) = self.statements.get_mut(handle) else {
                        unreachable!();
                    };
                    current.receiver_symbol = remapped(current.receiver_symbol, symbols);
                    current.target_symbol = remapped(current.target_symbol, symbols);
                    for argument in &mut current.machine_arguments {
                        argument.symbol = remapped(argument.symbol, symbols);
                    }
                }
                StatementNode::Expression(expression) => {
                    expressions.remap_symbols_in(expression, symbols);
                }
                StatementNode::LocalData(local) => {
                    types.remap_symbols_in(local.type_reference, expressions, symbols);
                    expressions.remap_symbols_in(local.initial_value, symbols);
                    let StatementNode::LocalData(current) = self.statements.get_mut(handle) else {
                        unreachable!();
                    };
                    current.symbol = remapped(current.symbol, symbols);
                }
                StatementNode::Transition(transition) => {
                    self.remap_transition_target_symbols(transition.target, expressions, symbols);
                    self.remap_transition_target_symbols(
                        transition.continuation,
                        expressions,
                        symbols,
                    );
                    if let TransitionGuardNode::When(guard) = transition.guard {
                        expressions.remap_symbols_in(guard, symbols);
                    }
                }
            }
        }
    }

    fn remap_transition_target_symbols(
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
            TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
        }
    }

    pub fn statement(&self, handle: StatementHandle) -> &StatementNode {
        self.statements.get(handle)
    }

    pub fn statement_mut(&mut self, handle: StatementHandle) -> &mut StatementNode {
        self.statements.get_mut(handle)
    }

    pub fn statements(&self, span: HandleSpan<StatementNode>) -> &[StatementNode] {
        self.statements.span_or_empty(span)
    }

    pub fn iter_statements(
        &self,
        span: HandleSpan<StatementNode>,
    ) -> impl Iterator<Item = (StatementHandle, &StatementNode)> {
        (0..span.count()).map(move |offset| {
            let handle = Handle::from_parts(
                span.start()
                    .arena_index()
                    .checked_add(offset)
                    .expect("statement span handle overflow"),
                span.start().generation(),
            );
            (handle, self.statements.get(handle))
        })
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

fn remapped(symbol: SymbolHandle, symbols: &[(SymbolHandle, SymbolHandle)]) -> SymbolHandle {
    symbols
        .iter()
        .find_map(|(source, target)| (*source == symbol).then_some(*target))
        .unwrap_or(symbol)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementNode {
    AssemblyFact(TableAssemblyFact),
    Assignment(TableAssignment),
    Call(TableCall),
    Expression(crate::expression::ExpressionHandle),
    LocalData(TableLocalData),
    Transition(TableTransition),
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
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub receiver: HandleSpan<Identifier>,
    pub target: Identifier,
    /// Public requirement identity plus private closed realization retained
    /// when static conformance dispatch rewrites `target_symbol`.
    pub static_requirement_dispatch: Option<crate::typed_trees::StaticRequirementDispatch>,
    pub machine_arguments: Box<[crate::expression::StaticMachineArgument]>,
    pub arguments: HandleSpan<crate::expression::ExpressionHandle>,
    pub evidence_arguments: Box<[Identifier]>,
    pub operational_acknowledgement: psi_language_semantics::CallOperationalAcknowledgement,
    /// `_ = call();` -- the caller explicitly discards a non-unit result.
    pub discards_result: bool,
    /// Exact authored call-target span. Compiler-generated calls retain the
    /// default span rather than inventing source provenance.
    pub source_span: SourceSpan,
    pub authored_call_selection: Option<
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
    >,
}

impl Default for TableCall {
    fn default() -> Self {
        Self {
            receiver_symbol: SymbolHandle::invalid(),
            target_symbol: SymbolHandle::invalid(),
            receiver: HandleSpan::empty(),
            target: Identifier::default(),
            static_requirement_dispatch: None,
            machine_arguments: Box::default(),
            arguments: HandleSpan::empty(),
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
            discards_result: false,
            source_span: Default::default(),
            authored_call_selection: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableLocalData {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub initial_value: crate::expression::ExpressionHandle,
    /// `let mut` -- see the syntax-tree twin.
    pub is_mutable: bool,
}

impl Default for TableLocalData {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
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
    pub proof_selectors: HandleSpan<OutcomeProofSelector>,
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
        path: TableNamePath,
        arguments: HandleSpan<crate::expression::ExpressionHandle>,
        evidence_arguments: Box<[Identifier]>,
        /// Exact authored target-name span. Generated targets retain the
        /// default span rather than inventing source provenance.
        source_span: SourceSpan,
        authored_call_selection: Option<
            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        >,
    },
    Value(crate::expression::ExpressionHandle),
    SelfTarget,
    #[default]
    Terminal,
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
    use crate::expression::{ExpressionNode, ExpressionTable};
    use crate::name::Identifier;
    use crate::types::{TypeReferenceNode, TypeReferenceTable};
    use psi_symbols::SymbolHandle;

    #[test]
    fn statement_table_appends_handle_native_payloads_directly() {
        let target_symbol = SymbolHandle::from_arena_index(11);
        let mut statements = StatementTable::new();
        let mut expressions = ExpressionTable::new();
        let argument = expressions.insert(crate::expression::ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::from_value(99),
        ));

        let mut arguments = psi_arena::HandleSpan::empty();
        statements.push_expression_handle(&mut arguments, argument);

        let mut path = psi_arena::HandleSpan::empty();
        statements.push_name_path_member(&mut path, Identifier::generated("next"));

        let target = statements.insert_transition_target(TransitionTargetNode::Named {
            path: super::TableNamePath {
                members: path,
                head_symbol: target_symbol,
                symbol: target_symbol,
            },
            arguments,
            evidence_arguments: Box::default(),
            source_span: Default::default(),
            authored_call_selection: None,
        });

        let mut state_statements = psi_arena::HandleSpan::empty();
        let statement = statements.push_statement(
            &mut state_statements,
            StatementNode::Transition(super::TableTransition {
                target,
                continuation: super::TransitionTargetHandle::invalid(),
                guard: super::TransitionGuardNode::Always,
                proof_selectors: psi_arena::HandleSpan::empty(),
                exit: Default::default(),
                source_span: Default::default(),
            }),
        );

        assert_eq!(state_statements.count(), 1);
        assert_eq!(statements.statement_count(), 1);
        assert_eq!(statements.transition_target_count(), 1);

        let StatementNode::Transition(transition) = statements.statement(statement) else {
            panic!("statement should be transition");
        };
        let TransitionTargetNode::Named {
            path, arguments, ..
        } = statements.transition_target(transition.target)
        else {
            panic!("transition target should be named");
        };

        assert_eq!(path.symbol, target_symbol);
        assert_eq!(arguments.count(), 1);
        assert_eq!(statements.expression_handles(*arguments), &[argument]);
    }

    #[test]
    fn deep_copy_owns_nested_statement_payloads() {
        let local_symbol = SymbolHandle::from_arena_index(21);
        let target_symbol = SymbolHandle::from_arena_index(22);
        let mut source_statements = StatementTable::new();
        let mut source_expressions = ExpressionTable::new();
        let mut source_types = TypeReferenceTable::new();

        let initial = source_expressions.insert(ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::from_value(7),
        ));
        let guard = source_expressions.insert(ExpressionNode::Boolean(true));
        let local_type = source_types.insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("i32"),
        });
        let mut source_span = psi_arena::HandleSpan::empty();
        source_statements.push_statement(
            &mut source_span,
            StatementNode::LocalData(super::TableLocalData {
                symbol: local_symbol,
                name: Identifier::generated("value"),
                type_reference: local_type,
                initial_value: initial,
                is_mutable: true,
            }),
        );

        let mut members = psi_arena::HandleSpan::empty();
        source_statements.push_name_path_member(&mut members, Identifier::generated("next"));
        let mut arguments = psi_arena::HandleSpan::empty();
        source_statements.push_expression_handle(&mut arguments, initial);
        let target = source_statements.insert_transition_target(TransitionTargetNode::Named {
            path: super::TableNamePath {
                members,
                head_symbol: target_symbol,
                symbol: target_symbol,
            },
            arguments,
            evidence_arguments: Box::default(),
            source_span: Default::default(),
            authored_call_selection: None,
        });
        source_statements.push_statement(
            &mut source_span,
            StatementNode::Transition(super::TableTransition {
                target,
                continuation: super::TransitionTargetHandle::invalid(),
                guard: super::TransitionGuardNode::When(guard),
                proof_selectors: psi_arena::HandleSpan::empty(),
                exit: Default::default(),
                source_span: Default::default(),
            }),
        );
        let mut expression_members = psi_arena::HandleSpan::empty();
        source_expressions
            .push_name_path_member(&mut expression_members, Identifier::generated("value"));
        let mut expression_member_symbols = psi_arena::HandleSpan::empty();
        source_expressions
            .push_name_path_member_symbol(&mut expression_member_symbols, local_symbol);
        let local_reference =
            source_expressions.insert(ExpressionNode::Name(crate::expression::TableNamePath {
                members: expression_members,
                member_symbols: expression_member_symbols,
                head_symbol: local_symbol,
                symbol: local_symbol,
            }));
        source_statements
            .push_statement(&mut source_span, StatementNode::Expression(local_reference));

        let mut copied_statements = StatementTable::new();
        let mut copied_expressions = ExpressionTable::new();
        let mut copied_types = TypeReferenceTable::new();
        let copied_span = copied_statements.copy_statement_nodes_deep_from(
            &source_statements,
            &source_expressions,
            &mut copied_expressions,
            &source_types,
            &mut copied_types,
            source_span,
        );

        // Mutating the source tables after the copy cannot alter the clone.
        *source_expressions.expression_mut(initial) =
            ExpressionNode::Integer(psi_numerics::literals::IntegerLiteral::from_value(99));
        source_types.substitute_node(local_type, TypeReferenceNode::Unit);
        let remapped_local = SymbolHandle::from_arena_index(31);
        let remapped_target = SymbolHandle::from_arena_index(32);
        copied_statements.remap_symbols_in(
            copied_span,
            &mut copied_expressions,
            &mut copied_types,
            &[
                (local_symbol, remapped_local),
                (target_symbol, remapped_target),
            ],
        );

        let copied = copied_statements.statements(copied_span);
        let StatementNode::LocalData(local) = &copied[0] else {
            panic!("first copied statement should be local data");
        };
        assert_eq!(local.symbol, remapped_local);
        assert_eq!(copied_types.display_name(local.type_reference), "i32");
        assert_eq!(
            copied_expressions.expression(local.initial_value),
            &ExpressionNode::Integer(psi_numerics::literals::IntegerLiteral::from_value(7))
        );

        let StatementNode::Transition(transition) = &copied[1] else {
            panic!("second copied statement should be transition");
        };
        let TransitionTargetNode::Named {
            path, arguments, ..
        } = copied_statements.transition_target(transition.target)
        else {
            panic!("copied transition target should be named");
        };
        assert_eq!(path.head_symbol, remapped_target);
        assert_eq!(path.symbol, remapped_target);
        assert_eq!(
            copied_statements.name_path_members(path.members)[0].as_str(),
            "next"
        );
        let copied_argument = copied_statements.expression_handles(*arguments)[0];
        assert_eq!(
            copied_expressions.expression(copied_argument),
            &ExpressionNode::Integer(psi_numerics::literals::IntegerLiteral::from_value(7))
        );
        let super::TransitionGuardNode::When(copied_guard) = transition.guard else {
            panic!("copied transition guard should be conditional");
        };
        assert!(matches!(
            copied_expressions.expression(copied_guard),
            ExpressionNode::Boolean(true)
        ));

        let StatementNode::Expression(reference) = copied[2] else {
            panic!("third copied statement should be an expression");
        };
        let ExpressionNode::Name(path) = copied_expressions.expression(reference) else {
            panic!("copied expression should be a name path");
        };
        assert_eq!(path.head_symbol, remapped_local);
        assert_eq!(path.symbol, remapped_local);
        assert_eq!(
            copied_expressions.name_path_member_symbols(path.member_symbols),
            &[remapped_local]
        );
    }
}
