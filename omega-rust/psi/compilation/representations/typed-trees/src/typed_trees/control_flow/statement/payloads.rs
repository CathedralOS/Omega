//! Statement copying and inspection own all referenced proof, value and type tables.
use crate::domain::ProofFact;
use crate::statement::*;
use crate::typed_trees::{TypedTreeTables, remapped};
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;
impl TypedTreeTables {
    /// Deep-copy a statement span and every table-owned payload reachable
    /// from it. Symbols are deliberately preserved: callers that clone a
    /// lexical scope mint fresh symbols first, then remap those identities in
    /// the copied graph. Keeping identity remapping out of this table primitive
    /// also makes it useful for source-to-source expansion within one scope.
    pub fn copy_statement_nodes_deep_from(
        &mut self,
        source_tables: &crate::typed_trees::TypedTreeTables,
        statements: HandleSpan<StatementNode>,
    ) -> HandleSpan<StatementNode> {
        let source = &source_tables.statement_table;
        let mut copied = Vec::new();
        let mut assembly_blocks = Vec::new();

        for statement in source.statements(statements) {
            let statement = match statement {
                StatementNode::Assembly(AssemblyControlPoint::Entry(handle)) => {
                    let target = self.statement_table.copy_assembly_from(source, *handle);
                    assembly_blocks.extend(
                        source
                            .assembly(*handle)
                            .blocks
                            .handles()
                            .zip(self.statement_table.assembly(target).blocks.handles()),
                    );
                    StatementNode::Assembly(AssemblyControlPoint::Entry(target))
                }
                StatementNode::Assembly(AssemblyControlPoint::Label(handle)) => {
                    let target = assembly_blocks
                        .iter()
                        .find(|(source, _)| source == handle)
                        .map(|(_, target)| *target)
                        .expect("assembly window copying includes its owning header");
                    StatementNode::Assembly(AssemblyControlPoint::Label(target))
                }
                StatementNode::RootBinding(binding) => StatementNode::RootBinding(RootBinding {
                    receiver: self
                        .expression_table
                        .copy_from(&source_tables.expression_table, binding.receiver),
                    implementation_operand: if binding.implementation_operand.is_valid() {
                        self.expression_table.copy_from(
                            &source_tables.expression_table,
                            binding.implementation_operand,
                        )
                    } else {
                        crate::expression::ExpressionHandle::invalid()
                    },
                    ..binding.clone()
                }),
                StatementNode::AssemblyFact(fact) => {
                    StatementNode::AssemblyFact(TableAssemblyFact {
                        kind: fact.kind,
                        fact: self.copy_proof_fact_from(source_tables, fact.fact),
                    })
                }
                StatementNode::Assignment(assignment) => {
                    StatementNode::Assignment(TableAssignment {
                        target: self
                            .expression_table
                            .copy_from(&source_tables.expression_table, assignment.target),
                        value: self
                            .expression_table
                            .copy_from(&source_tables.expression_table, assignment.value),
                        receiving_type: crate::types::TypeReferenceHandle::invalid(),
                    })
                }
                StatementNode::Call(call) => {
                    let receiver = self
                        .statement_table
                        .name_path_members
                        .insert_many(source.name_path_members(call.receiver).iter().cloned());
                    let arguments = call
                        .arguments
                        .is_empty()
                        .then(HandleSpan::empty)
                        .unwrap_or_else(|| {
                            self.statement_table.expression_handles.insert_many(
                                source
                                    .expression_handles(call.arguments)
                                    .iter()
                                    .map(|argument| {
                                        self.expression_table
                                            .copy_from(&source_tables.expression_table, *argument)
                                    }),
                            )
                        });
                    let (authored_machine_application, static_requirement_dispatch) =
                        self.expression_table.copy_call_static_applications_from(
                            &source_tables.expression_table,
                            call.authored_machine_application,
                            call.static_requirement_dispatch.clone(),
                        );
                    StatementNode::Call(TableCall {
                        authored_machine_application,
                        static_machine_parameter: call.static_machine_parameter,
                        receiver_root_symbol: call.receiver_root_symbol,
                        receiver_symbol: call.receiver_symbol,
                        target_symbol: call.target_symbol,
                        receiver,
                        target: call.target.clone(),
                        static_requirement_dispatch,
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
                    self.expression_table
                        .copy_from(&source_tables.expression_table, *expression),
                ),
                StatementNode::LocalData(local) => {
                    let type_reference = local.type_reference.is_valid().then(|| {
                        self.type_reference_table.copy_from(
                            &source_tables.type_reference_table,
                            &source_tables.expression_table,
                            &mut self.expression_table,
                            local.type_reference,
                        )
                    });
                    let initial_value = local.initial_value.is_valid().then(|| {
                        self.expression_table
                            .copy_from(&source_tables.expression_table, local.initial_value)
                    });
                    StatementNode::LocalData(TableLocalData {
                        role: local.role,
                        symbol: local.symbol,
                        name: local.name.clone(),
                        type_reference: type_reference
                            .unwrap_or_else(crate::types::TypeReferenceHandle::invalid),
                        initial_value: initial_value
                            .unwrap_or_else(crate::expression::ExpressionHandle::invalid),
                        is_mutable: local.is_mutable,
                        type_is_inferred: local.type_is_inferred,
                        is_operand_capture: local.is_operand_capture,
                        relevance: local.relevance,
                    })
                }
                StatementNode::Transition(transition) => {
                    let target = self.statement_table.copy_transition_target_deep_from(
                        source,
                        &source_tables.expression_table,
                        &mut self.expression_table,
                        transition.target,
                    );
                    let continuation = self.statement_table.copy_transition_target_deep_from(
                        source,
                        &source_tables.expression_table,
                        &mut self.expression_table,
                        transition.continuation,
                    );
                    let guard = match transition.guard {
                        TransitionGuardNode::Always => TransitionGuardNode::Always,
                        TransitionGuardNode::When(guard) => TransitionGuardNode::When(
                            self.expression_table
                                .copy_from(&source_tables.expression_table, guard),
                        ),
                    };
                    StatementNode::Transition(TableTransition {
                        target,
                        continuation,
                        guard,
                        proof_selectors: self.statement_table.outcome_proof_selectors.insert_many(
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
            copied.push(statement);
        }

        self.statement_table.insert_statements(copied)
    }
    /// Remap lexical symbols in a copied statement graph. Expression and type
    /// payloads are delegated to their owning tables so no arena internals
    /// escape this representation layer.
    pub fn remap_statement_symbols(
        &mut self,
        statements: HandleSpan<StatementNode>,
        symbols: &[(SymbolHandle, SymbolHandle)],
    ) {
        let statement_handles: Vec<_> = statements.handles().collect();
        for handle in statement_handles {
            let statement = self.statement_table.statement(handle).clone();
            match statement {
                StatementNode::Assembly(_) => {} // Control metadata has no lexical value operands.
                StatementNode::RootBinding(binding) => {
                    self.expression_table
                        .remap_symbols_in(binding.receiver, symbols);
                    if binding.implementation_operand.is_valid() {
                        self.expression_table
                            .remap_symbols_in(binding.implementation_operand, symbols);
                    }
                }
                StatementNode::AssemblyFact(fact) => {
                    self.remap_proof_fact_symbols(fact.fact, symbols);
                }
                StatementNode::Assignment(assignment) => {
                    self.expression_table
                        .remap_symbols_in(assignment.target, symbols);
                    self.expression_table
                        .remap_symbols_in(assignment.value, symbols);
                    if let StatementNode::Assignment(copied) =
                        self.statement_table.statement_mut(handle)
                    {
                        copied.receiving_type = crate::types::TypeReferenceHandle::invalid();
                    }
                }
                StatementNode::Call(call) => {
                    self.expression_table.remap_call_static_applications(
                        call.authored_machine_application,
                        call.static_requirement_dispatch.as_ref(),
                        symbols,
                    );
                    for argument in self
                        .statement_table
                        .expression_handles(call.arguments)
                        .to_vec()
                    {
                        self.expression_table.remap_symbols_in(argument, symbols);
                    }
                    let StatementNode::Call(current) =
                        self.statement_table.statements.get_mut(handle)
                    else {
                        unreachable!();
                    };
                    current.receiver_symbol = remapped(current.receiver_symbol, symbols);
                    current.receiver_root_symbol = remapped(current.receiver_root_symbol, symbols);
                    current.target_symbol = remapped(current.target_symbol, symbols);
                    for argument in &mut current.machine_arguments {
                        argument.remap_symbols(symbols);
                    }
                }
                StatementNode::Expression(expression) => {
                    self.expression_table.remap_symbols_in(expression, symbols);
                }
                StatementNode::LocalData(local) => {
                    self.type_reference_table.remap_symbols_in(
                        local.type_reference,
                        &mut self.expression_table,
                        symbols,
                    );
                    self.expression_table
                        .remap_symbols_in(local.initial_value, symbols);
                    let StatementNode::LocalData(current) =
                        self.statement_table.statements.get_mut(handle)
                    else {
                        unreachable!();
                    };
                    current.symbol = remapped(current.symbol, symbols);
                }
                StatementNode::Transition(transition) => {
                    self.statement_table.remap_transition_target_symbols(
                        transition.target,
                        &mut self.expression_table,
                        symbols,
                    );
                    self.statement_table.remap_transition_target_symbols(
                        transition.continuation,
                        &mut self.expression_table,
                        symbols,
                    );
                    if let TransitionGuardNode::When(guard) = transition.guard {
                        self.expression_table.remap_symbols_in(guard, symbols);
                    }
                }
            }
        }
    }
    /// Every expression root `statement` carries, in authored order, with
    /// the part of the statement it fills. A root the statement may omit (a
    /// static binding's implementation operand, an absent initializer) keeps
    /// its invalid handle; callers filter by role and validity as they need.
    pub fn expression_roots(
        &self,
        statement: &StatementNode,
    ) -> Vec<(StatementRootRole, crate::expression::ExpressionHandle)> {
        use StatementRootRole as Role;
        let mut roots = Vec::new();
        match statement {
            StatementNode::Assembly(_) => {} // Owned instructions follow in the lexical window.
            StatementNode::RootBinding(binding) => {
                roots.push((Role::BindingReceiver, binding.receiver));
                roots.push((Role::BindingImplementation, binding.implementation_operand));
            }
            StatementNode::AssemblyFact(fact) => roots.extend(
                self.proof_fact_expression_roots(fact.fact)
                    .into_iter()
                    .map(|expression| (Role::AssemblyFact, expression)),
            ),
            StatementNode::Assignment(assignment) => {
                roots.push((Role::AssignmentTarget, assignment.target));
                roots.push((Role::AssignmentValue, assignment.value));
            }
            StatementNode::Call(call) => roots.extend(
                self.statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|argument| (Role::CallArgument, *argument)),
            ),
            StatementNode::Expression(expression) => roots.push((Role::Expression, *expression)),
            StatementNode::LocalData(local) => {
                roots.push((Role::LocalInitializer, local.initial_value));
            }
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(guard) = transition.guard {
                    roots.push((Role::TransitionGuard, guard));
                }
                for target in [transition.target, transition.continuation] {
                    if !self.statement_table.transition_target_is_valid(target) {
                        continue;
                    }
                    match self.statement_table.transition_target(target) {
                        TransitionTargetNode::Named { arguments, .. } => roots.extend(
                            self.statement_table
                                .expression_handles(*arguments)
                                .iter()
                                .map(|argument| (Role::TransitionArgument, *argument)),
                        ),
                        TransitionTargetNode::Value(value) => {
                            roots.push((Role::TransitionValue, *value));
                        }
                        TransitionTargetNode::Terminal => {}
                    }
                }
            }
        }
        roots
    }
    /// Value roots carried by a proof fact; proposition identity is never a runtime call.
    pub fn proof_fact_expression_roots(
        &self,
        handle: Handle<ProofFact>,
    ) -> Vec<crate::expression::ExpressionHandle> {
        match self.proof_facts.get(handle) {
            ProofFact::Expression(expression) => vec![*expression],
            ProofFact::Membership(membership) => vec![membership.value],
            ProofFact::Proposition(application) => self
                .expression_table
                .expression_handles(application.arguments)
                .to_vec(),
        }
    }
    fn copy_proof_fact_from(
        &mut self,
        source: &TypedTreeTables,
        handle: Handle<ProofFact>,
    ) -> Handle<ProofFact> {
        let mut fact = source.proof_facts.get(handle).clone();
        match &mut fact {
            ProofFact::Expression(expression) => {
                *expression = self
                    .expression_table
                    .copy_from(&source.expression_table, *expression)
            }
            ProofFact::Membership(membership) => {
                membership.value = self
                    .expression_table
                    .copy_from(&source.expression_table, membership.value);
                membership.domain = self.domain_path_members.insert_many(
                    source
                        .domain_path_members
                        .span_or_empty(membership.domain)
                        .iter()
                        .cloned(),
                );
                let arguments = source
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments)
                    .iter()
                    .map(|argument| {
                        self.type_reference_table.copy_from(
                            &source.type_reference_table,
                            &source.expression_table,
                            &mut self.expression_table,
                            *argument,
                        )
                    })
                    .collect::<Vec<_>>();
                membership.domain_arguments = self
                    .type_reference_table
                    .insert_type_reference_handles(arguments);
            }
            ProofFact::Proposition(application) => {
                let arguments = source
                    .expression_table
                    .expression_handles(application.arguments)
                    .iter()
                    .map(|argument| {
                        self.expression_table
                            .copy_from(&source.expression_table, *argument)
                    })
                    .collect::<Vec<_>>();
                application.arguments = self.expression_table.insert_expression_handles(arguments);
            }
        }
        let copied = self.proof_facts.insert(fact);
        let source_index = crate::typed_trees::proof_fact_source_span_index(handle);
        if let Some(Some(span)) = source.proof_fact_source_spans.get(source_index) {
            let index = crate::typed_trees::proof_fact_source_span_index(copied);
            self.proof_fact_source_spans
                .resize(self.proof_fact_source_spans.len().max(index + 1), None);
            self.proof_fact_source_spans[index] = Some(*span);
        }
        copied
    }
    fn remap_proof_fact_symbols(
        &mut self,
        handle: Handle<ProofFact>,
        symbols: &[(SymbolHandle, SymbolHandle)],
    ) {
        for expression in self.proof_fact_expression_roots(handle) {
            self.expression_table.remap_symbols_in(expression, symbols);
        }
        match self.proof_facts.get_mut(handle) {
            ProofFact::Expression(_) => {}
            ProofFact::Membership(membership) => {
                membership.domain_symbol = remapped(membership.domain_symbol, symbols);
                for argument in self
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments)
                    .to_vec()
                {
                    self.type_reference_table.remap_symbols_in(
                        argument,
                        &mut self.expression_table,
                        symbols,
                    );
                }
            }
            ProofFact::Proposition(application) => {
                application.proposition = remapped(application.proposition, symbols);
                for argument in &mut application.binder_arguments {
                    argument.symbol = remapped(argument.symbol, symbols);
                }
            }
        }
    }
}
