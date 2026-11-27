use crate::SyntaxTrees;
use crate::expression::{
    ExpressionHandle, ExpressionNode, MatchPattern, MatchSubjectCapture, StaticMachineArgument,
    TableAtomicExpression, TableBinaryExpression, TableBorrowExpression, TableCallExpression,
    TableCastExpression, TableDestructurePattern, TableIndexedExpression, TableMatchArm,
    TableMatchExpression, TableMemberExpression, TableMembershipExpression, TableRangeExpression,
    TableSavedSubject, TableStructLiteral, TableStructLiteralField, TableUnaryExpression,
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
    TableProofOutputBindingStatement, TableTransition, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};
use crate::types::{DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
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
            parameters: $self.copy_state_parameter_handle_span($other, $signature.parameters),
            native_callback_parameters: $signature.native_callback_parameters.clone(),
            return_type: $self.copy_type_reference_handle($other, $signature.return_type),
            service_reach_is_installation_bound: $signature.service_reach_is_installation_bound,
            service_reach_keyword_source_spans: $signature
                .service_reach_keyword_source_spans
                .clone(),
            service_reaches: $self.copy_type_reference_handle_span($other, $signature.service_reaches),
            invokes: $self.copy_type_reference_handle_span($other, $signature.invokes),
            suspends_keyword_source_spans: $signature.suspends_keyword_source_spans.clone(),
            blocks_keyword_source_spans: $signature.blocks_keyword_source_spans.clone(),
            suspends: $signature.suspends,
            blocks: $signature.blocks,
                admitted_same_stack_bytes: $signature.admitted_same_stack_bytes,
            contracts: $self.copy_capability_contract_span($other, $signature.contracts),
            default_body: $signature
                .default_body
                .map(|body| crate::item::MachineBody {
                    assembly_target_requirement: body.assembly_target_requirement,
                    statements: $self.copy_statement_handle_span($other, body.statements),
                    nested_machines: $self.copy_nested_machines($other, body.nested_machines),
                }),
            terminates_guarantee: $signature.terminates_guarantee,
            declares_no_crash_route: $signature.declares_no_crash_route,
        }
    }};
}

