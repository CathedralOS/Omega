/*
We copy declarations (`item_copies`) and executable bodies (`body_copies`)
between `SyntaxTrees` instances while preserving shared expression-handle
identity and rebasing source-text pages across forests.
*/

pub(crate) mod body_copies {

    use crate::SyntaxTrees;
    use crate::expression::{
        ExpressionHandle, ExpressionNode, MatchPattern, MatchSubjectCapture, StaticMachineArgument,
        TableAtomicExpression, TableBinaryExpression, TableBorrowExpression, TableCallExpression,
        TableCastExpression, TableDestructurePattern, TableIndexedExpression, TableMatchArm,
        TableMatchExpression, TableMemberExpression, TableMembershipExpression,
        TableRangeExpression, TableSavedSubject, TableStructLiteral, TableStructLiteralField,
        TableUnaryExpression,
    };
    use crate::identifier::Identifier;
    use crate::item::{
        State, StateHandle, StateParameterHandle, StateParameterNode, StateSignature,
        StateSignatureHandle,
    };
    use crate::item_copies::SyntaxCopy;
    use crate::statement::{
        AssemblyBlock, AssemblyControlFlow, RootBinding, StatementHandle, StatementNode,
        TableAssemblyFact, TableAssignment, TableCall, TableLocalData,
        TableProofOutputBindingStatement, TableTransition, TransitionGuardNode,
        TransitionTargetHandle, TransitionTargetNode,
    };
    use crate::types::{
        DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    };
    use arena::{Handle, HandleSpan};

    macro_rules! copied_state_signature {
        ($self:ident, $other:ident, $signature:ident) => {{
            StateSignature {
                name: $signature.name.clone(),
                spelling: $signature.spelling,
                lifetime_parameters: $signature.lifetime_parameters.clone(),
                type_parameters: $self.copy_type_parameter_span($other, $signature.type_parameters),
                conformance_bounds: $signature
                    .conformance_bounds
                    .iter()
                    .map(|bound| crate::item::GenericConformanceBound {
                        binder: bound.binder.clone(),
                        trait_name: bound.trait_name.clone(),
                        lifetime_arguments: bound
                            .lifetime_arguments
                            .iter()
                            .map(|name| $self.copied_lifetime($other, name))
                            .collect(),
                        arguments: $self.copy_type_reference_handle_span($other, bound.arguments),
                    })
                    .collect(),
                is_default: $signature.is_default,
                parameters: $self.copy_state_parameter_handle_span($other, $signature.parameters),
                native_callback_parameters: $signature.native_callback_parameters.clone(),
                return_type: $self.copy_type_reference_handle($other, $signature.return_type),
                service_reach_is_installation_bound: $signature.service_reach_is_installation_bound,
                service_reach_keyword_source_spans: $signature
                    .service_reach_keyword_source_spans
                    .clone(),
                service_reaches: $self
                    .copy_item_identifier_span($other, $signature.service_reaches),
                invokes: $self.copy_item_identifier_span($other, $signature.invokes),
                suspends_keyword_source_spans: $signature.suspends_keyword_source_spans.clone(),
                blocks_keyword_source_spans: $signature.blocks_keyword_source_spans.clone(),
                suspends: $signature.suspends,
                blocks: $signature.blocks,
                contracts: $self.copy_capability_contract_span($other, $signature.contracts),
                default_body: $self.copy_statement_handle_span($other, $signature.default_body),
                terminates_guarantee: $signature.terminates_guarantee,
                declares_no_crash_route: $signature.declares_no_crash_route,
            }
        }};
    }

    impl SyntaxCopy<'_> {
        pub fn copy_static_machine_argument(
            &mut self,
            other: &SyntaxTrees,
            argument: &StaticMachineArgument,
        ) -> StaticMachineArgument {
            let mut copied = argument.clone();
            self.remap_static_machine_argument(other, &mut copied);
            copied
        }

        pub(crate) fn remap_static_machine_argument(
            &mut self,
            other: &SyntaxTrees,
            argument: &mut StaticMachineArgument,
        ) {
            argument.type_reference =
                self.copy_type_reference_handle(other, argument.type_reference);
            if let Some(application) = &mut argument.application {
                for nested in application.arguments.iter_mut() {
                    self.remap_static_machine_argument(other, nested);
                }
                if application.member.is_valid() {
                    let member = other
                        .expressions
                        .static_member(application.member)
                        .expect("qualified static member belongs to the copied syntax table");
                    let copied = self.copy_static_machine_argument(other, member);
                    application.member = self.expressions.insert_static_member(copied);
                }
            }
        }

