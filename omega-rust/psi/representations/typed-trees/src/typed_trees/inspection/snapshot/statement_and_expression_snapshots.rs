//! Snapshots of statements, transitions, match arms, expressions and static
//! arguments.

use crate::TypedTrees;
use crate::expression::{ExpressionHandle, ExpressionNode};
use crate::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use crate::typed_trees::inspection::snapshot::TypeReferenceSnapshot;
use crate::typed_trees::inspection::snapshot::type_snapshots::{
    path_snapshot, reference_access_name, type_reference_snapshot,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum StaticArgumentSnapshot {
    Type {
        type_reference: Box<TypeReferenceSnapshot>,
    },
    Path(Vec<String>),
    Application {
        path: Vec<String>,
        lifetime_arguments: Vec<String>,
        arguments: Vec<StaticArgumentSnapshot>,
    },
    Const(String),
    EvidenceProjection {
        term: String,
        member: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StatementSnapshot {
    RootBinding {
        receiver: ExpressionSnapshot,
        slot: Vec<String>,
        implementation: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        implementation_operand: Option<ExpressionSnapshot>,
    },
    AssemblyFact {
        contract_kind: &'static str,
        expression: ExpressionSnapshot,
    },
    Assignment {
        target: ExpressionSnapshot,
        value: ExpressionSnapshot,
    },
    Call {
        receiver: Option<Vec<String>>,
        target: String,
        machine_arguments: Vec<StaticArgumentSnapshot>,
        arguments: Vec<ExpressionSnapshot>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        evidence_arguments: Vec<String>,
        acknowledgement_synthesized: bool,
        acknowledges_suspend: bool,
        acknowledges_block: bool,
    },
    Expression {
        value: ExpressionSnapshot,
    },
    LocalData {
        name: String,
        type_reference: TypeReferenceSnapshot,
        initial_value: Option<ExpressionSnapshot>,
        type_is_inferred: bool,
        relevance: &'static str,
    },
    Transition {
        target: TransitionTargetSnapshot,
        continuation: Option<TransitionTargetSnapshot>,
        guard: TransitionGuardSnapshot,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        proof_selectors: Vec<(String, String)>,
        #[serde(skip_serializing_if = "Option::is_none")]
        crash_cause: Option<&'static str>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransitionGuardSnapshot {
    Always,
    When { value: ExpressionSnapshot },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransitionTargetSnapshot {
    Named {
        path: Vec<String>,
        arguments: Vec<ExpressionSnapshot>,
        evidence_arguments: Vec<String>,
    },
    Value {
        value: ExpressionSnapshot,
    },
    SelfTarget,
    Terminal,
    Invalid {
        handle: u32,
    },
    Unary {
        operator: String,
        operand: Box<ExpressionSnapshot>,
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
        operator: String,
        right: Box<ExpressionSnapshot>,
    },
    Boolean {
        value: bool,
    },
    Cast {
        value: Box<ExpressionSnapshot>,
        target_type: Box<TypeReferenceSnapshot>,
        result_type: Box<TypeReferenceSnapshot>,
        semantic_domain: Vec<String>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        semantic_domain_arguments: Vec<TypeReferenceSnapshot>,
        semantic_domain_symbol: u32,
        semantic_domain_id: u32,
    },
    Call {
        receiver: Option<Box<ExpressionSnapshot>>,
        target: String,
        machine_arguments: Vec<StaticArgumentSnapshot>,
        #[serde(skip_serializing_if = "Option::is_none")]
        private_layout_slot: Option<StaticArgumentSnapshot>,
        arguments: Vec<ExpressionSnapshot>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        evidence_arguments: Vec<String>,
        acknowledgement_synthesized: bool,
        acknowledges_suspend: bool,
        acknowledges_block: bool,
    },
    Float {
        value: String,
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
    Member {
        receiver: Box<ExpressionSnapshot>,
        member: String,
    },
    Borrow {
        access: &'static str,
        value: Box<ExpressionSnapshot>,
    },
    Name {
        path: Vec<String>,
    },
    Range {
        start: Option<Box<ExpressionSnapshot>>,
        end: Option<Box<ExpressionSnapshot>>,
        end_inclusive: bool,
    },
    StructLiteral {
        type_name: String,
        fields: Vec<StructLiteralFieldSnapshot>,
    },
    String {
        bytes: Vec<u8>,
    },
    Invalid {
        handle: u32,
    },
    Unary {
        operator: String,
        operand: Box<ExpressionSnapshot>,
    },
    ZeroValue {
        type_reference: Box<TypeReferenceSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StructLiteralFieldSnapshot {
    pub name: String,
    pub value: ExpressionSnapshot,
}

pub(crate) fn statement_snapshot(
    program: &TypedTrees,
    statement: &StatementNode,
) -> StatementSnapshot {
    match statement {
        StatementNode::RootBinding(binding) => StatementSnapshot::RootBinding {
            receiver: expression_snapshot(program, binding.receiver),
            slot: binding.slot.iter().map(ToString::to_string).collect(),
            implementation: binding
                .implementation
                .iter()
                .map(ToString::to_string)
                .collect(),
            implementation_operand: binding
                .implementation_operand
                .is_valid()
                .then(|| expression_snapshot(program, binding.implementation_operand)),
        },
        StatementNode::AssemblyFact(fact) => StatementSnapshot::AssemblyFact {
            contract_kind: match fact.kind {
                crate::statement::AssemblyFactKind::Requires => "requires",
                crate::statement::AssemblyFactKind::Ensures => "ensures",
            },
            expression: expression_snapshot(program, fact.expression),
        },
        StatementNode::Assignment(assignment) => StatementSnapshot::Assignment {
            target: expression_snapshot(program, assignment.target),
            value: expression_snapshot(program, assignment.value),
        },
        StatementNode::Call(call) => StatementSnapshot::Call {
            receiver: (!call.receiver.is_empty())
                .then(|| path_snapshot(program.statement_table.name_path_members(call.receiver))),
            target: call.target.to_string(),
            machine_arguments: call
                .machine_arguments
                .iter()
                .map(|argument| snapshot_static_argument(program, argument))
                .collect(),
            arguments: statement_expression_span_snapshot(program, call.arguments),
            evidence_arguments: call
                .evidence_arguments
                .iter()
                .map(ToString::to_string)
                .collect(),
            acknowledgement_synthesized: call.operational_acknowledgement.origin
                == language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
            acknowledges_suspend: call.operational_acknowledgement.acknowledges_suspend,
            acknowledges_block: call.operational_acknowledgement.acknowledges_block,
        },
        StatementNode::Expression(expression) => StatementSnapshot::Expression {
            value: expression_snapshot(program, *expression),
        },
        StatementNode::LocalData(local) => StatementSnapshot::LocalData {
            name: local.name.to_string(),
            type_reference: type_reference_snapshot(program, local.type_reference),
            initial_value: expression_snapshot_option(program, local.initial_value),
            type_is_inferred: local.type_is_inferred,
            relevance: super::declaration_snapshots::snapshot_binding_relevance(local.relevance),
        },
        StatementNode::Transition(transition) => StatementSnapshot::Transition {
            target: transition_target_snapshot(program, transition.target),
            continuation: transition
                .continuation
                .is_valid()
                .then(|| transition_target_snapshot(program, transition.continuation)),
            guard: transition_guard_snapshot(program, transition.guard),
            proof_selectors: program
                .statement_table
                .outcome_proof_selectors(transition.proof_selectors)
                .iter()
                .map(|selector| {
                    (
                        selector.output_field.to_string(),
                        selector.binding.to_string(),
                    )
                })
                .collect(),
            crash_cause: match transition.exit {
                crate::statement::TransitionExit::Ordinary => None,
                crate::statement::TransitionExit::Crash(crate::signature::CrashCause::Trap) => {
                    Some("Trap")
                }
                crate::statement::TransitionExit::Crash(crate::signature::CrashCause::Abort) => {
                    Some("Abort")
                }
            },
        },
    }
}

fn transition_guard_snapshot(
    program: &TypedTrees,
    guard: TransitionGuardNode,
) -> TransitionGuardSnapshot {
    match guard {
        TransitionGuardNode::Always => TransitionGuardSnapshot::Always,
        TransitionGuardNode::When(value) => TransitionGuardSnapshot::When {
            value: expression_snapshot(program, value),
        },
    }
}

fn transition_target_snapshot(
    program: &TypedTrees,
    target: crate::statement::TransitionTargetHandle,
) -> TransitionTargetSnapshot {
    if !target.is_valid() {
        return TransitionTargetSnapshot::Invalid {
            handle: target.arena_index(),
        };
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path,
            arguments,
            evidence_arguments,
            ..
        } => TransitionTargetSnapshot::Named {
            path: path_snapshot(program.statement_table.name_path_members(path.members)),
            arguments: statement_expression_span_snapshot(program, *arguments),
            evidence_arguments: evidence_arguments
                .iter()
                .map(|name| name.as_str().to_owned())
                .collect(),
        },
        TransitionTargetNode::Value(value) => TransitionTargetSnapshot::Value {
            value: expression_snapshot(program, *value),
        },
        TransitionTargetNode::SelfTarget => TransitionTargetSnapshot::SelfTarget,
        TransitionTargetNode::Terminal => TransitionTargetSnapshot::Terminal,
    }
}

pub(crate) fn expression_snapshot_option(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionSnapshot> {
    expression
        .is_valid()
        .then(|| expression_snapshot(program, expression))
}

pub(crate) fn expression_snapshot(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> ExpressionSnapshot {
    if !expression.is_valid() {
        return ExpressionSnapshot::Invalid {
            handle: expression.arena_index(),
        };
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => ExpressionSnapshot::Match {
            subject: Box::new(expression_snapshot(program, dispatch.subject)),
            arms: program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .map(|arm| MatchArmSnapshot {
                    source_id: arm.source_span.source_id.0,
                    source_start: arm.source_span.span.start,
                    source_end: arm.source_span.span.end,
                    pattern: match arm.pattern {
                        crate::expression::MatchPattern::Value(value) => {
                            MatchPatternSnapshot::Value {
                                value: Box::new(expression_snapshot(program, value)),
                            }
                        }
                        crate::expression::MatchPattern::Wildcard => MatchPatternSnapshot::Wildcard,
                    },
                    value: expression_snapshot(program, arm.value),
                })
                .collect(),
        },
        ExpressionNode::ArrayLiteral(values) => ExpressionSnapshot::ArrayLiteral {
            values: expression_span_snapshot(program, *values),
        },
        ExpressionNode::Binary(binary) => ExpressionSnapshot::Binary {
            left: Box::new(expression_snapshot(program, binary.left)),
            operator: binary.operator.display_name().to_owned(),
            right: Box::new(expression_snapshot(program, binary.right)),
        },
        ExpressionNode::Boolean(value) => ExpressionSnapshot::Boolean { value: *value },
        ExpressionNode::Cast(cast) => ExpressionSnapshot::Cast {
            value: Box::new(expression_snapshot(program, cast.value)),
            target_type: Box::new(type_reference_snapshot(program, cast.target_type)),
            result_type: Box::new(type_reference_snapshot(program, cast.result_type)),
            semantic_domain: path_snapshot(
                program
                    .expression_table
                    .name_path_members(cast.semantic_domain),
            ),
            semantic_domain_arguments: program
                .type_reference_table
                .type_reference_handles(cast.semantic_domain_arguments)
                .iter()
                .map(|argument| type_reference_snapshot(program, *argument))
                .collect(),
            semantic_domain_symbol: cast.semantic_domain_symbol.arena_index(),
            semantic_domain_id: cast.semantic_domain_id.0,
        },
        ExpressionNode::Call(call) => ExpressionSnapshot::Call {
            receiver: call
                .receiver
                .is_valid()
                .then(|| Box::new(expression_snapshot(program, call.receiver))),
            target: call.target.to_string(),
            machine_arguments: call
                .machine_arguments
                .iter()
                .map(|argument| snapshot_static_argument(program, argument))
                .collect(),
            private_layout_slot: call
                .private_layout_operation
                .as_ref()
                .map(|operation| snapshot_static_argument(program, &operation.selected_slot)),
            arguments: expression_span_snapshot(program, call.arguments),
            evidence_arguments: call
                .evidence_arguments
                .iter()
                .map(ToString::to_string)
                .collect(),
            acknowledgement_synthesized: call.operational_acknowledgement.origin
                == language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
            acknowledges_suspend: call.operational_acknowledgement.acknowledges_suspend,
            acknowledges_block: call.operational_acknowledgement.acknowledges_block,
        },
        ExpressionNode::Float(value) => ExpressionSnapshot::Float {
            value: value.to_string(),
        },
        ExpressionNode::Indexed(indexed) => ExpressionSnapshot::Indexed {
            collection: Box::new(expression_snapshot(program, indexed.collection)),
            index: Box::new(expression_snapshot(program, indexed.index)),
        },
        ExpressionNode::Integer(value) => ExpressionSnapshot::Integer {
            text: value.text().to_owned(),
        },
        ExpressionNode::Member(member) => ExpressionSnapshot::Member {
            receiver: Box::new(expression_snapshot(program, member.receiver)),
            member: member.member.to_string(),
        },
        ExpressionNode::Borrow(value) => ExpressionSnapshot::Borrow {
            access: reference_access_name(value.access),
            value: Box::new(expression_snapshot(program, value.target)),
        },
        ExpressionNode::Name(path) => ExpressionSnapshot::Name {
            path: path_snapshot(program.expression_table.name_path_members(path.members)),
        },
        ExpressionNode::Range(range) => ExpressionSnapshot::Range {
            start: range
                .start
                .is_valid()
                .then(|| Box::new(expression_snapshot(program, range.start))),
            end: range
                .end
                .is_valid()
                .then(|| Box::new(expression_snapshot(program, range.end))),
            end_inclusive: range.end_inclusive,
        },
        ExpressionNode::StructLiteral(struct_literal) => ExpressionSnapshot::StructLiteral {
            type_name: struct_literal.type_name.to_string(),
            fields: program
                .expression_table
                .struct_fields(struct_literal.fields)
                .iter()
                .map(|field| StructLiteralFieldSnapshot {
                    name: field.name.to_string(),
                    value: expression_snapshot(program, field.value),
                })
                .collect(),
        },
        ExpressionNode::Atomic(atomic) => ExpressionSnapshot::Atomic {
            value: Box::new(expression_snapshot(program, atomic.value)),
            result: atomic
                .result
                .is_valid()
                .then(|| Box::new(expression_snapshot(program, atomic.result))),
            ordering: format!("{:?}", atomic.ordering),
        },
        ExpressionNode::String(value) => ExpressionSnapshot::String {
            bytes: value.to_vec(),
        },
        ExpressionNode::Unary(unary) => ExpressionSnapshot::Unary {
            operator: unary.operator.display_name().to_owned(),
            operand: Box::new(expression_snapshot(program, unary.operand)),
        },
        ExpressionNode::ZeroValue(type_reference) => ExpressionSnapshot::ZeroValue {
            type_reference: Box::new(type_reference_snapshot(program, *type_reference)),
        },
    }
}

pub(crate) fn snapshot_static_argument(
    program: &TypedTrees,
    argument: &crate::expression::StaticMachineArgument,
) -> StaticArgumentSnapshot {
    if argument.type_reference.is_valid() {
        StaticArgumentSnapshot::Type {
            type_reference: Box::new(type_reference_snapshot(program, argument.type_reference)),
        }
    } else if let Some(literal) = &argument.const_literal {
        StaticArgumentSnapshot::Const(literal.text().to_owned())
    } else if let Some(projection) = &argument.evidence_projection {
        StaticArgumentSnapshot::EvidenceProjection {
            term: projection.term.as_str().to_owned(),
            member: projection.member.as_str().to_owned(),
        }
    } else if let Some(application) = &argument.application {
        StaticArgumentSnapshot::Application {
            path: path_snapshot(&argument.path),
            lifetime_arguments: path_snapshot(&application.lifetime_arguments),
            arguments: application
                .arguments
                .iter()
                .map(|argument| snapshot_static_argument(program, argument))
                .collect(),
        }
    } else {
        StaticArgumentSnapshot::Path(path_snapshot(&argument.path))
    }
}

fn expression_span_snapshot(
    program: &TypedTrees,
    expressions: arena::HandleSpan<ExpressionHandle>,
) -> Vec<ExpressionSnapshot> {
    program
        .expression_table
        .expression_handles(expressions)
        .iter()
        .map(|expression| expression_snapshot(program, *expression))
        .collect()
}

fn statement_expression_span_snapshot(
    program: &TypedTrees,
    expressions: arena::HandleSpan<ExpressionHandle>,
) -> Vec<ExpressionSnapshot> {
    program
        .statement_table
        .expression_handles(expressions)
        .iter()
        .map(|expression| expression_snapshot(program, *expression))
        .collect()
}