impl SyntaxCopy<'_> {
    /// A declaration operand keeps its name and telescope. A structural type or
    /// computed constant keeps its type-reference node, as it does at a written call.
    fn static_argument_for_actual(&self, actual: TypeReferenceHandle) -> StaticMachineArgument {
        let reference = || StaticMachineArgument {
            type_reference: actual,
            ..Default::default()
        };
        match self.type_references.type_reference(actual) {
            TypeReferenceNode::Named(name) => StaticMachineArgument {
                path: Box::new([name.clone()]),
                ..Default::default()
            },
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => StaticMachineArgument {
                path: Box::new([base_name.clone()]),
                application: Some(Box::new(crate::expression::StaticSymbolApplication {
                    member: arena::Handle::invalid(),
                    lifetime_arguments: lifetime_arguments.clone().into_boxed_slice(),
                    arguments: self
                        .type_references
                        .type_reference_handles(*arguments)
                        .iter()
                        .map(|argument| self.static_argument_for_actual(*argument))
                        .collect(),
                })),
                ..Default::default()
            },
            TypeReferenceNode::StaticSelection(argument) => argument.clone(),
            TypeReferenceNode::ConstExpression(expression) => {
                match self.expressions.expression(*expression) {
                    ExpressionNode::Integer(literal) => StaticMachineArgument {
                        const_literal: Some(literal.clone()),
                        ..Default::default()
                    },
                    ExpressionNode::Name {
                        members,
                        application,
                    } => StaticMachineArgument {
                        path: self.expressions.identifier_path_members(*members).into(),
                        application: application.clone(),
                        ..Default::default()
                    },
                    _ => reference(),
                }
            }
            _ => reference(),
        }
    }

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
        if !self.scoped_arguments.is_empty()
            && !argument.type_reference.is_valid()
            && argument.application.is_none()
            && argument.const_literal.is_none()
            && argument.evidence_projection.is_none()
            && let [name] = argument.path.as_ref()
            && let Some(actual) = self
                .scoped_arguments
                .iter()
                .find(|arg| arg.binder == name.as_str(other))
        {
            *argument = self.static_argument_for_actual(actual.ty);
            return;
        }
        argument.type_reference = self.copy_type_reference_handle(other, argument.type_reference);
        if let Some(application) = &mut argument.application {
            for name in &mut application.lifetime_arguments {
                *name = self.copied_lifetime(other, name);
            }
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
                    assembly_target_requirement: state.assembly_target_requirement,
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
        self.with_binders(
            other,
            signature.type_parameters,
            &signature.lifetime_parameters,
            |scope| copied_state_signature!(scope, other, signature),
        )
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
                let statement = this.copy_statement_node(other, other.statements.statement(handle));
                this.statements.insert(statement)
            },
            |this, handle| this.items.append_statement_handle(handle),
        )
    }

    pub(crate) fn copy_authored_assembly(
        &mut self,
        other: &SyntaxTrees,
        mut assembly: crate::statement::AuthoredAssembly,
    ) -> crate::statement::AuthoredAssembly {
        for expression in assembly
            .requires
            .iter_mut()
            .chain(assembly.ensures.iter_mut())
        {
            *expression = self.copy_expression_handle(other, *expression);
        }
        for block in &mut assembly.blocks {
            for expression in block.requires.iter_mut().chain(block.ensures.iter_mut()) {
                *expression = self.copy_expression_handle(other, *expression);
            }
            for instruction in &mut block.instructions {
                self.copy_assembly_operands(other, &mut instruction.operands);
            }
        }
        assembly
    }

    pub(crate) fn copy_assembly_operands(
        &mut self,
        other: &SyntaxTrees,
        operands: &mut crate::statement::AssemblyOperands,
    ) {
        match operands {
            crate::statement::AssemblyOperands::Expressions(values) => {
                for value in values {
                    *value = self.copy_expression_handle(other, *value);
                }
            }
            crate::statement::AssemblyOperands::Transition(target) => {
                *target = self.copy_transition_target(other, *target);
            }
        }
    }

    pub(crate) fn copy_statement_node(
        &mut self,
        other: &SyntaxTrees,
        statement: &StatementNode,
    ) -> StatementNode {
        match statement {
            StatementNode::AuthoredAssembly(handle) => {
                let assembly = self.copy_authored_assembly(
                    other,
                    other.statements.authored_assembly(*handle).clone(),
                );
                StatementNode::AuthoredAssembly(self.statements.insert_authored_assembly(assembly))
            }
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
            StatementNode::AssemblyFact(fact) => StatementNode::AssemblyFact(TableAssemblyFact {
                kind: fact.kind,
                expression: self.copy_expression_handle(other, fact.expression),
            }),
            StatementNode::Assignment(assignment) => StatementNode::Assignment(TableAssignment {
                target: self.copy_expression_handle(other, assignment.target),
                value: self.copy_expression_handle(other, assignment.value),
            }),
            StatementNode::Call(call) => StatementNode::Call(TableCall {
                expression: self.copy_expression_handle(other, call.expression),
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
                type_reference: self.copy_type_reference_handle(other, local_data.type_reference),
                initial_value: self.copy_expression_handle(other, local_data.initial_value),
                is_mutable: local_data.is_mutable,
                relevance: local_data.relevance,
                pattern_fields: self.copy_match_fields(other, local_data.pattern_fields),
            }),
            StatementNode::Dispatch(handle) => {
                let dispatch = other.statements.dispatch(*handle);
                let subjects = other
                    .statements
                    .expression_handles(dispatch.subjects)
                    .iter()
                    .map(|subject| self.copy_expression_handle(other, *subject))
                    .collect::<Vec<_>>();
                let arms = dispatch
                    .arms
                    .iter()
                    .map(|arm| {
                        let patterns = arm
                            .patterns
                            .iter()
                            .zip(&subjects)
                            .map(|(pattern, subject)| {
                                let capture =
                                    self.copy_capture(other, pattern.capture, Some(*subject));
                                let pattern = match pattern.pattern {
                                    MatchPattern::Wildcard => MatchPattern::Wildcard,
                                    MatchPattern::Value(value) => MatchPattern::Value(
                                        self.copy_expression_handle(other, value),
                                    ),
                                    MatchPattern::Guard(condition) => MatchPattern::Guard(
                                        self.copy_expression_handle(other, condition),
                                    ),
                                    MatchPattern::Destructure(pattern) => {
                                        MatchPattern::Destructure(
                                            self.copy_destructure_pattern(other, pattern),
                                        )
                                    }
                                };
                                crate::statement::DispatchPattern { capture, pattern }
                            })
                            .collect();
                        crate::statement::DispatchArm {
                            patterns,
                            guard: match arm.guard {
                                TransitionGuardNode::Always => TransitionGuardNode::Always,
                                TransitionGuardNode::When(value) => TransitionGuardNode::When(
                                    self.copy_expression_handle(other, value),
                                ),
                            },
                            target: self.copy_transition_target(other, arm.target),
                            source_span: arm.source_span,
                        }
                    })
                    .collect();
                let subjects = self.statements.insert_expression_handles(subjects);
                StatementNode::Dispatch(self.statements.insert_dispatch(
                    crate::statement::TableDispatch {
                        subjects,
                        arms,
                        subjects_span: dispatch.subjects_span,
                        source_span: dispatch.source_span,
                    },
                ))
            }
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
            TransitionTargetNode::Named(call) => {
                TransitionTargetNode::Named(self.copy_expression_handle(other, *call))
            }
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

        let supplied = self
            .type_substitutions
            .iter()
            .find_map(|(source, actual)| (*source == handle).then_some(*actual));
        let lexical = if !self.scoped_arguments.is_empty()
            && let TypeReferenceNode::Named(name) = other.type_references.type_reference(handle)
        {
            self.scoped_arguments
                .iter()
                .find(|arg| arg.binder == name.as_str(other))
                .map(|arg| arg.ty)
        } else {
            None
        };
        if let Some(actual) = supplied.or(lexical) {
            let origins = other
                .type_references
                .type_alias_applications(handle)
                .cloned()
                .collect::<Vec<_>>();
            if origins.is_empty() && lexical.is_none() {
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
                        .or_else(|| {
                            let actual = self.scoped_arguments.iter()
                                .find(|argument| argument.binder == name.as_str(other))?.ty;
                            if let TypeReferenceNode::Named(value) = self.type_references.type_reference(actual)
                                && let Ok(length) = value.as_str(&**self).parse()
                            {
                                return Some(crate::types::FixedArrayLength::Literal(length));
                            }
                            Some(crate::types::FixedArrayLength::Expression(actual))
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
            TypeReferenceNode::Callable { signature } => {
                let signature =
                    self.copy_state_signature(other, other.items.state_signature(*signature));
                let signature = self.items.insert_state_signature(&signature);
                self.type_references
                    .insert(TypeReferenceNode::Callable { signature })
            }
            TypeReferenceNode::ConstExpression(expression) => {
                let expression = self.copy_expression_handle(other, *expression);
                self.type_references
                    .insert(TypeReferenceNode::ConstExpression(expression))
            }
            TypeReferenceNode::Named(name) => self.type_references.insert_named(name.clone()),
            TypeReferenceNode::Unit => self.type_references.insert_unit(),
        };
        if self.retain_alias_header_role && other.type_references.is_alias_header_reference(handle)
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
                        arguments: this.copy_type_reference_handle_span(other, domain.arguments),
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
        self.copy_capture(other, capture, None)
    }

    fn copy_capture(
        &mut self,
        other: &SyntaxTrees,
        capture: Handle<MatchSubjectCapture>,
        copied_subject: Option<ExpressionHandle>,
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
        let subject = copied_subject.unwrap_or_else(|| {
            self.copy_expression_handle(other, other.expressions.match_subject(capture).subject)
        });
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
                selector: field.selector,
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
            subject_binding: self.copy_expression_handle(other, pattern.subject_binding),
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
                        MatchPattern::Guard(condition) => {
                            MatchPattern::Guard(self.copy_expression_handle(other, condition))
                        }
                        MatchPattern::Wildcard => MatchPattern::Wildcard,
                        MatchPattern::Destructure(pattern) => {
                            MatchPattern::Destructure(self.copy_destructure_pattern(other, pattern))
                        }
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
            ExpressionNode::Block(block) => {
                let statements = other
                    .statements
                    .block_statements(block.statements)
                    .iter()
                    .map(|statement| {
                        let statement =
                            self.copy_statement_node(other, other.statements.statement(*statement));
                        self.statements.insert(statement)
                    })
                    .collect::<Vec<_>>();
                ExpressionNode::Block(crate::expression::TableBlockExpression {
                    statements: self.statements.insert_block_statements(statements),
                    tail: if block.tail.is_valid() {
                        self.copy_expression_handle(other, block.tail)
                    } else {
                        ExpressionHandle::invalid()
                    },
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
                negation: binary.negation,
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
                callee: self.copy_expression_handle(other, call.callee),
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
            ExpressionNode::Indexed(indexed) => ExpressionNode::Indexed(TableIndexedExpression {
                collection: self.copy_expression_handle(other, indexed.collection),
                index: self.copy_expression_handle(other, indexed.index),
            }),
            ExpressionNode::Integer(value) => ExpressionNode::Integer(value.clone()),
            ExpressionNode::Membership(membership) => {
                ExpressionNode::Membership(TableMembershipExpression {
                    value: self.copy_expression_handle(other, membership.value),
                    domain: self.copy_expression_identifier_span(other, membership.domain),
                    domain_arguments: self
                        .copy_type_reference_handle_span(other, membership.domain_arguments),
                })
            }
            ExpressionNode::Member(member) => ExpressionNode::Member(TableMemberExpression {
                receiver: self.copy_expression_handle(other, member.receiver),
                access_source_span: member.access_source_span,
                member: member.member.clone(),
                position: member.position,
                case_variant: member.case_variant.clone(),
            }),
            ExpressionNode::Borrow(expression) => ExpressionNode::Borrow(TableBorrowExpression {
                target: self.copy_expression_handle(other, expression.target),
                access: expression.access,
            }),
            ExpressionNode::Name {
                members: path,
                application: path_application,
            } => {
                let mut selection = StaticMachineArgument {
                    application: path_application.clone(),
                    ..Default::default()
                };
                self.remap_static_machine_argument(other, &mut selection);
                ExpressionNode::Name {
                    members: self.copy_expression_identifier_span(other, path.to_owned()),
                    application: selection.application,
                }
            }
            ExpressionNode::Range(range) => ExpressionNode::Range(TableRangeExpression {
                start: self.copy_expression_handle(other, range.start),
                end: self.copy_expression_handle(other, range.end),
                end_inclusive: range.end_inclusive,
            }),
            ExpressionNode::StructLiteral(struct_literal) => {
                ExpressionNode::StructLiteral(TableStructLiteral {
                    zero_omitted_fields: struct_literal.zero_omitted_fields,
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
        let mut span = HandleSpan::empty();
        for value in mapped {
            span.push_contiguous(append(self, value));
        }
        span
    }

    pub(crate) fn copy_span<T>(
        &mut self,
        values: impl IntoIterator<Item = T>,
        append: impl FnMut(&mut Self, T) -> Handle<T>,
    ) -> HandleSpan<T> {
        self.copy_mapped_span(values, |_, value| value, append)
    }
}