        pub(crate) fn copy_state_handle_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<StateHandle>,
        ) -> HandleSpan<StateHandle> {
            self.copy_mapped_span(
                other.items.state_handles(span).iter().copied(),
                |this, handle| {
                    let state = other.items.state(handle);
                    let parameters = this.copy_state_parameter_handle_span(other, state.parameters);
                    let return_type = this.copy_type_reference_handle(other, state.return_type);
                    let contracts = this.copy_capability_contract_span(other, state.contracts);
                    let statements = this.copy_statement_handle_span(other, state.statements);
                    this.items.insert_state(&State {
                        name: state.name.clone(),
                        parameters,
                        return_type,
                        contracts,
                        statements,
                    })
                },
                |this, handle| this.items.append_state_handle(handle),
            )
        }

        pub(crate) fn copy_state_signature_handle_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<StateSignatureHandle>,
        ) -> HandleSpan<StateSignatureHandle> {
            self.copy_mapped_span(
                other.items.state_signatures(span).iter().copied(),
                |this, handle| {
                    let signature = other.items.state_signature(handle);
                    let copied_signature = this.copy_state_signature(other, signature);
                    this.items.insert_state_signature(&copied_signature)
                },
                |this, handle| this.items.append_state_signature_handle(handle),
            )
        }

        pub(crate) fn copy_state_signature(
            &mut self,
            other: &SyntaxTrees,
            signature: &StateSignature,
        ) -> StateSignature {
            let enclosing = self.scoped_lifetimes.clone();
            self.scoped_lifetimes.retain(|(name, _)| {
                !signature
                    .lifetime_parameters
                    .iter()
                    .any(|local| local.as_str(other) == name)
            });
            let copied = copied_state_signature!(self, other, signature);
            self.scoped_lifetimes = enclosing;
            copied
        }

        pub(crate) fn copy_state_parameter_handle_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<StateParameterHandle>,
        ) -> HandleSpan<StateParameterHandle> {
            self.copy_mapped_span(
                other.items.state_parameters(span).iter().copied(),
                |this, handle| {
                    let parameter = other.items.state_parameter(handle);
                    let type_reference =
                        this.copy_type_reference_handle(other, parameter.type_reference);
                    this.items.insert_state_parameter_node(StateParameterNode {
                        name: parameter.name.clone(),
                        type_reference,
                        is_const: parameter.is_const,
                        is_mutable: parameter.is_mutable,
                        relevance: parameter.relevance,
                    })
                },
                |this, handle| this.items.append_state_parameter_handle(handle),
            )
        }

        fn copy_statement_handle_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<StatementHandle>,
        ) -> HandleSpan<StatementHandle> {
            self.copy_mapped_span(
                other.items.statements(span).iter().copied(),
                |this, handle| {
                    let statement =
                        this.copy_statement_node(other, other.statements.statement(handle));
                    this.statements.insert(statement)
                },
                |this, handle| this.items.append_statement_handle(handle),
            )
        }

        fn copy_statement_node(
            &mut self,
            other: &SyntaxTrees,
            statement: &StatementNode,
        ) -> StatementNode {
            match statement {
                StatementNode::Assembly(handle) => {
                    let graph = other.statements.assembly(*handle);
                    let blocks: Vec<_> = other
                        .statements
                        .assembly_blocks(graph.blocks)
                        .iter()
                        .map(|block| {
                            let requires: Vec<_> = other
                                .statements
                                .assembly_expressions(block.requires)
                                .iter()
                                .map(|expression| self.copy_expression_handle(other, *expression))
                                .collect();
                            let ensures: Vec<_> = other
                                .statements
                                .assembly_expressions(block.ensures)
                                .iter()
                                .map(|expression| self.copy_expression_handle(other, *expression))
                                .collect();
                            let instructions: Vec<_> = other
                                .statements
                                .assembly_instructions(block.instructions)
                                .iter()
                                .map(|handle| {
                                    let statement = self.copy_statement_node(
                                        other,
                                        other.statements.statement(*handle),
                                    );
                                    self.statements.insert(statement)
                                })
                                .collect();
                            AssemblyBlock {
                                label: block.label.clone(),
                                requires: self.statements.insert_assembly_expressions(requires),
                                ensures: self.statements.insert_assembly_expressions(ensures),
                                instructions: self
                                    .statements
                                    .insert_assembly_instructions(instructions),
                                exit: block.exit.clone(),
                            }
                        })
                        .collect();
                    let primitives: Vec<_> = other
                        .statements
                        .assembly_expressions(graph.primitives)
                        .iter()
                        .map(|expression| self.copy_expression_handle(other, *expression))
                        .collect();
                    let primitives = self.statements.insert_assembly_expressions(primitives);
                    let mut scratch = graph.scratch;
                    for expression in &mut scratch {
                        if expression.is_valid() {
                            *expression = self.copy_expression_handle(other, *expression);
                        }
                    }
                    let blocks = self.statements.insert_assembly_blocks(blocks);
                    StatementNode::Assembly(self.statements.insert_assembly(AssemblyControlFlow {
                        target_requirement: graph.target_requirement,
                        blocks,
                        scratch,
                        primitives,
                    }))
                }
                StatementNode::RootBinding(binding) => StatementNode::RootBinding(RootBinding {
                    receiver: self.copy_expression_handle(other, binding.receiver),
                    implementation_operand: self
                        .copy_expression_handle(other, binding.implementation_operand),
                    ..binding.clone()
                }),
                StatementNode::AssemblyFact(fact) => {
                    StatementNode::AssemblyFact(TableAssemblyFact {
                        kind: fact.kind,
                        expression: self.copy_expression_handle(other, fact.expression),
                    })
                }
                StatementNode::Assignment(assignment) => {
                    StatementNode::Assignment(TableAssignment {
                        target: self.copy_expression_handle(other, assignment.target),
                        value: self.copy_expression_handle(other, assignment.value),
                    })
                }
                StatementNode::Call(call) => StatementNode::Call(TableCall {
                    target_is_static: call.target_is_static,
                    receiver: self.copy_statement_identifier_span(other, call.receiver),
                    target: call.target.clone(),
                    machine_arguments: call
                        .machine_arguments
                        .iter()
                        .map(|argument| self.copy_static_machine_argument(other, argument))
                        .collect(),
                    arguments: self.copy_statement_expression_span(other, call.arguments),
                    evidence_arguments: call.evidence_arguments.clone(),
                    operational_acknowledgement: call.operational_acknowledgement,
                    discards_result: call.discards_result,
                }),
                StatementNode::ProofOutputBindingStatement(binding) => {
                    StatementNode::ProofOutputBindingStatement(TableProofOutputBindingStatement {
                        bindings: binding.bindings.clone(),
                        call: self.copy_expression_handle(other, binding.call),
                    })
                }
                StatementNode::Expression(value) => {
                    StatementNode::Expression(self.copy_expression_handle(other, *value))
                }
                StatementNode::LocalData(local_data) => StatementNode::LocalData(TableLocalData {
                    role: local_data.role,
                    name: local_data.name.clone(),
                    type_reference: self
                        .copy_type_reference_handle(other, local_data.type_reference),
                    initial_value: self.copy_expression_handle(other, local_data.initial_value),
                    is_mutable: local_data.is_mutable,
                    relevance: local_data.relevance,
                    pattern_fields: self.copy_match_fields(other, local_data.pattern_fields),
                }),
                StatementNode::Transition(transition) => {
                    let patterns = other
                        .statements
                        .transition_patterns(transition.patterns)
                        .iter()
                        .map(|source| crate::statement::TransitionPattern {
                            is_structural: source.is_structural,
                            subject: self.copy_expression_handle(other, source.subject),
                            capture: self.copy_match_subject(other, source.capture),
                            pattern: self.copy_destructure_pattern(other, source.pattern),
                        })
                        .collect::<Vec<_>>();
                    let patterns = self.statements.insert_transition_patterns(patterns);
                    StatementNode::Transition(TableTransition {
                        patterns,
                        target: self.copy_transition_target(other, transition.target),
                        continuation: self.copy_transition_target(other, transition.continuation),
                        guard: match transition.guard {
                            TransitionGuardNode::Always => TransitionGuardNode::Always,
                            TransitionGuardNode::When(expression) => TransitionGuardNode::When(
                                self.copy_expression_handle(other, expression),
                            ),
                        },
                        proof_selectors: self.statements.insert_outcome_proof_selectors(
                            other
                                .statements
                                .outcome_proof_selectors(transition.proof_selectors)
                                .iter()
                                .cloned(),
                        ),
                        exit: transition.exit,
                        ends_dispatch: transition.ends_dispatch,
                        source_span: transition.source_span,
                    })
                }
            }
        }

        fn copy_transition_target(
            &mut self,
            other: &SyntaxTrees,
            handle: TransitionTargetHandle,
        ) -> TransitionTargetHandle {
            if !handle.is_valid() {
                return TransitionTargetHandle::invalid();
            }

            let target = match other.statements.transition_target(handle) {
                TransitionTargetNode::Named {
                    path,
                    arguments,
                    evidence_arguments,
                    source_span,
                } => TransitionTargetNode::Named {
                    path: self.copy_statement_identifier_span(other, *path),
                    arguments: self.copy_statement_expression_span(other, *arguments),
                    evidence_arguments: evidence_arguments.clone(),
                    source_span: *source_span,
                },
                TransitionTargetNode::Value(value) => {
                    TransitionTargetNode::Value(self.copy_expression_handle(other, *value))
                }
                TransitionTargetNode::Terminal => TransitionTargetNode::Terminal,
            };

            self.statements.insert_transition_target(target)
        }

        pub(crate) fn copy_type_reference_handle(
            &mut self,
            other: &SyntaxTrees,
            handle: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            if !handle.is_valid() {
                return TypeReferenceHandle::invalid();
            }

            if let Some(actual) = self
                .type_substitutions
                .iter()
                .find_map(|(source, actual)| (*source == handle).then_some(*actual))
            {
                let origins = other
                    .type_references
                    .type_alias_applications(handle)
                    .cloned()
                    .collect::<Vec<_>>();
                if origins.is_empty() {
                    self.supplied_type_copies.push(actual);
                    return actual;
                }
                let copied = self.type_references.fresh_type_occurrence(actual);
                for origin in origins {
                    let application = self.copy_type_reference_handle(other, origin.application);
                    self.type_references.retain_type_alias_application(
                        crate::types::TypeAliasApplicationOrigin {
                            instance: copied,
                            application,
                            declaration: origin.declaration,
                            definition_references: origin.definition_references.clone(),
                        },
                    );
                }
                self.supplied_type_copies.push(copied);
                return copied;
            }

            let copied = match other.type_references.type_reference(handle) {
                TypeReferenceNode::Reference {
                    referee,
                    access,
                    lifetime,
                } => {
                    let referee = self.copy_type_reference_handle(other, *referee);
                    let lifetime = lifetime
                        .as_ref()
                        .map(|name| self.copied_lifetime(other, name));
                    self.type_references
                        .insert_reference_with_lifetime(referee, *access, lifetime)
                }
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    let base_type = self.copy_type_reference_handle(other, *base_type);
                    let constraints = self.copy_constraint_span(other, *constraints);
                    self.type_references
                        .insert_constrained(base_type, constraints)
                }
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    let element_type = self.copy_type_reference_handle(other, *element_type);
                    let length = match length {
                        crate::types::FixedArrayLength::Expression(use_type) => {
                            crate::types::FixedArrayLength::Expression(
                                self.copy_type_reference_handle(other, *use_type),
                            )
                        }
                        crate::types::FixedArrayLength::ConstParameter(name) => self
                            .array_length_substitutions
                            .iter()
                            .find_map(|(span, spelling, actual)| {
                                (*span == name.source_span() && spelling == name.as_str(other))
                                    .then_some(crate::types::FixedArrayLength::Expression(*actual))
                            })
                            .unwrap_or_else(|| length.clone()),
                        length => length.clone(),
                    };
                    self.type_references
                        .insert_fixed_array(element_type, length)
                }
                TypeReferenceNode::Slice { element_type } => {
                    let element_type = self.copy_type_reference_handle(other, *element_type);
                    self.type_references.insert_slice(element_type)
                }
                TypeReferenceNode::Generic {
                    base_name,
                    lifetime_arguments,
                    arguments,
                } => {
                    let arguments = self.copy_type_reference_handle_span(other, *arguments);
                    let lifetime_arguments = lifetime_arguments
                        .iter()
                        .map(|name| self.copied_lifetime(other, name))
                        .collect();
                    self.type_references.insert(TypeReferenceNode::Generic {
                        base_name: base_name.clone(),
                        lifetime_arguments,
                        arguments,
                    })
                }
                TypeReferenceNode::StaticSelection(selection) => {
                    let selection = self.copy_static_machine_argument(other, selection);
                    self.type_references
                        .insert(TypeReferenceNode::StaticSelection(selection))
                }
                TypeReferenceNode::ConstExpression(expression) => {
                    let expression = self.copy_expression_handle(other, *expression);
                    self.type_references
                        .insert(TypeReferenceNode::ConstExpression(expression))
                }
                TypeReferenceNode::DynamicTrait {
                    trait_name,
                    trait_arguments,
                } => {
                    let trait_arguments =
                        self.copy_type_reference_handle_span(other, *trait_arguments);
                    self.type_references
                        .insert(TypeReferenceNode::DynamicTrait {
                            trait_name: trait_name.clone(),
                            trait_arguments,
                        })
                }
                TypeReferenceNode::Named(name) => self.type_references.insert_named(name.clone()),
                TypeReferenceNode::Unit => self.type_references.insert_unit(),
            };
            if self.retain_alias_header_role
                && other.type_references.is_alias_header_reference(handle)
            {
                self.type_references.retain_alias_header_reference(copied);
            }
            for pending in other.type_references.pending_const_arguments(handle) {
                self.type_references
                    .retain_pending_const_argument(copied, pending);
            }
            match other.type_references.supplied_operand_owner(handle) {
                Ok(owner) => self
                    .type_references
                    .retain_supplied_operand_owner(copied, owner),
                Err(_) => self
                    .type_references
                    .retain_unresolved_supplied_operand_owner(copied),
            }
            let origin = other.type_references.generic_application_origin(handle);
            if let Some(normalization) = other.type_references.const_argument_normalization(handle)
            {
                self.type_references.retain_const_argument_normalization(
                    copied,
                    normalization.reference,
                    normalization.canonical_result_encoding.clone(),
                    other
                        .type_references
                        .const_argument_origins(normalization.selections)
                        .iter()
                        .cloned(),
                    other
                        .type_references
                        .const_argument_builtin_operators(normalization.builtin_operators)
                        .iter()
                        .copied(),
                );
            }
            if let Some(normalization) = other.type_references.const_argument_normalization(handle)
                && normalization.authored_expression.is_valid()
            {
                let expression =
                    self.copy_expression_handle(other, normalization.authored_expression);
                self.type_references
                    .retain_const_argument_expression(copied, expression);
            }
            if let Some(normalization) = other.type_references.const_argument_normalization(handle)
                && normalization.materialized_expression.is_valid()
            {
                let expression =
                    self.copy_expression_handle(other, normalization.materialized_expression);
                self.type_references
                    .retain_const_argument_materialized_expression(copied, expression);
            }
            if origin.is_valid() {
                let application = self.copy_type_reference_handle(other, origin);
                self.type_references
                    .retain_generic_application_origin(copied, application);
            }
            for origin in other.type_references.type_alias_applications(handle) {
                let application = self.copy_type_reference_handle(other, origin.application);
                self.type_references.retain_type_alias_application(
                    crate::types::TypeAliasApplicationOrigin {
                        instance: copied,
                        application,
                        declaration: origin.declaration,
                        definition_references: origin.definition_references.clone(),
                    },
                );
            }
            copied
        }

        pub(crate) fn copy_type_reference_handle_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<TypeReferenceHandle>,
        ) -> HandleSpan<TypeReferenceHandle> {
            self.copy_mapped_span(
                other
                    .type_references
                    .type_reference_handles(span)
                    .iter()
                    .copied(),
                |this, handle| this.copy_type_reference_handle(other, handle),
                |this, handle| this.type_references.append_type_reference_handle(handle),
            )
        }

        fn copy_constraint_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<TypeConstraintNode>,
        ) -> HandleSpan<TypeConstraintNode> {
            self.copy_mapped_span(
                other.type_references.constraints(span),
                |this, constraint| match constraint {
                    TypeConstraintNode::Named(name) => TypeConstraintNode::Named(name.clone()),
                    TypeConstraintNode::Domain(domain) => {
                        TypeConstraintNode::Domain(DomainConstraint {
                            name: domain.name.clone(),
                            arguments: this
                                .copy_type_reference_handle_span(other, domain.arguments),
                        })
                    }

                    TypeConstraintNode::ArithmeticDomain(domain) => {
                        TypeConstraintNode::ArithmeticDomain(*domain)
                    }
                },
                |this, constraint| this.type_references.append_constraint(constraint),
            )
        }

        fn copy_match_subject(
            &mut self,
            other: &SyntaxTrees,
            capture: Handle<MatchSubjectCapture>,
        ) -> Handle<MatchSubjectCapture> {
            if !capture.is_valid() {
                return Handle::invalid();
            }
            let position = capture.arena_index() as usize;
            if let Some((source, copied)) = self.match_subject_copies.get(position)
                && *source == capture
            {
                return *copied;
            }
            let subject = self
                .copy_expression_handle(other, other.expressions.match_subject(capture).subject);
            let copied = match other.expressions.match_subject(capture).purpose {
                crate::expression::SubjectCapturePurpose::Match => {
                    self.expressions.insert_match_subject(subject)
                }
                crate::expression::SubjectCapturePurpose::Transition => {
                    self.expressions.insert_transition_subject(subject)
                }
            };
            if self.match_subject_copies.len() <= position {
                self.match_subject_copies
                    .resize(position + 1, (Handle::invalid(), Handle::invalid()));
            }
            self.match_subject_copies[position] = (capture, copied);
            copied
        }

        /// One pattern's field roster, for a dispatch arm or a record
        /// pattern's marker local alike.
        fn copy_match_fields(
            &mut self,
            other: &SyntaxTrees,
            fields: HandleSpan<crate::expression::TableMatchField>,
        ) -> HandleSpan<crate::expression::TableMatchField> {
            use crate::expression::{MatchFieldAction, TableMatchField};
            let fields = other
                .expressions
                .match_fields(fields)
                .iter()
                .map(|field| TableMatchField {
                    component: field.component,
                    member: field.member.clone(),
                    action: match &field.action {
                        MatchFieldAction::Bind(binding) => MatchFieldAction::Bind(binding.clone()),
                        MatchFieldAction::Equal(value) => {
                            MatchFieldAction::Equal(self.copy_expression_handle(other, *value))
                        }
                        MatchFieldAction::Waive => MatchFieldAction::Waive,
                    },
                })
                .collect::<Vec<_>>();
            self.expressions.insert_match_fields(fields)
        }

        fn copy_destructure_pattern(
            &mut self,
            other: &SyntaxTrees,
            pattern: TableDestructurePattern,
        ) -> TableDestructurePattern {
            let head = self.copy_expression_handle(other, pattern.head);
            let fields = self.copy_match_fields(other, pattern.fields);
            let guard = self.copy_expression_handle(other, pattern.guard);
            let proof_selectors = self.statements.insert_outcome_proof_selectors(
                other
                    .statements
                    .outcome_proof_selectors(pattern.proof_selectors)
                    .iter()
                    .cloned(),
            );
            TableDestructurePattern {
                application_type: self.copy_type_reference_handle(other, pattern.application_type),
                head,
                fields,
                has_rest: pattern.has_rest,
                guard,
                proof_selectors,
            }
        }

        pub(crate) fn copy_expression_handle(
            &mut self,
            other: &SyntaxTrees,
            handle: ExpressionHandle,
        ) -> ExpressionHandle {
            if !handle.is_valid() {
                return ExpressionHandle::invalid();
            }

            if let Some((_, actual)) = self
                .expression_substitutions
                .iter()
                .find(|(source, _)| *source == handle)
            {
                return *actual;
            }

            let source_position = handle.arena_index() as usize;
            if let Some((source, copied)) = self.expression_copies.get(source_position)
                && *source == handle
            {
                return *copied;
            }

            let expression = match other.expressions.expression(handle) {
                ExpressionNode::Match(dispatch) => {
                    let subject = self.copy_expression_handle(other, dispatch.subject);
                    let capture = self.copy_match_subject(other, dispatch.capture);
                    let mut arms =
                        Vec::with_capacity(other.expressions.match_arms(dispatch.arms).len());
                    for arm in other.expressions.match_arms(dispatch.arms) {
                        let pattern = match arm.pattern {
                            MatchPattern::Value(value) => {
                                MatchPattern::Value(self.copy_expression_handle(other, value))
                            }
                            MatchPattern::Wildcard => MatchPattern::Wildcard,
                            MatchPattern::Destructure(pattern) => MatchPattern::Destructure(
                                self.copy_destructure_pattern(other, pattern),
                            ),
                        };
                        let value = self.copy_expression_handle(other, arm.value);
                        arms.push(TableMatchArm {
                            pattern,
                            value,
                            source_span: arm.source_span,
                        });
                    }
                    let arms = self.expressions.insert_match_arms(arms);
                    ExpressionNode::Match(TableMatchExpression {
                        subject,
                        capture,
                        arms,
                    })
                }
                ExpressionNode::SavedSubject(saved) => {
                    ExpressionNode::SavedSubject(TableSavedSubject {
                        component: saved.component,
                        capture: self.copy_match_subject(other, saved.capture),
                        pattern_head: self.copy_expression_handle(other, saved.pattern_head),
                    })
                }
                ExpressionNode::ArrayLiteral(values) => {
                    ExpressionNode::ArrayLiteral(self.copy_expression_handle_list(other, *values))
                }
                ExpressionNode::Atomic(atomic) => ExpressionNode::Atomic(TableAtomicExpression {
                    value: self.copy_expression_handle(other, atomic.value),
                    result: if atomic.result.is_valid() {
                        self.copy_expression_handle(other, atomic.result)
                    } else {
                        ExpressionHandle::invalid()
                    },
                    ordering: atomic.ordering,
                    result_custody: atomic.result_custody,
                }),
                ExpressionNode::Binary(binary) => ExpressionNode::Binary(TableBinaryExpression {
                    policy: binary.policy,
                    left: self.copy_expression_handle(other, binary.left),
                    operator: binary.operator,
                    right: self.copy_expression_handle(other, binary.right),
                }),
                ExpressionNode::Boolean(value) => ExpressionNode::Boolean(*value),
                ExpressionNode::Cast(cast) => ExpressionNode::Cast(TableCastExpression {
                    value: self.copy_expression_handle(other, cast.value),
                    target_type: self.copy_type_reference_handle(other, cast.target_type),
                    target_label: self.copy_expression_identifier_span(other, cast.target_label),
                    domain: cast.domain,
                    semantic_domain: self
                        .copy_expression_identifier_span(other, cast.semantic_domain),
                    semantic_domain_arguments: self
                        .copy_type_reference_handle_span(other, cast.semantic_domain_arguments),
                    form: cast.form,
                }),
                ExpressionNode::Call(call) => ExpressionNode::Call(TableCallExpression {
                    target_is_static: call.target_is_static,
                    receiver: self.copy_expression_handle(other, call.receiver),
                    target: call.target.clone(),
                    machine_arguments: call
                        .machine_arguments
                        .iter()
                        .map(|argument| self.copy_static_machine_argument(other, argument))
                        .collect(),
                    arguments: self.copy_expression_handle_list(other, call.arguments),
                    evidence_arguments: call.evidence_arguments.clone(),
                    operational_acknowledgement: call.operational_acknowledgement,
                }),
                ExpressionNode::Float(value) => ExpressionNode::Float(value.clone()),
                ExpressionNode::UndeterminedNaN(format) => ExpressionNode::UndeterminedNaN(*format),
                ExpressionNode::Indexed(indexed) => {
                    ExpressionNode::Indexed(TableIndexedExpression {
                        collection: self.copy_expression_handle(other, indexed.collection),
                        index: self.copy_expression_handle(other, indexed.index),
                    })
                }
                ExpressionNode::Integer(value) => ExpressionNode::Integer(value.clone()),
                ExpressionNode::Membership(membership) => {
                    ExpressionNode::Membership(TableMembershipExpression {
                        value: self.copy_expression_handle(other, membership.value),
                        domain: self.copy_expression_identifier_span(other, membership.domain),
                    })
                }
                ExpressionNode::Member(member) => ExpressionNode::Member(TableMemberExpression {
                    receiver: self.copy_expression_handle(other, member.receiver),
                    access_source_span: member.access_source_span,
                    member: member.member.clone(),
                    case_variant: member.case_variant.clone(),
                }),
                ExpressionNode::Borrow(expression) => {
                    ExpressionNode::Borrow(TableBorrowExpression {
                        target: self.copy_expression_handle(other, expression.target),
                        access: expression.access,
                    })
                }
                ExpressionNode::Name(path) => {
                    let mut selection = StaticMachineArgument {
                        application: path.application.clone(),
                        ..Default::default()
                    };
                    self.remap_static_machine_argument(other, &mut selection);
                    ExpressionNode::Name(crate::expression::TableNamePath {
                        members: self.copy_expression_identifier_span(other, path.members),
                        application: selection.application,
                    })
                }
                ExpressionNode::Range(range) => ExpressionNode::Range(TableRangeExpression {
                    start: self.copy_expression_handle(other, range.start),
                    end: self.copy_expression_handle(other, range.end),
                    end_inclusive: range.end_inclusive,
                }),
                ExpressionNode::StructLiteral(struct_literal) => {
                    ExpressionNode::StructLiteral(TableStructLiteral {
                        application_type: self
                            .copy_type_reference_handle(other, struct_literal.application_type),
                        constructor_name: struct_literal.constructor_name.clone(),
                        fields: self.copy_struct_field_span(other, struct_literal.fields),
                    })
                }
                ExpressionNode::String(value) => ExpressionNode::String(value.clone()),
                ExpressionNode::Unary(unary) => {
                    let operand = self.copy_expression_handle(other, unary.operand);
                    ExpressionNode::Unary(TableUnaryExpression {
                        operator: unary.operator,
                        operand,
                    })
                }
                ExpressionNode::ZeroValue(type_reference) => ExpressionNode::ZeroValue(
                    self.copy_type_reference_handle(other, *type_reference),
                ),
                ExpressionNode::TypeExpression(type_reference) => ExpressionNode::TypeExpression(
                    self.copy_type_reference_handle(other, *type_reference),
                ),
            };

            let source_span = other.expressions.source_span(handle);
            let copied = self.expressions.insert(expression);
            self.expressions.set_source_span(copied, source_span);
            if self.expression_copies.len() <= source_position {
                self.expression_copies.resize(
                    source_position + 1,
                    (ExpressionHandle::invalid(), ExpressionHandle::invalid()),
                );
            }
            self.expression_copies[source_position] = (handle, copied);
            copied
        }

        pub(crate) fn copy_expression_handle_list(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<ExpressionHandle>,
        ) -> HandleSpan<ExpressionHandle> {
            let copied_handles: Vec<_> = other
                .expressions
                .expression_handles(span)
                .iter()
                .copied()
                .map(|handle| self.copy_expression_handle(other, handle))
                .collect();

            self.expressions.insert_expression_handles(copied_handles)
        }

        fn copy_struct_field_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<TableStructLiteralField>,
        ) -> HandleSpan<TableStructLiteralField> {
            let copied_fields: Vec<_> = other
                .expressions
                .struct_fields(span)
                .iter()
                .map(|field| TableStructLiteralField {
                    name: field.name.clone(),
                    value: self.copy_expression_handle(other, field.value),
                })
                .collect();

            self.expressions.insert_struct_fields(copied_fields)
        }

        pub(crate) fn copy_item_identifier_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<Identifier>,
        ) -> HandleSpan<Identifier> {
            self.copy_span(
                other.items.identifier_path_members(span).iter().cloned(),
                |this, member| this.items.append_identifier_path_member(member),
            )
        }

        fn copy_statement_identifier_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<Identifier>,
        ) -> HandleSpan<Identifier> {
            self.copy_span(
                other
                    .statements
                    .identifier_path_members(span)
                    .iter()
                    .cloned(),
                |this, member| this.statements.append_identifier_path_member(member),
            )
        }

        fn copy_expression_identifier_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<Identifier>,
        ) -> HandleSpan<Identifier> {
            self.copy_span(
                other
                    .expressions
                    .identifier_path_members(span)
                    .iter()
                    .cloned(),
                |this, member| this.expressions.append_identifier_path_member(member),
            )
        }

        fn copy_statement_expression_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<ExpressionHandle>,
        ) -> HandleSpan<ExpressionHandle> {
            let copied_handles: Vec<_> = other
                .statements
                .expression_handles(span)
                .iter()
                .copied()
                .map(|handle| self.copy_expression_handle(other, handle))
                .collect();

            self.statements.insert_expression_handles(copied_handles)
        }

        pub(crate) fn copy_mapped_span<S, T>(
            &mut self,
            values: impl IntoIterator<Item = S>,
            mut map: impl FnMut(&mut Self, S) -> T,
            mut append: impl FnMut(&mut Self, T) -> Handle<T>,
        ) -> HandleSpan<T> {
            let mapped = values
                .into_iter()
                .map(|value| map(self, value))
                .collect::<Vec<_>>();
            let mut start = Handle::invalid();
            let mut count = 0u32;
            for value in mapped {
                let handle = append(self, value);
                if count == 0 {
                    start = handle;
                }
                count = count.checked_add(1).expect("copied span count overflow");
            }

            HandleSpan::from_parts_or_empty(start, count)
        }

        pub(crate) fn copy_span<T>(
            &mut self,
            values: impl IntoIterator<Item = T>,
            append: impl FnMut(&mut Self, T) -> Handle<T>,
        ) -> HandleSpan<T> {
            self.copy_mapped_span(values, |_, value| value, append)
        }
    }
}

