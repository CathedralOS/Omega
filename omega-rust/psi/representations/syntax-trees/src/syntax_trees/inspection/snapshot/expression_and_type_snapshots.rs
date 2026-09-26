//! Snapshots of expressions, match arms, static arguments, identifiers,
//! type references and constraints.

use crate::expression::{
    BinaryOperator, ExpressionNode, TableCallExpression, TableStructLiteralField,
};
use crate::identifier::Identifier;
use crate::syntax_trees::SyntaxTrees;
use crate::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceNode};
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
        name: IdentifierSnapshot,
        #[serde(skip_serializing_if = "Option::is_none")]
        conformance: Option<IdentifierSnapshot>,
    },
    Named {
        name: IdentifierSnapshot,
    },
    SelfType,
    Unit,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FixedArrayLengthSnapshot {
    Literal { value: usize },
    ConstParameter { name: IdentifierSnapshot },
    ConstCall { name: IdentifierSnapshot },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeConstraintSnapshot {
    Named {
        name: IdentifierSnapshot,
    },
    Range {
        minimum: ExpressionSnapshot,
        maximum: ExpressionSnapshot,
        end_inclusive: bool,
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
    Value { value: Box<ExpressionSnapshot> },
    Wildcard,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExpressionSnapshot {
    Match {
        subject: Box<ExpressionSnapshot>,
        arms: Vec<MatchArmSnapshot>,
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
    Float {
        text: String,
    },
    Indexed {
        collection: Box<ExpressionSnapshot>,
        index: Box<ExpressionSnapshot>,
    },
    Integer {
        /// Canonical literal spelling (see `numerics::literals::IntegerLiteral`) --
        /// snapshots stay anonymous like the nodes they mirror (D14).
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
    SelfValue,
    StructLiteral {
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

fn reference_access_name(access: language_core::ReferenceAccess) -> &'static str {
    match access {
        language_core::ReferenceAccess::Shared => "shared",
        language_core::ReferenceAccess::Mutable => "mutable",
        language_core::ReferenceAccess::WriteOnly => "write_only",
    }
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
            access: reference_access_name(*access),
            lifetime: lifetime.as_ref().map(snapshot_identifier),
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
            element_type: Box::new(snapshot_type_reference_handle(syntax_trees, *element_type)),
            length: snapshot_fixed_array_length(length),
        },
        TypeReferenceNode::Slice { element_type } => TypeReferenceSnapshot::Slice {
            element_type: Box::new(snapshot_type_reference_handle(syntax_trees, *element_type)),
        },
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } => TypeReferenceSnapshot::Generic {
            base_name: snapshot_identifier(base_name),
            lifetime_arguments: lifetime_arguments.iter().map(snapshot_identifier).collect(),
            arguments: syntax_trees
                .type_references
                .type_reference_handles(*arguments)
                .iter()
                .map(|handle| snapshot_type_reference_handle(syntax_trees, *handle))
                .collect(),
        },
        TypeReferenceNode::ConstExpression(expression) => TypeReferenceSnapshot::ConstExpression {
            expression: snapshot_expression_handle(syntax_trees, *expression),
        },
        TypeReferenceNode::DynamicTrait { name, conformance } => {
            TypeReferenceSnapshot::DynamicTrait {
                name: snapshot_identifier(name),
                conformance: conformance.as_ref().map(snapshot_identifier),
            }
        }
        TypeReferenceNode::Named(name) => TypeReferenceSnapshot::Named {
            name: snapshot_identifier(name),
        },
        TypeReferenceNode::SelfType => TypeReferenceSnapshot::SelfType,
        TypeReferenceNode::Unit => TypeReferenceSnapshot::Unit,
    }
}

fn snapshot_fixed_array_length(length: &FixedArrayLength) -> FixedArrayLengthSnapshot {
    match length {
        FixedArrayLength::Literal(value) => FixedArrayLengthSnapshot::Literal { value: *value },
        FixedArrayLength::ConstParameter(name) => FixedArrayLengthSnapshot::ConstParameter {
            name: snapshot_identifier(name),
        },
        FixedArrayLength::ConstCall(name) => FixedArrayLengthSnapshot::ConstCall {
            name: snapshot_identifier(name),
        },
    }
}

fn snapshot_type_constraint(
    syntax_trees: &SyntaxTrees,
    constraint: &TypeConstraintNode,
) -> TypeConstraintSnapshot {
    match constraint {
        TypeConstraintNode::Named(name) => TypeConstraintSnapshot::Named {
            name: snapshot_identifier(name),
        },
        TypeConstraintNode::Domain(domain) => TypeConstraintSnapshot::Domain {
            name: snapshot_identifier(&domain.name),
            arguments: syntax_trees
                .type_references
                .type_reference_handles(domain.arguments)
                .iter()
                .map(|argument| snapshot_type_reference_handle(syntax_trees, *argument))
                .collect(),
        },
        TypeConstraintNode::Range {
            minimum,
            maximum,
            end_inclusive,
        } => TypeConstraintSnapshot::Range {
            minimum: snapshot_expression_handle(syntax_trees, *minimum),
            maximum: snapshot_expression_handle(syntax_trees, *maximum),
            end_inclusive: *end_inclusive,
        },
        TypeConstraintNode::ArithmeticDomain(domain) => TypeConstraintSnapshot::ArithmeticDomain {
            domain: domain.name().to_owned(),
        },
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
            arms: syntax_trees
                .expressions
                .match_arms(dispatch.arms)
                .iter()
                .map(|arm| MatchArmSnapshot {
                    source_id: arm.source_span.source_id.0,
                    source_start: arm.source_span.span.start,
                    source_end: arm.source_span.span.end,
                    pattern: match arm.pattern {
                        crate::expression::MatchPattern::Value(value) => {
                            MatchPatternSnapshot::Value {
                                value: Box::new(snapshot_expression_handle(syntax_trees, value)),
                            }
                        }
                        crate::expression::MatchPattern::Wildcard => MatchPatternSnapshot::Wildcard,
                    },
                    value: snapshot_expression_handle(syntax_trees, arm.value),
                })
                .collect(),
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
                language_core::cast_form::CastForm::RecastShared => "recast_shared",
                language_core::cast_form::CastForm::RecastMutable => "recast_mutable",
            },
            semantic_domain: snapshot_identifier_slice(
                syntax_trees
                    .expressions
                    .identifier_path_members(cast.semantic_domain),
            ),
            semantic_domain_arguments: syntax_trees
                .type_references
                .type_reference_handles(cast.semantic_domain_arguments)
                .iter()
                .map(|argument| snapshot_type_reference_handle(syntax_trees, *argument))
                .collect(),
        },
        ExpressionNode::Call(call) => snapshot_call_expression(syntax_trees, call),
        ExpressionNode::Float(value) => ExpressionSnapshot::Float {
            text: value.as_str().to_owned(),
        },
        ExpressionNode::Indexed(indexed) => ExpressionSnapshot::Indexed {
            collection: Box::new(snapshot_expression_handle(syntax_trees, indexed.collection)),
            index: Box::new(snapshot_expression_handle(syntax_trees, indexed.index)),
        },
        ExpressionNode::Integer(value) => ExpressionSnapshot::Integer {
            text: value.text().to_owned(),
        },
        ExpressionNode::Membership(membership) => ExpressionSnapshot::Membership {
            value: Box::new(snapshot_expression_handle(syntax_trees, membership.value)),
            domain: snapshot_identifier_slice(
                syntax_trees
                    .expressions
                    .identifier_path_members(membership.domain),
            ),
        },
        ExpressionNode::Member(member) => ExpressionSnapshot::Member {
            receiver: Box::new(snapshot_expression_handle(syntax_trees, member.receiver)),
            member: snapshot_identifier(&member.member),
            case_variant: member.case_variant.as_ref().map(snapshot_identifier),
        },
        ExpressionNode::Borrow(value) => ExpressionSnapshot::Borrow {
            access: reference_access_name(value.access),
            value: Box::new(snapshot_expression_handle(syntax_trees, value.target)),
        },
        ExpressionNode::Name(path) => ExpressionSnapshot::Name {
            path: snapshot_identifier_slice(
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
        ExpressionNode::SelfValue => ExpressionSnapshot::SelfValue,
        ExpressionNode::StructLiteral(value) => ExpressionSnapshot::StructLiteral {
            constructor_name: snapshot_identifier(&value.constructor_name),
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
        ExpressionNode::TypeExpression(type_reference) => ExpressionSnapshot::TypeExpression {
            type_reference: Box::new(snapshot_type_reference_handle(
                syntax_trees,
                *type_reference,
            )),
        },
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
        target: snapshot_identifier(&call.target),
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
        evidence_arguments: snapshot_identifier_slice(&call.evidence_arguments),
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
        StaticArgumentSnapshot::Const(literal.text().to_owned())
    } else if let Some(projection) = &argument.evidence_projection {
        StaticArgumentSnapshot::EvidenceProjection {
            term: snapshot_identifier(&projection.term),
            member: snapshot_identifier(&projection.member),
        }
    } else if let Some(application) = &argument.application {
        StaticArgumentSnapshot::Application {
            path: snapshot_identifier_slice(&argument.path),
            lifetime_arguments: snapshot_identifier_slice(&application.lifetime_arguments),
            arguments: application
                .arguments
                .iter()
                .map(|argument| snapshot_static_argument(syntax_trees, argument))
                .collect(),
        }
    } else {
        StaticArgumentSnapshot::Path(snapshot_identifier_slice(&argument.path))
    }
}

fn snapshot_struct_field(
    syntax_trees: &SyntaxTrees,
    field: &TableStructLiteralField,
) -> StructLiteralFieldSnapshot {
    StructLiteralFieldSnapshot {
        name: snapshot_identifier(&field.name),
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

pub(crate) fn snapshot_identifier(identifier: &Identifier) -> IdentifierSnapshot {
    let source_span = identifier.source_span();
    IdentifierSnapshot {
        text: identifier.as_str().to_owned(),
        source_id: source_span.source_id.0,
        start: source_span.span.start,
        end: source_span.span.end,
        source_backed: identifier.is_source_backed(),
    }
}

pub(crate) fn snapshot_identifier_slice(path: &[Identifier]) -> Vec<IdentifierSnapshot> {
    path.iter().map(snapshot_identifier).collect()
}
