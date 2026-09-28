//! Snapshots of states, signatures, parameters, statements and transitions.

use crate::item::{StateParameterNode, StateSignature};
use crate::statement::{
    AssemblyFactKind, StatementNode, TransitionGuardNode, TransitionTargetNode,
};
use crate::syntax_trees::SyntaxTrees;
use crate::syntax_trees::inspection::snapshot::expression_and_type_snapshots::{
    snapshot_expression_handle, snapshot_identifier, snapshot_identifier_slice,
    snapshot_static_argument, snapshot_type_reference_handle,
};
use crate::syntax_trees::inspection::snapshot::item_snapshots::{
    snapshot_capability_contract, snapshot_capability_contracts, snapshot_type_parameter,
};
use crate::syntax_trees::inspection::snapshot::{
    CapabilityContractSnapshot, ExpressionSnapshot, IdentifierSnapshot, StaticArgumentSnapshot,
    TypeParameterSnapshot, TypeReferenceSnapshot,
};
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
    pub is_self: bool,
    pub relevance: &'static str,
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
        contract_kind: AssemblyFactKindSnapshot,
        expression: ExpressionSnapshot,
    },
    Assignment {
        target: ExpressionSnapshot,
        value: ExpressionSnapshot,
    },
    Call {
        receiver: Vec<IdentifierSnapshot>,
        receiver_starts_at_self: bool,
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
        name: IdentifierSnapshot,
        type_reference: TypeReferenceSnapshot,
        initial_value: ExpressionSnapshot,
        is_mutable: bool,
        relevance: &'static str,
    },
    Transition {
        target: TransitionTargetSnapshot,
        continuation: Option<TransitionTargetSnapshot>,
        guard: TransitionGuardSnapshot,
        #[serde(skip_serializing_if = "Option::is_none")]
        crash_cause: Option<&'static str>,
    },
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
        path_starts_at_self: bool,
        arguments: Vec<ExpressionSnapshot>,
        evidence_arguments: Vec<IdentifierSnapshot>,
    },
    Value {
        expression: ExpressionSnapshot,
    },
    SelfTarget,
    Terminal,
}

pub(crate) fn snapshot_state_node(
    syntax_trees: &SyntaxTrees,
    state: &crate::item::StateNode,
) -> StateSnapshot {
    StateSnapshot {
        name: snapshot_identifier(&state.name),
        parameters: syntax_trees
            .items
            .state_parameters(state.parameters)
            .iter()
            .map(|handle| {
                snapshot_state_parameter(syntax_trees, syntax_trees.items.state_parameter(*handle))
            })
            .collect(),
        return_type: snapshot_type_reference_handle(syntax_trees, state.return_type),
        contracts: syntax_trees
            .items
            .capability_contracts(state.contracts)
            .iter()
            .map(|contract| snapshot_capability_contract(syntax_trees, contract))
            .collect(),
        statements: syntax_trees
            .items
            .statements(state.statements)
            .iter()
            .map(|handle| {
                snapshot_statement(syntax_trees, syntax_trees.statements.statement(*handle))
            })
            .collect(),
    }
}

pub(crate) fn snapshot_state_signature(
    syntax_trees: &SyntaxTrees,
    signature: &StateSignature,
) -> StateSignatureSnapshot {
    StateSignatureSnapshot {
        name: snapshot_identifier(&signature.name),
        spelling: signature.spelling.map(|spelling| spelling.symbol()),
        lifetime_parameters: signature
            .lifetime_parameters
            .iter()
            .map(snapshot_identifier)
            .collect(),
        type_parameters: syntax_trees
            .items
            .type_parameters(signature.type_parameters)
            .iter()
            .map(|parameter| snapshot_type_parameter(syntax_trees, parameter))
            .collect(),
        is_default: signature.is_default,
        parameters: syntax_trees
            .items
            .state_parameters(signature.parameters)
            .iter()
            .map(|handle| {
                snapshot_state_parameter(syntax_trees, syntax_trees.items.state_parameter(*handle))
            })
            .collect(),
        native_callback_parameters: signature
            .native_callback_parameters
            .iter()
            .map(|parameter| NativeCallbackParameterSnapshot {
                name: snapshot_identifier(&parameter.name),
                binder: snapshot_identifier(&parameter.binder),
                native_ordinal: parameter.native_ordinal,
            })
            .collect(),
        return_type: snapshot_type_reference_handle(syntax_trees, signature.return_type),
        service_reach_is_installation_bound: signature.service_reach_is_installation_bound,
        service_reaches: snapshot_identifier_slice(
            syntax_trees
                .items
                .identifier_path_members(signature.service_reaches),
        ),
        invokes: snapshot_identifier_slice(
            syntax_trees
                .items
                .identifier_path_members(signature.invokes),
        ),
        suspends: signature.suspends,
        blocks: signature.blocks,
        contracts: snapshot_capability_contracts(syntax_trees, signature.contracts),
        default_body: syntax_trees
            .items
            .statements(signature.default_body)
            .iter()
            .map(|handle| {
                snapshot_statement(syntax_trees, syntax_trees.statements.statement(*handle))
            })
            .collect(),
        terminates_guarantee: signature.terminates_guarantee,
    }
}

