use crate::expression::{
    BinaryOperator, ExpressionNode, TableCallExpression, TableStructLiteralField,
};
use crate::identifier::Identifier;
use crate::item::{
    BoundaryLevel, CapabilityContract, CapabilityContractKind, CapabilityMember, DataMember, Item,
    LibraryFunction, ProofFact, StateParameterNode, StateSignature,
};
use crate::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use crate::syntax_trees::SyntaxTrees;
use crate::types::{TypeConstraintNode, TypeReferenceNode};
use omega_core::diagnostics::PhaseSnapshot;
use serde::Serialize;

#[cfg(test)]
mod tests;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentifierSnapshot {
    pub text: String,
    pub source_id: usize,
    pub start: usize,
    pub end: usize,
    pub source_backed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ItemSnapshot {
    Capability {
        name: IdentifierSnapshot,
        members: Vec<CapabilityMemberSnapshot>,
    },
    Data {
        name: IdentifierSnapshot,
        type_parameters: Vec<TypeParameterSnapshot>,
        members: Vec<DataMemberSnapshot>,
    },
    Domain {
        name: IdentifierSnapshot,
        target_type: TypeReferenceSnapshot,
        facts: Vec<ProofFactSnapshot>,
        operators: Vec<OperatorSnapshot>,
        body_token_count: usize,
    },
    Invariant {
        name: IdentifierSnapshot,
        constraints: Vec<TypeConstraintSnapshot>,
    },
    Library {
        name: Option<IdentifierSnapshot>,
        path: String,
        calling_convention: IdentifierSnapshot,
        functions: Vec<LibraryFunctionSnapshot>,
    },
    Operator {
        operator: OperatorSnapshot,
    },
    Provider {
        name: Vec<IdentifierSnapshot>,
        category: &'static str,
    },
    Export {
        path: Vec<IdentifierSnapshot>,
        alias: Option<IdentifierSnapshot>,
    },
    Use {
        path: Vec<IdentifierSnapshot>,
    },
    Machine {
        name: IdentifierSnapshot,
        attached_data: Option<IdentifierSnapshot>,
        terminates: bool,
        decreases: Vec<ExpressionSnapshot>,
        decrease_order: Vec<IdentifierSnapshot>,
        effects: Vec<IdentifierSnapshot>,
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
        requires: Vec<IdentifierSnapshot>,
        machines: Vec<StateSignatureSnapshot>,
    },
    Target {
        name: IdentifierSnapshot,
        host: Option<TargetHostSnapshot>,
        boundary_policies: Vec<BoundaryPolicySnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypeParameterSnapshot {
    pub name: IdentifierSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OperatorSnapshot {
    pub is_boundary: bool,
    pub name: Vec<IdentifierSnapshot>,
    pub type_parameters: Vec<TypeParameterSnapshot>,
    pub parameters: Vec<StateParameterSnapshot>,
    pub return_type: TypeReferenceSnapshot,
    pub contracts: Vec<CapabilityContractSnapshot>,
    pub spelling: Option<&'static str>,
    pub provider: Option<Vec<IdentifierSnapshot>>,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataMemberSnapshot {
    Field {
        name: IdentifierSnapshot,
        type_reference: TypeReferenceSnapshot,
        initial_value: ExpressionSnapshot,
    },
    Variant {
        name: IdentifierSnapshot,
    },
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
    pub facts: Vec<ProofFactSnapshot>,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CapabilityContractKindSnapshot {
    Ensures,
    Requires,
    Boundary { boundary: BoundaryLevelSnapshot },
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
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LibraryFunctionSnapshot {
    pub signature: StateSignatureSnapshot,
    pub symbol: Option<String>,
    pub calling_convention: Option<IdentifierSnapshot>,
    pub boundaries: Vec<BoundaryLevelSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BoundaryLevelSnapshot {
    Host,
    Named { name: IdentifierSnapshot },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TargetHostSnapshot {
    pub provider: Vec<IdentifierSnapshot>,
    pub settings: Vec<TargetHostSettingSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TargetHostSettingSnapshot {
    pub name: IdentifierSnapshot,
    pub value: TargetHostSettingValueSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TargetHostSettingValueSnapshot {
    Call {
        name: IdentifierSnapshot,
        argument_tokens: usize,
    },
    Named {
        name: IdentifierSnapshot,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BoundaryPolicySnapshot {
    pub mode: &'static str,
    pub path: Vec<IdentifierSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateSnapshot {
    pub name: IdentifierSnapshot,
    pub parameters: Vec<StateParameterSnapshot>,
    pub return_type: TypeReferenceSnapshot,
    pub statements: Vec<StatementSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateSignatureSnapshot {
    pub name: IdentifierSnapshot,
    pub parameters: Vec<StateParameterSnapshot>,
    pub return_type: TypeReferenceSnapshot,
    pub effects: Vec<IdentifierSnapshot>,
    pub contracts: Vec<CapabilityContractSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateParameterSnapshot {
    pub name: IdentifierSnapshot,
    pub type_reference: TypeReferenceSnapshot,
    pub is_const: bool,
    pub is_mutable: bool,
    pub is_self: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StatementSnapshot {
    Assignment {
        target: ExpressionSnapshot,
        value: ExpressionSnapshot,
    },
    Call {
        receiver: Vec<IdentifierSnapshot>,
        target: IdentifierSnapshot,
        arguments: Vec<ExpressionSnapshot>,
    },
    Expression {
        value: ExpressionSnapshot,
    },
    LocalData {
        name: IdentifierSnapshot,
        type_reference: TypeReferenceSnapshot,
        initial_value: ExpressionSnapshot,
    },
    Transition {
        target: TransitionTargetSnapshot,
        continuation: Option<TransitionTargetSnapshot>,
        guard: TransitionGuardSnapshot,
    },
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
    },
    Value {
        expression: ExpressionSnapshot,
    },
    SelfTarget,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeReferenceSnapshot {
    Reference {
        referee: Box<TypeReferenceSnapshot>,
        is_mutable: bool,
    },
    Constrained {
        base_type: Box<TypeReferenceSnapshot>,
        constraints: Vec<TypeConstraintSnapshot>,
    },
    FixedArray {
        element_type: Box<TypeReferenceSnapshot>,
        length: usize,
    },
    Slice {
        element_type: Box<TypeReferenceSnapshot>,
    },
    Generic {
        base_name: IdentifierSnapshot,
        arguments: Vec<TypeReferenceSnapshot>,
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
pub enum TypeConstraintSnapshot {
    Named {
        name: IdentifierSnapshot,
    },
    Range {
        minimum: ExpressionSnapshot,
        maximum: ExpressionSnapshot,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExpressionSnapshot {
    ArrayLiteral {
        values: Vec<ExpressionSnapshot>,
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
        target_type: Vec<IdentifierSnapshot>,
    },
    Call {
        receiver: Option<Box<ExpressionSnapshot>>,
        target: IdentifierSnapshot,
        arguments: Vec<ExpressionSnapshot>,
    },
    Float {
        text: String,
    },
    Indexed {
        collection: Box<ExpressionSnapshot>,
        index: Box<ExpressionSnapshot>,
    },
    Integer {
        value: i64,
    },
    Membership {
        value: Box<ExpressionSnapshot>,
        domain: Vec<IdentifierSnapshot>,
    },
    Member {
        receiver: Box<ExpressionSnapshot>,
        member: IdentifierSnapshot,
    },
    Mutable {
        value: Box<ExpressionSnapshot>,
    },
    Name {
        path: Vec<IdentifierSnapshot>,
    },
    Range {
        start: Option<Box<ExpressionSnapshot>>,
        end: Option<Box<ExpressionSnapshot>>,
    },
    SelfValue,
    StructLiteral {
        type_name: IdentifierSnapshot,
        fields: Vec<StructLiteralFieldSnapshot>,
    },
    String {
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StructLiteralFieldSnapshot {
    pub name: IdentifierSnapshot,
    pub value: ExpressionSnapshot,
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

fn snapshot_item(syntax_trees: &SyntaxTrees, item: &Item) -> ItemSnapshot {
    match item {
        Item::Capability(value) => ItemSnapshot::Capability {
            name: snapshot_identifier(&value.name),
            members: syntax_trees
                .items
                .capability_members(value.members)
                .iter()
                .map(|member| snapshot_capability_member(syntax_trees, member))
                .collect(),
        },
        Item::Data(value) => ItemSnapshot::Data {
            name: snapshot_identifier(&value.name),
            type_parameters: syntax_trees
                .items
                .type_parameters(value.type_parameters)
                .iter()
                .map(|parameter| TypeParameterSnapshot {
                    name: snapshot_identifier(&parameter.name),
                })
                .collect(),
            members: syntax_trees
                .items
                .data_members(value.members)
                .iter()
                .map(|member| snapshot_data_member(syntax_trees, member))
                .collect(),
        },
        Item::Domain(value) => ItemSnapshot::Domain {
            name: snapshot_identifier(&value.name),
            target_type: snapshot_type_reference_handle(syntax_trees, value.target_type),
            facts: snapshot_proof_facts(syntax_trees, value.facts),
            operators: syntax_trees
                .items
                .operators(value.operators)
                .iter()
                .map(|operator| snapshot_operator(syntax_trees, operator))
                .collect(),
            body_token_count: value.body_token_count,
        },
        Item::Invariant(value) => ItemSnapshot::Invariant {
            name: snapshot_identifier(&value.name),
            constraints: syntax_trees
                .type_references
                .constraints(value.constraints)
                .iter()
                .map(|constraint| snapshot_type_constraint(syntax_trees, constraint))
                .collect(),
        },
        Item::Library(value) => ItemSnapshot::Library {
            name: value.name.as_ref().map(snapshot_identifier),
            path: value.path.clone(),
            calling_convention: snapshot_identifier(&value.calling_convention),
            functions: syntax_trees
                .items
                .library_functions(value.functions)
                .iter()
                .map(|function| snapshot_library_function(syntax_trees, function))
                .collect(),
        },
        Item::Operator(value) => ItemSnapshot::Operator {
            operator: snapshot_operator(syntax_trees, value),
        },
        Item::Provider(value) => ItemSnapshot::Provider {
            name: snapshot_identifier_slice(syntax_trees.items.identifier_path_members(value.name)),
            category: value.category.name(),
        },
        Item::Export(value) => ItemSnapshot::Export {
            path: snapshot_identifier_slice(syntax_trees.items.identifier_path_members(value.path)),
            alias: value.alias.as_ref().map(snapshot_identifier),
        },
        Item::Use(value) => ItemSnapshot::Use {
            path: snapshot_identifier_slice(syntax_trees.items.identifier_path_members(value.path)),
        },
        Item::Machine(value) => ItemSnapshot::Machine {
            name: snapshot_identifier(&value.name),
            attached_data: value.attached_data.as_ref().map(snapshot_identifier),
            terminates: value.terminates,
            decreases: syntax_trees
                .expressions
                .expression_handles(value.decreases)
                .iter()
                .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                .collect(),
            decrease_order: snapshot_identifier_slice(
                syntax_trees
                    .items
                    .identifier_path_members(value.decrease_order),
            ),
            effects: snapshot_identifier_slice(
                syntax_trees.items.identifier_path_members(value.effects),
            ),
            contracts: snapshot_capability_contracts(syntax_trees, value.contracts),
            states: syntax_trees
                .items
                .state_handles(value.states)
                .iter()
                .map(|handle| snapshot_state_node(syntax_trees, syntax_trees.items.state(*handle)))
                .collect(),
        },
        Item::Platform(value) => ItemSnapshot::Platform {
            name: snapshot_identifier(&value.name),
            states: syntax_trees
                .items
                .state_signatures(value.states)
                .iter()
                .map(|handle| {
                    snapshot_state_signature_node(
                        syntax_trees,
                        syntax_trees.items.state_signature(*handle),
                    )
                })
                .collect(),
        },
        Item::Trait(value) => ItemSnapshot::Trait {
            name: snapshot_identifier(&value.name),
            is_boundary: value.is_boundary,
            requires: snapshot_identifier_slice(
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
        Item::Target(value) => ItemSnapshot::Target {
            name: snapshot_identifier(&value.name),
            host: value.host.as_ref().map(|host| TargetHostSnapshot {
                provider: snapshot_identifier_slice(
                    syntax_trees.items.identifier_path_members(host.provider),
                ),
                settings: syntax_trees
                    .items
                    .target_host_settings(host.settings)
                    .iter()
                    .map(|setting| TargetHostSettingSnapshot {
                        name: snapshot_identifier(&setting.name),
                        value: match &setting.value {
                            crate::item::TargetHostSettingValue::Call {
                                name,
                                argument_tokens,
                            } => TargetHostSettingValueSnapshot::Call {
                                name: snapshot_identifier(name),
                                argument_tokens: *argument_tokens,
                            },
                            crate::item::TargetHostSettingValue::Named(name) => {
                                TargetHostSettingValueSnapshot::Named {
                                    name: snapshot_identifier(name),
                                }
                            }
                        },
                    })
                    .collect(),
            }),
            boundary_policies: syntax_trees
                .items
                .boundary_policies(value.boundary_policies)
                .iter()
                .map(|policy| BoundaryPolicySnapshot {
                    mode: match policy.mode {
                        crate::item::BoundaryMode::Checked => "checked",
                        crate::item::BoundaryMode::Unchecked => "unchecked",
                    },
                    path: snapshot_identifier_slice(
                        syntax_trees.items.identifier_path_members(policy.path),
                    ),
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
        is_boundary: operator.is_boundary,
        name: snapshot_identifier_slice(syntax_trees.items.identifier_path_members(operator.name)),
        type_parameters: syntax_trees
            .items
            .type_parameters(operator.type_parameters)
            .iter()
            .map(|parameter| TypeParameterSnapshot {
                name: snapshot_identifier(&parameter.name),
            })
            .collect(),
        parameters: syntax_trees
            .items
            .state_parameters(operator.parameters)
            .iter()
            .map(|handle| {
                snapshot_state_parameter(syntax_trees, syntax_trees.items.state_parameter(*handle))
            })
            .collect(),
        return_type: snapshot_type_reference_handle(syntax_trees, operator.return_type),
        contracts: snapshot_capability_contracts(syntax_trees, operator.contracts),
        spelling: operator.spelling.map(|spelling| spelling.symbol()),
        provider: operator.provider.map(|provider| {
            snapshot_identifier_slice(syntax_trees.items.identifier_path_members(provider))
        }),
        token_count: operator.token_count,
    }
}

fn snapshot_capability_member(
    syntax_trees: &SyntaxTrees,
    member: &CapabilityMember,
) -> CapabilityMemberSnapshot {
    match member {
        CapabilityMember::Field(field) => CapabilityMemberSnapshot::Field {
            name: snapshot_identifier(&field.name),
            type_reference: snapshot_type_reference_handle(syntax_trees, field.type_reference),
        },
        CapabilityMember::State(state) => CapabilityMemberSnapshot::State {
            signature: snapshot_state_signature(syntax_trees, &state.signature),
            contracts: snapshot_capability_contracts(syntax_trees, state.contracts),
        },
    }
}

fn snapshot_capability_contracts(
    syntax_trees: &SyntaxTrees,
    contracts: omega_core::arena::HandleSpan<CapabilityContract>,
) -> Vec<CapabilityContractSnapshot> {
    syntax_trees
        .items
        .capability_contracts(contracts)
        .iter()
        .map(|contract| snapshot_capability_contract(syntax_trees, contract))
        .collect()
}

fn snapshot_capability_contract(
    syntax_trees: &SyntaxTrees,
    contract: &CapabilityContract,
) -> CapabilityContractSnapshot {
    CapabilityContractSnapshot {
        kind: match &contract.kind {
            CapabilityContractKind::Ensures => CapabilityContractKindSnapshot::Ensures,
            CapabilityContractKind::Requires => CapabilityContractKindSnapshot::Requires,
            CapabilityContractKind::Boundary(level) => CapabilityContractKindSnapshot::Boundary {
                boundary: snapshot_boundary_level(level),
            },
        },
        facts: snapshot_proof_facts(syntax_trees, contract.facts),
        token_count: contract.token_count,
    }
}

fn snapshot_proof_facts(
    syntax_trees: &SyntaxTrees,
    facts: omega_core::arena::HandleSpan<ProofFact>,
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
            domain: snapshot_identifier_slice(
                syntax_trees
                    .items
                    .identifier_path_members(membership.domain),
            ),
        },
    }
}

fn snapshot_data_member(syntax_trees: &SyntaxTrees, member: &DataMember) -> DataMemberSnapshot {
    match member {
        DataMember::Field(field) => DataMemberSnapshot::Field {
            name: snapshot_identifier(&field.name),
            type_reference: snapshot_type_reference_handle(syntax_trees, field.type_reference),
            initial_value: snapshot_expression_handle(syntax_trees, field.initial_value),
        },
        DataMember::Variant(variant) => DataMemberSnapshot::Variant {
            name: snapshot_identifier(&variant.name),
        },
    }
}

fn snapshot_library_function(
    syntax_trees: &SyntaxTrees,
    function: &LibraryFunction,
) -> LibraryFunctionSnapshot {
    LibraryFunctionSnapshot {
        signature: snapshot_state_signature(syntax_trees, &function.signature),
        symbol: function.symbol.clone(),
        calling_convention: function
            .calling_convention
            .as_ref()
            .map(snapshot_identifier),
        boundaries: syntax_trees
            .items
            .boundary_levels(function.boundaries)
            .iter()
            .map(snapshot_boundary_level)
            .collect(),
    }
}

fn snapshot_boundary_level(level: &BoundaryLevel) -> BoundaryLevelSnapshot {
    match level {
        BoundaryLevel::Host => BoundaryLevelSnapshot::Host,
        BoundaryLevel::Named(name) => BoundaryLevelSnapshot::Named {
            name: snapshot_identifier(name),
        },
    }
}

fn snapshot_state_node(
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

fn snapshot_state_signature(
    syntax_trees: &SyntaxTrees,
    signature: &StateSignature,
) -> StateSignatureSnapshot {
    StateSignatureSnapshot {
        name: snapshot_identifier(&signature.name),
        parameters: syntax_trees
            .items
            .state_parameters(signature.parameters)
            .iter()
            .map(|handle| {
                snapshot_state_parameter(syntax_trees, syntax_trees.items.state_parameter(*handle))
            })
            .collect(),
        return_type: snapshot_type_reference_handle(syntax_trees, signature.return_type),
        effects: snapshot_identifier_slice(
            syntax_trees
                .items
                .identifier_path_members(signature.effects),
        ),
        contracts: snapshot_capability_contracts(syntax_trees, signature.contracts),
    }
}

fn snapshot_state_signature_node(
    syntax_trees: &SyntaxTrees,
    signature: &crate::item::StateSignatureNode,
) -> StateSignatureSnapshot {
    StateSignatureSnapshot {
        name: snapshot_identifier(&signature.name),
        parameters: syntax_trees
            .items
            .state_parameters(signature.parameters)
            .iter()
            .map(|handle| {
                snapshot_state_parameter(syntax_trees, syntax_trees.items.state_parameter(*handle))
            })
            .collect(),
        return_type: snapshot_type_reference_handle(syntax_trees, signature.return_type),
        effects: snapshot_identifier_slice(
            syntax_trees
                .items
                .identifier_path_members(signature.effects),
        ),
        contracts: snapshot_capability_contracts(syntax_trees, signature.contracts),
    }
}

fn snapshot_state_parameter(
    syntax_trees: &SyntaxTrees,
    parameter: &StateParameterNode,
) -> StateParameterSnapshot {
    StateParameterSnapshot {
        name: snapshot_identifier(&parameter.name),
        type_reference: snapshot_type_reference_handle(syntax_trees, parameter.type_reference),
        is_const: parameter.is_const,
        is_mutable: parameter.is_mutable,
        is_self: parameter.is_self,
    }
}

fn snapshot_statement(syntax_trees: &SyntaxTrees, statement: &StatementNode) -> StatementSnapshot {
    match statement {
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
            target: snapshot_identifier(&call.target),
            arguments: syntax_trees
                .statements
                .expression_handles(call.arguments)
                .iter()
                .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                .collect(),
        },
        StatementNode::Expression(value) => StatementSnapshot::Expression {
            value: snapshot_expression_handle(syntax_trees, *value),
        },
        StatementNode::LocalData(value) => StatementSnapshot::LocalData {
            name: snapshot_identifier(&value.name),
            type_reference: snapshot_type_reference_handle(syntax_trees, value.type_reference),
            initial_value: snapshot_expression_handle(syntax_trees, value.initial_value),
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
        },
    }
}

fn snapshot_transition_target(
    syntax_trees: &SyntaxTrees,
    target: &TransitionTargetNode,
) -> TransitionTargetSnapshot {
    match target {
        TransitionTargetNode::Named {
            path, arguments, ..
        } => TransitionTargetSnapshot::Named {
            path: snapshot_identifier_slice(syntax_trees.statements.identifier_path_members(*path)),
            arguments: syntax_trees
                .statements
                .expression_handles(*arguments)
                .iter()
                .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                .collect(),
        },
        TransitionTargetNode::Value(expression) => TransitionTargetSnapshot::Value {
            expression: snapshot_expression_handle(syntax_trees, *expression),
        },
        TransitionTargetNode::SelfTarget => TransitionTargetSnapshot::SelfTarget,
        TransitionTargetNode::Terminal => TransitionTargetSnapshot::Terminal,
    }
}

fn snapshot_type_reference_handle(
    syntax_trees: &SyntaxTrees,
    handle: crate::types::TypeReferenceHandle,
) -> TypeReferenceSnapshot {
    if !handle.is_valid() {
        return TypeReferenceSnapshot::Missing;
    }

    match syntax_trees.type_references.type_reference(handle) {
        TypeReferenceNode::Reference {
            referee,
            is_mutable,
        } => TypeReferenceSnapshot::Reference {
            referee: Box::new(snapshot_type_reference_handle(syntax_trees, *referee)),
            is_mutable: *is_mutable,
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
            length: *length,
        },
        TypeReferenceNode::Slice { element_type } => TypeReferenceSnapshot::Slice {
            element_type: Box::new(snapshot_type_reference_handle(syntax_trees, *element_type)),
        },
        TypeReferenceNode::Generic {
            base_name,
            arguments,
        } => TypeReferenceSnapshot::Generic {
            base_name: snapshot_identifier(base_name),
            arguments: syntax_trees
                .type_references
                .type_reference_handles(*arguments)
                .iter()
                .map(|handle| snapshot_type_reference_handle(syntax_trees, *handle))
                .collect(),
        },
        TypeReferenceNode::Named(name) => TypeReferenceSnapshot::Named {
            name: snapshot_identifier(name),
        },
        TypeReferenceNode::SelfType => TypeReferenceSnapshot::SelfType,
        TypeReferenceNode::Unit => TypeReferenceSnapshot::Unit,
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
        TypeConstraintNode::Range { minimum, maximum } => TypeConstraintSnapshot::Range {
            minimum: snapshot_expression_handle(syntax_trees, *minimum),
            maximum: snapshot_expression_handle(syntax_trees, *maximum),
        },
    }
}

fn snapshot_expression_handle(
    syntax_trees: &SyntaxTrees,
    handle: crate::expression::ExpressionHandle,
) -> ExpressionSnapshot {
    if !handle.is_valid() {
        return ExpressionSnapshot::Name { path: Vec::new() };
    }

    match syntax_trees.expressions.expression(handle) {
        ExpressionNode::ArrayLiteral(values) => ExpressionSnapshot::ArrayLiteral {
            values: syntax_trees
                .expressions
                .expression_handles(*values)
                .iter()
                .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                .collect(),
        },
        ExpressionNode::Binary(binary) => ExpressionSnapshot::Binary {
            left: Box::new(snapshot_expression_handle(syntax_trees, binary.left)),
            operator: snapshot_binary_operator(binary.operator),
            right: Box::new(snapshot_expression_handle(syntax_trees, binary.right)),
        },
        ExpressionNode::Boolean(value) => ExpressionSnapshot::Boolean { value: *value },
        ExpressionNode::Cast(cast) => ExpressionSnapshot::Cast {
            value: Box::new(snapshot_expression_handle(syntax_trees, cast.value)),
            target_type: snapshot_identifier_slice(
                syntax_trees
                    .expressions
                    .identifier_path_members(cast.target_type),
            ),
        },
        ExpressionNode::Call(call) => snapshot_call_expression(syntax_trees, call),
        ExpressionNode::Float(value) => ExpressionSnapshot::Float {
            text: value.as_str().to_owned(),
        },
        ExpressionNode::Indexed(indexed) => ExpressionSnapshot::Indexed {
            collection: Box::new(snapshot_expression_handle(syntax_trees, indexed.collection)),
            index: Box::new(snapshot_expression_handle(syntax_trees, indexed.index)),
        },
        ExpressionNode::Integer(value) => ExpressionSnapshot::Integer { value: *value },
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
        },
        ExpressionNode::Mutable(value) => ExpressionSnapshot::Mutable {
            value: Box::new(snapshot_expression_handle(syntax_trees, *value)),
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
        },
        ExpressionNode::SelfValue => ExpressionSnapshot::SelfValue,
        ExpressionNode::StructLiteral(value) => ExpressionSnapshot::StructLiteral {
            type_name: snapshot_identifier(&value.type_name),
            fields: syntax_trees
                .expressions
                .struct_fields(value.fields)
                .iter()
                .map(|field| snapshot_struct_field(syntax_trees, field))
                .collect(),
        },
        ExpressionNode::String(value) => ExpressionSnapshot::String {
            text: value.as_str().to_owned(),
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
        arguments: syntax_trees
            .expressions
            .expression_handles(call.arguments)
            .iter()
            .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
            .collect(),
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

fn snapshot_identifier(identifier: &Identifier) -> IdentifierSnapshot {
    let source_span = identifier.source_span();
    IdentifierSnapshot {
        text: identifier.as_str().to_owned(),
        source_id: source_span.source_id.0,
        start: source_span.span.start,
        end: source_span.span.end,
        source_backed: identifier.is_source_backed(),
    }
}

fn snapshot_identifier_slice(path: &[Identifier]) -> Vec<IdentifierSnapshot> {
    path.iter().map(snapshot_identifier).collect()
}
