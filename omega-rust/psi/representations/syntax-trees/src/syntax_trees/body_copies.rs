//! Copying states, statements, transitions, type references, expressions
//! and spans between syntax trees.

use crate::expression::{
    ExpressionHandle, ExpressionNode, TableBinaryExpression, TableCallExpression,
    TableCastExpression, TableIndexedExpression, TableMemberExpression, TableStructLiteral,
    TableStructLiteralField,
};
use crate::identifier::Identifier;
use crate::item::{
    State, StateHandle, StateParameterHandle, StateParameterNode, StateSignature,
    StateSignatureHandle,
};
use crate::statement::{
    StatementHandle, StatementNode, TableAssemblyFact, TableAssignment, TableCall, TableLocalData,
    TableTransition, TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode,
};
use crate::syntax_trees::SyntaxTrees;
use crate::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use arena::{Handle, HandleSpan};

impl SyntaxTrees {
    pub(crate) fn copy_state_handle_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<StateHandle>,
    ) -> HandleSpan<StateHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;
        for handle in other.items.state_handles(span).iter().copied() {
            let state = other.items.state(handle);
            let parameters = self.copy_state_parameter_handle_span(other, state.parameters);
            let return_type = self.copy_type_reference_handle(other, state.return_type);
            let contracts = self.copy_capability_contract_span(other, state.contracts);
            let statements = self.copy_statement_handle_span(other, state.statements);
            let copied = self.items.insert_state(&State {
                name: state.name.clone(),
                parameters,
                return_type,
                contracts,
                statements,
            });
            let copied = self.items.append_state_handle(copied);
            if count == 0 {
                start = copied;
            }
            count += 1;
        }
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    pub(crate) fn copy_state_signature_handle_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<StateSignatureHandle>,
    ) -> HandleSpan<StateSignatureHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;
        for handle in other.items.state_signatures(span).iter().copied() {
            let signature = other.items.state_signature(handle);
            let copied_signature = self.copy_state_signature_node(other, signature);
            let copied = self.items.insert_state_signature(&copied_signature);
            let copied = self.items.append_state_signature_handle(copied);
            if count == 0 {
                start = copied;
            }
            count += 1;
        }
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    pub(crate) fn copy_state_signature_value(
        &mut self,
        other: &SyntaxTrees,
        signature: &StateSignature,
    ) -> StateSignature {
        StateSignature {
            name: signature.name.clone(),
            spelling: signature.spelling,
            lifetime_parameters: signature.lifetime_parameters.clone(),
            type_parameters: self.copy_type_parameter_span(other, signature.type_parameters),
            is_default: signature.is_default,
            parameters: self.copy_state_parameter_handle_span(other, signature.parameters),
            native_callback_parameters: signature.native_callback_parameters.clone(),
            return_type: self.copy_type_reference_handle(other, signature.return_type),
            service_reach_is_installation_bound: signature.service_reach_is_installation_bound,
            service_reach_keyword_source_spans: signature
                .service_reach_keyword_source_spans
                .clone(),
            service_reaches: self.copy_item_identifier_span(other, signature.service_reaches),
            invokes: self.copy_item_identifier_span(other, signature.invokes),
            suspends_keyword_source_spans: signature.suspends_keyword_source_spans.clone(),
            blocks_keyword_source_spans: signature.blocks_keyword_source_spans.clone(),
            suspends: signature.suspends,
            blocks: signature.blocks,
            contracts: self.copy_capability_contract_span(other, signature.contracts),
            default_body: self.copy_statement_handle_span(other, signature.default_body),
            terminates_guarantee: signature.terminates_guarantee,
            where_facts: self.copy_domain_fact_span(other, signature.where_facts),
        }
    }

    pub(crate) fn copy_state_signature_node(
        &mut self,
        other: &SyntaxTrees,
        signature: &crate::item::StateSignatureNode,
    ) -> StateSignature {
        StateSignature {
            name: signature.name.clone(),
            spelling: signature.spelling,
            lifetime_parameters: signature.lifetime_parameters.clone(),
            type_parameters: self.copy_type_parameter_span(other, signature.type_parameters),
            is_default: signature.is_default,
            parameters: self.copy_state_parameter_handle_span(other, signature.parameters),
            native_callback_parameters: signature.native_callback_parameters.clone(),
            return_type: self.copy_type_reference_handle(other, signature.return_type),
            service_reach_is_installation_bound: signature.service_reach_is_installation_bound,
            service_reach_keyword_source_spans: signature
                .service_reach_keyword_source_spans
                .clone(),
            service_reaches: self.copy_item_identifier_span(other, signature.service_reaches),
            invokes: self.copy_item_identifier_span(other, signature.invokes),
            suspends_keyword_source_spans: signature.suspends_keyword_source_spans.clone(),
            blocks_keyword_source_spans: signature.blocks_keyword_source_spans.clone(),
            suspends: signature.suspends,
            blocks: signature.blocks,
            contracts: self.copy_capability_contract_span(other, signature.contracts),
            default_body: self.copy_statement_handle_span(other, signature.default_body),
            terminates_guarantee: signature.terminates_guarantee,
            where_facts: self.copy_domain_fact_span(other, signature.where_facts),
        }
    }

    pub(crate) fn copy_state_parameter_handle_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<StateParameterHandle>,
    ) -> HandleSpan<StateParameterHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;
        for handle in other.items.state_parameters(span).iter().copied() {
            let parameter = other.items.state_parameter(handle);
            let type_reference = self.copy_type_reference_handle(other, parameter.type_reference);
            let copied = self.items.insert_state_parameter_node(StateParameterNode {
                name: parameter.name.clone(),
                type_reference,
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
                is_self: parameter.is_self,
                relevance: parameter.relevance,
            });
            let copied = self.items.append_state_parameter_handle(copied);
            if count == 0 {
                start = copied;
            }
            count += 1;
        }
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn copy_statement_handle_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<StatementHandle>,
    ) -> HandleSpan<StatementHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;
        for handle in other.items.statements(span).iter().copied() {
            let statement = self.copy_statement_node(other, other.statements.statement(handle));
            let copied = self.statements.insert(statement);
            let copied = self.items.append_statement_handle(copied);
            if count == 0 {
                start = copied;
            }
            count += 1;
        }
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn copy_statement_node(
        &mut self,
        other: &SyntaxTrees,
        statement: &StatementNode,
    ) -> StatementNode {
        match statement {
            StatementNode::RootBinding(binding) => {
                StatementNode::RootBinding(crate::statement::RootBinding {
                    receiver: self.copy_expression_handle(other, binding.receiver),
                    implementation_operand: self
                        .copy_expression_handle(other, binding.implementation_operand),
                    ..binding.clone()
                })
            }
            StatementNode::AssemblyFact(fact) => StatementNode::AssemblyFact(TableAssemblyFact {
                kind: fact.kind,
                expression: self.copy_expression_handle(other, fact.expression),
            }),
            StatementNode::Assignment(assignment) => StatementNode::Assignment(TableAssignment {
                target: self.copy_expression_handle(other, assignment.target),
                value: self.copy_expression_handle(other, assignment.value),
            }),
            StatementNode::Call(call) => StatementNode::Call(TableCall {
                target_is_static: call.target_is_static,
                receiver: self.copy_statement_identifier_span(other, call.receiver),
                receiver_starts_at_self: call.receiver_starts_at_self,
                target: call.target.clone(),
                machine_arguments: call.machine_arguments.clone(),
                arguments: self.copy_statement_expression_span(other, call.arguments),
                evidence_arguments: call.evidence_arguments.clone(),
                operational_acknowledgement: call.operational_acknowledgement,
                discards_result: call.discards_result,
            }),
            StatementNode::ProofOutputBindingStatement(binding) => {
                StatementNode::ProofOutputBindingStatement(
                    crate::statement::TableProofOutputBindingStatement {
                        bindings: binding.bindings.clone(),
                        call: self.copy_expression_handle(other, binding.call),
                    },
                )
            }
            StatementNode::Expression(value) => {
                StatementNode::Expression(self.copy_expression_handle(other, *value))
            }
            StatementNode::LocalData(local_data) => StatementNode::LocalData(TableLocalData {
                name: local_data.name.clone(),
                type_reference: self.copy_type_reference_handle(other, local_data.type_reference),
                initial_value: self.copy_expression_handle(other, local_data.initial_value),
                is_mutable: local_data.is_mutable,
                relevance: local_data.relevance,
            }),
            StatementNode::Transition(transition) => StatementNode::Transition(TableTransition {
                target: self.copy_transition_target(other, transition.target),
                continuation: self.copy_transition_target(other, transition.continuation),
                guard: match transition.guard {
                    TransitionGuardNode::Always => TransitionGuardNode::Always,
                    TransitionGuardNode::When(expression) => {
                        TransitionGuardNode::When(self.copy_expression_handle(other, expression))
                    }
                },
                proof_selectors: self.statements.insert_outcome_proof_selectors(
                    other
                        .statements
                        .outcome_proof_selectors(transition.proof_selectors)
                        .iter()
                        .cloned(),
                ),
                exit: transition.exit,
                source_span: transition.source_span,
            }),
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
                path_starts_at_self,
                arguments,
                evidence_arguments,
                source_span,
            } => TransitionTargetNode::Named {
                path: self.copy_statement_identifier_span(other, *path),
                path_starts_at_self: *path_starts_at_self,
                arguments: self.copy_statement_expression_span(other, *arguments),
                evidence_arguments: evidence_arguments.clone(),
                source_span: *source_span,
            },
            TransitionTargetNode::Value(value) => {
                TransitionTargetNode::Value(self.copy_expression_handle(other, *value))
            }
            TransitionTargetNode::SelfTarget => TransitionTargetNode::SelfTarget,
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

        let copied = match other.type_references.type_reference(handle) {
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => {
                let referee = self.copy_type_reference_handle(other, *referee);
                self.type_references.insert_reference_with_lifetime(
                    referee,
                    *access,
                    lifetime.clone(),
                )
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
                self.type_references
                    .insert_fixed_array(element_type, length.clone())
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
                self.type_references.insert(TypeReferenceNode::Generic {
                    base_name: base_name.clone(),
                    lifetime_arguments: lifetime_arguments.clone(),
                    arguments,
                })
            }
            TypeReferenceNode::ConstExpression(expression) => {
                let expression = self.copy_expression_handle(other, *expression);
                self.type_references
                    .insert(TypeReferenceNode::ConstExpression(expression))
            }
            TypeReferenceNode::DynamicTrait { name, conformance } => {
                self.type_references
                    .insert(TypeReferenceNode::DynamicTrait {
                        name: name.clone(),
                        conformance: conformance.clone(),
                    })
            }
            TypeReferenceNode::Named(name) => self.type_references.insert_named(name.clone()),
            TypeReferenceNode::SelfType => self.type_references.insert_self_type(),
            TypeReferenceNode::Unit => self.type_references.insert_unit(),
        };
        let origin = other.type_references.generic_application_origin(handle);
        if let TypeReferenceNode::Constrained { constraints, .. } =
            other.type_references.type_reference(handle)
        {
            for ordinal in 0..constraints.count() as usize {
                if let Some(value) = other
                    .type_references
                    .integer_range_normalization(handle, ordinal)
                {
                    self.type_references.retain_integer_range_normalization(
                        copied,
                        ordinal,
                        value.clone(),
                    );
                }
            }
        }
        if let Some(normalization) = other.type_references.const_argument_normalization(handle) {
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
            let expression = self.copy_expression_handle(other, normalization.authored_expression);
            self.type_references
                .retain_const_argument_expression(copied, expression);
        }
        if origin.is_valid() {
            let application = self.copy_type_reference_handle(other, origin);
            self.type_references
                .retain_generic_application_origin(copied, application);
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
                    TypeConstraintNode::Domain(crate::types::DomainConstraint {
                        name: domain.name.clone(),
                        arguments: this.copy_type_reference_handle_span(other, domain.arguments),
                    })
                }
                TypeConstraintNode::Range {
                    minimum,
                    maximum,
                    end_inclusive,
                } => TypeConstraintNode::Range {
                    minimum: this.copy_expression_handle(other, *minimum),
                    maximum: this.copy_expression_handle(other, *maximum),
                    end_inclusive: *end_inclusive,
                },
                TypeConstraintNode::ArithmeticDomain(domain) => {
                    TypeConstraintNode::ArithmeticDomain(*domain)
                }
            },
            |this, constraint| this.type_references.append_constraint(constraint),
        )
    }

    pub(crate) fn copy_expression_handle(
        &mut self,
        other: &SyntaxTrees,
        handle: ExpressionHandle,
    ) -> ExpressionHandle {
        if !handle.is_valid() {
            return ExpressionHandle::invalid();
        }

        let expression = match other.expressions.expression(handle) {
            ExpressionNode::Match(dispatch) => {
                let subject = self.copy_expression_handle(other, dispatch.subject);
                let mut arms =
                    Vec::with_capacity(other.expressions.match_arms(dispatch.arms).len());
                for arm in other.expressions.match_arms(dispatch.arms) {
                    let pattern = match arm.pattern {
                        crate::expression::MatchPattern::Value(value) => {
                            crate::expression::MatchPattern::Value(
                                self.copy_expression_handle(other, value),
                            )
                        }
                        crate::expression::MatchPattern::Wildcard => {
                            crate::expression::MatchPattern::Wildcard
                        }
                    };
                    let value = self.copy_expression_handle(other, arm.value);
                    arms.push(crate::expression::TableMatchArm {
                        pattern,
                        value,
                        source_span: arm.source_span,
                    });
                }
                let arms = self.expressions.insert_match_arms(arms);
                ExpressionNode::Match(crate::expression::TableMatchExpression { subject, arms })
            }
            ExpressionNode::ArrayLiteral(values) => {
                ExpressionNode::ArrayLiteral(self.copy_expression_handle_list(other, *values))
            }
            ExpressionNode::Atomic(atomic) => {
                ExpressionNode::Atomic(crate::expression::TableAtomicExpression {
                    value: self.copy_expression_handle(other, atomic.value),
                    result: if atomic.result.is_valid() {
                        self.copy_expression_handle(other, atomic.result)
                    } else {
                        ExpressionHandle::invalid()
                    },
                    ordering: atomic.ordering,
                    result_custody: atomic.result_custody,
                })
            }
            ExpressionNode::Binary(binary) => ExpressionNode::Binary(TableBinaryExpression {
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
                semantic_domain: self.copy_expression_identifier_span(other, cast.semantic_domain),
                semantic_domain_arguments: self
                    .copy_type_reference_handle_span(other, cast.semantic_domain_arguments),
                form: cast.form,
            }),
            ExpressionNode::Call(call) => ExpressionNode::Call(TableCallExpression {
                target_is_static: call.target_is_static,
                receiver: self.copy_expression_handle(other, call.receiver),
                target: call.target.clone(),
                machine_arguments: call.machine_arguments.clone(),
                arguments: self.copy_expression_handle_list(other, call.arguments),
                evidence_arguments: call.evidence_arguments.clone(),
                operational_acknowledgement: call.operational_acknowledgement,
            }),
            ExpressionNode::Float(value) => ExpressionNode::Float(value.clone()),
            ExpressionNode::Indexed(indexed) => ExpressionNode::Indexed(TableIndexedExpression {
                collection: self.copy_expression_handle(other, indexed.collection),
                index: self.copy_expression_handle(other, indexed.index),
            }),
            ExpressionNode::Integer(value) => ExpressionNode::Integer(value.clone()),
            ExpressionNode::Membership(membership) => {
                ExpressionNode::Membership(crate::expression::TableMembershipExpression {
                    value: self.copy_expression_handle(other, membership.value),
                    domain: self.copy_expression_identifier_span(other, membership.domain),
                })
            }
            ExpressionNode::Member(member) => ExpressionNode::Member(TableMemberExpression {
                receiver: self.copy_expression_handle(other, member.receiver),
                member: member.member.clone(),
                case_variant: member.case_variant.clone(),
            }),
            ExpressionNode::Borrow(expression) => {
                ExpressionNode::Borrow(crate::expression::TableBorrowExpression {
                    target: self.copy_expression_handle(other, expression.target),
                    access: expression.access,
                })
            }
            ExpressionNode::Name(path) => {
                ExpressionNode::Name(self.copy_expression_identifier_span(other, *path))
            }
            ExpressionNode::Range(range) => {
                ExpressionNode::Range(crate::expression::TableRangeExpression {
                    start: self.copy_expression_handle(other, range.start),
                    end: self.copy_expression_handle(other, range.end),
                    end_inclusive: range.end_inclusive,
                })
            }
            ExpressionNode::SelfValue => ExpressionNode::SelfValue,
            ExpressionNode::StructLiteral(struct_literal) => {
                ExpressionNode::StructLiteral(TableStructLiteral {
                    constructor_name: struct_literal.constructor_name.clone(),
                    fields: self.copy_struct_field_span(other, struct_literal.fields),
                })
            }
            ExpressionNode::String(value) => ExpressionNode::String(value.clone()),
            ExpressionNode::Unary(unary) => {
                let operand = self.copy_expression_handle(other, unary.operand);
                ExpressionNode::Unary(crate::expression::TableUnaryExpression {
                    operator: unary.operator,
                    operand,
                })
            }
            ExpressionNode::ZeroValue(type_reference) => {
                ExpressionNode::ZeroValue(self.copy_type_reference_handle(other, *type_reference))
            }
            ExpressionNode::TypeExpression(type_reference) => ExpressionNode::TypeExpression(
                self.copy_type_reference_handle(other, *type_reference),
            ),
        };

        let source_span = other.expressions.source_span(handle);
        let copied = self.expressions.insert(expression);
        self.expressions.set_source_span(copied, source_span);
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
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for value in values {
            let value = map(self, value);
            let handle = append(self, value);
            if count == 0 {
                start = handle;
            }
            count = count.checked_add(1).expect("copied span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    pub(crate) fn copy_span<T>(
        &mut self,
        values: impl IntoIterator<Item = T>,
        mut append: impl FnMut(&mut Self, T) -> Handle<T>,
    ) -> HandleSpan<T> {
        let mut start = Handle::invalid();
        let mut count = 0u32;
        for value in values {
            let handle = append(self, value);
            if count == 0 {
                start = handle;
            }
            count += 1;
        }
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }
}