pub(crate) mod item_copies {
    use crate::SyntaxTrees;
    use crate::expression::{ExpressionHandle, StaticMachineArgument};
    use crate::item::{
        CapabilityContract, CapabilityContractKind, CapabilityDefinition, CapabilityField,
        CapabilityMember, CapabilityState, ConformanceItem, ConformanceMember, ConstDefinition,
        ConstDestinationObligation, ConstInitializerNormalization, ConstQualification,
        DataDefinition, DataField, DataMember, DataVariant, DomainAliasDefinition,
        DomainDefinition, GenericConformanceBound, Item, Machine, MachineParameterContract,
        MathematicalDefinition, MathematicalDefinitionBody, MathematicalParameterNode,
        MathematicalTypeHandle, MathematicalTypeNode, MeasureDefinition, ModuleDeclaration,
        OperatorDefinition, PackageDeclaration, ProofFact, ProofMembershipFact, PropositionBody,
        PropositionDefinition, PropositionParameterSignature, QuotientDefinition,
        QuotientEquivalenceSelection, SatisfiesClause, StateParameterHandle, StateParameterNode,
        StateSignature, TraitDefinition, TraitRefinementClause, TypeParameter, TypeParameterKind,
        UseItem,
    };
    use crate::types::TypeReferenceHandle;
    use arena::{Handle, HandleSpan};
    use std::ops::{Deref, DerefMut};