pub(crate) fn snapshot_state_signature_node(
    syntax_trees: &SyntaxTrees,
    signature: &crate::item::StateSignatureNode,
) -> StateSignatureSnapshot {
    StateSignatureSnapshot {
        name: snapshot_identifier(&signature.name),
        spelling: signature.spelling.map(|spelling| spelling.symbol()),
        lifetime_parameters: signature
            .lifetime_parameters
            .iter()
            .map(snapshot_identifier)
            .collect(),
        type_parameters: syntax_trees
            .items
            .type_parameters(signature.type_parameters)
            .iter()
            .map(|parameter| snapshot_type_parameter(syntax_trees, parameter))
            .collect(),
        is_default: signature.is_default,
        parameters: syntax_trees
            .items
            .state_parameters(signature.parameters)
            .iter()
            .map(|handle| {
                snapshot_state_parameter(syntax_trees, syntax_trees.items.state_parameter(*handle))
            })
            .collect(),
        native_callback_parameters: signature
            .native_callback_parameters
            .iter()
            .map(|parameter| NativeCallbackParameterSnapshot {
                name: snapshot_identifier(&parameter.name),
                binder: snapshot_identifier(&parameter.binder),
                native_ordinal: parameter.native_ordinal,
            })
            .collect(),
        return_type: snapshot_type_reference_handle(syntax_trees, signature.return_type),
        service_reach_is_installation_bound: signature.service_reach_is_installation_bound,
        service_reaches: snapshot_identifier_slice(
            syntax_trees
                .items
                .identifier_path_members(signature.service_reaches),
        ),
        invokes: snapshot_identifier_slice(
            syntax_trees
                .items
                .identifier_path_members(signature.invokes),
        ),
        suspends: signature.suspends,
        blocks: signature.blocks,
        contracts: snapshot_capability_contracts(syntax_trees, signature.contracts),
        default_body: syntax_trees
            .items
            .statements(signature.default_body)
            .iter()
            .map(|handle| {
                snapshot_statement(syntax_trees, syntax_trees.statements.statement(*handle))
            })
            .collect(),
        terminates_guarantee: signature.terminates_guarantee,
    }
}

pub(crate) fn snapshot_state_parameter(
    syntax_trees: &SyntaxTrees,
    parameter: &StateParameterNode,
) -> StateParameterSnapshot {
    StateParameterSnapshot {
        name: snapshot_identifier(&parameter.name),
        type_reference: snapshot_type_reference_handle(syntax_trees, parameter.type_reference),
        is_const: parameter.is_const,
        is_mutable: parameter.is_mutable,
        is_self: parameter.is_self,
        relevance: super::item_snapshots::snapshot_binding_relevance(parameter.relevance),
    }
}

fn snapshot_statement(syntax_trees: &SyntaxTrees, statement: &StatementNode) -> StatementSnapshot {
    match statement {
        StatementNode::RootBinding(binding) => StatementSnapshot::RootBinding {
            receiver: snapshot_expression_handle(syntax_trees, binding.receiver),
            slot: binding.slot.iter().map(ToString::to_string).collect(),
            implementation: binding
                .implementation
                .iter()
                .map(ToString::to_string)
                .collect(),
            implementation_operand: binding
                .implementation_operand
                .is_valid()
                .then(|| snapshot_expression_handle(syntax_trees, binding.implementation_operand)),
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
                syntax_trees
                    .statements
                    .identifier_path_members(call.receiver),
            ),
            receiver_starts_at_self: call.receiver_starts_at_self,
            target: snapshot_identifier(&call.target),
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
            evidence_arguments: snapshot_identifier_slice(&call.evidence_arguments),
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
                            snapshot_identifier(&binding.output_field),
                            snapshot_identifier(&binding.binding),
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
            name: snapshot_identifier(&value.name),
            type_reference: snapshot_type_reference_handle(syntax_trees, value.type_reference),
            initial_value: snapshot_expression_handle(syntax_trees, value.initial_value),
            is_mutable: value.is_mutable,
            relevance: super::item_snapshots::snapshot_binding_relevance(value.relevance),
        },
        StatementNode::Transition(value) => StatementSnapshot::Transition {
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
                crate::statement::TransitionExit::Ordinary => None,
                crate::statement::TransitionExit::Crash(crate::item::CrashCause::Trap) => {
                    Some("Trap")
                }
                crate::statement::TransitionExit::Crash(crate::item::CrashCause::Abort) => {
                    Some("Abort")
                }
            },
        },
    }
}

fn snapshot_transition_target(
    syntax_trees: &SyntaxTrees,
    target: &TransitionTargetNode,
) -> TransitionTargetSnapshot {
    match target {
        TransitionTargetNode::Named {
            path,
            path_starts_at_self,
            arguments,
            evidence_arguments,
            ..
        } => TransitionTargetSnapshot::Named {
            path: snapshot_identifier_slice(syntax_trees.statements.identifier_path_members(*path)),
            path_starts_at_self: *path_starts_at_self,
            arguments: syntax_trees
                .statements
                .expression_handles(*arguments)
                .iter()
                .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                .collect(),
            evidence_arguments: snapshot_identifier_slice(evidence_arguments),
        },
        TransitionTargetNode::Value(expression) => TransitionTargetSnapshot::Value {
            expression: snapshot_expression_handle(syntax_trees, *expression),
        },
        TransitionTargetNode::SelfTarget => TransitionTargetSnapshot::SelfTarget,
        TransitionTargetNode::Terminal => TransitionTargetSnapshot::Terminal,
    }
}
