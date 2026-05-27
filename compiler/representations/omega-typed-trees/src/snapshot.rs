use crate::TypedTrees;
use crate::data::{DataDefinition, DataMember};
use crate::domain::{DomainDefinition, ProofFact};
use crate::expression::{ExpressionHandle, ExpressionNode};
use crate::invariant::InvariantDefinition;
use crate::machine::{Machine, OwnedData};
use crate::name::Identifier;
use crate::platform::Platform;
use crate::signature::{StateParameter, StateSignature};
use crate::state::State;
use crate::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use crate::trait_definition::TraitDefinition;
use crate::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypedTreesSnapshot {
    pub roots: TypedRootsSnapshot,
    pub tables: TypedTableSnapshot,
}

impl TypedTreesSnapshot {
    pub fn from_typed_trees(program: &TypedTrees) -> Self {
        Self {
            roots: TypedRootsSnapshot {
                data_definitions: program
                    .data_definitions()
                    .iter()
                    .map(|data| data_definition_snapshot(program, data))
                    .collect(),
                domain_definitions: program
                    .domain_definitions()
                    .iter()
                    .map(|domain| domain_definition_snapshot(program, domain))
                    .collect(),
                invariant_definitions: program
                    .invariant_definitions()
                    .iter()
                    .map(|invariant| invariant_definition_snapshot(program, invariant))
                    .collect(),
                machines: program
                    .machines()
                    .iter()
                    .map(|machine| machine_snapshot(program, machine))
                    .collect(),
                platforms: program
                    .platforms()
                    .iter()
                    .map(|platform| platform_snapshot(program, platform))
                    .collect(),
                traits: program
                    .traits()
                    .iter()
                    .map(|trait_definition| trait_definition_snapshot(program, trait_definition))
                    .collect(),
            },
            tables: TypedTableSnapshot {
                data_definition_count: program.data_definitions.len(),
                data_type_parameter_count: program.data_type_parameters.len(),
                data_member_count: program.data_members.len(),
                domain_definition_count: program.domain_definitions.len(),
                invariant_definition_count: program.invariant_definitions.len(),
                machine_count: program.machines.len(),
                machine_contained_object_count: program.machine_contained_objects.len(),
                machine_owned_data_count: program.machine_owned_data.len(),
                machine_state_count: program.machine_states.len(),
                state_parameter_count: program.state_parameters.len(),
                platform_count: program.platforms.len(),
                platform_state_signature_count: program.platform_state_signatures.len(),
                trait_count: program.traits.len(),
                trait_requirement_count: program.trait_requirements.len(),
                trait_machine_signature_count: program.trait_machine_signatures.len(),
                expression_count: program.expression_table.expression_count(),
                expression_struct_field_count: program.expression_table.struct_field_count(),
                statement_count: program.statement_table.statement_count(),
                transition_target_count: program.statement_table.transition_target_count(),
                type_reference_count: program.type_reference_table.type_reference_count(),
                type_constraint_count: program.type_reference_table.constraint_count(),
            },
        }
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypedRootsSnapshot {
    pub data_definitions: Vec<DataDefinitionSnapshot>,
    pub domain_definitions: Vec<DomainDefinitionSnapshot>,
    pub invariant_definitions: Vec<InvariantDefinitionSnapshot>,
    pub machines: Vec<MachineSnapshot>,
    pub platforms: Vec<PlatformSnapshot>,
    pub traits: Vec<TraitSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypedTableSnapshot {
    pub data_definition_count: usize,
    pub data_type_parameter_count: usize,
    pub data_member_count: usize,
    pub domain_definition_count: usize,
    pub invariant_definition_count: usize,
    pub machine_count: usize,
    pub machine_contained_object_count: usize,
    pub machine_owned_data_count: usize,
    pub machine_state_count: usize,
    pub state_parameter_count: usize,
    pub platform_count: usize,
    pub platform_state_signature_count: usize,
    pub trait_count: usize,
    pub trait_requirement_count: usize,
    pub trait_machine_signature_count: usize,
    pub expression_count: usize,
    pub expression_struct_field_count: usize,
    pub statement_count: usize,
    pub transition_target_count: usize,
    pub type_reference_count: usize,
    pub type_constraint_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DataDefinitionSnapshot {
    pub name: String,
    pub type_parameters: Vec<String>,
    pub members: Vec<DataMemberSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataMemberSnapshot {
    Field {
        name: String,
        type_reference: TypeReferenceSnapshot,
    },
    Variant {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomainDefinitionSnapshot {
    pub name: String,
    pub target_type: TypeReferenceSnapshot,
    pub facts: Vec<ProofFactSnapshot>,
    pub body_token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProofFactSnapshot {
    Expression {
        value: ExpressionSnapshot,
    },
    Membership {
        value: ExpressionSnapshot,
        domain: Vec<String>,
        domain_symbol: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InvariantDefinitionSnapshot {
    pub name: String,
    pub constraints: Vec<TypeConstraintSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MachineSnapshot {
    pub name: String,
    pub attached_data: Option<String>,
    pub terminates: bool,
    pub decreases: Vec<ExpressionSnapshot>,
    pub effects: Vec<String>,
    pub contracts: Vec<SignatureContractSnapshot>,
    pub contains: Vec<ContainedObjectSnapshot>,
    pub owned_data: Vec<OwnedDataSnapshot>,
    pub states: Vec<StateSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContainedObjectSnapshot {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnedDataSnapshot {
    pub name: String,
    pub type_reference: TypeReferenceSnapshot,
    pub initial_value: Option<ExpressionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlatformSnapshot {
    pub name: String,
    pub states: Vec<StateSignatureSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TraitSnapshot {
    pub name: String,
    pub is_boundary: bool,
    pub requires: Vec<String>,
    pub machines: Vec<StateSignatureSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateSnapshot {
    pub name: String,
    pub parameters: Vec<StateParameterSnapshot>,
    pub return_type: Option<TypeReferenceSnapshot>,
    pub statements: Vec<StatementSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateSignatureSnapshot {
    pub name: String,
    pub parameters: Vec<StateParameterSnapshot>,
    pub return_type: Option<TypeReferenceSnapshot>,
    pub effects: Vec<String>,
    pub contracts: Vec<SignatureContractSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignatureContractSnapshot {
    pub kind: &'static str,
    pub facts: Vec<ProofFactSnapshot>,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateParameterSnapshot {
    pub name: String,
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
        receiver: Option<Vec<String>>,
        target: String,
        arguments: Vec<ExpressionSnapshot>,
    },
    Expression {
        value: ExpressionSnapshot,
    },
    LocalData {
        name: String,
        type_reference: TypeReferenceSnapshot,
        initial_value: Option<ExpressionSnapshot>,
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
    When { value: ExpressionSnapshot },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransitionTargetSnapshot {
    Named {
        path: Vec<String>,
        arguments: Vec<ExpressionSnapshot>,
    },
    Value {
        value: ExpressionSnapshot,
    },
    SelfTarget,
    Terminal,
    Invalid {
        handle: u32,
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
        operator: String,
        right: Box<ExpressionSnapshot>,
    },
    Boolean {
        value: bool,
    },
    Cast {
        value: Box<ExpressionSnapshot>,
        target_type: Vec<String>,
    },
    Call {
        receiver: Option<Box<ExpressionSnapshot>>,
        target: String,
        arguments: Vec<ExpressionSnapshot>,
    },
    Float {
        value: String,
    },
    Indexed {
        collection: Box<ExpressionSnapshot>,
        index: Box<ExpressionSnapshot>,
    },
    Integer {
        value: i64,
    },
    Member {
        receiver: Box<ExpressionSnapshot>,
        member: String,
    },
    Mutable {
        value: Box<ExpressionSnapshot>,
    },
    Name {
        path: Vec<String>,
    },
    StructLiteral {
        type_name: String,
        fields: Vec<StructLiteralFieldSnapshot>,
    },
    String {
        value: String,
    },
    Invalid {
        handle: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StructLiteralFieldSnapshot {
    pub name: String,
    pub value: ExpressionSnapshot,
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
        base_name: String,
        arguments: Vec<TypeReferenceSnapshot>,
    },
    Named {
        name: String,
    },
    Unit,
    Invalid {
        handle: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeConstraintSnapshot {
    Named {
        name: String,
    },
    Range {
        minimum: ExpressionSnapshot,
        maximum: ExpressionSnapshot,
    },
}

fn data_definition_snapshot(program: &TypedTrees, data: &DataDefinition) -> DataDefinitionSnapshot {
    DataDefinitionSnapshot {
        name: data.name.to_string(),
        type_parameters: program
            .data_type_parameters(data)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        members: program
            .data_members(data)
            .iter()
            .map(|member| data_member_snapshot(program, member))
            .collect(),
    }
}

fn data_member_snapshot(program: &TypedTrees, member: &DataMember) -> DataMemberSnapshot {
    match member {
        DataMember::Field(field) => DataMemberSnapshot::Field {
            name: field.name.to_string(),
            type_reference: type_reference_snapshot(program, field.type_reference),
        },
        DataMember::Variant(variant) => DataMemberSnapshot::Variant {
            name: variant.name.to_string(),
        },
    }
}

fn domain_definition_snapshot(
    program: &TypedTrees,
    domain: &DomainDefinition,
) -> DomainDefinitionSnapshot {
    DomainDefinitionSnapshot {
        name: domain.name.to_string(),
        target_type: type_reference_snapshot(program, domain.target_type),
        facts: domain_fact_snapshots(program, domain),
        body_token_count: domain.body_token_count,
    }
}

fn domain_fact_snapshots(
    program: &TypedTrees,
    domain: &DomainDefinition,
) -> Vec<ProofFactSnapshot> {
    program
        .proof_facts(domain)
        .iter()
        .map(|fact| match fact {
            ProofFact::Expression(expression) => ProofFactSnapshot::Expression {
                value: expression_snapshot(program, *expression),
            },
            ProofFact::Membership(membership) => ProofFactSnapshot::Membership {
                value: expression_snapshot(program, membership.value),
                domain: program
                    .domain_path_members(membership.domain)
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                domain_symbol: membership.domain_symbol.arena_index(),
            },
        })
        .collect()
}

fn invariant_definition_snapshot(
    program: &TypedTrees,
    invariant: &InvariantDefinition,
) -> InvariantDefinitionSnapshot {
    InvariantDefinitionSnapshot {
        name: invariant.name.to_string(),
        constraints: program
            .type_reference_table
            .constraints(invariant.constraints)
            .iter()
            .map(|constraint| type_constraint_snapshot(program, constraint))
            .collect(),
    }
}

fn machine_snapshot(program: &TypedTrees, machine: &Machine) -> MachineSnapshot {
    MachineSnapshot {
        name: machine.name.to_string(),
        attached_data: machine.attached_data.as_ref().map(ToString::to_string),
        terminates: machine.terminates,
        decreases: program
            .expression_table
            .expression_handles(machine.decreases)
            .iter()
            .map(|handle| expression_snapshot(program, *handle))
            .collect(),
        effects: program
            .machine_effects(machine)
            .iter()
            .map(ToString::to_string)
            .collect(),
        contracts: program
            .machine_contracts(machine)
            .iter()
            .map(|contract| signature_contract_snapshot(program, contract))
            .collect(),
        contains: program
            .machine_contained_objects(machine)
            .iter()
            .map(|contained| ContainedObjectSnapshot {
                name: contained.name.to_string(),
                type_name: contained.type_name.to_string(),
            })
            .collect(),
        owned_data: program
            .machine_owned_data(machine)
            .iter()
            .map(|owned| owned_data_snapshot(program, owned))
            .collect(),
        states: program
            .machine_states(machine)
            .iter()
            .map(|state| state_snapshot(program, state))
            .collect(),
    }
}

fn owned_data_snapshot(program: &TypedTrees, owned: &OwnedData) -> OwnedDataSnapshot {
    OwnedDataSnapshot {
        name: owned.name.to_string(),
        type_reference: type_reference_snapshot(program, owned.type_reference),
        initial_value: expression_snapshot_option(program, owned.initial_value),
    }
}

fn platform_snapshot(program: &TypedTrees, platform: &Platform) -> PlatformSnapshot {
    PlatformSnapshot {
        name: platform.name.to_string(),
        states: program
            .platform_state_signatures(platform)
            .iter()
            .map(|signature| state_signature_snapshot(program, signature))
            .collect(),
    }
}

fn trait_definition_snapshot(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
) -> TraitSnapshot {
    TraitSnapshot {
        name: trait_definition.name.to_string(),
        is_boundary: trait_definition.is_boundary,
        requires: program
            .trait_requirements(trait_definition)
            .iter()
            .map(|requirement| requirement.name.to_string())
            .collect(),
        machines: program
            .trait_machine_signatures(trait_definition)
            .iter()
            .map(|signature| state_signature_snapshot(program, signature))
            .collect(),
    }
}

fn state_snapshot(program: &TypedTrees, state: &State) -> StateSnapshot {
    StateSnapshot {
        name: state.name.to_string(),
        parameters: program
            .state_parameters(state)
            .iter()
            .map(|parameter| state_parameter_snapshot(program, parameter))
            .collect(),
        return_type: type_reference_snapshot_option(program, state.return_type),
        statements: program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .map(|statement| statement_snapshot(program, statement))
            .collect(),
    }
}

fn state_signature_snapshot(
    program: &TypedTrees,
    signature: &StateSignature,
) -> StateSignatureSnapshot {
    StateSignatureSnapshot {
        name: signature.name.to_string(),
        parameters: program
            .state_signature_parameters(signature)
            .iter()
            .map(|parameter| state_parameter_snapshot(program, parameter))
            .collect(),
        return_type: type_reference_snapshot_option(program, signature.return_type),
        effects: program
            .state_signature_effects(signature)
            .iter()
            .map(ToString::to_string)
            .collect(),
        contracts: program
            .state_signature_contracts(signature)
            .iter()
            .map(|contract| signature_contract_snapshot(program, contract))
            .collect(),
    }
}

fn signature_contract_snapshot(
    program: &TypedTrees,
    contract: &crate::signature::SignatureContract,
) -> SignatureContractSnapshot {
    SignatureContractSnapshot {
        kind: match contract.kind {
            crate::signature::SignatureContractKind::Requires => "requires",
            crate::signature::SignatureContractKind::Ensures => "ensures",
            crate::signature::SignatureContractKind::Trusted => "trusted",
        },
        facts: contract_fact_snapshots(program, contract.facts),
        token_count: contract.token_count,
    }
}

fn contract_fact_snapshots(
    program: &TypedTrees,
    facts: omega_core::arena::HandleSpan<ProofFact>,
) -> Vec<ProofFactSnapshot> {
    program
        .proof_facts
        .span_or_empty(facts)
        .iter()
        .map(|fact| match fact {
            ProofFact::Expression(expression) => ProofFactSnapshot::Expression {
                value: expression_snapshot(program, *expression),
            },
            ProofFact::Membership(membership) => ProofFactSnapshot::Membership {
                value: expression_snapshot(program, membership.value),
                domain: program
                    .domain_path_members(membership.domain)
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                domain_symbol: membership.domain_symbol.arena_index(),
            },
        })
        .collect()
}

fn state_parameter_snapshot(
    program: &TypedTrees,
    parameter: &StateParameter,
) -> StateParameterSnapshot {
    StateParameterSnapshot {
        name: parameter.name.to_string(),
        type_reference: type_reference_snapshot(program, parameter.type_reference),
        is_const: parameter.is_const,
        is_mutable: parameter.is_mutable,
        is_self: parameter.is_self,
    }
}

fn statement_snapshot(program: &TypedTrees, statement: &StatementNode) -> StatementSnapshot {
    match statement {
        StatementNode::Assignment(assignment) => StatementSnapshot::Assignment {
            target: expression_snapshot(program, assignment.target),
            value: expression_snapshot(program, assignment.value),
        },
        StatementNode::Call(call) => StatementSnapshot::Call {
            receiver: (!call.receiver.is_empty())
                .then(|| path_snapshot(program.statement_table.name_path_members(call.receiver))),
            target: call.target.to_string(),
            arguments: statement_expression_span_snapshot(program, call.arguments),
        },
        StatementNode::Expression(expression) => StatementSnapshot::Expression {
            value: expression_snapshot(program, *expression),
        },
        StatementNode::LocalData(local) => StatementSnapshot::LocalData {
            name: local.name.to_string(),
            type_reference: type_reference_snapshot(program, local.type_reference),
            initial_value: expression_snapshot_option(program, local.initial_value),
        },
        StatementNode::Transition(transition) => StatementSnapshot::Transition {
            target: transition_target_snapshot(program, transition.target),
            continuation: transition
                .continuation
                .is_valid()
                .then(|| transition_target_snapshot(program, transition.continuation)),
            guard: transition_guard_snapshot(program, transition.guard),
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
        TransitionTargetNode::Named { path, arguments } => TransitionTargetSnapshot::Named {
            path: path_snapshot(program.statement_table.name_path_members(path.members)),
            arguments: statement_expression_span_snapshot(program, *arguments),
        },
        TransitionTargetNode::Value(value) => TransitionTargetSnapshot::Value {
            value: expression_snapshot(program, *value),
        },
        TransitionTargetNode::SelfTarget => TransitionTargetSnapshot::SelfTarget,
        TransitionTargetNode::Terminal => TransitionTargetSnapshot::Terminal,
    }
}

fn expression_snapshot_option(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionSnapshot> {
    expression
        .is_valid()
        .then(|| expression_snapshot(program, expression))
}

fn expression_snapshot(program: &TypedTrees, expression: ExpressionHandle) -> ExpressionSnapshot {
    if !expression.is_valid() {
        return ExpressionSnapshot::Invalid {
            handle: expression.arena_index(),
        };
    }

    match program.expression_table.expression(expression) {
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
            target_type: path_snapshot(
                program.expression_table.name_path_members(cast.target_type),
            ),
        },
        ExpressionNode::Call(call) => ExpressionSnapshot::Call {
            receiver: call
                .receiver
                .is_valid()
                .then(|| Box::new(expression_snapshot(program, call.receiver))),
            target: call.target.to_string(),
            arguments: expression_span_snapshot(program, call.arguments),
        },
        ExpressionNode::Float(value) => ExpressionSnapshot::Float {
            value: value.to_string(),
        },
        ExpressionNode::Indexed(indexed) => ExpressionSnapshot::Indexed {
            collection: Box::new(expression_snapshot(program, indexed.collection)),
            index: Box::new(expression_snapshot(program, indexed.index)),
        },
        ExpressionNode::Integer(value) => ExpressionSnapshot::Integer { value: *value },
        ExpressionNode::Member(member) => ExpressionSnapshot::Member {
            receiver: Box::new(expression_snapshot(program, member.receiver)),
            member: member.member.to_string(),
        },
        ExpressionNode::Mutable(value) => ExpressionSnapshot::Mutable {
            value: Box::new(expression_snapshot(program, *value)),
        },
        ExpressionNode::Name(path) => ExpressionSnapshot::Name {
            path: path_snapshot(program.expression_table.name_path_members(path.members)),
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
        ExpressionNode::String(value) => ExpressionSnapshot::String {
            value: value.to_string(),
        },
    }
}

fn expression_span_snapshot(
    program: &TypedTrees,
    expressions: omega_core::arena::HandleSpan<ExpressionHandle>,
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
    expressions: omega_core::arena::HandleSpan<ExpressionHandle>,
) -> Vec<ExpressionSnapshot> {
    program
        .statement_table
        .expression_handles(expressions)
        .iter()
        .map(|expression| expression_snapshot(program, *expression))
        .collect()
}

fn type_reference_snapshot_option(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceSnapshot> {
    type_reference
        .is_valid()
        .then(|| type_reference_snapshot(program, type_reference))
}

fn type_reference_snapshot(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> TypeReferenceSnapshot {
    if !type_reference.is_valid() {
        return TypeReferenceSnapshot::Invalid {
            handle: type_reference.arena_index(),
        };
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee,
            is_mutable,
        } => TypeReferenceSnapshot::Reference {
            referee: Box::new(type_reference_snapshot(program, *referee)),
            is_mutable: *is_mutable,
        },
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => TypeReferenceSnapshot::Constrained {
            base_type: Box::new(type_reference_snapshot(program, *base_type)),
            constraints: program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .map(|constraint| type_constraint_snapshot(program, constraint))
                .collect(),
        },
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => TypeReferenceSnapshot::FixedArray {
            element_type: Box::new(type_reference_snapshot(program, *element_type)),
            length: *length,
        },
        TypeReferenceNode::Slice { element_type } => TypeReferenceSnapshot::Slice {
            element_type: Box::new(type_reference_snapshot(program, *element_type)),
        },
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => TypeReferenceSnapshot::Generic {
            base_name: base_name.to_string(),
            arguments: program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| type_reference_snapshot(program, *argument))
                .collect(),
        },
        TypeReferenceNode::Named { name, .. } => TypeReferenceSnapshot::Named {
            name: name.to_string(),
        },
        TypeReferenceNode::Unit => TypeReferenceSnapshot::Unit,
    }
}

fn type_constraint_snapshot(
    program: &TypedTrees,
    constraint: &TypeConstraintNode,
) -> TypeConstraintSnapshot {
    match constraint {
        TypeConstraintNode::Named(name) => TypeConstraintSnapshot::Named {
            name: name.to_string(),
        },
        TypeConstraintNode::Range { minimum, maximum } => TypeConstraintSnapshot::Range {
            minimum: expression_snapshot(program, *minimum),
            maximum: expression_snapshot(program, *maximum),
        },
    }
}

fn path_snapshot(path: &[Identifier]) -> Vec<String> {
    path.iter().map(ToString::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::TypedTreesSnapshot;
    use crate::TypedTrees;

    #[test]
    fn snapshots_empty_typed_tree_as_json() {
        let program = TypedTrees::default();
        let snapshot = TypedTreesSnapshot::from_typed_trees(&program);

        assert_eq!(snapshot.roots.data_definitions.len(), 0);
        assert!(snapshot.to_json_pretty().is_ok());
    }
}