    pub(crate) struct SyntaxCopy<'destination> {
        destination: &'destination mut SyntaxTrees,
        pub(crate) expression_copies: Vec<(ExpressionHandle, ExpressionHandle)>,
        pub(crate) type_substitutions: Vec<(
            crate::types::TypeReferenceHandle,
            crate::types::TypeReferenceHandle,
        )>,
        pub(crate) expression_substitutions: Vec<(ExpressionHandle, ExpressionHandle)>,
        pub(crate) supplied_type_copies: Vec<crate::types::TypeReferenceHandle>,
        pub(crate) retain_alias_header_role: bool,
        lifetime_substitutions: Vec<(source::SourceSpan, String, crate::identifier::Identifier)>,
        pub(crate) scoped_lifetimes: Vec<(String, crate::identifier::Identifier)>,
        pub(crate) array_length_substitutions: Vec<(
            source::SourceSpan,
            String,
            crate::types::TypeReferenceHandle,
        )>,
        pub(crate) match_subject_copies: Vec<(
            Handle<crate::expression::MatchSubjectCapture>,
            Handle<crate::expression::MatchSubjectCapture>,
        )>,
    }

    impl<'destination> SyntaxCopy<'destination> {
        fn new(destination: &'destination mut SyntaxTrees, other: &SyntaxTrees) -> Self {
            destination.text.retain_from(&other.text);
            Self {
                destination,
                expression_copies: Vec::new(),
                type_substitutions: Vec::new(),
                expression_substitutions: Vec::new(),
                supplied_type_copies: Vec::new(),
                retain_alias_header_role: true,
                lifetime_substitutions: Vec::new(),
                scoped_lifetimes: Vec::new(),
                array_length_substitutions: Vec::new(),
                match_subject_copies: Vec::new(),
            }
        }
    }

    impl Deref for SyntaxCopy<'_> {
        type Target = SyntaxTrees;

        fn deref(&self) -> &Self::Target {
            self.destination
        }
    }

    impl DerefMut for SyntaxCopy<'_> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.destination
        }
    }

    impl SyntaxTrees {
        pub fn copy_item_from(&mut self, other: &SyntaxTrees, item: &Item) -> Item {
            SyntaxCopy::new(self, other).copy_item(other, item)
        }

        pub fn copy_expression_from(
            &mut self,
            other: &SyntaxTrees,
            handle: ExpressionHandle,
        ) -> ExpressionHandle {
            SyntaxCopy::new(self, other).copy_expression_handle(other, handle)
        }

        pub fn copy_proof_fact_from(
            &mut self,
            other: &SyntaxTrees,
            source: Handle<ProofFact>,
        ) -> Handle<ProofFact> {
            SyntaxCopy::new(self, other).copy_proof_fact(other, source)
        }

        pub fn copy_state_signature_from(
            &mut self,
            other: &SyntaxTrees,
            signature: &StateSignature,
        ) -> StateSignature {
            SyntaxCopy::new(self, other).copy_state_signature(other, signature)
        }

        /// Copy a selected trait requirement under its enclosing lifetime bindings.
        /// The requirement's own telescope shadows the enclosing trait telescope.
        pub fn copy_state_signature_in_lifetime_scope(
            &mut self,
            other: &SyntaxTrees,
            signature: &StateSignature,
            lifetimes: &[(String, crate::identifier::Identifier)],
        ) -> StateSignature {
            let mut copy = SyntaxCopy::new(self, other);
            copy.scoped_lifetimes.extend(
                lifetimes
                    .iter()
                    .filter(|(name, _)| {
                        !signature
                            .lifetime_parameters
                            .iter()
                            .any(|local| local.as_str(other) == name)
                    })
                    .cloned(),
            );
            copy.copy_state_signature(other, signature)
        }

        pub fn copy_static_machine_argument(
            &mut self,
            other: &SyntaxTrees,
            argument: &StaticMachineArgument,
        ) -> StaticMachineArgument {
            SyntaxCopy::new(self, other).copy_static_machine_argument(other, argument)
        }

        pub fn transfer_static_machine_arguments(
            &mut self,
            other: &SyntaxTrees,
            mut arguments: Box<[StaticMachineArgument]>,
        ) -> Box<[StaticMachineArgument]> {
            for argument in arguments.iter_mut() {
                SyntaxCopy::new(self, other).remap_static_machine_argument(other, argument);
            }
            arguments
        }

        pub(crate) fn copy_mathematical_definition(
            &mut self,
            other: &SyntaxTrees,
            definition: &MathematicalDefinition,
        ) -> MathematicalDefinition {
            SyntaxCopy::new(self, other).copy_mathematical_definition(other, definition)
        }

        pub fn instantiate_type_reference_from(
            &mut self,
            other: &SyntaxTrees,
            reference: crate::types::TypeReferenceHandle,
            types: &[(
                crate::types::TypeReferenceHandle,
                crate::types::TypeReferenceHandle,
            )],
            expressions: &[(ExpressionHandle, ExpressionHandle)],
            lifetimes: &[(source::SourceSpan, String, crate::identifier::Identifier)],
            array_lengths: &[(
                source::SourceSpan,
                String,
                crate::types::TypeReferenceHandle,
            )],
        ) -> (
            crate::types::TypeReferenceHandle,
            Vec<crate::types::TypeReferenceHandle>,
        ) {
            let mut copy = SyntaxCopy::new(self, other);
            copy.retain_alias_header_role = false;
            copy.type_substitutions.extend_from_slice(types);
            copy.expression_substitutions.extend_from_slice(expressions);
            copy.lifetime_substitutions.extend_from_slice(lifetimes);
            copy.array_length_substitutions
                .extend_from_slice(array_lengths);
            let reference = copy.copy_type_reference_handle(other, reference);
            (reference, copy.supplied_type_copies)
        }

        pub fn copy_type_reference_handle(
            &mut self,
            other: &SyntaxTrees,
            handle: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            SyntaxCopy::new(self, other).copy_type_reference_handle(other, handle)
        }
    }

    impl SyntaxCopy<'_> {
        pub(crate) fn copied_lifetime(
            &self,
            other: &SyntaxTrees,
            name: &crate::identifier::Identifier,
        ) -> crate::identifier::Identifier {
            self.lifetime_substitutions
                .iter()
                .find_map(|(source, spelling, actual)| {
                    (*source == name.source_span() && spelling == name.as_str(other))
                        .then(|| actual.clone())
                })
                .or_else(|| {
                    self.scoped_lifetimes.iter().find_map(|(spelling, actual)| {
                        (spelling == name.as_str(other)).then(|| actual.clone())
                    })
                })
                .unwrap_or_else(|| name.clone())
        }

        fn copy_proof_fact(
            &mut self,
            other: &SyntaxTrees,
            source: Handle<ProofFact>,
        ) -> Handle<ProofFact> {
            let fact = other.items.proof_fact(source);
            let copied = match fact {
                ProofFact::Expression(expression) => {
                    ProofFact::Expression(self.copy_expression_handle(other, *expression))
                }
                ProofFact::Membership(membership) => ProofFact::Membership(ProofMembershipFact {
                    value: self.copy_expression_handle(other, membership.value),
                    domain: self.copy_item_identifier_span(other, membership.domain),
                    domain_arguments: self
                        .copy_type_reference_handle_span(other, membership.domain_arguments),
                }),
            };
            let copied = self.items.append_proof_fact(copied);
            if let Some(source_span) = other.items.proof_fact_source_span(source) {
                self.items.set_proof_fact_source_span(copied, source_span);
            }
            copied
        }

        pub(crate) fn copy_mathematical_definition(
            &mut self,
            other: &SyntaxTrees,
            definition: &MathematicalDefinition,
        ) -> MathematicalDefinition {
            MathematicalDefinition {
                name: definition.name.clone(),
                is_public: definition.is_public,
                type_parameters: self.copy_type_parameter_span(other, definition.type_parameters),
                parameters: self.copy_mapped_span(
                    other
                        .items
                        .mathematical_parameters(definition.parameters)
                        .to_vec(),
                    |this, handle| {
                        let parameter = other.items.mathematical_parameter(handle);
                        let node = MathematicalParameterNode {
                            name: parameter.name.clone(),
                            relevance: parameter.relevance,
                            ty: this.copy_mathematical_type(other, parameter.ty),
                        };
                        this.items.insert_mathematical_parameter(node)
                    },
                    |this, handle| this.items.append_mathematical_parameter_handle(handle),
                ),
                result: self.copy_mathematical_type(other, definition.result),
                body: match definition.body {
                    MathematicalDefinitionBody::Definition(term) => {
                        MathematicalDefinitionBody::Definition(
                            self.copy_expression_handle(other, term),
                        )
                    }
                    MathematicalDefinitionBody::Assumption => {
                        MathematicalDefinitionBody::Assumption
                    }
                },
            }
        }

        fn copy_mathematical_type(
            &mut self,
            other: &SyntaxTrees,
            handle: MathematicalTypeHandle,
        ) -> MathematicalTypeHandle {
            let node = match other.items.mathematical_type(handle) {
                MathematicalTypeNode::Ordinary(type_reference) => MathematicalTypeNode::Ordinary(
                    self.copy_type_reference_handle(other, *type_reference),
                ),
                MathematicalTypeNode::Arrow {
                    binder,
                    domain,
                    codomain,
                } => MathematicalTypeNode::Arrow {
                    binder: binder.clone(),
                    domain: self.copy_mathematical_type(other, *domain),
                    codomain: self.copy_mathematical_type(other, *codomain),
                },
                MathematicalTypeNode::Application { callee, arguments } => {
                    MathematicalTypeNode::Application {
                        callee: self.copy_mathematical_type(other, *callee),
                        arguments: self.copy_expression_handle_list(other, *arguments),
                    }
                }
            };
            self.items.insert_mathematical_type(node)
        }

        pub(crate) fn copy_item(&mut self, other: &SyntaxTrees, item: &Item) -> Item {
            match item {
                Item::Capability(capability) => {
                    Item::Capability(self.copy_capability_definition(other, capability))
                }
                Item::Conformance(conformance) => {
                    let copied = other
                        .items
                        .conformance_members(conformance.members)
                        .iter()
                        .map(|member| match member {
                            ConformanceMember::Machine(machine) => {
                                ConformanceMember::Machine(self.copy_machine(other, machine))
                            }
                            ConformanceMember::Synthesized(machine) => {
                                ConformanceMember::Synthesized(self.copy_machine(other, machine))
                            }
                            ConformanceMember::TraitDefault {
                                declaring_trait,
                                requirement_ordinal,
                                machine,
                            } => ConformanceMember::TraitDefault {
                                declaring_trait: declaring_trait.clone(),
                                requirement_ordinal: *requirement_ordinal,
                                machine: self.copy_machine(other, machine),
                            },
                            ConformanceMember::Selection {
                                declaring_trait,
                                requirement,
                                target,
                            } => ConformanceMember::Selection {
                                declaring_trait: declaring_trait.clone(),
                                requirement: requirement.clone(),
                                target: self.copy_item_identifier_span(other, *target),
                            },
                        })
                        .collect::<Vec<_>>();
                    let mut copied_start = arena::Handle::invalid();
                    let mut copied_count = 0u32;
                    for member in copied {
                        let handle = self.items.append_conformance_member(member);
                        if copied_count == 0 {
                            copied_start = handle;
                        }
                        copied_count = copied_count
                            .checked_add(1)
                            .expect("conformance member span count overflow");
                    }
                    let members = if copied_count == 0 {
                        arena::HandleSpan::empty()
                    } else {
                        arena::HandleSpan::from_parts(copied_start, copied_count)
                    };
                    Item::Conformance(ConformanceItem {
                        is_public: conformance.is_public,
                        lifetime_parameters: conformance.lifetime_parameters.clone(),
                        type_parameters: self
                            .copy_type_parameter_span(other, conformance.type_parameters),
                        trait_name: conformance.trait_name.clone(),
                        trait_lifetime_arguments: conformance.trait_lifetime_arguments.clone(),
                        trait_arguments: self
                            .copy_type_reference_handle_span(other, conformance.trait_arguments),
                        alias: conformance.alias.clone(),
                        members,
                    })
                }
                Item::Const(constant) => {
                    let type_reference =
                        self.copy_type_reference_handle(other, constant.type_reference);
                    let value = self.copy_expression_handle(other, constant.value);
                    Item::Const(ConstDefinition {
                        scope: constant.scope.clone(),
                        name: constant.name.clone(),
                        is_public: constant.is_public,
                        type_reference,
                        value,
                        pending_target_observation: constant.pending_target_observation,
                        normalization: constant.normalization.as_ref().map(|normalization| {
                            ConstInitializerNormalization {
                                authored_custody: normalization.authored_custody.clone(),
                                declaration_custody: normalization.declaration_custody.clone(),
                                authored_expression: self.copy_expression_handle(
                                    other,
                                    normalization.authored_expression,
                                ),
                                result_value: normalization.result_value.clone(),
                                selections: normalization.selections.clone(),
                                builtin_operators: normalization.builtin_operators.clone(),
                                call_selections: normalization.call_selections.clone(),
                            }
                        }),
                        pending_qualification: constant.pending_qualification.as_ref().map(
                            |obligation| {
                                if obligation.declaration == constant.name.source_span()
                                    && obligation.destination == constant.type_reference
                                    && obligation.initializer == constant.value
                                {
                                    ConstDestinationObligation {
                                        declaration: constant.name.source_span(),
                                        destination: type_reference,
                                        initializer: value,
                                    }
                                } else {
                                    obligation.clone()
                                }
                            },
                        ),
                        qualification: constant.qualification.as_ref().and_then(|qualification| {
                            (qualification.destination == constant.type_reference
                                && qualification.initializer == constant.value)
                                .then_some(ConstQualification {
                                    destination: type_reference,
                                    initializer: value,
                                })
                        }),
                    })
                }
                Item::Data(data) => Item::Data(self.copy_data_definition(other, data)),
                Item::TypeAlias(alias) => Item::TypeAlias(crate::item::TypeAliasDefinition {
                    name: alias.name.clone(),
                    is_public: alias.is_public,
                    lifetime_parameters: alias.lifetime_parameters.clone(),
                    type_parameters: self.copy_type_parameter_span(other, alias.type_parameters),
                    conformance_bounds: alias
                        .conformance_bounds
                        .iter()
                        .map(|bound| crate::item::GenericConformanceBound {
                            binder: bound.binder.clone(),
                            trait_name: bound.trait_name.clone(),
                            lifetime_arguments: bound.lifetime_arguments.clone(),
                            arguments: self.copy_type_reference_handle_span(other, bound.arguments),
                        })
                        .collect(),
                    authored_expansion: self
                        .copy_type_reference_handle(other, alias.authored_expansion),
                    expansion: self.copy_type_reference_handle(other, alias.expansion),
                }),
                Item::Domain(domain) => Item::Domain(DomainDefinition {
                    name: domain.name.clone(),
                    type_parameters: self.copy_type_parameter_span(other, domain.type_parameters),
                    target_type: self.copy_type_reference_handle(other, domain.target_type),
                    index_arguments: self
                        .copy_type_reference_handle_span(other, domain.index_arguments),
                    is_public: domain.is_public,
                    alias: domain.alias.as_ref().map(|alias| DomainAliasDefinition {
                        constituents: alias
                            .constituents
                            .iter()
                            .map(|constituent| self.copy_item_identifier_span(other, *constituent))
                            .collect(),
                    }),
                    cleanup_selection: domain
                        .cleanup_selection
                        .as_ref()
                        .map(|selection| self.copy_static_machine_argument(other, selection)),
                    authored_routes: domain.authored_routes.clone(),
                    classification: domain.classification,
                    predicate_presence: domain.predicate_presence,
                    facts: self.copy_domain_fact_span(other, domain.facts),
                    operators: self.copy_operator_definition_span(other, domain.operators),
                    semantic_clause_token_count: domain.semantic_clause_token_count,
                }),
                Item::Measure(measure) => {
                    Item::Measure(self.copy_measure_definition(other, measure))
                }
                Item::Module(module) => Item::Module(ModuleDeclaration {
                    path: self.copy_item_identifier_span(other, module.path),
                }),
                Item::Operator(operator) => {
                    Item::Operator(self.copy_operator_definition(other, operator))
                }
                Item::Package(package) => Item::Package(PackageDeclaration {
                    path: self.copy_item_identifier_span(other, package.path),
                }),
                Item::Proposition(proposition) => Item::Proposition(PropositionDefinition {
                    name: proposition.name.clone(),
                    is_public: proposition.is_public,
                    type_parameters: self
                        .copy_type_parameter_span(other, proposition.type_parameters),
                    parameters: self
                        .copy_state_parameter_handle_span(other, proposition.parameters),
                    transparent_formula_source_span: proposition.transparent_formula_source_span,
                    body: match proposition.body {
                        PropositionBody::Primitive => PropositionBody::Primitive,
                        PropositionBody::Witness { evidence } => PropositionBody::Witness {
                            evidence: self.copy_type_reference_handle(other, evidence),
                        },
                        PropositionBody::Transparent { proposition } => {
                            PropositionBody::Transparent {
                                proposition: self.copy_expression_handle(other, proposition),
                            }
                        }
                    },
                }),
                Item::Use(use_item) => Item::Use(UseItem {
                    path: self.copy_item_identifier_span(other, use_item.path),
                }),
                Item::Machine(machine) => Item::Machine(self.copy_machine(other, machine)),
                Item::Trait(trait_definition) => {
                    Item::Trait(self.copy_trait_definition(other, trait_definition))
                }
            }
        }

        fn copy_capability_definition(
            &mut self,
            other: &SyntaxTrees,
            capability: &CapabilityDefinition,
        ) -> CapabilityDefinition {
            CapabilityDefinition {
                name: capability.name.clone(),
                members: self.copy_capability_member_span(other, capability.members),
            }
        }

        fn copy_data_definition(
            &mut self,
            other: &SyntaxTrees,
            data: &DataDefinition,
        ) -> DataDefinition {
            DataDefinition {
                name: data.name.clone(),
                is_public: data.is_public,
                cleanup_selection: data
                    .cleanup_selection
                    .as_ref()
                    .map(|selection| self.copy_static_machine_argument(other, selection)),
                supply_mode: data.supply_mode,
                lifetime_parameters: data.lifetime_parameters.clone(),
                type_parameters: self.copy_type_parameter_span(other, data.type_parameters),
                conformance_bounds: data
                    .conformance_bounds
                    .iter()
                    .map(|bound| GenericConformanceBound {
                        binder: bound.binder.clone(),
                        trait_name: bound.trait_name.clone(),
                        lifetime_arguments: bound.lifetime_arguments.clone(),
                        arguments: self.copy_type_reference_handle_span(other, bound.arguments),
                    })
                    .collect(),
                generic_instance: data
                    .generic_instance
                    .map(|origin| self.copy_type_reference_handle(other, origin)),
                properties: data.properties,
                quotient: data.quotient.as_ref().map(|quotient| QuotientDefinition {
                    carrier: self.copy_type_reference_handle(other, quotient.carrier),
                    relation: self.copy_item_identifier_span(other, quotient.relation),
                    equivalence: quotient.equivalence.as_ref().map(|selection| {
                        QuotientEquivalenceSelection {
                            relation: self.copy_item_identifier_span(other, selection.relation),
                            trait_name: selection.trait_name.clone(),
                            trait_arguments: self
                                .copy_type_reference_handle_span(other, selection.trait_arguments),
                            conformance_name: selection.conformance_name.clone(),
                        }
                    }),
                }),
                requires_facts: self.copy_domain_fact_span(other, data.requires_facts),
                members: self.copy_data_member_span(other, data.members),
            }
        }

        fn copy_operator_definition(
            &mut self,
            other: &SyntaxTrees,
            operator: &OperatorDefinition,
        ) -> OperatorDefinition {
            OperatorDefinition {
                is_public: operator.is_public,
                is_boundary: operator.is_boundary,
                name: self.copy_item_identifier_span(other, operator.name),
                lifetime_parameters: operator.lifetime_parameters.clone(),
                type_parameters: self.copy_type_parameter_span(other, operator.type_parameters),
                parameters: self.copy_state_parameter_handle_span(other, operator.parameters),
                return_type: self.copy_type_reference_handle(other, operator.return_type),
                contracts: self.copy_capability_contract_span(other, operator.contracts),
                spelling: operator.spelling,
                token_count: operator.token_count,
            }
        }

        fn copy_measure_definition(
            &mut self,
            other: &SyntaxTrees,
            measure: &MeasureDefinition,
        ) -> MeasureDefinition {
            let parameter = if measure.parameter.is_valid() {
                let source_parameter = other.items.state_parameter(measure.parameter);
                let type_reference =
                    self.copy_type_reference_handle(other, source_parameter.type_reference);
                self.items.insert_state_parameter_node(StateParameterNode {
                    name: source_parameter.name.clone(),
                    type_reference,
                    is_const: source_parameter.is_const,
                    is_mutable: source_parameter.is_mutable,
                    relevance: source_parameter.relevance,
                })
            } else {
                StateParameterHandle::invalid()
            };

            MeasureDefinition {
                name: self.copy_item_identifier_span(other, measure.name),
                parameter,
                return_type: self.copy_type_reference_handle(other, measure.return_type),
                lexicographic: measure.lexicographic,
                body: self.copy_expression_handle_list(other, measure.body),
                token_count: measure.token_count,
            }
        }

        fn copy_operator_definition_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<OperatorDefinition>,
        ) -> HandleSpan<OperatorDefinition> {
            self.copy_mapped_span(
                other.items.operators(span),
                |this, operator| this.copy_operator_definition(other, operator),
                |this, operator| this.items.append_operator(operator),
            )
        }

        fn copy_machine(&mut self, other: &SyntaxTrees, machine: &Machine) -> Machine {
            let children = machine
                .nested_machines
                .handles()
                .map(|handle| self.copy_item(other, other.items.item(handle)))
                .collect::<Vec<_>>();
            let mut nested_machines = HandleSpan::empty();
            for child in children {
                let handle = self.items.append_item(child);
                nested_machines.push_contiguous(handle);
            }
            Machine {
                target_family_interface: machine.target_family_interface,
                name: machine.name.clone(),
                generic_data_template: machine.generic_data_template.clone(),
                default_body_trait: machine.default_body_trait.clone(),
                attached_data: machine.attached_data.clone(),
                spelling: machine.spelling,
                is_public: machine.is_public,
                target: machine.target.clone(),
                authored_target: machine.authored_target.clone(),
                boundary: machine.boundary,
                bodyless: machine.bodyless,
                lifetime_parameters: machine.lifetime_parameters.clone(),
                type_parameters: self.copy_type_parameter_span(other, machine.type_parameters),
                satisfies: self.copy_mapped_span(
                    other.items.satisfies_clauses(machine.satisfies).to_vec(),
                    |this, clause| SatisfiesClause {
                        trait_name: clause.trait_name,
                        lifetime_arguments: clause.lifetime_arguments,
                        arguments: this.copy_type_reference_handle_span(other, clause.arguments),
                        requirement: clause.requirement,
                        alias: clause.alias,
                        via: clause.via,
                        via_expression: if clause.via_expression.is_valid() {
                            this.copy_expression_handle(other, clause.via_expression)
                        } else {
                            ExpressionHandle::invalid()
                        },
                        via_keyword_source_span: clause.via_keyword_source_span,
                    },
                    |this, clause| this.items.append_satisfies_clause(clause),
                ),
                conformance_bounds: machine
                    .conformance_bounds
                    .iter()
                    .map(|bound| GenericConformanceBound {
                        binder: bound.binder.clone(),
                        trait_name: bound.trait_name.clone(),
                        lifetime_arguments: bound.lifetime_arguments.clone(),
                        arguments: self.copy_type_reference_handle_span(other, bound.arguments),
                    })
                    .collect(),
                terminates_guarantee: machine.terminates_guarantee,
                ranking_subjects: self.copy_expression_handle_list(other, machine.ranking_subjects),
                ranking_view: self.copy_item_identifier_span(other, machine.ranking_view),
                ranking_view_arguments: self
                    .copy_expression_handle_list(other, machine.ranking_view_arguments),
                ranking_range: if machine.ranking_range.is_valid() {
                    self.copy_expression_handle(other, machine.ranking_range)
                } else {
                    ExpressionHandle::invalid()
                },
                service_reach_is_installation_bound: machine.service_reach_is_installation_bound,
                service_reach_keyword_source_spans: machine
                    .service_reach_keyword_source_spans
                    .clone(),
                service_reaches: self.copy_item_identifier_span(other, machine.service_reaches),
                invokes: self.copy_item_identifier_span(other, machine.invokes),
                suspends_keyword_source_spans: machine.suspends_keyword_source_spans.clone(),
                blocks_keyword_source_spans: machine.blocks_keyword_source_spans.clone(),
                suspends: machine.suspends,
                blocks: machine.blocks,
                admitted_same_stack_bytes: machine.admitted_same_stack_bytes,
                declares_no_crash_route: machine.declares_no_crash_route,
                contracts: self.copy_capability_contract_span(other, machine.contracts),
                states: self.copy_state_handle_span(other, machine.states),
                nested_machines,
            }
        }

        fn copy_trait_definition(
            &mut self,
            other: &SyntaxTrees,
            trait_definition: &TraitDefinition,
        ) -> TraitDefinition {
            TraitDefinition {
                is_boundary: trait_definition.is_boundary,
                is_public: trait_definition.is_public,
                name: trait_definition.name.clone(),
                lifetime_parameters: trait_definition.lifetime_parameters.clone(),
                type_parameters: self
                    .copy_type_parameter_span(other, trait_definition.type_parameters),
                conformance_bounds: trait_definition
                    .conformance_bounds
                    .iter()
                    .map(|bound| GenericConformanceBound {
                        binder: bound.binder.clone(),
                        trait_name: bound.trait_name.clone(),
                        lifetime_arguments: bound.lifetime_arguments.clone(),
                        arguments: self.copy_type_reference_handle_span(other, bound.arguments),
                    })
                    .collect(),
                parents: self.copy_type_reference_handle_span(other, trait_definition.parents),
                requires: self.copy_item_identifier_span(other, trait_definition.requires),
                machines: self.copy_state_signature_handle_span(other, trait_definition.machines),
                refines: trait_definition
                    .refines
                    .map(|base| self.copy_type_reference_handle(other, base)),
                refinement_clauses: trait_definition
                    .refinement_clauses
                    .iter()
                    .map(|clause| TraitRefinementClause {
                        requirement: clause.requirement.clone(),
                        signature: self.copy_state_signature(other, &clause.signature),
                    })
                    .collect(),
            }
        }

        pub(crate) fn copy_type_parameter_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<TypeParameter>,
        ) -> HandleSpan<TypeParameter> {
            self.copy_mapped_span(
                other.items.type_parameters(span).to_vec(),
                |this, parameter| {
                    let kind = match &parameter.kind {
                        TypeParameterKind::Type => TypeParameterKind::Type,
                        TypeParameterKind::Implementation => TypeParameterKind::Implementation,
                        TypeParameterKind::Const { type_reference } => TypeParameterKind::Const {
                            type_reference: this.copy_type_reference_handle(other, *type_reference),
                        },
                        TypeParameterKind::Value { type_reference } => TypeParameterKind::Value {
                            type_reference: this.copy_type_reference_handle(other, *type_reference),
                        },
                        TypeParameterKind::Machine { contract } => TypeParameterKind::Machine {
                            contract: contract.as_ref().map(|contract| match contract {
                                MachineParameterContract::Inferred => {
                                    MachineParameterContract::Inferred
                                }
                                MachineParameterContract::RequirementIdentity => {
                                    MachineParameterContract::RequirementIdentity
                                }
                                MachineParameterContract::Structural(signature) => {
                                    MachineParameterContract::Structural(
                                        this.copy_state_signature(other, signature),
                                    )
                                }
                                MachineParameterContract::Nominal {
                                    requirement,
                                    trait_arguments,
                                } => MachineParameterContract::Nominal {
                                    requirement: this
                                        .copy_item_identifier_span(other, *requirement),
                                    trait_arguments: this
                                        .copy_type_reference_handle_span(other, *trait_arguments),
                                },
                            }),
                        },
                        TypeParameterKind::Proposition { contract } => {
                            TypeParameterKind::Proposition {
                                contract: contract.as_ref().map(|contract| {
                                    PropositionParameterSignature {
                                        name: contract.name.clone(),
                                        parameters: this.copy_state_parameter_handle_span(
                                            other,
                                            contract.parameters,
                                        ),
                                    }
                                }),
                            }
                        }
                    };
                    TypeParameter {
                        name: parameter.name.clone(),
                        kind,
                        bounds: parameter.bounds,
                    }
                },
                |this, parameter| this.items.append_type_parameter(parameter),
            )
        }

        fn copy_capability_member_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<CapabilityMember>,
        ) -> HandleSpan<CapabilityMember> {
            self.copy_mapped_span(
                other.items.capability_members(span),
                |this, member| match member {
                    CapabilityMember::Field(field) => CapabilityMember::Field(CapabilityField {
                        name: field.name.clone(),
                        type_reference: this
                            .copy_type_reference_handle(other, field.type_reference),
                    }),
                    CapabilityMember::State(state) => CapabilityMember::State(CapabilityState {
                        signature: this.copy_state_signature(other, &state.signature),
                        contracts: this.copy_capability_contract_span(other, state.contracts),
                    }),
                },
                |this, member| this.items.append_capability_member(member),
            )
        }

        pub(crate) fn copy_capability_contract_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<CapabilityContract>,
        ) -> HandleSpan<CapabilityContract> {
            let contracts = other
                .items
                .capability_contracts(span)
                .iter()
                .map(|contract| CapabilityContract {
                    kind: match &contract.kind {
                        CapabilityContractKind::Ensures => CapabilityContractKind::Ensures,
                        CapabilityContractKind::EnsuresForResultCase { result_case } => {
                            CapabilityContractKind::EnsuresForResultCase {
                                result_case: self.copy_item_identifier_span(other, *result_case),
                            }
                        }
                        CapabilityContractKind::Requires => CapabilityContractKind::Requires,
                        CapabilityContractKind::Crashes { cause } => {
                            CapabilityContractKind::Crashes { cause: *cause }
                        }
                    },
                    keyword_source_span: contract.keyword_source_span,
                    binding: contract.binding.clone(),
                    facts: self.copy_domain_fact_span(other, contract.facts),
                    token_count: contract.token_count,
                })
                .collect::<Vec<_>>();
            self.copy_span(contracts, |this, contract| {
                this.items.append_capability_contract(contract)
            })
        }

        fn copy_data_member_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<DataMember>,
        ) -> HandleSpan<DataMember> {
            self.copy_mapped_span(
                other.items.data_members(span),
                |this, member| match member {
                    DataMember::Field(field) => DataMember::Field(DataField {
                        identity: field.identity,
                        name: field.name.clone(),
                        relevance: field.relevance,
                        type_reference: this
                            .copy_type_reference_handle(other, field.type_reference),
                    }),
                    DataMember::Variant(variant) => DataMember::Variant(DataVariant {
                        identity: variant.identity,
                        name: variant.name.clone(),
                        payload: this.copy_data_payload_field_span(other, variant.payload),
                        requires_facts: this.copy_domain_fact_span(other, variant.requires_facts),
                        retired_payload_identities: variant.retired_payload_identities.clone(),
                    }),
                    DataMember::Retired(identity) => DataMember::Retired(*identity),
                },
                |this, member| this.items.append_data_member(member),
            )
        }

        fn copy_data_payload_field_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<DataField>,
        ) -> HandleSpan<DataField> {
            self.copy_mapped_span(
                other.items.data_payload_fields(span),
                |this, field| DataField {
                    identity: field.identity,
                    name: field.name.clone(),
                    relevance: field.relevance,
                    type_reference: this.copy_type_reference_handle(other, field.type_reference),
                },
                |this, field| this.items.append_data_payload_field(field),
            )
        }

        pub(crate) fn copy_domain_fact_span(
            &mut self,
            other: &SyntaxTrees,
            span: HandleSpan<ProofFact>,
        ) -> HandleSpan<ProofFact> {
            let mut copied = HandleSpan::empty();
            for source in span.handles() {
                copied.push_contiguous(self.copy_proof_fact(other, source));
            }
            copied
        }
    }
}
