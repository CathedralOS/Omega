//! Snapshots of statements, transitions, match arms, expressions and static
//! arguments.

use crate::SymbolResolvedTrees;
use crate::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use crate::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
use crate::symbol_resolved_trees::inspection::snapshot::TypeReferenceSnapshot;
use crate::symbol_resolved_trees::inspection::snapshot::type_snapshots::{
    reference_access_name, type_reference_snapshot, type_reference_snapshot_from_program,
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
    ProofOutputBindingStatement {
        bindings: Vec<(String, String)>,
        call: ExpressionSnapshot,
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
        semantic_domain: Vec<String>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        semantic_domain_arguments: Vec<TypeReferenceSnapshot>,
    },
    Call {
        receiver: Option<Box<ExpressionSnapshot>>,
        target: String,
        machine_arguments: Vec<StaticArgumentSnapshot>,
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
    Membership {
        value: Box<ExpressionSnapshot>,
        domain: Vec<String>,
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
    Unary {
        operator: &'static str,
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
    program: &SymbolResolvedTrees,
    statement: &Statement,
) -> StatementSnapshot {
    match statement {
        Statement::RootBinding(binding) => StatementSnapshot::RootBinding {
            receiver: statement_expression_snapshot(program, binding.receiver),
            slot: binding.slot.iter().map(ToString::to_string).collect(),
            implementation: binding
                .implementation
                .iter()
                .map(ToString::to_string)
                .collect(),
            implementation_operand: binding
                .implementation_operand
                .is_valid()
                .then(|| statement_expression_snapshot(program, binding.implementation_operand)),
        },
        Statement::AssemblyFact(fact) => StatementSnapshot::AssemblyFact {
            contract_kind: match fact.kind {
                crate::statement::AssemblyFactKind::Requires => "requires",
                crate::statement::AssemblyFactKind::Ensures => "ensures",
            },
            expression: statement_expression_snapshot(program, fact.expression),
        },
        Statement::Assignment(assignment) => StatementSnapshot::Assignment {
            target: statement_expression_snapshot(program, assignment.target),
            value: statement_expression_snapshot(program, assignment.value),
        },
        Statement::Call(call) => StatementSnapshot::Call {
            receiver: (!call.receiver.is_empty()).then(|| {
                diagnostic_name_span_snapshot(
                    program
                        .tables
                        .declarations
                        .statement_path_members
                        .span_or_empty(call.receiver),
                )
            }),
            target: call.target.to_string(),
            machine_arguments: call
                .machine_arguments
                .iter()
                .map(|argument| snapshot_static_argument(program, argument))
                .collect(),
            arguments: program
                .tables
                .bodies
                .expressions
                .expression_handles(call.arguments)
                .iter()
                .map(|expression| table_expression_snapshot(program, *expression))
                .collect(),
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
        Statement::ProofOutputBindingStatement(package) => {
            StatementSnapshot::ProofOutputBindingStatement {
                bindings: package
                    .bindings
                    .iter()
                    .map(|binding| {
                        (
                            binding.output_field.to_string(),
                            binding.binding.to_string(),
                        )
                    })
                    .collect(),
                call: statement_expression_snapshot(program, package.call),
            }
        }
        Statement::Expression(expression) => StatementSnapshot::Expression {
            value: statement_expression_snapshot(program, *expression),
        },
        Statement::LocalData(local_data) => StatementSnapshot::LocalData {
            name: local_data.name.to_string(),
            type_reference: type_reference_snapshot(program, &local_data.type_reference),
            type_is_inferred: local_data.type_is_inferred,
            relevance: super::declaration_snapshots::snapshot_binding_relevance(
                local_data.relevance,
            ),
            initial_value: local_data
                .initial_value
                .is_valid()
                .then(|| statement_expression_snapshot(program, local_data.initial_value)),
        },
        Statement::Transition(transition) => transition_snapshot(program, transition),
    }
}

fn transition_snapshot(
    program: &SymbolResolvedTrees,
    transition: &Transition,
) -> StatementSnapshot {
    StatementSnapshot::Transition {
        target: transition_target_snapshot(program, &transition.target),
        continuation: transition
            .continuation
            .as_ref()
            .map(|target| transition_target_snapshot(program, target)),
        guard: match &transition.guard {
            TransitionGuard::Always => TransitionGuardSnapshot::Always,
            TransitionGuard::When(expression) => TransitionGuardSnapshot::When {
                value: statement_expression_snapshot(program, *expression),
            },
        },
        proof_selectors: transition
            .proof_selectors
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
    }
}

fn transition_target_snapshot(
    program: &SymbolResolvedTrees,
    target: &TransitionTarget,
) -> TransitionTargetSnapshot {
    match target {
        TransitionTarget::Named(named) => TransitionTargetSnapshot::Named {
            path: diagnostic_name_span_snapshot(
                program
                    .tables
                    .declarations
                    .statement_path_members
                    .span_or_empty(named.path),
            ),
            arguments: program
                .tables
                .bodies
                .expressions
                .expression_handles(named.arguments)
                .iter()
                .map(|expression| table_expression_snapshot(program, *expression))
                .collect(),
            evidence_arguments: named
                .evidence_arguments
                .iter()
                .map(|name| name.as_str().to_owned())
                .collect(),
        },
        TransitionTarget::Value(expression) => TransitionTargetSnapshot::Value {
            value: statement_expression_snapshot(program, *expression),
        },
        TransitionTarget::SelfTarget => TransitionTargetSnapshot::SelfTarget,
        TransitionTarget::Terminal => TransitionTargetSnapshot::Terminal,
    }
}

fn statement_expression_snapshot(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
) -> ExpressionSnapshot {
    table_expression_snapshot(program, expression)
}

pub(crate) fn table_expression_snapshot(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
) -> ExpressionSnapshot {
    let table = &program.tables.bodies.expressions;

    match table.expression(expression) {
        ExpressionNode::Match(dispatch) => ExpressionSnapshot::Match {
            subject: Box::new(table_expression_snapshot(program, dispatch.subject)),
            arms: table
                .match_arms(dispatch.arms)
                .iter()
                .map(|arm| MatchArmSnapshot {
                    source_id: arm.source_span.source_id.0,
                    source_start: arm.source_span.span.start,
                    source_end: arm.source_span.span.end,
                    pattern: match arm.pattern {
                        crate::expression::MatchPattern::Value(value) => {
                            MatchPatternSnapshot::Value {
                                value: Box::new(table_expression_snapshot(program, value)),
                            }
                        }
                        crate::expression::MatchPattern::Wildcard => MatchPatternSnapshot::Wildcard,
                    },
                    value: table_expression_snapshot(program, arm.value),
                })
                .collect(),
        },
        ExpressionNode::ArrayLiteral(values) => ExpressionSnapshot::ArrayLiteral {
            values: table
                .expression_handles(*values)
                .iter()
                .map(|value| table_expression_snapshot(program, *value))
                .collect(),
        },
        ExpressionNode::Atomic(atomic) => ExpressionSnapshot::Atomic {
            value: Box::new(table_expression_snapshot(program, atomic.value)),
            result: atomic
                .result
                .is_valid()
                .then(|| Box::new(table_expression_snapshot(program, atomic.result))),
            ordering: format!("{:?}", atomic.ordering),
        },
        ExpressionNode::Binary(binary) => ExpressionSnapshot::Binary {
            left: Box::new(table_expression_snapshot(program, binary.left)),
            operator: binary_operator_name(binary.operator),
            right: Box::new(table_expression_snapshot(program, binary.right)),
        },
        ExpressionNode::Boolean(value) => ExpressionSnapshot::Boolean { value: *value },
        ExpressionNode::Cast(cast) => ExpressionSnapshot::Cast {
            value: Box::new(table_expression_snapshot(program, cast.value)),
            target_type: Box::new(type_reference_snapshot_from_program(
                program,
                program.child_type_reference(cast.target_type),
            )),
            semantic_domain: diagnostic_name_span_snapshot(
                table.name_path_members(cast.semantic_domain),
            ),
            semantic_domain_arguments: program
                .child_type_references(cast.semantic_domain_arguments)
                .iter()
                .map(|argument| type_reference_snapshot_from_program(program, argument))
                .collect(),
        },
        ExpressionNode::Call(call) => ExpressionSnapshot::Call {
            receiver: call
                .receiver
                .is_valid()
                .then(|| Box::new(table_expression_snapshot(program, call.receiver))),
            target: call.target.to_string(),
            machine_arguments: call
                .machine_arguments
                .iter()
                .map(|argument| snapshot_static_argument(program, argument))
                .collect(),
            arguments: table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| table_expression_snapshot(program, *argument))
                .collect(),
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
            collection: Box::new(table_expression_snapshot(program, indexed.collection)),
            index: Box::new(table_expression_snapshot(program, indexed.index)),
        },
        ExpressionNode::Integer(value) => ExpressionSnapshot::Integer {
            text: value.text().to_owned(),
        },
        ExpressionNode::Membership(membership) => ExpressionSnapshot::Membership {
            value: Box::new(table_expression_snapshot(program, membership.value)),
            domain: table
                .name_path_members(membership.domain)
                .iter()
                .map(ToString::to_string)
                .collect(),
        },
        ExpressionNode::Member(member) => ExpressionSnapshot::Member {
            receiver: Box::new(table_expression_snapshot(program, member.receiver)),
            member: member.member.to_string(),
        },
        ExpressionNode::Borrow(value) => ExpressionSnapshot::Borrow {
            access: reference_access_name(value.access),
            value: Box::new(table_expression_snapshot(program, value.target)),
        },
        ExpressionNode::Name(path) => ExpressionSnapshot::Name {
            path: table
                .name_path_members(path.members)
                .iter()
                .map(ToString::to_string)
                .collect(),
        },
        ExpressionNode::Range(range) => ExpressionSnapshot::Range {
            start: range
                .start
                .is_valid()
                .then(|| Box::new(table_expression_snapshot(program, range.start))),
            end: range
                .end
                .is_valid()
                .then(|| Box::new(table_expression_snapshot(program, range.end))),
            end_inclusive: range.end_inclusive,
        },
        ExpressionNode::StructLiteral(struct_literal) => ExpressionSnapshot::StructLiteral {
            type_name: struct_literal.type_name.to_string(),
            fields: table
                .struct_fields(struct_literal.fields)
                .iter()
                .map(|field| StructLiteralFieldSnapshot {
                    name: field.name.to_string(),
                    value: table_expression_snapshot(program, field.value),
                })
                .collect(),
        },
        ExpressionNode::String(value) => ExpressionSnapshot::String {
            bytes: value.to_vec(),
        },
        ExpressionNode::Unary(unary) => ExpressionSnapshot::Unary {
            operator: unary.operator.display_name(),
            operand: Box::new(table_expression_snapshot(program, unary.operand)),
        },
        ExpressionNode::ZeroValue(type_reference) => ExpressionSnapshot::ZeroValue {
            type_reference: Box::new(type_reference_snapshot_from_program(
                program,
                program.child_type_reference(*type_reference),
            )),
        },
    }
}

pub(crate) fn snapshot_static_argument(
    program: &SymbolResolvedTrees,
    argument: &crate::expression::StaticMachineArgument,
) -> StaticArgumentSnapshot {
    if argument.type_reference.is_valid() {
        StaticArgumentSnapshot::Type {
            type_reference: Box::new(type_reference_snapshot_from_program(
                program,
                program.child_type_reference(argument.type_reference),
            )),
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
            path: diagnostic_name_span_snapshot(&argument.path),
            lifetime_arguments: diagnostic_name_span_snapshot(&application.lifetime_arguments),
            arguments: application
                .arguments
                .iter()
                .map(|argument| snapshot_static_argument(program, argument))
                .collect(),
        }
    } else {
        StaticArgumentSnapshot::Path(diagnostic_name_span_snapshot(&argument.path))
    }
}

fn diagnostic_name_span_snapshot(path: &[crate::name::DiagnosticName]) -> Vec<String> {
    path.iter().map(ToString::to_string).collect()
}

fn binary_operator_name(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Add => "+",
        BinaryOperator::And => "&&",
        BinaryOperator::BitwiseAnd => "&",
        BinaryOperator::BitwiseOr => "|",
        BinaryOperator::BitwiseXor => "^",
        BinaryOperator::Divide => "/",
        BinaryOperator::Equal => "==",
        BinaryOperator::Greater => ">",
        BinaryOperator::GreaterOrEqual => ">=",
        BinaryOperator::Less => "<",
        BinaryOperator::LessOrEqual => "<=",
        BinaryOperator::Modulo => "%",
        BinaryOperator::Multiply => "*",
        BinaryOperator::NotEqual => "!=",
        BinaryOperator::Or => "||",
        BinaryOperator::ShiftLeft => "<<",
        BinaryOperator::ShiftRight => ">>",
        BinaryOperator::Subtract => "-",
    }
}
