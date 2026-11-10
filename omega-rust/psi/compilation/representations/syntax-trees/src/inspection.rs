/*
We project `SyntaxTrees` into deterministic `SyntaxTreesSnapshot` records for
compiler phase inspection without exposing raw arena handles.
*/

pub mod snapshot {
    pub(crate) mod expression_and_type_snapshots {

        use crate::SyntaxTrees;
        use crate::expression::{
            BinaryOperator, ExpressionNode, MatchFieldAction, MatchPattern, TableCallExpression,
            TableStructLiteralField,
        };
        use crate::identifier::Identifier;
        use crate::types::{
            FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
        };
        use arena::HandleSpan;
        use serde::Serialize;

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct IdentifierSnapshot {
            pub text: String,
            pub source_id: usize,
            pub start: usize,
            pub end: usize,
            pub source_backed: bool,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(untagged)]
        pub enum StaticArgumentSnapshot {
            Type {
                type_reference: Box<TypeReferenceSnapshot>,
            },
            Path(Vec<IdentifierSnapshot>),
            Application {
                #[serde(skip_serializing_if = "Option::is_none")]
                member: Option<Box<StaticArgumentSnapshot>>,
                path: Vec<IdentifierSnapshot>,
                lifetime_arguments: Vec<IdentifierSnapshot>,
                arguments: Vec<StaticArgumentSnapshot>,
            },
            Const(String),
            EvidenceProjection {
                term: IdentifierSnapshot,
                member: IdentifierSnapshot,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum TypeReferenceSnapshot {
            Reference {
                referee: Box<TypeReferenceSnapshot>,
                access: &'static str,
                #[serde(skip_serializing_if = "Option::is_none")]
                lifetime: Option<IdentifierSnapshot>,
            },
            Constrained {
                base_type: Box<TypeReferenceSnapshot>,
                constraints: Vec<TypeConstraintSnapshot>,
            },
            FixedArray {
                element_type: Box<TypeReferenceSnapshot>,
                length: FixedArrayLengthSnapshot,
            },
            Slice {
                element_type: Box<TypeReferenceSnapshot>,
            },
            Generic {
                base_name: IdentifierSnapshot,
                lifetime_arguments: Vec<IdentifierSnapshot>,
                arguments: Vec<TypeReferenceSnapshot>,
            },
            ConstExpression {
                expression: ExpressionSnapshot,
            },
            DynamicTrait {
                trait_name: IdentifierSnapshot,
                trait_arguments: Vec<TypeReferenceSnapshot>,
            },
            Named {
                name: IdentifierSnapshot,
            },
            Unit,
            Missing,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum FixedArrayLengthSnapshot {
            Literal { value: usize },
            ConstParameter { name: IdentifierSnapshot },
            ConstCall { name: IdentifierSnapshot },
            Expression { value: Box<TypeReferenceSnapshot> },
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum TypeConstraintSnapshot {
            Named {
                name: IdentifierSnapshot,
            },

            ArithmeticDomain {
                domain: String,
            },
            Domain {
                name: IdentifierSnapshot,
                arguments: Vec<TypeReferenceSnapshot>,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct MatchArmSnapshot {
            pub pattern: MatchPatternSnapshot,
            pub value: ExpressionSnapshot,
            pub source_id: usize,
            pub source_start: usize,
            pub source_end: usize,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum MatchPatternSnapshot {
            Value {
                value: Box<ExpressionSnapshot>,
            },
            Destructure {
                #[serde(skip_serializing_if = "Option::is_none")]
                application_type: Option<Box<TypeReferenceSnapshot>>,
                head: Box<ExpressionSnapshot>,
                fields: Vec<MatchFieldSnapshot>,
                has_rest: bool,
                guard: Option<Box<ExpressionSnapshot>>,
                proof_selectors: Vec<(String, String)>,
            },
            Wildcard,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct MatchFieldSnapshot {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub component: Option<&'static str>,
            pub member: IdentifierSnapshot,
            pub action: MatchFieldActionSnapshot,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum MatchFieldActionSnapshot {
            Bind { binding: IdentifierSnapshot },
            Equal { value: Box<ExpressionSnapshot> },
            Waive,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum ExpressionSnapshot {
            Match {
                subject: Box<ExpressionSnapshot>,
                #[serde(skip_serializing_if = "Option::is_none")]
                capture: Option<(u32, u32)>,
                arms: Vec<MatchArmSnapshot>,
            },
            SavedSubject {
                #[serde(skip_serializing_if = "Option::is_none")]
                component: Option<&'static str>,
                capture: (u32, u32),
                pattern_head: Box<ExpressionSnapshot>,
            },
            ArrayLiteral {
                values: Vec<ExpressionSnapshot>,
            },
            Atomic {
                value: Box<ExpressionSnapshot>,
                result: Option<Box<ExpressionSnapshot>>,
                ordering: String,
            },
            Binary {
                left: Box<ExpressionSnapshot>,
                operator: &'static str,
                right: Box<ExpressionSnapshot>,
            },
            Boolean {
                value: bool,
            },
            Cast {
                value: Box<ExpressionSnapshot>,
                target_type: Box<TypeReferenceSnapshot>,
                arithmetic_domain: &'static str,
                form: &'static str,
                semantic_domain: Vec<IdentifierSnapshot>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                semantic_domain_arguments: Vec<TypeReferenceSnapshot>,
            },
            Call {
                receiver: Option<Box<ExpressionSnapshot>>,
                target: IdentifierSnapshot,
                machine_arguments: Vec<StaticArgumentSnapshot>,
                arguments: Vec<ExpressionSnapshot>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                evidence_arguments: Vec<IdentifierSnapshot>,
                acknowledgement_synthesized: bool,
                acknowledges_suspend: bool,
                acknowledges_block: bool,
            },
            UndeterminedNaN {
                format: String,
            },
            Float {
                text: String,
            },
            Indexed {
                collection: Box<ExpressionSnapshot>,
                index: Box<ExpressionSnapshot>,
            },
            Integer {
                text: String,
            },
            Membership {
                value: Box<ExpressionSnapshot>,
                domain: Vec<IdentifierSnapshot>,
            },
            Member {
                receiver: Box<ExpressionSnapshot>,
                member: IdentifierSnapshot,
                #[serde(skip_serializing_if = "Option::is_none")]
                case_variant: Option<IdentifierSnapshot>,
            },
            Borrow {
                access: &'static str,
                value: Box<ExpressionSnapshot>,
            },
            Name {
                path: Vec<IdentifierSnapshot>,
            },
            Range {
                start: Option<Box<ExpressionSnapshot>>,
                end: Option<Box<ExpressionSnapshot>>,
                end_inclusive: bool,
            },
            StructLiteral {
                #[serde(skip_serializing_if = "Option::is_none")]
                application_type: Option<Box<TypeReferenceSnapshot>>,
                constructor_name: IdentifierSnapshot,
                fields: Vec<StructLiteralFieldSnapshot>,
            },
            String {
                bytes: Vec<u8>,
            },
            Unary {
                operator: &'static str,
                operand: Box<ExpressionSnapshot>,
            },
            ZeroValue {
                type_reference: Box<TypeReferenceSnapshot>,
            },
            TypeExpression {
                type_reference: Box<TypeReferenceSnapshot>,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct StructLiteralFieldSnapshot {
            pub name: IdentifierSnapshot,
            pub value: ExpressionSnapshot,
        }

        pub(crate) fn snapshot_type_reference_handle(
            syntax_trees: &SyntaxTrees,
            handle: crate::types::TypeReferenceHandle,
        ) -> TypeReferenceSnapshot {
            if !handle.is_valid() {
                return TypeReferenceSnapshot::Missing;
            }

            match syntax_trees.type_references.type_reference(handle) {
                TypeReferenceNode::Reference {
                    referee,
                    access,
                    lifetime,
                } => TypeReferenceSnapshot::Reference {
                    referee: Box::new(snapshot_type_reference_handle(syntax_trees, *referee)),
                    access: access.name(),
                    lifetime: lifetime
                        .as_ref()
                        .map(|value| snapshot_identifier(syntax_trees, value)),
                },
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => TypeReferenceSnapshot::Constrained {
                    base_type: Box::new(snapshot_type_reference_handle(syntax_trees, *base_type)),
                    constraints: syntax_trees
                        .type_references
                        .constraints(*constraints)
                        .iter()
                        .map(|constraint| snapshot_type_constraint(syntax_trees, constraint))
                        .collect(),
                },
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => TypeReferenceSnapshot::FixedArray {
                    element_type: Box::new(snapshot_type_reference_handle(
                        syntax_trees,
                        *element_type,
                    )),
                    length: snapshot_fixed_array_length(syntax_trees, length),
                },
                TypeReferenceNode::Slice { element_type } => TypeReferenceSnapshot::Slice {
                    element_type: Box::new(snapshot_type_reference_handle(
                        syntax_trees,
                        *element_type,
                    )),
                },
                TypeReferenceNode::Generic {
                    base_name,
                    lifetime_arguments,
                    arguments,
                } => TypeReferenceSnapshot::Generic {
                    base_name: snapshot_identifier(syntax_trees, base_name),
                    lifetime_arguments: snapshot_identifier_slice(syntax_trees, lifetime_arguments),
                    arguments: snapshot_type_references(syntax_trees, *arguments),
                },
                TypeReferenceNode::ConstExpression(expression) => {
                    TypeReferenceSnapshot::ConstExpression {
                        expression: snapshot_expression_handle(syntax_trees, *expression),
                    }
                }
                TypeReferenceNode::DynamicTrait {
                    trait_name,
                    trait_arguments,
                } => TypeReferenceSnapshot::DynamicTrait {
                    trait_name: snapshot_identifier(syntax_trees, trait_name),
                    trait_arguments: snapshot_type_references(syntax_trees, *trait_arguments),
                },
                TypeReferenceNode::Named(name) => TypeReferenceSnapshot::Named {
                    name: snapshot_identifier(syntax_trees, name),
                },
                TypeReferenceNode::Unit => TypeReferenceSnapshot::Unit,
            }
        }

        pub(crate) fn snapshot_type_references(
            syntax_trees: &SyntaxTrees,
            span: HandleSpan<TypeReferenceHandle>,
        ) -> Vec<TypeReferenceSnapshot> {
            syntax_trees
                .type_references
                .type_reference_handles(span)
                .iter()
                .map(|handle| snapshot_type_reference_handle(syntax_trees, *handle))
                .collect()
        }

        fn snapshot_fixed_array_length(
            syntax_trees: &SyntaxTrees,
            length: &FixedArrayLength,
        ) -> FixedArrayLengthSnapshot {
            match length {
                FixedArrayLength::Literal(value) => {
                    FixedArrayLengthSnapshot::Literal { value: *value }
                }
                FixedArrayLength::ConstParameter(name) => {
                    FixedArrayLengthSnapshot::ConstParameter {
                        name: snapshot_identifier(syntax_trees, name),
                    }
                }
                FixedArrayLength::ConstCall(name) => FixedArrayLengthSnapshot::ConstCall {
                    name: snapshot_identifier(syntax_trees, name),
                },
                FixedArrayLength::Expression(value) => FixedArrayLengthSnapshot::Expression {
                    value: Box::new(snapshot_type_reference_handle(syntax_trees, *value)),
                },
            }
        }

        fn snapshot_type_constraint(
            syntax_trees: &SyntaxTrees,
            constraint: &TypeConstraintNode,
        ) -> TypeConstraintSnapshot {
            match constraint {
                TypeConstraintNode::Named(name) => TypeConstraintSnapshot::Named {
                    name: snapshot_identifier(syntax_trees, name),
                },
                TypeConstraintNode::Domain(domain) => TypeConstraintSnapshot::Domain {
                    name: snapshot_identifier(syntax_trees, &domain.name),
                    arguments: snapshot_type_references(syntax_trees, domain.arguments),
                },

                TypeConstraintNode::ArithmeticDomain(domain) => {
                    TypeConstraintSnapshot::ArithmeticDomain {
                        domain: domain.name().to_owned(),
                    }
                }
            }
        }

        pub(crate) fn snapshot_expression_handle(
            syntax_trees: &SyntaxTrees,
            handle: crate::expression::ExpressionHandle,
        ) -> ExpressionSnapshot {
            if !handle.is_valid() {
                return ExpressionSnapshot::Name { path: Vec::new() };
            }

            match syntax_trees.expressions.expression(handle) {
                ExpressionNode::Match(dispatch) => ExpressionSnapshot::Match {
                    subject: Box::new(snapshot_expression_handle(syntax_trees, dispatch.subject)),
                    capture: dispatch.capture.is_valid().then_some((
                        dispatch.capture.arena_index(),
                        dispatch.capture.generation(),
                    )),
                    arms: syntax_trees
                        .expressions
                        .match_arms(dispatch.arms)
                        .iter()
                        .map(|arm| MatchArmSnapshot {
                            source_id: arm.source_span.source_id.0,
                            source_start: arm.source_span.span.start,
                            source_end: arm.source_span.span.end,
                            pattern: snapshot_match_pattern(syntax_trees, arm.pattern),
                            value: snapshot_expression_handle(syntax_trees, arm.value),
                        })
                        .collect(),
                },
                ExpressionNode::SavedSubject(saved) => ExpressionSnapshot::SavedSubject {
                    component: (saved.component == language_core::PatternComponent::Case)
                        .then_some("case"),
                    capture: (saved.capture.arena_index(), saved.capture.generation()),
                    pattern_head: Box::new(snapshot_expression_handle(
                        syntax_trees,
                        saved.pattern_head,
                    )),
                },
                ExpressionNode::ArrayLiteral(values) => ExpressionSnapshot::ArrayLiteral {
                    values: syntax_trees
                        .expressions
                        .expression_handles(*values)
                        .iter()
                        .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                        .collect(),
                },
                ExpressionNode::Atomic(atomic) => ExpressionSnapshot::Atomic {
                    value: Box::new(snapshot_expression_handle(syntax_trees, atomic.value)),
                    result: atomic
                        .result
                        .is_valid()
                        .then(|| Box::new(snapshot_expression_handle(syntax_trees, atomic.result))),
                    ordering: format!("{:?}", atomic.ordering),
                },
                ExpressionNode::Binary(binary) => ExpressionSnapshot::Binary {
                    left: Box::new(snapshot_expression_handle(syntax_trees, binary.left)),
                    operator: snapshot_binary_operator(binary.operator),
                    right: Box::new(snapshot_expression_handle(syntax_trees, binary.right)),
                },
                ExpressionNode::Boolean(value) => ExpressionSnapshot::Boolean { value: *value },
                ExpressionNode::Cast(cast) => ExpressionSnapshot::Cast {
                    value: Box::new(snapshot_expression_handle(syntax_trees, cast.value)),
                    target_type: Box::new(snapshot_type_reference_handle(
                        syntax_trees,
                        cast.target_type,
                    )),
                    arithmetic_domain: cast.domain.name(),
                    form: match cast.form {
                        language_core::cast_form::CastForm::Value => "value",
                        language_core::cast_form::CastForm::Recast(access) => match access {
                            language_core::ReferenceAccess::Shared => "recast_shared",
                            language_core::ReferenceAccess::Mutable => "recast_mutable",
                            language_core::ReferenceAccess::WriteOnly => "recast_write_only",
                        },
                    },
                    semantic_domain: snapshot_identifier_slice(
                        syntax_trees,
                        syntax_trees
                            .expressions
                            .identifier_path_members(cast.semantic_domain),
                    ),
                    semantic_domain_arguments: snapshot_type_references(
                        syntax_trees,
                        cast.semantic_domain_arguments,
                    ),
                },
                ExpressionNode::Call(call) => snapshot_call_expression(syntax_trees, call),
                ExpressionNode::UndeterminedNaN(format) => ExpressionSnapshot::UndeterminedNaN {
                    format: format.name().to_owned(),
                },
                ExpressionNode::Float(value) => ExpressionSnapshot::Float {
                    text: value.as_str(syntax_trees).to_owned(),
                },
                ExpressionNode::Indexed(indexed) => ExpressionSnapshot::Indexed {
                    collection: Box::new(snapshot_expression_handle(
                        syntax_trees,
                        indexed.collection,
                    )),
                    index: Box::new(snapshot_expression_handle(syntax_trees, indexed.index)),
                },
                ExpressionNode::Integer(value) => ExpressionSnapshot::Integer {
                    text: value.to_string(),
                },
                ExpressionNode::Membership(membership) => ExpressionSnapshot::Membership {
                    value: Box::new(snapshot_expression_handle(syntax_trees, membership.value)),
                    domain: snapshot_identifier_slice(
                        syntax_trees,
                        syntax_trees
                            .expressions
                            .identifier_path_members(membership.domain),
                    ),
                },
                ExpressionNode::Member(member) => ExpressionSnapshot::Member {
                    receiver: Box::new(snapshot_expression_handle(syntax_trees, member.receiver)),
                    member: snapshot_identifier(syntax_trees, &member.member),
                    case_variant: member
                        .case_variant
                        .as_ref()
                        .map(|value| snapshot_identifier(syntax_trees, value)),
                },
                ExpressionNode::Borrow(value) => ExpressionSnapshot::Borrow {
                    access: value.access.name(),
                    value: Box::new(snapshot_expression_handle(syntax_trees, value.target)),
                },
                ExpressionNode::Name(path) => ExpressionSnapshot::Name {
                    path: snapshot_identifier_slice(
                        syntax_trees,
                        syntax_trees.expressions.identifier_path_members(*path),
                    ),
                },
                ExpressionNode::Range(range) => ExpressionSnapshot::Range {
                    start: range
                        .start
                        .is_valid()
                        .then(|| Box::new(snapshot_expression_handle(syntax_trees, range.start))),
                    end: range
                        .end
                        .is_valid()
                        .then(|| Box::new(snapshot_expression_handle(syntax_trees, range.end))),
                    end_inclusive: range.end_inclusive,
                },
                ExpressionNode::StructLiteral(value) => ExpressionSnapshot::StructLiteral {
                    application_type: value.application_type.is_valid().then(|| {
                        Box::new(snapshot_type_reference_handle(
                            syntax_trees,
                            value.application_type,
                        ))
                    }),
                    constructor_name: snapshot_identifier(syntax_trees, &value.constructor_name),
                    fields: syntax_trees
                        .expressions
                        .struct_fields(value.fields)
                        .iter()
                        .map(|field| snapshot_struct_field(syntax_trees, field))
                        .collect(),
                },
                ExpressionNode::String(value) => ExpressionSnapshot::String {
                    bytes: value.to_vec(),
                },
                ExpressionNode::Unary(unary) => ExpressionSnapshot::Unary {
                    operator: unary.operator.display_name(),
                    operand: Box::new(snapshot_expression_handle(syntax_trees, unary.operand)),
                },
                ExpressionNode::ZeroValue(type_reference) => ExpressionSnapshot::ZeroValue {
                    type_reference: Box::new(snapshot_type_reference_handle(
                        syntax_trees,
                        *type_reference,
                    )),
                },
                ExpressionNode::TypeExpression(type_reference) => {
                    ExpressionSnapshot::TypeExpression {
                        type_reference: Box::new(snapshot_type_reference_handle(
                            syntax_trees,
                            *type_reference,
                        )),
                    }
                }
            }
        }

        fn snapshot_call_expression(
            syntax_trees: &SyntaxTrees,
            call: &TableCallExpression,
        ) -> ExpressionSnapshot {
            ExpressionSnapshot::Call {
                receiver: call
                    .receiver
                    .is_valid()
                    .then(|| Box::new(snapshot_expression_handle(syntax_trees, call.receiver))),
                target: snapshot_identifier(syntax_trees, &call.target),
                machine_arguments: call
                    .machine_arguments
                    .iter()
                    .map(|argument| snapshot_static_argument(syntax_trees, argument))
                    .collect(),
                arguments: syntax_trees
                    .expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                    .collect(),
                evidence_arguments: snapshot_identifier_slice(
                    syntax_trees,
                    &call.evidence_arguments,
                ),
                acknowledgement_synthesized: call.operational_acknowledgement.origin
                    == language_core::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                acknowledges_suspend: call.operational_acknowledgement.acknowledges_suspend,
                acknowledges_block: call.operational_acknowledgement.acknowledges_block,
            }
        }

        pub(crate) fn snapshot_static_argument(
            syntax_trees: &SyntaxTrees,
            argument: &crate::expression::StaticMachineArgument,
        ) -> StaticArgumentSnapshot {
            if argument.type_reference.is_valid() {
                StaticArgumentSnapshot::Type {
                    type_reference: Box::new(snapshot_type_reference_handle(
                        syntax_trees,
                        argument.type_reference,
                    )),
                }
            } else if let Some(literal) = &argument.const_literal {
                StaticArgumentSnapshot::Const(literal.to_string())
            } else if let Some(projection) = &argument.evidence_projection {
                StaticArgumentSnapshot::EvidenceProjection {
                    term: snapshot_identifier(syntax_trees, &projection.term),
                    member: snapshot_identifier(syntax_trees, &projection.member),
                }
            } else if let Some(application) = &argument.application {
                StaticArgumentSnapshot::Application {
                    member: syntax_trees
                        .expressions
                        .static_member(application.member)
                        .map(|member| Box::new(snapshot_static_argument(syntax_trees, member))),
                    path: snapshot_identifier_slice(syntax_trees, &argument.path),
                    lifetime_arguments: snapshot_identifier_slice(
                        syntax_trees,
                        &application.lifetime_arguments,
                    ),
                    arguments: application
                        .arguments
                        .iter()
                        .map(|argument| snapshot_static_argument(syntax_trees, argument))
                        .collect(),
                }
            } else {
                StaticArgumentSnapshot::Path(snapshot_identifier_slice(
                    syntax_trees,
                    &argument.path,
                ))
            }
        }

        fn snapshot_struct_field(
            syntax_trees: &SyntaxTrees,
            field: &TableStructLiteralField,
        ) -> StructLiteralFieldSnapshot {
            StructLiteralFieldSnapshot {
                name: snapshot_identifier(syntax_trees, &field.name),
                value: snapshot_expression_handle(syntax_trees, field.value),
            }
        }

        fn snapshot_binary_operator(operator: BinaryOperator) -> &'static str {
            match operator {
                BinaryOperator::Add => "add",
                BinaryOperator::And => "and",
                BinaryOperator::BitwiseAnd => "bitwise_and",
                BinaryOperator::BitwiseOr => "bitwise_or",
                BinaryOperator::BitwiseXor => "bitwise_xor",
                BinaryOperator::Divide => "divide",
                BinaryOperator::Equal => "equal",
                BinaryOperator::Greater => "greater",
                BinaryOperator::GreaterOrEqual => "greater_or_equal",
                BinaryOperator::Less => "less",
                BinaryOperator::LessOrEqual => "less_or_equal",
                BinaryOperator::Modulo => "modulo",
                BinaryOperator::Multiply => "multiply",
                BinaryOperator::NotEqual => "not_equal",
                BinaryOperator::Or => "or",
                BinaryOperator::ShiftLeft => "shift_left",
                BinaryOperator::ShiftRight => "shift_right",
                BinaryOperator::Subtract => "subtract",
            }
        }

        pub(crate) fn snapshot_identifier(
            syntax_trees: &SyntaxTrees,
            identifier: &Identifier,
        ) -> IdentifierSnapshot {
            let source_span = identifier.source_span();
            IdentifierSnapshot {
                text: identifier.as_str(syntax_trees).to_owned(),
                source_id: source_span.source_id.0,
                start: source_span.span.start,
                end: source_span.span.end,
                source_backed: identifier.is_source_backed(),
            }
        }

        pub(crate) fn snapshot_identifier_slice(
            syntax_trees: &SyntaxTrees,
            path: &[Identifier],
        ) -> Vec<IdentifierSnapshot> {
            path.iter()
                .map(|value| snapshot_identifier(syntax_trees, value))
                .collect()
        }

        pub(crate) fn snapshot_item_identifier_path(
            syntax_trees: &SyntaxTrees,
            span: HandleSpan<Identifier>,
        ) -> Vec<IdentifierSnapshot> {
            snapshot_identifier_slice(
                syntax_trees,
                syntax_trees.items.identifier_path_members(span),
            )
        }

        pub(crate) fn snapshot_match_pattern(
            syntax_trees: &SyntaxTrees,
            pattern: MatchPattern,
        ) -> MatchPatternSnapshot {
            match pattern {
                MatchPattern::Value(value) => MatchPatternSnapshot::Value {
                    value: Box::new(snapshot_expression_handle(syntax_trees, value)),
                },
                MatchPattern::Destructure(pattern) => MatchPatternSnapshot::Destructure {
                    application_type: pattern.application_type.is_valid().then(|| {
                        Box::new(snapshot_type_reference_handle(
                            syntax_trees,
                            pattern.application_type,
                        ))
                    }),
                    head: Box::new(snapshot_expression_handle(syntax_trees, pattern.head)),
                    fields: syntax_trees
                        .expressions
                        .match_fields(pattern.fields)
                        .iter()
                        .map(|field| MatchFieldSnapshot {
                            component: (field.component == language_core::PatternComponent::Case)
                                .then_some("case"),
                            member: snapshot_identifier(syntax_trees, &field.member),
                            action: match &field.action {
                                MatchFieldAction::Bind(binding) => MatchFieldActionSnapshot::Bind {
                                    binding: snapshot_identifier(syntax_trees, binding),
                                },
                                MatchFieldAction::Equal(value) => MatchFieldActionSnapshot::Equal {
                                    value: Box::new(snapshot_expression_handle(
                                        syntax_trees,
                                        *value,
                                    )),
                                },
                                MatchFieldAction::Waive => MatchFieldActionSnapshot::Waive,
                            },
                        })
                        .collect(),
                    has_rest: pattern.has_rest,
                    guard: pattern
                        .guard
                        .is_valid()
                        .then(|| Box::new(snapshot_expression_handle(syntax_trees, pattern.guard))),
                    proof_selectors: syntax_trees
                        .statements
                        .outcome_proof_selectors(pattern.proof_selectors)
                        .iter()
                        .map(|selector| {
                            (
                                selector.output_field.as_str(syntax_trees).to_owned(),
                                selector.binding.as_str(syntax_trees).to_owned(),
                            )
                        })
                        .collect(),
                },
                MatchPattern::Wildcard => MatchPatternSnapshot::Wildcard,
            }
        }
    }
    pub(crate) mod item_snapshots {
        use crate::SyntaxTrees;
        use crate::inspection::snapshot::expression_and_type_snapshots::{
            snapshot_expression_handle, snapshot_identifier, snapshot_identifier_slice,
            snapshot_item_identifier_path, snapshot_type_reference_handle,
            snapshot_type_references,
        };
        use crate::inspection::snapshot::state_and_statement_snapshots::{
            snapshot_state_node, snapshot_state_parameter, snapshot_state_parameters,
            snapshot_state_signature, snapshot_state_signature_node,
        };
        use crate::inspection::snapshot::{
            ExpressionSnapshot, IdentifierSnapshot, StateParameterSnapshot, StateSignatureSnapshot,
            StateSnapshot, TypeReferenceSnapshot,
        };
        use crate::item::{
            CapabilityContract, CapabilityContractKind, CapabilityMember, ConformanceMember,
            CrashCause, DataMember, ExternalBinding, GenericConformanceBound, Item,
            MachineParameterContract, ProofFact, PropositionBody, SatisfiesClause, TypeParameter,
            TypeParameterKind,
        };
        use arena::HandleSpan;
        use serde::Serialize;

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum ItemSnapshot {
            Capability {
                name: IdentifierSnapshot,
                members: Vec<CapabilityMemberSnapshot>,
            },
            Conformance {
                #[serde(skip_serializing_if = "is_false")]
                is_public: bool,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                lifetime_parameters: Vec<IdentifierSnapshot>,
                type_parameters: Vec<TypeParameterSnapshot>,
                trait_name: IdentifierSnapshot,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                trait_lifetime_arguments: Vec<IdentifierSnapshot>,
                trait_arguments: Vec<TypeReferenceSnapshot>,
                #[serde(skip_serializing_if = "Option::is_none")]
                alias: Option<IdentifierSnapshot>,
                members: Vec<ConformanceMemberSnapshot>,
            },
            Const {
                scope: IdentifierSnapshot,
                name: IdentifierSnapshot,
                is_public: bool,
                type_reference: TypeReferenceSnapshot,
                value: Box<ExpressionSnapshot>,
            },
            Data {
                name: IdentifierSnapshot,
                is_public: bool,
                supply: &'static str,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                lifetime_parameters: Vec<IdentifierSnapshot>,
                type_parameters: Vec<TypeParameterSnapshot>,
                #[serde(skip_serializing_if = "Option::is_none")]
                generic_instance: Option<TypeReferenceSnapshot>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                conformance_bounds: Vec<GenericConformanceBoundSnapshot>,
                properties: DataPropertiesSnapshot,
                #[serde(skip_serializing_if = "Option::is_none")]
                quotient: Option<QuotientSnapshot>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                where_facts: Vec<ProofFactSnapshot>,
                members: Vec<DataMemberSnapshot>,
            },
            TypeAlias {
                name: IdentifierSnapshot,
                is_public: bool,
                lifetime_parameters: Vec<IdentifierSnapshot>,
                type_parameters: Vec<TypeParameterSnapshot>,
                conformance_bounds: Vec<GenericConformanceBoundSnapshot>,
                expansion: TypeReferenceSnapshot,
            },
            Domain {
                name: IdentifierSnapshot,
                type_parameters: Vec<TypeParameterSnapshot>,
                target_type: TypeReferenceSnapshot,
                index_arguments: Vec<TypeReferenceSnapshot>,
                is_public: bool,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                alias: Vec<Vec<IdentifierSnapshot>>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                authored_routes: Vec<Vec<IdentifierSnapshot>>,
                #[serde(skip_serializing_if = "Option::is_none")]
                classification: Option<&'static str>,
                predicate_presence: &'static str,
                facts: Vec<ProofFactSnapshot>,
                operators: Vec<OperatorSnapshot>,
                semantic_clause_token_count: usize,
            },
            Measure {
                name: Vec<IdentifierSnapshot>,
                parameter: Option<StateParameterSnapshot>,
                return_type: TypeReferenceSnapshot,
                lexicographic: bool,
                body: Vec<ExpressionSnapshot>,
                token_count: usize,
            },
            Operator {
                operator: OperatorSnapshot,
            },
            Module {
                path: Vec<IdentifierSnapshot>,
            },
            Package {
                path: Vec<IdentifierSnapshot>,
            },
            Proposition {
                name: IdentifierSnapshot,
                is_public: bool,
                type_parameters: Vec<TypeParameterSnapshot>,
                parameters: Vec<StateParameterSnapshot>,
                body: PropositionBodySnapshot,
            },
            Use {
                path: Vec<IdentifierSnapshot>,
            },
            Machine {
                #[serde(skip_serializing_if = "Vec::is_empty")]
                nested_machines: Vec<ItemSnapshot>,
                name: IdentifierSnapshot,
                attached_data: Option<IdentifierSnapshot>,
                #[serde(skip_serializing_if = "Option::is_none")]
                spelling: Option<&'static str>,
                is_public: bool,
                bodyless: bool,
                target: Option<IdentifierSnapshot>,
                #[serde(skip_serializing_if = "Option::is_none")]
                authored_target: Option<IdentifierSnapshot>,
                boundary: bool,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                lifetime_parameters: Vec<IdentifierSnapshot>,
                type_parameters: Vec<TypeParameterSnapshot>,
                satisfies: Vec<SatisfiesClauseSnapshot>,
                conformance_bounds: Vec<GenericConformanceBoundSnapshot>,
                terminates_guarantee: bool,
                ranking_subjects: Vec<ExpressionSnapshot>,
                ranking_view: Vec<IdentifierSnapshot>,
                ranking_view_arguments: Vec<ExpressionSnapshot>,
                #[serde(skip_serializing_if = "Option::is_none")]
                ranking_range: Option<Box<ExpressionSnapshot>>,
                #[serde(skip_serializing_if = "is_false")]
                service_reach_is_installation_bound: bool,
                service_reaches: Vec<IdentifierSnapshot>,
                invokes: Vec<IdentifierSnapshot>,
                suspends: bool,
                blocks: bool,
                #[serde(skip_serializing_if = "is_zero_u64")]
                admitted_same_stack_bytes: u64,
                contracts: Vec<CapabilityContractSnapshot>,
                states: Vec<StateSnapshot>,
            },
            Platform {
                name: IdentifierSnapshot,
                states: Vec<StateSignatureSnapshot>,
            },
            Trait {
                name: IdentifierSnapshot,
                is_boundary: bool,
                is_public: bool,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                lifetime_parameters: Vec<IdentifierSnapshot>,
                type_parameters: Vec<TypeParameterSnapshot>,
                conformance_bounds: Vec<GenericConformanceBoundSnapshot>,
                parents: Vec<TypeReferenceSnapshot>,
                requires: Vec<IdentifierSnapshot>,
                machines: Vec<StateSignatureSnapshot>,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct SatisfiesClauseSnapshot {
            pub trait_name: IdentifierSnapshot,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub lifetime_arguments: Vec<IdentifierSnapshot>,
            pub arguments: Vec<TypeReferenceSnapshot>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub requirement: Option<IdentifierSnapshot>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub alias: Option<IdentifierSnapshot>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub via: Option<ExternalBindingSnapshot>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub via_expression: Option<Box<ExpressionSnapshot>>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct GenericConformanceBoundSnapshot {
            pub binder: IdentifierSnapshot,
            pub trait_name: IdentifierSnapshot,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub lifetime_arguments: Vec<IdentifierSnapshot>,
            pub arguments: Vec<TypeReferenceSnapshot>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum ExternalBindingSnapshot {
            Syscall { number: i64 },
            CompilerIntrinsic,
            VtableSlot { index: i64 },
            VtableField { field: IdentifierSnapshot },
            TableFunction { field: IdentifierSnapshot },
        }

        fn is_false(value: &bool) -> bool {
            !*value
        }

        fn is_zero_u64(value: &u64) -> bool {
            *value == 0
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum ConformanceMemberSnapshot {
            Machine {
                declaration: Box<ItemSnapshot>,
            },
            Synthesized {
                declaration: Box<ItemSnapshot>,
            },
            TraitDefault {
                declaring_trait: IdentifierSnapshot,
                requirement_ordinal: usize,
                declaration: Box<ItemSnapshot>,
            },
            Selection {
                declaring_trait: IdentifierSnapshot,
                requirement: IdentifierSnapshot,
                target: Vec<IdentifierSnapshot>,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum PropositionBodySnapshot {
            Primitive,
            Witness {
                evidence: TypeReferenceSnapshot,
            },
            Transparent {
                proposition: Box<ExpressionSnapshot>,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct TypeParameterSnapshot {
            pub name: IdentifierSnapshot,
            pub kind: &'static str,
            pub const_type: Option<TypeReferenceSnapshot>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub machine_contract: Option<StateSignatureSnapshot>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub machine_requirement: Option<Vec<IdentifierSnapshot>>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub proposition_contract: Option<PropositionParameterSignatureSnapshot>,
            pub bounds: DataPropertiesSnapshot,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct PropositionParameterSignatureSnapshot {
            pub name: IdentifierSnapshot,
            pub parameters: Vec<StateParameterSnapshot>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct DataPropertiesSnapshot {
            pub multiplicity: &'static str,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub carry: Option<CarryPolicySnapshot>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct QuotientSnapshot {
            pub carrier: TypeReferenceSnapshot,
            pub relation: Vec<IdentifierSnapshot>,
            pub equivalence: Option<QuotientEquivalenceSelectionSnapshot>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct QuotientEquivalenceSelectionSnapshot {
            pub relation: Vec<IdentifierSnapshot>,
            pub trait_name: IdentifierSnapshot,
            pub trait_arguments: Vec<TypeReferenceSnapshot>,
            pub conformance_name: IdentifierSnapshot,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct CarryPolicySnapshot {
            pub suspension: &'static str,
            pub cpu: &'static str,
            pub thread: &'static str,
            pub address: &'static str,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct OperatorSnapshot {
            pub is_public: bool,
            pub is_boundary: bool,
            pub name: Vec<IdentifierSnapshot>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub lifetime_parameters: Vec<IdentifierSnapshot>,
            pub type_parameters: Vec<TypeParameterSnapshot>,
            pub parameters: Vec<StateParameterSnapshot>,
            pub return_type: TypeReferenceSnapshot,
            pub contracts: Vec<CapabilityContractSnapshot>,
            pub spelling: Option<&'static str>,
            pub token_count: usize,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum DataMemberSnapshot {
            Field {
                identity: Option<u64>,
                name: IdentifierSnapshot,
                relevance: &'static str,
                type_reference: TypeReferenceSnapshot,
            },
            Variant {
                identity: Option<u64>,
                name: IdentifierSnapshot,
                payload: Vec<DataPayloadFieldSnapshot>,
                retired_payload_identities: Vec<u64>,
            },
            Retired {
                identity: u64,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct DataPayloadFieldSnapshot {
            pub identity: Option<u64>,
            pub name: IdentifierSnapshot,
            pub relevance: &'static str,
            pub type_reference: TypeReferenceSnapshot,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum CapabilityMemberSnapshot {
            Field {
                name: IdentifierSnapshot,
                type_reference: TypeReferenceSnapshot,
            },
            State {
                signature: StateSignatureSnapshot,
                contracts: Vec<CapabilityContractSnapshot>,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct CapabilityContractSnapshot {
            pub kind: CapabilityContractKindSnapshot,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub binding: Option<IdentifierSnapshot>,
            pub facts: Vec<ProofFactSnapshot>,
            pub token_count: usize,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum CapabilityContractKindSnapshot {
            Ensures,
            EnsuresForResultCase {
                result_case: Vec<IdentifierSnapshot>,
            },
            Requires,
            Crashes {
                cause: &'static str,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum ProofFactSnapshot {
            Expression {
                expression: ExpressionSnapshot,
            },
            Membership {
                value: ExpressionSnapshot,
                domain: Vec<IdentifierSnapshot>,
                domain_arguments: Vec<TypeReferenceSnapshot>,
            },
        }

        pub(crate) fn snapshot_item(syntax_trees: &SyntaxTrees, item: &Item) -> ItemSnapshot {
            match item {
                Item::Capability(value) => ItemSnapshot::Capability {
                    name: snapshot_identifier(syntax_trees, &value.name),
                    members: syntax_trees
                        .items
                        .capability_members(value.members)
                        .iter()
                        .map(|member| snapshot_capability_member(syntax_trees, member))
                        .collect(),
                },
                Item::Conformance(value) => ItemSnapshot::Conformance {
                    is_public: value.is_public,
                    lifetime_parameters: snapshot_identifier_slice(
                        syntax_trees,
                        &value.lifetime_parameters,
                    ),
                    type_parameters: snapshot_type_parameters(syntax_trees, value.type_parameters),
                    trait_name: snapshot_identifier(syntax_trees, &value.trait_name),
                    trait_lifetime_arguments: snapshot_identifier_slice(
                        syntax_trees,
                        &value.trait_lifetime_arguments,
                    ),
                    trait_arguments: snapshot_type_references(syntax_trees, value.trait_arguments),
                    alias: value
                        .alias
                        .as_ref()
                        .map(|value| snapshot_identifier(syntax_trees, value)),
                    members: syntax_trees
                        .items
                        .conformance_members(value.members)
                        .iter()
                        .map(|member| match member {
                            ConformanceMember::Machine(machine) => {
                                ConformanceMemberSnapshot::Machine {
                                    declaration: Box::new(snapshot_item(
                                        syntax_trees,
                                        &Item::Machine(machine.clone()),
                                    )),
                                }
                            }
                            ConformanceMember::Synthesized(machine) => {
                                ConformanceMemberSnapshot::Synthesized {
                                    declaration: Box::new(snapshot_item(
                                        syntax_trees,
                                        &Item::Machine(machine.clone()),
                                    )),
                                }
                            }
                            ConformanceMember::TraitDefault {
                                declaring_trait,
                                requirement_ordinal,
                                machine,
                            } => ConformanceMemberSnapshot::TraitDefault {
                                declaring_trait: snapshot_identifier(syntax_trees, declaring_trait),
                                requirement_ordinal: *requirement_ordinal,
                                declaration: Box::new(snapshot_item(
                                    syntax_trees,
                                    &Item::Machine(machine.clone()),
                                )),
                            },
                            ConformanceMember::Selection {
                                declaring_trait,
                                requirement,
                                target,
                            } => ConformanceMemberSnapshot::Selection {
                                declaring_trait: snapshot_identifier(syntax_trees, declaring_trait),
                                requirement: snapshot_identifier(syntax_trees, requirement),
                                target: snapshot_identifier_slice(
                                    syntax_trees,
                                    syntax_trees.items.identifier_path_members(*target),
                                ),
                            },
                        })
                        .collect(),
                },
                Item::Const(value) => ItemSnapshot::Const {
                    scope: snapshot_identifier(syntax_trees, &value.scope),
                    name: snapshot_identifier(syntax_trees, &value.name),
                    is_public: value.is_public,
                    type_reference: snapshot_type_reference_handle(
                        syntax_trees,
                        value.type_reference,
                    ),
                    value: Box::new(snapshot_expression_handle(syntax_trees, value.value)),
                },
                Item::Data(value) => ItemSnapshot::Data {
                    name: snapshot_identifier(syntax_trees, &value.name),
                    is_public: value.is_public,
                    supply: match value.supply_mode {
                        language_core::DataSupplyMode::CheckedShape => "checked_shape",
                        language_core::DataSupplyMode::BoundaryOpaque => "boundary_opaque",
                    },
                    lifetime_parameters: snapshot_identifier_slice(
                        syntax_trees,
                        &value.lifetime_parameters,
                    ),
                    type_parameters: snapshot_type_parameters(syntax_trees, value.type_parameters),
                    conformance_bounds: value
                        .conformance_bounds
                        .iter()
                        .map(|bound| snapshot_generic_conformance_bound(syntax_trees, bound))
                        .collect(),
                    generic_instance: value
                        .generic_instance
                        .map(|origin| snapshot_type_reference_handle(syntax_trees, origin)),
                    properties: snapshot_data_properties(value.properties),
                    quotient: value.quotient.as_ref().map(|quotient| QuotientSnapshot {
                        carrier: snapshot_type_reference_handle(syntax_trees, quotient.carrier),
                        relation: snapshot_item_identifier_path(syntax_trees, quotient.relation),
                        equivalence: quotient.equivalence.as_ref().map(|selection| {
                            QuotientEquivalenceSelectionSnapshot {
                                relation: snapshot_item_identifier_path(
                                    syntax_trees,
                                    selection.relation,
                                ),
                                trait_name: snapshot_identifier(
                                    syntax_trees,
                                    &selection.trait_name,
                                ),
                                trait_arguments: snapshot_type_references(
                                    syntax_trees,
                                    selection.trait_arguments,
                                ),
                                conformance_name: snapshot_identifier(
                                    syntax_trees,
                                    &selection.conformance_name,
                                ),
                            }
                        }),
                    }),
                    where_facts: snapshot_proof_facts(syntax_trees, value.where_facts),
                    members: syntax_trees
                        .items
                        .data_members(value.members)
                        .iter()
                        .map(|member| snapshot_data_member(syntax_trees, member))
                        .collect(),
                },
                Item::TypeAlias(value) => ItemSnapshot::TypeAlias {
                    name: snapshot_identifier(syntax_trees, &value.name),
                    is_public: value.is_public,
                    lifetime_parameters: snapshot_identifier_slice(
                        syntax_trees,
                        &value.lifetime_parameters,
                    ),
                    type_parameters: snapshot_type_parameters(syntax_trees, value.type_parameters),
                    conformance_bounds: value
                        .conformance_bounds
                        .iter()
                        .map(|bound| snapshot_generic_conformance_bound(syntax_trees, bound))
                        .collect(),
                    expansion: snapshot_type_reference_handle(syntax_trees, value.expansion),
                },
                Item::Domain(value) => ItemSnapshot::Domain {
                    name: snapshot_identifier(syntax_trees, &value.name),
                    type_parameters: snapshot_type_parameters(syntax_trees, value.type_parameters),
                    target_type: snapshot_type_reference_handle(syntax_trees, value.target_type),
                    index_arguments: snapshot_type_references(syntax_trees, value.index_arguments),
                    is_public: value.is_public,
                    alias: value
                        .alias
                        .as_ref()
                        .map(|alias| {
                            alias
                                .constituents
                                .iter()
                                .map(|constituent| {
                                    snapshot_identifier_slice(
                                        syntax_trees,
                                        syntax_trees.items.identifier_path_members(*constituent),
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    authored_routes: value
                        .authored_routes
                        .iter()
                        .map(|route| snapshot_identifier_slice(syntax_trees, route))
                        .collect(),
                    classification: value.classification.map(|value| value.as_str()),
                    predicate_presence: value.predicate_presence.as_str(),
                    facts: snapshot_proof_facts(syntax_trees, value.facts),
                    operators: syntax_trees
                        .items
                        .operators(value.operators)
                        .iter()
                        .map(|operator| snapshot_operator(syntax_trees, operator))
                        .collect(),
                    semantic_clause_token_count: value.semantic_clause_token_count,
                },
                Item::Measure(value) => ItemSnapshot::Measure {
                    name: snapshot_identifier_slice(
                        syntax_trees,
                        syntax_trees.items.identifier_path_members(value.name),
                    ),
                    parameter: value.parameter.is_valid().then(|| {
                        snapshot_state_parameter(
                            syntax_trees,
                            syntax_trees.items.state_parameter(value.parameter),
                        )
                    }),
                    return_type: snapshot_type_reference_handle(syntax_trees, value.return_type),
                    lexicographic: value.lexicographic,
                    body: syntax_trees
                        .expressions
                        .expression_handles(value.body)
                        .iter()
                        .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                        .collect(),
                    token_count: value.token_count,
                },
                Item::Operator(value) => ItemSnapshot::Operator {
                    operator: snapshot_operator(syntax_trees, value),
                },
                Item::Module(value) => ItemSnapshot::Module {
                    path: snapshot_identifier_slice(
                        syntax_trees,
                        syntax_trees.items.identifier_path_members(value.path),
                    ),
                },
                Item::Package(value) => ItemSnapshot::Package {
                    path: snapshot_identifier_slice(
                        syntax_trees,
                        syntax_trees.items.identifier_path_members(value.path),
                    ),
                },
                Item::Proposition(value) => ItemSnapshot::Proposition {
                    name: snapshot_identifier(syntax_trees, &value.name),
                    is_public: value.is_public,
                    type_parameters: snapshot_type_parameters(syntax_trees, value.type_parameters),
                    parameters: snapshot_state_parameters(syntax_trees, value.parameters),
                    body: match value.body {
                        PropositionBody::Primitive => PropositionBodySnapshot::Primitive,
                        PropositionBody::Witness { evidence } => PropositionBodySnapshot::Witness {
                            evidence: snapshot_type_reference_handle(syntax_trees, evidence),
                        },
                        PropositionBody::Transparent { proposition } => {
                            PropositionBodySnapshot::Transparent {
                                proposition: Box::new(snapshot_expression_handle(
                                    syntax_trees,
                                    proposition,
                                )),
                            }
                        }
                    },
                },
                Item::Use(value) => ItemSnapshot::Use {
                    path: snapshot_identifier_slice(
                        syntax_trees,
                        syntax_trees.items.identifier_path_members(value.path),
                    ),
                },
                Item::Machine(value) => ItemSnapshot::Machine {
                    nested_machines: value
                        .nested_machines
                        .handles()
                        .map(|handle| snapshot_item(syntax_trees, syntax_trees.items.item(handle)))
                        .collect(),
                    name: snapshot_identifier(syntax_trees, &value.name),
                    attached_data: value
                        .attached_data
                        .as_ref()
                        .map(|value| snapshot_identifier(syntax_trees, value)),
                    spelling: value.spelling.map(|spelling| spelling.symbol()),
                    is_public: value.is_public,
                    bodyless: value.bodyless,
                    target: value
                        .target
                        .as_ref()
                        .map(|value| snapshot_identifier(syntax_trees, value)),
                    authored_target: value
                        .authored_target
                        .as_ref()
                        .map(|value| snapshot_identifier(syntax_trees, value)),
                    boundary: value.boundary,
                    lifetime_parameters: snapshot_identifier_slice(
                        syntax_trees,
                        &value.lifetime_parameters,
                    ),
                    type_parameters: snapshot_type_parameters(syntax_trees, value.type_parameters),
                    satisfies: syntax_trees
                        .items
                        .satisfies_clauses(value.satisfies)
                        .iter()
                        .map(|clause| snapshot_satisfies_clause(syntax_trees, clause))
                        .collect(),
                    conformance_bounds: value
                        .conformance_bounds
                        .iter()
                        .map(|bound| snapshot_generic_conformance_bound(syntax_trees, bound))
                        .collect(),
                    terminates_guarantee: value.terminates_guarantee,
                    ranking_subjects: syntax_trees
                        .expressions
                        .expression_handles(value.ranking_subjects)
                        .iter()
                        .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                        .collect(),
                    ranking_view: snapshot_item_identifier_path(syntax_trees, value.ranking_view),
                    ranking_view_arguments: syntax_trees
                        .expressions
                        .expression_handles(value.ranking_view_arguments)
                        .iter()
                        .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                        .collect(),
                    ranking_range: value.ranking_range.is_valid().then(|| {
                        Box::new(snapshot_expression_handle(
                            syntax_trees,
                            value.ranking_range,
                        ))
                    }),
                    service_reach_is_installation_bound: value.service_reach_is_installation_bound,
                    service_reaches: snapshot_item_identifier_path(
                        syntax_trees,
                        value.service_reaches,
                    ),
                    invokes: snapshot_identifier_slice(
                        syntax_trees,
                        syntax_trees.items.identifier_path_members(value.invokes),
                    ),
                    suspends: value.suspends,
                    blocks: value.blocks,
                    admitted_same_stack_bytes: value.admitted_same_stack_bytes,
                    contracts: snapshot_capability_contracts(syntax_trees, value.contracts),
                    states: syntax_trees
                        .items
                        .state_handles(value.states)
                        .iter()
                        .map(|handle| {
                            snapshot_state_node(syntax_trees, syntax_trees.items.state(*handle))
                        })
                        .collect(),
                },
                Item::Trait(value) => ItemSnapshot::Trait {
                    name: snapshot_identifier(syntax_trees, &value.name),
                    is_boundary: value.is_boundary,
                    is_public: value.is_public,
                    lifetime_parameters: snapshot_identifier_slice(
                        syntax_trees,
                        &value.lifetime_parameters,
                    ),
                    type_parameters: snapshot_type_parameters(syntax_trees, value.type_parameters),
                    conformance_bounds: value
                        .conformance_bounds
                        .iter()
                        .map(|bound| snapshot_generic_conformance_bound(syntax_trees, bound))
                        .collect(),
                    parents: snapshot_type_references(syntax_trees, value.parents),
                    requires: snapshot_identifier_slice(
                        syntax_trees,
                        syntax_trees.items.identifier_path_members(value.requires),
                    ),
                    machines: syntax_trees
                        .items
                        .state_signatures(value.machines)
                        .iter()
                        .map(|handle| {
                            snapshot_state_signature_node(
                                syntax_trees,
                                syntax_trees.items.state_signature(*handle),
                            )
                        })
                        .collect(),
                },
            }
        }

        fn snapshot_operator(
            syntax_trees: &SyntaxTrees,
            operator: &crate::item::OperatorDefinition,
        ) -> OperatorSnapshot {
            OperatorSnapshot {
                is_public: operator.is_public,
                is_boundary: operator.is_boundary,
                name: snapshot_identifier_slice(
                    syntax_trees,
                    syntax_trees.items.identifier_path_members(operator.name),
                ),
                lifetime_parameters: snapshot_identifier_slice(
                    syntax_trees,
                    &operator.lifetime_parameters,
                ),
                type_parameters: snapshot_type_parameters(syntax_trees, operator.type_parameters),
                parameters: snapshot_state_parameters(syntax_trees, operator.parameters),
                return_type: snapshot_type_reference_handle(syntax_trees, operator.return_type),
                contracts: snapshot_capability_contracts(syntax_trees, operator.contracts),
                spelling: operator.spelling.map(|spelling| spelling.symbol()),
                token_count: operator.token_count,
            }
        }

        pub(crate) fn snapshot_type_parameter(
            syntax_trees: &SyntaxTrees,
            parameter: &crate::item::TypeParameter,
        ) -> TypeParameterSnapshot {
            let (kind, const_type, machine_contract, machine_requirement, proposition_contract) =
                match &parameter.kind {
                    TypeParameterKind::Type => ("type", None, None, None, None),
                    TypeParameterKind::Implementation => ("implementation", None, None, None, None),
                    TypeParameterKind::Const { type_reference } => (
                        "const",
                        Some(snapshot_type_reference_handle(
                            syntax_trees,
                            *type_reference,
                        )),
                        None,
                        None,
                        None,
                    ),
                    TypeParameterKind::Value { type_reference } => (
                        "value",
                        Some(snapshot_type_reference_handle(
                            syntax_trees,
                            *type_reference,
                        )),
                        None,
                        None,
                        None,
                    ),
                    TypeParameterKind::Machine { contract } => match contract {
                        Some(MachineParameterContract::RequirementIdentity) => {
                            ("machine_requirement", None, None, None, None)
                        }
                        Some(MachineParameterContract::Structural(signature)) => (
                            "machine",
                            None,
                            Some(snapshot_state_signature(syntax_trees, signature)),
                            None,
                            None,
                        ),
                        Some(MachineParameterContract::Nominal { requirement, .. }) => (
                            "machine",
                            None,
                            None,
                            Some(snapshot_identifier_slice(
                                syntax_trees,
                                syntax_trees.items.identifier_path_members(*requirement),
                            )),
                            None,
                        ),
                        None => ("machine", None, None, None, None),
                    },
                    TypeParameterKind::Proposition { contract } => (
                        "proposition",
                        None,
                        None,
                        None,
                        contract
                            .as_ref()
                            .map(|contract| PropositionParameterSignatureSnapshot {
                                name: snapshot_identifier(syntax_trees, &contract.name),
                                parameters: snapshot_state_parameters(
                                    syntax_trees,
                                    contract.parameters,
                                ),
                            }),
                    ),
                };
            TypeParameterSnapshot {
                name: snapshot_identifier(syntax_trees, &parameter.name),
                kind,
                const_type,
                machine_contract,
                machine_requirement,
                proposition_contract,
                bounds: snapshot_data_properties(parameter.bounds),
            }
        }

        pub(crate) fn snapshot_type_parameters(
            syntax_trees: &SyntaxTrees,
            span: HandleSpan<TypeParameter>,
        ) -> Vec<TypeParameterSnapshot> {
            syntax_trees
                .items
                .type_parameters(span)
                .iter()
                .map(|parameter| snapshot_type_parameter(syntax_trees, parameter))
                .collect()
        }
        fn snapshot_data_properties(
            properties: crate::item::DataProperties,
        ) -> DataPropertiesSnapshot {
            use language_core::{
                CarryAddress, CarryCpu, CarryHostThread, CarrySuspension, Multiplicity,
            };
            DataPropertiesSnapshot {
                multiplicity: match properties.multiplicity {
                    Multiplicity::Unrestricted => "unrestricted",
                    Multiplicity::Affine => "affine",
                    Multiplicity::Linear => "linear",
                },
                carry: properties.carry.map(|carry| CarryPolicySnapshot {
                    suspension: match carry.suspension {
                        CarrySuspension::Forbidden => "forbidden",
                        CarrySuspension::Allowed => "allowed",
                    },
                    cpu: match carry.cpu {
                        CarryCpu::Origin => "same",
                        CarryCpu::Any => "any",
                    },
                    thread: match carry.host_thread {
                        CarryHostThread::Origin => "same",
                        CarryHostThread::Any => "any",
                    },
                    address: match carry.address {
                        CarryAddress::Stable => "stable",
                        CarryAddress::Movable => "movable",
                    },
                }),
            }
        }

        fn snapshot_capability_member(
            syntax_trees: &SyntaxTrees,
            member: &CapabilityMember,
        ) -> CapabilityMemberSnapshot {
            match member {
                CapabilityMember::Field(field) => CapabilityMemberSnapshot::Field {
                    name: snapshot_identifier(syntax_trees, &field.name),
                    type_reference: snapshot_type_reference_handle(
                        syntax_trees,
                        field.type_reference,
                    ),
                },
                CapabilityMember::State(state) => CapabilityMemberSnapshot::State {
                    signature: snapshot_state_signature(syntax_trees, &state.signature),
                    contracts: snapshot_capability_contracts(syntax_trees, state.contracts),
                },
            }
        }

        pub(crate) fn snapshot_capability_contracts(
            syntax_trees: &SyntaxTrees,
            contracts: arena::HandleSpan<CapabilityContract>,
        ) -> Vec<CapabilityContractSnapshot> {
            syntax_trees
                .items
                .capability_contracts(contracts)
                .iter()
                .map(|contract| snapshot_capability_contract(syntax_trees, contract))
                .collect()
        }

        pub(crate) fn snapshot_capability_contract(
            syntax_trees: &SyntaxTrees,
            contract: &CapabilityContract,
        ) -> CapabilityContractSnapshot {
            CapabilityContractSnapshot {
                kind: match &contract.kind {
                    CapabilityContractKind::Ensures => CapabilityContractKindSnapshot::Ensures,
                    CapabilityContractKind::EnsuresForResultCase { result_case } => {
                        CapabilityContractKindSnapshot::EnsuresForResultCase {
                            result_case: snapshot_item_identifier_path(syntax_trees, *result_case),
                        }
                    }
                    CapabilityContractKind::Requires => CapabilityContractKindSnapshot::Requires,
                    CapabilityContractKind::Crashes { cause } => {
                        CapabilityContractKindSnapshot::Crashes {
                            cause: match cause {
                                CrashCause::Trap => "Trap",
                                CrashCause::Abort => "Abort",
                            },
                        }
                    }
                },
                binding: contract
                    .binding
                    .as_ref()
                    .map(|value| snapshot_identifier(syntax_trees, value)),
                facts: snapshot_proof_facts(syntax_trees, contract.facts),
                token_count: contract.token_count,
            }
        }

        fn snapshot_proof_facts(
            syntax_trees: &SyntaxTrees,
            facts: arena::HandleSpan<ProofFact>,
        ) -> Vec<ProofFactSnapshot> {
            syntax_trees
                .items
                .proof_facts(facts)
                .iter()
                .map(|fact| snapshot_proof_fact(syntax_trees, fact))
                .collect()
        }

        fn snapshot_proof_fact(syntax_trees: &SyntaxTrees, fact: &ProofFact) -> ProofFactSnapshot {
            match fact {
                ProofFact::Expression(expression) => ProofFactSnapshot::Expression {
                    expression: snapshot_expression_handle(syntax_trees, *expression),
                },
                ProofFact::Membership(membership) => ProofFactSnapshot::Membership {
                    value: snapshot_expression_handle(syntax_trees, membership.value),
                    domain: snapshot_item_identifier_path(syntax_trees, membership.domain),
                    domain_arguments: snapshot_type_references(
                        syntax_trees,
                        membership.domain_arguments,
                    ),
                },
            }
        }

        fn snapshot_data_member(
            syntax_trees: &SyntaxTrees,
            member: &DataMember,
        ) -> DataMemberSnapshot {
            match member {
                DataMember::Field(field) => DataMemberSnapshot::Field {
                    identity: field.identity,
                    name: snapshot_identifier(syntax_trees, &field.name),
                    relevance: snapshot_binding_relevance(field.relevance),
                    type_reference: snapshot_type_reference_handle(
                        syntax_trees,
                        field.type_reference,
                    ),
                },
                DataMember::Variant(variant) => DataMemberSnapshot::Variant {
                    identity: variant.identity,
                    name: snapshot_identifier(syntax_trees, &variant.name),
                    payload: syntax_trees
                        .items
                        .data_payload_fields(variant.payload)
                        .iter()
                        .map(|field| DataPayloadFieldSnapshot {
                            identity: field.identity,
                            name: snapshot_identifier(syntax_trees, &field.name),
                            relevance: snapshot_binding_relevance(field.relevance),
                            type_reference: snapshot_type_reference_handle(
                                syntax_trees,
                                field.type_reference,
                            ),
                        })
                        .collect(),
                    retired_payload_identities: variant.retired_payload_identities.clone(),
                },
                DataMember::Retired(identity) => DataMemberSnapshot::Retired {
                    identity: *identity,
                },
            }
        }

        pub(crate) fn snapshot_binding_relevance(
            relevance: language_core::BindingRelevance,
        ) -> &'static str {
            match relevance {
                language_core::BindingRelevance::Relevant => "relevant",
                language_core::BindingRelevance::Erased => "erased",
            }
        }

        fn snapshot_satisfies_clause(
            syntax_trees: &SyntaxTrees,
            clause: &SatisfiesClause,
        ) -> SatisfiesClauseSnapshot {
            SatisfiesClauseSnapshot {
                trait_name: snapshot_identifier(syntax_trees, &clause.trait_name),
                lifetime_arguments: snapshot_identifier_slice(
                    syntax_trees,
                    &clause.lifetime_arguments,
                ),
                arguments: snapshot_type_references(syntax_trees, clause.arguments),
                requirement: clause
                    .requirement
                    .as_ref()
                    .map(|value| snapshot_identifier(syntax_trees, value)),
                alias: clause
                    .alias
                    .as_ref()
                    .map(|value| snapshot_identifier(syntax_trees, value)),
                via: clause
                    .via
                    .as_ref()
                    .map(|value| snapshot_external_binding(syntax_trees, value)),
                via_expression: clause.via_expression.is_valid().then(|| {
                    Box::new(snapshot_expression_handle(
                        syntax_trees,
                        clause.via_expression,
                    ))
                }),
            }
        }

        pub(crate) fn snapshot_generic_conformance_bound(
            syntax_trees: &SyntaxTrees,
            bound: &GenericConformanceBound,
        ) -> GenericConformanceBoundSnapshot {
            GenericConformanceBoundSnapshot {
                binder: snapshot_identifier(syntax_trees, &bound.binder),
                trait_name: snapshot_identifier(syntax_trees, &bound.trait_name),
                lifetime_arguments: bound
                    .lifetime_arguments
                    .iter()
                    .map(|name| snapshot_identifier(syntax_trees, name))
                    .collect(),
                arguments: snapshot_type_references(syntax_trees, bound.arguments),
            }
        }

        fn snapshot_external_binding(
            syntax_trees: &SyntaxTrees,
            binding: &ExternalBinding,
        ) -> ExternalBindingSnapshot {
            match binding {
                ExternalBinding::Syscall { number } => {
                    ExternalBindingSnapshot::Syscall { number: *number }
                }
                ExternalBinding::CompilerIntrinsic => ExternalBindingSnapshot::CompilerIntrinsic,
                ExternalBinding::VtableSlot { index } => {
                    ExternalBindingSnapshot::VtableSlot { index: *index }
                }
                ExternalBinding::VtableField { field } => ExternalBindingSnapshot::VtableField {
                    field: snapshot_identifier(syntax_trees, field),
                },
                ExternalBinding::TableFunction { field } => {
                    ExternalBindingSnapshot::TableFunction {
                        field: snapshot_identifier(syntax_trees, field),
                    }
                }
            }
        }
    }
    pub(crate) mod state_and_statement_snapshots {
        use crate::SyntaxTrees;
        use crate::inspection::snapshot::expression_and_type_snapshots::{
            snapshot_expression_handle, snapshot_identifier, snapshot_identifier_slice,
            snapshot_item_identifier_path, snapshot_static_argument,
            snapshot_type_reference_handle,
        };
        use crate::inspection::snapshot::item_snapshots::{
            snapshot_capability_contract, snapshot_capability_contracts,
            snapshot_generic_conformance_bound, snapshot_type_parameters,
        };
        use crate::inspection::snapshot::{
            CapabilityContractSnapshot, ExpressionSnapshot, GenericConformanceBoundSnapshot,
            IdentifierSnapshot, StaticArgumentSnapshot, TypeParameterSnapshot,
            TypeReferenceSnapshot,
        };
        use crate::item::{
            CrashCause, State, StateParameterHandle, StateParameterNode, StateSignature,
        };
        use crate::statement::{
            AssemblyExit, AssemblyFactKind, StatementHandle, StatementNode, TransitionExit,
            TransitionGuardNode, TransitionTargetNode,
        };
        use arena::HandleSpan;
        use serde::Serialize;

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct StateSnapshot {
            pub name: IdentifierSnapshot,
            pub parameters: Vec<StateParameterSnapshot>,
            pub return_type: TypeReferenceSnapshot,
            pub contracts: Vec<CapabilityContractSnapshot>,
            pub statements: Vec<StatementSnapshot>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct StateSignatureSnapshot {
            pub name: IdentifierSnapshot,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub spelling: Option<&'static str>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub lifetime_parameters: Vec<IdentifierSnapshot>,
            pub type_parameters: Vec<TypeParameterSnapshot>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub conformance_bounds: Vec<GenericConformanceBoundSnapshot>,
            pub is_default: bool,
            pub parameters: Vec<StateParameterSnapshot>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub native_callback_parameters: Vec<NativeCallbackParameterSnapshot>,
            pub return_type: TypeReferenceSnapshot,
            pub service_reach_is_installation_bound: bool,
            pub service_reaches: Vec<IdentifierSnapshot>,
            pub invokes: Vec<IdentifierSnapshot>,
            pub suspends: bool,
            pub blocks: bool,
            pub contracts: Vec<CapabilityContractSnapshot>,
            pub default_body: Vec<StatementSnapshot>,
            pub terminates_guarantee: bool,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct NativeCallbackParameterSnapshot {
            pub name: IdentifierSnapshot,
            pub binder: IdentifierSnapshot,
            pub native_ordinal: u32,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct StateParameterSnapshot {
            pub name: IdentifierSnapshot,
            pub type_reference: TypeReferenceSnapshot,
            pub is_const: bool,
            pub is_mutable: bool,
            pub relevance: &'static str,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum StatementSnapshot {
            Assembly {
                primitives: Vec<ExpressionSnapshot>,
                scratch: Vec<ExpressionSnapshot>,
                blocks: Vec<AssemblyBlockSnapshot>,
            },
            RootBinding {
                receiver: ExpressionSnapshot,
                slot: Vec<String>,
                implementation: Vec<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                implementation_operand: Option<ExpressionSnapshot>,
            },
            AssemblyFact {
                contract_kind: AssemblyFactKindSnapshot,
                expression: ExpressionSnapshot,
            },
            Assignment {
                target: ExpressionSnapshot,
                value: ExpressionSnapshot,
            },
            Call {
                receiver: Vec<IdentifierSnapshot>,
                target: IdentifierSnapshot,
                machine_arguments: Vec<StaticArgumentSnapshot>,
                arguments: Vec<ExpressionSnapshot>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                evidence_arguments: Vec<IdentifierSnapshot>,
                acknowledgement_synthesized: bool,
                acknowledges_suspend: bool,
                acknowledges_block: bool,
                discards_result: bool,
            },
            ProofOutputBindingStatement {
                bindings: Vec<(IdentifierSnapshot, IdentifierSnapshot)>,
                call: ExpressionSnapshot,
            },
            Expression {
                value: ExpressionSnapshot,
            },
            LocalData {
                role: String,
                name: IdentifierSnapshot,
                type_reference: TypeReferenceSnapshot,
                initial_value: ExpressionSnapshot,
                is_mutable: bool,
                relevance: &'static str,
            },
            Transition {
                #[serde(skip_serializing_if = "Vec::is_empty")]
                patterns: Vec<(
                    bool,
                    ExpressionSnapshot,
                    super::expression_and_type_snapshots::MatchPatternSnapshot,
                )>,
                target: TransitionTargetSnapshot,
                continuation: Option<TransitionTargetSnapshot>,
                guard: TransitionGuardSnapshot,
                #[serde(skip_serializing_if = "Option::is_none")]
                crash_cause: Option<&'static str>,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct AssemblyBlockSnapshot {
            pub label: IdentifierSnapshot,
            pub requires: Vec<ExpressionSnapshot>,
            pub ensures: Vec<ExpressionSnapshot>,
            pub instructions: Vec<StatementSnapshot>,
            pub exit: AssemblyExitSnapshot,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub enum AssemblyExitSnapshot {
            Fallthrough,
            Jump(IdentifierSnapshot),
            ZeroFlag(IdentifierSnapshot, bool),
            EnclosingTransition,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub enum AssemblyFactKindSnapshot {
            Requires,
            Ensures,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum TransitionGuardSnapshot {
            Always,
            When { expression: ExpressionSnapshot },
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum TransitionTargetSnapshot {
            Named {
                path: Vec<IdentifierSnapshot>,
                arguments: Vec<ExpressionSnapshot>,
                evidence_arguments: Vec<IdentifierSnapshot>,
            },
            Value {
                expression: ExpressionSnapshot,
            },
            Terminal,
        }

        pub(crate) fn snapshot_state_node(
            syntax_trees: &SyntaxTrees,
            state: &State,
        ) -> StateSnapshot {
            StateSnapshot {
                name: snapshot_identifier(syntax_trees, &state.name),
                parameters: snapshot_state_parameters(syntax_trees, state.parameters),
                return_type: snapshot_type_reference_handle(syntax_trees, state.return_type),
                contracts: syntax_trees
                    .items
                    .capability_contracts(state.contracts)
                    .iter()
                    .map(|contract| snapshot_capability_contract(syntax_trees, contract))
                    .collect(),
                statements: snapshot_statements(syntax_trees, state.statements),
            }
        }

        macro_rules! state_signature_snapshot {
            ($syntax_trees:ident, $signature:ident) => {
                StateSignatureSnapshot {
                    name: snapshot_identifier($syntax_trees, &$signature.name),
                    spelling: $signature.spelling.map(|spelling| spelling.symbol()),
                    lifetime_parameters: snapshot_identifier_slice(
                        $syntax_trees,
                        &$signature.lifetime_parameters,
                    ),
                    type_parameters: snapshot_type_parameters(
                        $syntax_trees,
                        $signature.type_parameters,
                    ),
                    conformance_bounds: $signature
                        .conformance_bounds
                        .iter()
                        .map(|bound| snapshot_generic_conformance_bound($syntax_trees, bound))
                        .collect(),
                    is_default: $signature.is_default,
                    parameters: snapshot_state_parameters($syntax_trees, $signature.parameters),
                    native_callback_parameters: $signature
                        .native_callback_parameters
                        .iter()
                        .map(|parameter| NativeCallbackParameterSnapshot {
                            name: snapshot_identifier($syntax_trees, &parameter.name),
                            binder: snapshot_identifier($syntax_trees, &parameter.binder),
                            native_ordinal: parameter.native_ordinal,
                        })
                        .collect(),
                    return_type: snapshot_type_reference_handle(
                        $syntax_trees,
                        $signature.return_type,
                    ),
                    service_reach_is_installation_bound: $signature
                        .service_reach_is_installation_bound,
                    service_reaches: snapshot_item_identifier_path(
                        $syntax_trees,
                        $signature.service_reaches,
                    ),
                    invokes: snapshot_item_identifier_path($syntax_trees, $signature.invokes),
                    suspends: $signature.suspends,
                    blocks: $signature.blocks,
                    contracts: snapshot_capability_contracts($syntax_trees, $signature.contracts),
                    default_body: snapshot_statements($syntax_trees, $signature.default_body),
                    terminates_guarantee: $signature.terminates_guarantee,
                }
            };
        }

        pub(crate) fn snapshot_state_signature(
            syntax_trees: &SyntaxTrees,
            signature: &StateSignature,
        ) -> StateSignatureSnapshot {
            state_signature_snapshot!(syntax_trees, signature)
        }

        pub(crate) fn snapshot_state_signature_node(
            syntax_trees: &SyntaxTrees,
            signature: &StateSignature,
        ) -> StateSignatureSnapshot {
            state_signature_snapshot!(syntax_trees, signature)
        }

        pub(crate) fn snapshot_state_parameter(
            syntax_trees: &SyntaxTrees,
            parameter: &StateParameterNode,
        ) -> StateParameterSnapshot {
            StateParameterSnapshot {
                name: snapshot_identifier(syntax_trees, &parameter.name),
                type_reference: snapshot_type_reference_handle(
                    syntax_trees,
                    parameter.type_reference,
                ),
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
                relevance: super::item_snapshots::snapshot_binding_relevance(parameter.relevance),
            }
        }

        pub(crate) fn snapshot_state_parameters(
            syntax_trees: &SyntaxTrees,
            span: HandleSpan<StateParameterHandle>,
        ) -> Vec<StateParameterSnapshot> {
            syntax_trees
                .items
                .state_parameters(span)
                .iter()
                .map(|handle| {
                    snapshot_state_parameter(
                        syntax_trees,
                        syntax_trees.items.state_parameter(*handle),
                    )
                })
                .collect()
        }

        fn snapshot_statement(
            syntax_trees: &SyntaxTrees,
            statement: &StatementNode,
        ) -> StatementSnapshot {
            match statement {
                StatementNode::Assembly(handle) => {
                    let graph = syntax_trees.statements.assembly(*handle);
                    StatementSnapshot::Assembly {
                        primitives: syntax_trees
                            .statements
                            .assembly_expressions(graph.primitives)
                            .iter()
                            .map(|expression| snapshot_expression_handle(syntax_trees, *expression))
                            .collect(),
                        scratch: graph
                            .scratch
                            .iter()
                            .copied()
                            .filter(|expression| expression.is_valid())
                            .map(|expression| snapshot_expression_handle(syntax_trees, expression))
                            .collect(),
                        blocks: syntax_trees
                            .statements
                            .assembly_blocks(graph.blocks)
                            .iter()
                            .map(|block| AssemblyBlockSnapshot {
                                label: snapshot_identifier(syntax_trees, &block.label),
                                requires: syntax_trees
                                    .statements
                                    .assembly_expressions(block.requires)
                                    .iter()
                                    .map(|expression| {
                                        snapshot_expression_handle(syntax_trees, *expression)
                                    })
                                    .collect(),
                                ensures: syntax_trees
                                    .statements
                                    .assembly_expressions(block.ensures)
                                    .iter()
                                    .map(|expression| {
                                        snapshot_expression_handle(syntax_trees, *expression)
                                    })
                                    .collect(),
                                instructions: syntax_trees
                                    .statements
                                    .assembly_instructions(block.instructions)
                                    .iter()
                                    .map(|handle| {
                                        snapshot_statement(
                                            syntax_trees,
                                            syntax_trees.statements.statement(*handle),
                                        )
                                    })
                                    .collect(),
                                exit: match &block.exit {
                                    AssemblyExit::Fallthrough => AssemblyExitSnapshot::Fallthrough,
                                    AssemblyExit::ZeroFlag(label, when_set) => {
                                        AssemblyExitSnapshot::ZeroFlag(
                                            snapshot_identifier(syntax_trees, label),
                                            *when_set,
                                        )
                                    }
                                    AssemblyExit::Jump(label) => AssemblyExitSnapshot::Jump(
                                        snapshot_identifier(syntax_trees, label),
                                    ),
                                    AssemblyExit::EnclosingTransition => {
                                        AssemblyExitSnapshot::EnclosingTransition
                                    }
                                },
                            })
                            .collect(),
                    }
                }
                StatementNode::RootBinding(binding) => StatementSnapshot::RootBinding {
                    receiver: snapshot_expression_handle(syntax_trees, binding.receiver),
                    slot: binding
                        .slot
                        .iter()
                        .map(|name| name.as_str(syntax_trees).to_owned())
                        .collect(),
                    implementation: binding
                        .implementation
                        .iter()
                        .map(|name| name.as_str(syntax_trees).to_owned())
                        .collect(),
                    implementation_operand: binding.implementation_operand.is_valid().then(|| {
                        snapshot_expression_handle(syntax_trees, binding.implementation_operand)
                    }),
                },
                StatementNode::AssemblyFact(fact) => StatementSnapshot::AssemblyFact {
                    contract_kind: match fact.kind {
                        AssemblyFactKind::Requires => AssemblyFactKindSnapshot::Requires,
                        AssemblyFactKind::Ensures => AssemblyFactKindSnapshot::Ensures,
                    },
                    expression: snapshot_expression_handle(syntax_trees, fact.expression),
                },
                StatementNode::Assignment(assignment) => StatementSnapshot::Assignment {
                    target: snapshot_expression_handle(syntax_trees, assignment.target),
                    value: snapshot_expression_handle(syntax_trees, assignment.value),
                },
                StatementNode::Call(call) => StatementSnapshot::Call {
                    receiver: snapshot_identifier_slice(
                        syntax_trees,
                        syntax_trees
                            .statements
                            .identifier_path_members(call.receiver),
                    ),
                    target: snapshot_identifier(syntax_trees, &call.target),
                    machine_arguments: call
                        .machine_arguments
                        .iter()
                        .map(|argument| snapshot_static_argument(syntax_trees, argument))
                        .collect(),
                    arguments: syntax_trees
                        .statements
                        .expression_handles(call.arguments)
                        .iter()
                        .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                        .collect(),
                    evidence_arguments: snapshot_identifier_slice(
                        syntax_trees,
                        &call.evidence_arguments,
                    ),
                    acknowledgement_synthesized: call.operational_acknowledgement.origin
                        == language_core::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                    acknowledges_suspend: call.operational_acknowledgement.acknowledges_suspend,
                    acknowledges_block: call.operational_acknowledgement.acknowledges_block,
                    discards_result: call.discards_result,
                },
                StatementNode::ProofOutputBindingStatement(binding) => {
                    StatementSnapshot::ProofOutputBindingStatement {
                        bindings: binding
                            .bindings
                            .iter()
                            .map(|binding| {
                                (
                                    snapshot_identifier(syntax_trees, &binding.output_field),
                                    snapshot_identifier(syntax_trees, &binding.binding),
                                )
                            })
                            .collect(),
                        call: snapshot_expression_handle(syntax_trees, binding.call),
                    }
                }
                StatementNode::Expression(value) => StatementSnapshot::Expression {
                    value: snapshot_expression_handle(syntax_trees, *value),
                },
                StatementNode::LocalData(value) => StatementSnapshot::LocalData {
                    role: format!("{:?}", value.role),
                    name: snapshot_identifier(syntax_trees, &value.name),
                    type_reference: snapshot_type_reference_handle(
                        syntax_trees,
                        value.type_reference,
                    ),
                    initial_value: snapshot_expression_handle(syntax_trees, value.initial_value),
                    is_mutable: value.is_mutable,
                    relevance: super::item_snapshots::snapshot_binding_relevance(value.relevance),
                },
                StatementNode::Transition(value) => StatementSnapshot::Transition {
                    patterns: syntax_trees
                        .statements
                        .transition_patterns(value.patterns)
                        .iter()
                        .map(|retained| {
                            (
                                retained.is_structural,
                                snapshot_expression_handle(syntax_trees, retained.subject),
                                super::expression_and_type_snapshots::snapshot_match_pattern(
                                    syntax_trees,
                                    crate::expression::MatchPattern::Destructure(retained.pattern),
                                ),
                            )
                        })
                        .collect(),
                    target: snapshot_transition_target(
                        syntax_trees,
                        syntax_trees.statements.transition_target(value.target),
                    ),
                    continuation: value.continuation.is_valid().then(|| {
                        snapshot_transition_target(
                            syntax_trees,
                            syntax_trees
                                .statements
                                .transition_target(value.continuation),
                        )
                    }),
                    guard: match value.guard {
                        TransitionGuardNode::Always => TransitionGuardSnapshot::Always,
                        TransitionGuardNode::When(expression) => TransitionGuardSnapshot::When {
                            expression: snapshot_expression_handle(syntax_trees, expression),
                        },
                    },
                    crash_cause: match value.exit {
                        TransitionExit::Ordinary => None,
                        TransitionExit::Crash(CrashCause::Trap) => Some("Trap"),
                        TransitionExit::Crash(CrashCause::Abort) => Some("Abort"),
                    },
                },
            }
        }

        pub(crate) fn snapshot_statements(
            syntax_trees: &SyntaxTrees,
            span: HandleSpan<StatementHandle>,
        ) -> Vec<StatementSnapshot> {
            syntax_trees
                .items
                .statements(span)
                .iter()
                .map(|handle| {
                    snapshot_statement(syntax_trees, syntax_trees.statements.statement(*handle))
                })
                .collect()
        }

        fn snapshot_transition_target(
            syntax_trees: &SyntaxTrees,
            target: &TransitionTargetNode,
        ) -> TransitionTargetSnapshot {
            match target {
                TransitionTargetNode::Named {
                    path,
                    arguments,
                    evidence_arguments,
                    ..
                } => TransitionTargetSnapshot::Named {
                    path: snapshot_identifier_slice(
                        syntax_trees,
                        syntax_trees.statements.identifier_path_members(*path),
                    ),
                    arguments: syntax_trees
                        .statements
                        .expression_handles(*arguments)
                        .iter()
                        .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                        .collect(),
                    evidence_arguments: snapshot_identifier_slice(syntax_trees, evidence_arguments),
                },
                TransitionTargetNode::Value(expression) => TransitionTargetSnapshot::Value {
                    expression: snapshot_expression_handle(syntax_trees, *expression),
                },
                TransitionTargetNode::Terminal => TransitionTargetSnapshot::Terminal,
            }
        }
    }
    #[cfg(test)]
    pub(crate) mod tests {
        use super::{ItemSnapshot, SyntaxTreesSnapshot, TypeReferenceSnapshot};
        use crate::SyntaxTrees;
        use crate::identifier::Identifier;
        use crate::item::{DataDefinition, DataField, DataMember, DataVariant, Item};
        use crate::types::TypeReferenceNode;

        #[test]
        fn snapshots_materialize_handle_backed_syntax_shape() {
            let mut syntax_trees = SyntaxTrees::new(Default::default());
            let i32_type = syntax_trees
                .type_references
                .insert(TypeReferenceNode::Named(Identifier::generated("i32")));
            let data = Item::Data(DataDefinition {
                cleanup_selection: None,
                name: Identifier::generated("Example"),
                is_public: true,
                supply_mode: language_core::DataSupplyMode::CheckedShape,
                lifetime_parameters: Vec::new(),
                type_parameters: arena::HandleSpan::empty(),
                conformance_bounds: Vec::new(),
                generic_instance: None,
                properties: crate::item::DataProperties::default(),
                quotient: None,
                where_facts: arena::HandleSpan::empty(),
                members: arena::HandleSpan::from_parts(
                    syntax_trees
                        .items
                        .append_data_member(DataMember::Field(DataField {
                            identity: None,
                            name: Identifier::generated("field"),
                            relevance: language_core::BindingRelevance::Relevant,
                            type_reference: i32_type,
                        })),
                    1,
                ),
            });
            syntax_trees.push_root_item(data);

            let snapshot = syntax_trees.snapshot();

            assert_eq!(
                snapshot,
                SyntaxTreesSnapshot {
                    source_id: 0,
                    root_items: vec![ItemSnapshot::Data {
                        name: super::IdentifierSnapshot {
                            text: "Example".to_owned(),
                            source_id: 0,
                            start: 0,
                            end: 0,
                            source_backed: false,
                        },
                        is_public: true,
                        supply: "checked_shape",
                        lifetime_parameters: Vec::new(),
                        type_parameters: Vec::new(),
                        conformance_bounds: Vec::new(),
                        generic_instance: None,
                        properties: super::DataPropertiesSnapshot {
                            multiplicity: "affine",
                            carry: None,
                        },
                        quotient: None,
                        where_facts: Vec::new(),
                        members: vec![super::DataMemberSnapshot::Field {
                            identity: None,
                            name: super::IdentifierSnapshot {
                                text: "field".to_owned(),
                                source_id: 0,
                                start: 0,
                                end: 0,
                                source_backed: false,
                            },
                            relevance: "relevant",
                            type_reference: TypeReferenceSnapshot::Named {
                                name: super::IdentifierSnapshot {
                                    text: "i32".to_owned(),
                                    source_id: 0,
                                    start: 0,
                                    end: 0,
                                    source_backed: false,
                                },
                            },
                        }],
                    }],
                }
            );

            let json = syntax_trees
                .snapshot_json_pretty()
                .expect("snapshot json should serialize");
            assert!(json.contains("\"root_items\""));
        }

        #[test]
        fn variant_snapshot_retains_payload_only_erased_field() {
            let mut syntax_trees = SyntaxTrees::new(Default::default());
            let evidence_type = syntax_trees
                .type_references
                .insert(TypeReferenceNode::Named(Identifier::generated("Evidence")));
            let payload = syntax_trees.items.append_data_payload_field(DataField {
                identity: Some(7),
                name: Identifier::generated("proof"),
                relevance: language_core::BindingRelevance::Erased,
                type_reference: evidence_type,
            });
            let member = syntax_trees
                .items
                .append_data_member(DataMember::Variant(DataVariant {
                    identity: Some(3),
                    name: Identifier::generated("Certified"),
                    payload: arena::HandleSpan::from_parts(payload, 1),
                    where_facts: arena::HandleSpan::empty(),
                    retired_payload_identities: Vec::new(),
                }));
            syntax_trees.push_root_item(Item::Data(DataDefinition {
                cleanup_selection: None,
                name: Identifier::generated("Envelope"),
                is_public: false,
                supply_mode: language_core::DataSupplyMode::CheckedShape,
                lifetime_parameters: Vec::new(),
                type_parameters: arena::HandleSpan::empty(),
                conformance_bounds: Vec::new(),
                generic_instance: None,
                properties: crate::item::DataProperties::default(),
                quotient: None,
                where_facts: arena::HandleSpan::empty(),
                members: arena::HandleSpan::from_parts(member, 1),
            }));

            let snapshot = syntax_trees.snapshot();
            let ItemSnapshot::Data { members, .. } = &snapshot.root_items[0] else {
                panic!("data snapshot");
            };
            let super::DataMemberSnapshot::Variant { payload, .. } = &members[0] else {
                panic!("variant snapshot");
            };
            assert_eq!(payload.len(), 1);
            assert_eq!(payload[0].name.text, "proof");
            assert_eq!(payload[0].relevance, "erased");
            assert_eq!(payload[0].identity, Some(7));
            assert!(matches!(
                &payload[0].type_reference,
                TypeReferenceSnapshot::Named { name } if name.text == "Evidence"
            ));
        }

        #[test]
        fn numbered_data_snapshot_retains_erased_field_relevance() {
            let mut syntax_trees = SyntaxTrees::new(Default::default());
            let evidence_type = syntax_trees
                .type_references
                .insert(TypeReferenceNode::Named(Identifier::generated("Evidence")));
            let field = syntax_trees
                .items
                .append_data_member(DataMember::Field(DataField {
                    identity: Some(7),
                    name: Identifier::generated("proof"),
                    relevance: language_core::BindingRelevance::Erased,
                    type_reference: evidence_type,
                }));
            syntax_trees.push_root_item(Item::Data(DataDefinition {
                cleanup_selection: None,
                is_public: false,
                name: Identifier::generated("Certified"),
                members: arena::HandleSpan::from_parts(field, 1),
                supply_mode: language_core::DataSupplyMode::CheckedShape,
                lifetime_parameters: Vec::new(),
                type_parameters: arena::HandleSpan::empty(),
                conformance_bounds: Vec::new(),
                generic_instance: None,
                properties: Default::default(),
                quotient: None,
                where_facts: arena::HandleSpan::empty(),
            }));

            let snapshot = syntax_trees.snapshot();
            let ItemSnapshot::Data { members, .. } = &snapshot.root_items[0] else {
                panic!("ordinary data snapshot");
            };
            let super::DataMemberSnapshot::Field {
                identity,
                name,
                relevance,
                type_reference,
            } = &members[0]
            else {
                panic!("numbered field snapshot");
            };
            assert_eq!(*identity, Some(7));
            assert_eq!(name.text, "proof");
            assert_eq!(*relevance, "erased");
            assert!(matches!(
                type_reference,
                TypeReferenceSnapshot::Named { name } if name.text == "Evidence"
            ));
        }
    }

    pub use expression_and_type_snapshots::{
        ExpressionSnapshot, FixedArrayLengthSnapshot, IdentifierSnapshot, MatchArmSnapshot,
        MatchPatternSnapshot, StaticArgumentSnapshot, StructLiteralFieldSnapshot,
        TypeConstraintSnapshot, TypeReferenceSnapshot,
    };
    pub use item_snapshots::{
        CapabilityContractKindSnapshot, CapabilityContractSnapshot, CapabilityMemberSnapshot,
        CarryPolicySnapshot, ConformanceMemberSnapshot, DataMemberSnapshot,
        DataPayloadFieldSnapshot, DataPropertiesSnapshot, ExternalBindingSnapshot,
        GenericConformanceBoundSnapshot, ItemSnapshot, OperatorSnapshot, ProofFactSnapshot,
        PropositionBodySnapshot, PropositionParameterSignatureSnapshot,
        QuotientEquivalenceSelectionSnapshot, QuotientSnapshot, SatisfiesClauseSnapshot,
        TypeParameterSnapshot,
    };
    pub use state_and_statement_snapshots::{
        AssemblyFactKindSnapshot, NativeCallbackParameterSnapshot, StateParameterSnapshot,
        StateSignatureSnapshot, StateSnapshot, StatementSnapshot, TransitionGuardSnapshot,
        TransitionTargetSnapshot,
    };

    use crate::SyntaxTrees;
    use crate::inspection::snapshot::item_snapshots::snapshot_item;
    use diagnostics::PhaseSnapshot;
    use serde::Serialize;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
    pub struct SyntaxTreesSnapshot {
        pub source_id: usize,
        pub root_items: Vec<ItemSnapshot>,
    }

    impl SyntaxTreesSnapshot {
        pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
            serde_json::to_string_pretty(self)
        }

        pub fn to_json(&self) -> Result<String, serde_json::Error> {
            serde_json::to_string(self)
        }
    }

    impl SyntaxTrees {
        pub fn snapshot(&self) -> SyntaxTreesSnapshot {
            SyntaxTreesSnapshot {
                source_id: self.source_id.0,
                root_items: self
                    .root_item_handles()
                    .iter()
                    .map(|handle| snapshot_item(self, self.items.item(*handle)))
                    .collect(),
            }
        }

        pub fn snapshot_json_pretty(&self) -> Result<String, serde_json::Error> {
            self.snapshot().to_json_pretty()
        }

        pub fn snapshot_json(&self) -> Result<String, serde_json::Error> {
            self.snapshot().to_json()
        }
    }

    impl PhaseSnapshot for SyntaxTrees {
        type Snapshot = SyntaxTreesSnapshot;

        fn snapshot(&self) -> Self::Snapshot {
            SyntaxTrees::snapshot(self)
        }
    }
}
