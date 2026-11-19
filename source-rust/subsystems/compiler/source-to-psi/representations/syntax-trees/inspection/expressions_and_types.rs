use crate::SyntaxTrees;
use crate::expression::{
    BinaryOperator, ExpressionNode, MatchFieldAction, MatchPattern, TableCallExpression,
    TableStructLiteralField,
};
use crate::identifier::Identifier;
use crate::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
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
    StaticSelection {
        selection: Box<StaticArgumentSnapshot>,
    },
    Callable {
        signature: Box<crate::snapshot::state_and_statement_snapshots::StateSignatureSnapshot>,
    },
    ConstExpression {
        expression: ExpressionSnapshot,
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
        callee: Option<Box<ExpressionSnapshot>>,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        application: Option<Box<StaticArgumentSnapshot>>,
    },
    Range {
        start: Option<Box<ExpressionSnapshot>>,
        end: Option<Box<ExpressionSnapshot>>,
        end_inclusive: bool,
    },
    StructLiteral {
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        zero_omitted_fields: bool,
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
            element_type: Box::new(snapshot_type_reference_handle(syntax_trees, *element_type)),
            length: snapshot_fixed_array_length(syntax_trees, length),
        },
        TypeReferenceNode::Slice { element_type } => TypeReferenceSnapshot::Slice {
            element_type: Box::new(snapshot_type_reference_handle(syntax_trees, *element_type)),
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
        TypeReferenceNode::StaticSelection(selection) => TypeReferenceSnapshot::StaticSelection {
            selection: Box::new(snapshot_static_argument(syntax_trees, selection)),
        },
        TypeReferenceNode::Callable { signature } => TypeReferenceSnapshot::Callable {
            signature: Box::new(
                crate::snapshot::state_and_statement_snapshots::snapshot_state_signature(
                    syntax_trees,
                    syntax_trees.items.state_signature(*signature),
                ),
            ),
        },
        TypeReferenceNode::ConstExpression(expression) => TypeReferenceSnapshot::ConstExpression {
            expression: snapshot_expression_handle(syntax_trees, *expression),
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
        FixedArrayLength::Literal(value) => FixedArrayLengthSnapshot::Literal { value: *value },
        FixedArrayLength::ConstParameter(name) => FixedArrayLengthSnapshot::ConstParameter {
            name: snapshot_identifier(syntax_trees, name),
        },
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
        return ExpressionSnapshot::Name {
            path: Vec::new(),
            application: None,
        };
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
            component: (saved.component == language_core::PatternComponent::Case).then_some("case"),
            capture: (saved.capture.arena_index(), saved.capture.generation()),
            pattern_head: Box::new(snapshot_expression_handle(syntax_trees, saved.pattern_head)),
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
            operator: snapshot_policy_operator(binary.operator, binary.policy)
                .unwrap_or_else(|| snapshot_binary_operator(binary.operator)),
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
                language_core::cast_form::CastForm::Entry(entry) => match entry.kind {
                    language_core::cast_form::CastEntryKind::Keep => "entry_keep",
                    language_core::cast_form::CastEntryKind::Remove => "entry_remove",
                    language_core::cast_form::CastEntryKind::Add => "entry_add",
                },
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
            collection: Box::new(snapshot_expression_handle(syntax_trees, indexed.collection)),
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
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } => ExpressionSnapshot::Name {
            path: snapshot_identifier_slice(
                syntax_trees,
                syntax_trees
                    .expressions
                    .identifier_path_members(path.to_owned()),
            ),
            application: path_application.as_ref().map(|application| {
                Box::new(snapshot_static_argument(
                    syntax_trees,
                    &crate::expression::StaticMachineArgument {
                        path: syntax_trees
                            .expressions
                            .identifier_path_members(path.to_owned())
                            .to_vec()
                            .into_boxed_slice(),
                        application: Some(application.clone()),
                        ..Default::default()
                    },
                ))
            }),
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
            zero_omitted_fields: value.zero_omitted_fields,
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
        callee: call
            .callee
            .is_valid()
            .then(|| Box::new(snapshot_expression_handle(syntax_trees, call.callee))),
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
        evidence_arguments: snapshot_identifier_slice(syntax_trees, &call.evidence_arguments),
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
        StaticArgumentSnapshot::Path(snapshot_identifier_slice(syntax_trees, &argument.path))
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

/// A suffixed policy operator snapshots under its own name, so two
/// parsers that disagree about a suffix do not compare equal.
fn snapshot_policy_operator(
    operator: BinaryOperator,
    policy: numerics::arithmetic::ArithmeticDomain,
) -> Option<&'static str> {
    use numerics::arithmetic::ArithmeticDomain;
    Some(match (operator, policy) {
        (BinaryOperator::Add, ArithmeticDomain::Wrapping) => "add_wrapping",
        (BinaryOperator::Add, ArithmeticDomain::Saturating) => "add_saturating",
        (BinaryOperator::Add, ArithmeticDomain::Trapping) => "add_trapping",
        (BinaryOperator::Subtract, ArithmeticDomain::Wrapping) => "subtract_wrapping",
        (BinaryOperator::Subtract, ArithmeticDomain::Saturating) => "subtract_saturating",
        (BinaryOperator::Subtract, ArithmeticDomain::Trapping) => "subtract_trapping",
        (BinaryOperator::Multiply, ArithmeticDomain::Wrapping) => "multiply_wrapping",
        (BinaryOperator::Multiply, ArithmeticDomain::Saturating) => "multiply_saturating",
        (BinaryOperator::Multiply, ArithmeticDomain::Trapping) => "multiply_trapping",
        (BinaryOperator::Divide, ArithmeticDomain::Wrapping) => "divide_wrapping",
        (BinaryOperator::Divide, ArithmeticDomain::Saturating) => "divide_saturating",
        (BinaryOperator::Divide, ArithmeticDomain::Trapping) => "divide_trapping",
        (BinaryOperator::Modulo, ArithmeticDomain::Wrapping) => "modulo_wrapping",
        (BinaryOperator::Modulo, ArithmeticDomain::Saturating) => "modulo_saturating",
        (BinaryOperator::Modulo, ArithmeticDomain::Trapping) => "modulo_trapping",
        (BinaryOperator::ShiftLeft, ArithmeticDomain::Wrapping) => "shift_left_wrapping",
        (BinaryOperator::ShiftLeft, ArithmeticDomain::Saturating) => "shift_left_saturating",
        (BinaryOperator::ShiftLeft, ArithmeticDomain::Trapping) => "shift_left_trapping",
        (BinaryOperator::ShiftRight, ArithmeticDomain::Wrapping) => "shift_right_wrapping",
        (BinaryOperator::ShiftRight, ArithmeticDomain::Saturating) => "shift_right_saturating",
        (BinaryOperator::ShiftRight, ArithmeticDomain::Trapping) => "shift_right_trapping",
        _ => return None,
    })
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

/// One pattern's field roster, for a dispatch arm or a record
/// pattern's marker local alike.
pub(crate) fn snapshot_match_fields(
    syntax_trees: &SyntaxTrees,
    fields: arena::HandleSpan<crate::expression::TableMatchField>,
) -> Vec<MatchFieldSnapshot> {
    syntax_trees
        .expressions
        .match_fields(fields)
        .iter()
        .map(|field| MatchFieldSnapshot {
            component: (field.component == language_core::PatternComponent::Case).then_some("case"),
            member: snapshot_identifier(syntax_trees, &field.member),
            action: match &field.action {
                MatchFieldAction::Bind(binding) => MatchFieldActionSnapshot::Bind {
                    binding: snapshot_identifier(syntax_trees, binding),
                },
                MatchFieldAction::Equal(value) => MatchFieldActionSnapshot::Equal {
                    value: Box::new(snapshot_expression_handle(syntax_trees, *value)),
                },
                MatchFieldAction::Waive => MatchFieldActionSnapshot::Waive,
            },
        })
        .collect()
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
            fields: snapshot_match_fields(syntax_trees, pattern.fields),
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
