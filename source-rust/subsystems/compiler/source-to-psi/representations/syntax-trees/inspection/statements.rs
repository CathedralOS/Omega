use crate::SyntaxTrees;
use crate::item::{CrashCause, State, StateParameterHandle, StateParameterNode, StateSignature};
use crate::snapshot::expression_and_type_snapshots::{
    snapshot_expression_handle, snapshot_identifier, snapshot_identifier_slice,
    snapshot_type_reference_handle, snapshot_type_references,
};
use crate::snapshot::item_snapshots::{
    snapshot_capability_contract, snapshot_capability_contracts,
    snapshot_generic_conformance_bound, snapshot_type_parameters,
};
use crate::snapshot::{
    CapabilityContractSnapshot, ExpressionSnapshot, GenericConformanceBoundSnapshot,
    IdentifierSnapshot, TypeParameterSnapshot, TypeReferenceSnapshot,
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
    pub service_reaches: Vec<TypeReferenceSnapshot>,
    pub invokes: Vec<TypeReferenceSnapshot>,
    pub suspends: bool,
    pub blocks: bool,
    pub admitted_same_stack_bytes: u64,
    pub contracts: Vec<CapabilityContractSnapshot>,
    pub default_body: Vec<StatementSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nested_machines: Vec<crate::snapshot::item_snapshots::ItemSnapshot>,
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
    AuthoredAssembly {
        clobbers: Option<Vec<String>>,
        requires: Vec<ExpressionSnapshot>,
        ensures: Vec<ExpressionSnapshot>,
        blocks: Vec<AuthoredAssemblyBlockSnapshot>,
    },
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
        expression: ExpressionSnapshot,
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
        #[serde(skip_serializing_if = "Vec::is_empty")]
        pattern_fields: Vec<crate::snapshot::expression_and_type_snapshots::MatchFieldSnapshot>,
    },
    Dispatch {
        subjects: Vec<ExpressionSnapshot>,
        arms: Vec<DispatchArmSnapshot>,
    },
    Transition {
        #[serde(skip_serializing_if = "Vec::is_empty")]
        patterns: Vec<(
            bool,
            ExpressionSnapshot,
            crate::snapshot::expression_and_type_snapshots::MatchPatternSnapshot,
        )>,
        target: TransitionTargetSnapshot,
        continuation: Option<TransitionTargetSnapshot>,
        guard: TransitionGuardSnapshot,
        #[serde(skip_serializing_if = "Option::is_none")]
        crash_cause: Option<&'static str>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DispatchArmSnapshot {
    pub patterns: Vec<crate::snapshot::expression_and_type_snapshots::MatchPatternSnapshot>,
    pub guard: TransitionGuardSnapshot,
    pub target: TransitionTargetSnapshot,
    pub arrow_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthoredAssemblyBlockSnapshot {
    pub label: Option<String>,
    pub requires: Vec<ExpressionSnapshot>,
    pub ensures: Vec<ExpressionSnapshot>,
    pub instructions: Vec<AuthoredInstructionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthoredInstructionSnapshot {
    pub mnemonic: String,
    pub operands: AuthoredOperandsSnapshot,
    pub source_range: [usize; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoredOperandsSnapshot {
    Expressions(Vec<ExpressionSnapshot>),
    Transition(TransitionTargetSnapshot),
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
        acknowledgement_synthesized: bool,
        acknowledges_suspend: bool,
        acknowledges_block: bool,
    },
    Value {
        expression: ExpressionSnapshot,
    },
    Terminal,
}

pub(crate) fn snapshot_state_node(syntax_trees: &SyntaxTrees, state: &State) -> StateSnapshot {
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
            type_parameters: snapshot_type_parameters($syntax_trees, $signature.type_parameters),
            conformance_bounds: $signature
                .conformance_bounds
                .iter()
                .map(|bound| snapshot_generic_conformance_bound($syntax_trees, bound))
                .collect(),
            is_default: $signature.default_body.is_some(),
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
            return_type: snapshot_type_reference_handle($syntax_trees, $signature.return_type),
            service_reach_is_installation_bound: $signature.service_reach_is_installation_bound,
            service_reaches: snapshot_type_references($syntax_trees, $signature.service_reaches),
            invokes: snapshot_type_references($syntax_trees, $signature.invokes),
            suspends: $signature.suspends,
            blocks: $signature.blocks,
            admitted_same_stack_bytes: $signature.admitted_same_stack_bytes,
            contracts: snapshot_capability_contracts($syntax_trees, $signature.contracts),
            default_body: snapshot_statements($syntax_trees, $signature.default_body.unwrap_or_default().statements),
            nested_machines: $signature.default_body.unwrap_or_default().nested_machines.handles()
                .map(|handle| crate::snapshot::item_snapshots::snapshot_item($syntax_trees, $syntax_trees.items.item(handle))).collect(),
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
        type_reference: snapshot_type_reference_handle(syntax_trees, parameter.type_reference),
        is_const: parameter.is_const,
        is_mutable: parameter.is_mutable,
        relevance: crate::snapshot::item_snapshots::snapshot_binding_relevance(parameter.relevance),
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
            snapshot_state_parameter(syntax_trees, syntax_trees.items.state_parameter(*handle))
        })
        .collect()
}

pub(crate) fn snapshot_statement(
    syntax_trees: &SyntaxTrees,
    statement: &StatementNode,
) -> StatementSnapshot {
    match statement {
        StatementNode::AuthoredAssembly(handle) => {
            let graph = syntax_trees.statements.authored_assembly(*handle);
            let expressions = |values: &[crate::expression::ExpressionHandle]| {
                values
                    .iter()
                    .map(|expr| snapshot_expression_handle(syntax_trees, *expr))
                    .collect()
            };
            StatementSnapshot::AuthoredAssembly {
                clobbers: graph.clobbers.as_ref().map(|names| {
                    names
                        .iter()
                        .map(|name| name.as_str(syntax_trees).to_owned())
                        .collect()
                }),
                requires: expressions(&graph.requires),
                ensures: expressions(&graph.ensures),
                blocks: graph
                    .blocks
                    .iter()
                    .map(|block| AuthoredAssemblyBlockSnapshot {
                        label: block
                            .label
                            .as_ref()
                            .map(|name| name.as_str(syntax_trees).to_owned()),
                        requires: expressions(&block.requires),
                        ensures: expressions(&block.ensures),
                        instructions: block
                            .instructions
                            .iter()
                            .map(|instruction| AuthoredInstructionSnapshot {
                                mnemonic: instruction.mnemonic.as_str(syntax_trees).to_owned(),
                                operands: match &instruction.operands {
                                    crate::statement::AssemblyOperands::Expressions(values) => {
                                        AuthoredOperandsSnapshot::Expressions(expressions(values))
                                    }
                                    crate::statement::AssemblyOperands::Transition(target) => {
                                        AuthoredOperandsSnapshot::Transition(
                                            snapshot_transition_target(
                                                syntax_trees,
                                                syntax_trees.statements.transition_target(*target),
                                            ),
                                        )
                                    }
                                },
                                source_range: [
                                    instruction.source_span.span.start,
                                    instruction.source_span.span.end,
                                ],
                            })
                            .collect(),
                    })
                    .collect(),
            }
        }
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
                            .map(|expression| snapshot_expression_handle(syntax_trees, *expression))
                            .collect(),
                        ensures: syntax_trees
                            .statements
                            .assembly_expressions(block.ensures)
                            .iter()
                            .map(|expression| snapshot_expression_handle(syntax_trees, *expression))
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
                            AssemblyExit::Jump(label) => {
                                AssemblyExitSnapshot::Jump(snapshot_identifier(syntax_trees, label))
                            }
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
            expression: snapshot_expression_handle(syntax_trees, call.expression),
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
            type_reference: snapshot_type_reference_handle(syntax_trees, value.type_reference),
            initial_value: snapshot_expression_handle(syntax_trees, value.initial_value),
            is_mutable: value.is_mutable,
            relevance: crate::snapshot::item_snapshots::snapshot_binding_relevance(value.relevance),
            pattern_fields: crate::snapshot::expression_and_type_snapshots::snapshot_match_fields(
                syntax_trees,
                value.pattern_fields,
            ),
        },
        StatementNode::Dispatch(handle) => {
            let dispatch = syntax_trees.statements.dispatch(*handle);
            StatementSnapshot::Dispatch {
                subjects: syntax_trees
                    .statements
                    .expression_handles(dispatch.subjects)
                    .iter()
                    .map(|&subject| snapshot_expression_handle(syntax_trees, subject))
                    .collect(),
                arms: dispatch
                    .arms
                    .iter()
                    .map(|arm| snapshot_dispatch_arm(syntax_trees, arm))
                    .collect(),
            }
        }
        StatementNode::Transition(value) => StatementSnapshot::Transition {
            patterns: syntax_trees
                .statements
                .transition_patterns(value.patterns)
                .iter()
                .map(|retained| {
                    (
                        retained.is_structural,
                        snapshot_expression_handle(syntax_trees, retained.subject),
                        crate::snapshot::expression_and_type_snapshots::snapshot_match_pattern(
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

fn snapshot_dispatch_arm(
    trees: &SyntaxTrees,
    arm: &crate::statement::DispatchArm,
) -> DispatchArmSnapshot {
    use crate::snapshot::expression_and_type_snapshots::snapshot_match_pattern;
    DispatchArmSnapshot {
        patterns: arm
            .patterns
            .iter()
            .map(|slot| snapshot_match_pattern(trees, slot.pattern))
            .collect(),
        guard: match arm.guard {
            TransitionGuardNode::Always => TransitionGuardSnapshot::Always,
            TransitionGuardNode::When(value) => TransitionGuardSnapshot::When {
                expression: snapshot_expression_handle(trees, value),
            },
        },
        target: snapshot_transition_target(trees, trees.statements.transition_target(arm.target)),
        arrow_offset: arm.source_span.span.start,
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
        .map(|handle| snapshot_statement(syntax_trees, syntax_trees.statements.statement(*handle)))
        .collect()
}

fn snapshot_transition_target(
    syntax_trees: &SyntaxTrees,
    target: &TransitionTargetNode,
) -> TransitionTargetSnapshot {
    match target {
        TransitionTargetNode::Named(expression) => {
            let call = syntax_trees.expressions.call(*expression);
            let receiver = syntax_trees
                .expression_identifier_path(call.receiver)
                .expect("a named transition has a static receiver path");
            TransitionTargetSnapshot::Named {
                path: receiver
                    .into_iter()
                    .chain([&call.target])
                    .map(|name| snapshot_identifier(syntax_trees, name))
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
        TransitionTargetNode::Value(expression) => TransitionTargetSnapshot::Value {
            expression: snapshot_expression_handle(syntax_trees, *expression),
        },
        TransitionTargetNode::Terminal => TransitionTargetSnapshot::Terminal,
    }
}
