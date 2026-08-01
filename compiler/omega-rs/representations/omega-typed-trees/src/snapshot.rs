use crate::TypedTrees;
use crate::data::{DataDefinition, DataMember};
use crate::domain::{DomainDefinition, ProofFact};
use crate::expression::{ExpressionHandle, ExpressionNode};
use crate::invariant::InvariantDefinition;
use crate::machine::{Machine, OwnedData};
use crate::name::Identifier;
use crate::operator::OperatorDefinition;
use crate::signature::{StateParameter, StateSignature};
use crate::state::State;
use crate::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use crate::trait_definition::TraitDefinition;
use crate::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use serde::Serialize;

#[cfg(test)]
mod tests;

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
                operators: program
                    .operators()
                    .iter()
                    .map(|operator| operator_snapshot(program, operator))
                    .collect(),
                traits: program
                    .traits()
                    .iter()
                    .map(|trait_definition| trait_definition_snapshot(program, trait_definition))
                    .collect(),
                wire_schemas: program
                    .wire_schemas()
                    .iter()
                    .map(|wire_schema| wire_schema_snapshot(program, wire_schema))
                    .collect(),
            },
            tables: TypedTableSnapshot {
                data_definition_count: program.data_definitions.len(),
                data_type_parameter_count: program.data_type_parameters.len(),
                data_member_count: program.data_members.len(),
                domain_definition_count: program.domain_definitions.len(),
                invariant_definition_count: program.invariant_definitions.len(),
                machine_count: program.machines.len(),
                operator_count: program.operators.len(),
                machine_owned_data_count: program.machine_owned_data.len(),
                machine_state_count: program.machine_states.len(),
                state_parameter_count: program.state_parameters.len(),
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
    pub operators: Vec<OperatorDefinitionSnapshot>,
    pub traits: Vec<TraitSnapshot>,
    pub wire_schemas: Vec<WireSchemaSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WireSchemaSnapshot {
    pub has_symbol: bool,
    pub name: String,
    pub encoding: Option<String>,
    pub members: Vec<WireMemberSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WireMemberSnapshot {
    Field {
        number: u64,
        name: String,
        type_reference: TypeReferenceSnapshot,
    },
    Reserved {
        number: u64,
    },
    Version {
        name: String,
        members: Vec<WireMemberSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypedTableSnapshot {
    pub data_definition_count: usize,
    pub data_type_parameter_count: usize,
    pub data_member_count: usize,
    pub domain_definition_count: usize,
    pub invariant_definition_count: usize,
    pub machine_count: usize,
    pub operator_count: usize,
    pub machine_owned_data_count: usize,
    pub machine_state_count: usize,
    pub state_parameter_count: usize,
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
pub struct OperatorDefinitionSnapshot {
    pub is_boundary: bool,
    pub has_symbol: bool,
    pub name: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_parameters: Vec<String>,
    pub type_parameters: Vec<String>,
    pub parameter_count: usize,
    pub has_return_type: bool,
    pub contract_count: usize,
    pub spelling: Option<&'static str>,
    pub token_count: usize,
}

fn operator_snapshot(
    program: &TypedTrees,
    operator: &OperatorDefinition,
) -> OperatorDefinitionSnapshot {
    OperatorDefinitionSnapshot {
        is_boundary: operator.is_boundary,
        has_symbol: operator.symbol.is_valid(),
        name: program
            .operator_path_members(operator.name)
            .iter()
            .map(ToString::to_string)
            .collect(),
        lifetime_parameters: operator
            .lifetime_parameters
            .iter()
            .map(ToString::to_string)
            .collect(),
        type_parameters: program
            .data_type_parameters
            .span_or_empty(operator.type_parameters)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        parameter_count: program
            .state_parameters
            .span_or_empty(operator.parameters)
            .len(),
        has_return_type: operator.return_type.is_valid(),
        contract_count: program
            .signature_contracts
            .span_or_empty(operator.contracts)
            .len(),
        spelling: operator.spelling.map(|spelling| spelling.symbol()),
        token_count: operator.token_count,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DataDefinitionSnapshot {
    pub name: String,
    pub supply: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_parameters: Vec<String>,
    pub type_parameters: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quotient: Option<QuotientDefinitionSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub retired_identities: Vec<u64>,
    pub members: Vec<DataMemberSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuotientDefinitionSnapshot {
    pub carrier: TypeReferenceSnapshot,
    pub relation: Vec<String>,
    pub relation_symbol: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataMemberSnapshot {
    Field {
        identity: Option<u64>,
        name: String,
        type_reference: TypeReferenceSnapshot,
    },
    Variant {
        identity: Option<u64>,
        name: String,
        payload: Vec<DataPayloadFieldSnapshot>,
        retired_payload_identities: Vec<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DataPayloadFieldSnapshot {
    pub identity: Option<u64>,
    pub name: String,
    pub type_reference: TypeReferenceSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomainDefinitionSnapshot {
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub type_parameters: Vec<DomainTypeParameterSnapshot>,
    pub target_type: TypeReferenceSnapshot,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub index_arguments: Vec<TypeReferenceSnapshot>,
    pub is_public: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub alias: Vec<DomainAliasConstituentSnapshot>,
    pub predicate_body: &'static str,
    pub semantic_id: u32,
    pub semantic_roles: DomainSemanticRolesSnapshot,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub establishment_routes: Vec<DomainEstablishmentRouteSnapshot>,
    pub facts: Vec<ProofFactSnapshot>,
    pub operators: Vec<OperatorDefinitionSnapshot>,
    pub body_token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomainTypeParameterSnapshot {
    pub name: String,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub const_type: Option<TypeReferenceSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomainAliasConstituentSnapshot {
    pub domain: Vec<String>,
    pub domain_symbol: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomainSemanticRolesSnapshot {
    pub denotation_dimension: Option<u32>,
    pub arithmetic_policy: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomainEstablishmentRouteSnapshot {
    pub kind: &'static str,
    pub source_symbol: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_symbol: Option<u32>,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_parameters: Vec<String>,
    pub type_parameters: Vec<String>,
    pub supply: MachineSupplySnapshot,
    pub termination: TerminationInterfaceSnapshot,
    pub decreases: Vec<ExpressionSnapshot>,
    pub decrease_order: Vec<String>,
    pub service_reaches: Vec<String>,
    pub invokes: Vec<String>,
    pub service_reach: Vec<String>,
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: Vec<SignatureContractSnapshot>,
    pub owned_data: Vec<OwnedDataSnapshot>,
    pub states: Vec<StateSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MachineSupplySnapshot {
    CheckedBody,
    Requirement,
    Boundary,
    Accepted,
    ExternalRealization { binding: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "interface", rename_all = "snake_case")]
pub enum TerminationInterfaceSnapshot {
    InternalDerived,
    Published {
        guarantee: TerminationGuaranteeSnapshot,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TerminationGuaranteeSnapshot {
    NoGuarantee,
    Terminates { premises: Vec<u32> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnedDataSnapshot {
    pub name: String,
    pub type_reference: TypeReferenceSnapshot,
    pub initial_value: Option<ExpressionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TraitSnapshot {
    pub name: String,
    pub is_boundary: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_parameters: Vec<String>,
    pub type_parameters: Vec<String>,
    pub invariants: Vec<ProofFactSnapshot>,
    pub requires: Vec<String>,
    pub machines: Vec<StateSignatureSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateSnapshot {
    pub name: String,
    pub parameters: Vec<StateParameterSnapshot>,
    pub return_type: Option<TypeReferenceSnapshot>,
    pub contracts: Vec<SignatureContractSnapshot>,
    pub statements: Vec<StatementSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateSignatureSnapshot {
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_parameters: Vec<String>,
    pub type_parameters: Vec<String>,
    pub is_default: bool,
    pub parameters: Vec<StateParameterSnapshot>,
    pub return_type: Option<TypeReferenceSnapshot>,
    pub service_reaches: Vec<String>,
    pub invokes: Vec<String>,
    pub service_reach: Vec<String>,
    pub suspends: bool,
    pub blocks: bool,
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
        machine_arguments: Vec<Vec<String>>,
        arguments: Vec<ExpressionSnapshot>,
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
    Unary {
        operator: String,
        operand: Box<ExpressionSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExpressionSnapshot {
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
    },
    Call {
        receiver: Option<Box<ExpressionSnapshot>>,
        target: String,
        machine_arguments: Vec<Vec<String>>,
        arguments: Vec<ExpressionSnapshot>,
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
        /// Canonical literal spelling (see `omega_core::literals::IntegerLiteral`) --
        /// snapshots stay anonymous like the nodes they mirror (D14).
        text: String,
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
        value: String,
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
        length: String,
    },
    Slice {
        element_type: Box<TypeReferenceSnapshot>,
    },
    Generic {
        base_name: String,
        lifetime_arguments: Vec<String>,
        arguments: Vec<TypeReferenceSnapshot>,
    },
    DynamicTrait {
        name: String,
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
    Domain {
        name: String,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        arguments: Vec<TypeReferenceSnapshot>,
        symbol: u32,
        semantic_id: u32,
        predicate_body: &'static str,
        semantic_roles: DomainSemanticRolesSnapshot,
        establishment_routes: Vec<DomainEstablishmentRouteSnapshot>,
    },
    Range {
        minimum: ExpressionSnapshot,
        maximum: ExpressionSnapshot,
    },
    ArithmeticDomain {
        domain: String,
    },
}

fn data_definition_snapshot(program: &TypedTrees, data: &DataDefinition) -> DataDefinitionSnapshot {
    DataDefinitionSnapshot {
        name: data.name.to_string(),
        supply: match data.supply_mode {
            omega_core::semantics::DataSupplyMode::CheckedShape => "checked_shape",
            omega_core::semantics::DataSupplyMode::BoundaryOpaque => "boundary_opaque",
        }
        .to_owned(),
        lifetime_parameters: data
            .lifetime_parameters
            .iter()
            .map(ToString::to_string)
            .collect(),
        type_parameters: program
            .data_type_parameters(data)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        quotient: data
            .quotient
            .as_ref()
            .map(|quotient| QuotientDefinitionSnapshot {
                carrier: type_reference_snapshot(program, quotient.carrier),
                relation: quotient.relation.iter().map(ToString::to_string).collect(),
                relation_symbol: quotient.relation_symbol.arena_index(),
            }),
        retired_identities: data.retired_identities.clone(),
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
            identity: field.identity,
            name: field.name.to_string(),
            type_reference: type_reference_snapshot(program, field.type_reference),
        },
        DataMember::Variant(variant) => DataMemberSnapshot::Variant {
            identity: variant.identity,
            name: variant.name.to_string(),
            payload: program
                .data_payload_fields(variant)
                .iter()
                .map(|field| DataPayloadFieldSnapshot {
                    identity: field.identity,
                    name: field.name.to_string(),
                    type_reference: type_reference_snapshot(program, field.type_reference),
                })
                .collect(),
            retired_payload_identities: variant.retired_payload_identities.clone(),
        },
    }
}

fn domain_definition_snapshot(
    program: &TypedTrees,
    domain: &DomainDefinition,
) -> DomainDefinitionSnapshot {
    DomainDefinitionSnapshot {
        name: domain.name.to_string(),
        type_parameters: program
            .domain_type_parameters(domain)
            .iter()
            .map(|parameter| DomainTypeParameterSnapshot {
                name: parameter.name.to_string(),
                kind: match parameter.kind {
                    crate::data::TypeParameterKind::Type => "type",
                    crate::data::TypeParameterKind::Const { .. } => "const",
                    crate::data::TypeParameterKind::Machine { .. } => "machine",
                },
                const_type: match parameter.kind {
                    crate::data::TypeParameterKind::Const { type_reference } => {
                        Some(type_reference_snapshot(program, type_reference))
                    }
                    crate::data::TypeParameterKind::Type
                    | crate::data::TypeParameterKind::Machine { .. } => None,
                },
            })
            .collect(),
        target_type: type_reference_snapshot(program, domain.target_type),
        index_arguments: domain
            .index_arguments
            .iter()
            .map(|argument| type_reference_snapshot(program, *argument))
            .collect(),
        is_public: domain.is_public,
        alias: domain
            .alias
            .as_ref()
            .map(|alias| {
                alias
                    .constituents
                    .iter()
                    .map(|constituent| DomainAliasConstituentSnapshot {
                        domain: program
                            .domain_path_members(constituent.domain)
                            .iter()
                            .map(ToString::to_string)
                            .collect(),
                        domain_symbol: constituent.domain_symbol.arena_index(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        predicate_body: domain.predicate_body.as_str(),
        semantic_id: domain.semantic_id.0,
        semantic_roles: semantic_roles_snapshot(domain.semantic_roles),
        establishment_routes: domain
            .establishment_routes
            .iter()
            .copied()
            .map(establishment_route_snapshot)
            .collect(),
        facts: domain_fact_snapshots(program, domain),
        operators: program
            .domain_operators(domain)
            .iter()
            .map(|operator| operator_snapshot(program, operator))
            .collect(),
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
        lifetime_parameters: machine
            .lifetime_parameters
            .iter()
            .map(ToString::to_string)
            .collect(),
        type_parameters: program
            .machine_type_parameters(machine)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        supply: machine_supply_snapshot(machine.supply_mode),
        termination: termination_interface_snapshot(&machine.termination_plan.interface),
        decreases: program
            .expression_table
            .expression_handles(machine.decreases)
            .iter()
            .map(|handle| expression_snapshot(program, *handle))
            .collect(),
        decrease_order: program
            .machine_decrease_order(machine.decrease_order)
            .iter()
            .map(ToString::to_string)
            .collect(),
        service_reaches: program
            .machine_service_reaches(machine)
            .iter()
            .map(ToString::to_string)
            .collect(),
        invokes: program
            .machine_invokes(machine)
            .iter()
            .map(ToString::to_string)
            .collect(),
        service_reach: service_reach_names(program, machine.service_reach_row),
        suspends: machine.suspends,
        blocks: machine.blocks,
        contracts: program
            .machine_contracts(machine)
            .iter()
            .map(|contract| signature_contract_snapshot(program, contract))
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

fn machine_supply_snapshot(
    supply: omega_core::semantics::MachineSupplyMode,
) -> MachineSupplySnapshot {
    use omega_core::semantics::MachineSupplyMode;
    match supply {
        MachineSupplyMode::CheckedBody => MachineSupplySnapshot::CheckedBody,
        MachineSupplyMode::Requirement => MachineSupplySnapshot::Requirement,
        MachineSupplyMode::Boundary => MachineSupplySnapshot::Boundary,
        MachineSupplyMode::Accepted => MachineSupplySnapshot::Accepted,
        MachineSupplyMode::ExternalRealization { binding } => {
            MachineSupplySnapshot::ExternalRealization { binding: binding.0 }
        }
    }
}

fn termination_interface_snapshot(
    interface: &omega_core::semantics::TerminationInterface,
) -> TerminationInterfaceSnapshot {
    use omega_core::semantics::{TerminationGuarantee, TerminationInterface};
    match interface {
        TerminationInterface::InternalDerived => TerminationInterfaceSnapshot::InternalDerived,
        TerminationInterface::Published(TerminationGuarantee::NoGuarantee) => {
            TerminationInterfaceSnapshot::Published {
                guarantee: TerminationGuaranteeSnapshot::NoGuarantee,
            }
        }
        TerminationInterface::Published(TerminationGuarantee::Terminates { premises }) => {
            TerminationInterfaceSnapshot::Published {
                guarantee: TerminationGuaranteeSnapshot::Terminates {
                    premises: premises.iter().map(|premise| premise.0).collect(),
                },
            }
        }
    }
}

fn owned_data_snapshot(program: &TypedTrees, owned: &OwnedData) -> OwnedDataSnapshot {
    OwnedDataSnapshot {
        name: owned.name.to_string(),
        type_reference: type_reference_snapshot(program, owned.type_reference),
        initial_value: expression_snapshot_option(program, owned.initial_value),
    }
}

fn trait_definition_snapshot(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
) -> TraitSnapshot {
    TraitSnapshot {
        name: trait_definition.name.to_string(),
        is_boundary: trait_definition.is_boundary,
        lifetime_parameters: trait_definition
            .lifetime_parameters
            .iter()
            .map(ToString::to_string)
            .collect(),
        type_parameters: program
            .trait_type_parameters(trait_definition)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        invariants: contract_fact_snapshots(program, trait_definition.invariants),
        requires: program
            .trait_requirements(trait_definition)
            .iter()
            .map(|requirement| {
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(requirement.arguments);
                if arguments.is_empty() {
                    requirement.name.to_string()
                } else {
                    format!(
                        "{}<{}>",
                        requirement.name,
                        arguments
                            .iter()
                            .map(|argument| program.display_type_reference(*argument))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            })
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
        contracts: program
            .state_contracts(state)
            .iter()
            .map(|contract| signature_contract_snapshot(program, contract))
            .collect(),
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
        lifetime_parameters: signature
            .lifetime_parameters
            .iter()
            .map(ToString::to_string)
            .collect(),
        type_parameters: program
            .state_signature_type_parameters(signature)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        is_default: signature.is_default,
        parameters: program
            .state_signature_parameters(signature)
            .iter()
            .map(|parameter| state_parameter_snapshot(program, parameter))
            .collect(),
        return_type: type_reference_snapshot_option(program, signature.return_type),
        service_reaches: program
            .state_signature_service_reaches(signature)
            .iter()
            .map(ToString::to_string)
            .collect(),
        invokes: program
            .state_signature_invokes(signature)
            .iter()
            .map(ToString::to_string)
            .collect(),
        service_reach: service_reach_names(program, signature.service_reach_row),
        suspends: signature.suspends,
        blocks: signature.blocks,
        contracts: program
            .state_signature_contracts(signature)
            .iter()
            .map(|contract| signature_contract_snapshot(program, contract))
            .collect(),
    }
}

fn service_reach_names(
    program: &TypedTrees,
    row: omega_core::semantics::ServiceReachRowId,
) -> Vec<String> {
    program
        .service_reach_rows
        .services(row)
        .iter()
        .filter_map(|service| program.service_reaches.definition(*service))
        .map(|definition| definition.name.clone())
        .collect()
}

fn signature_contract_snapshot(
    program: &TypedTrees,
    contract: &crate::signature::SignatureContract,
) -> SignatureContractSnapshot {
    SignatureContractSnapshot {
        kind: match contract.kind {
            crate::signature::SignatureContractKind::Requires => "requires",
            crate::signature::SignatureContractKind::Ensures => "ensures",
            crate::signature::SignatureContractKind::Boundary => "boundary",
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
                .map(|argument| path_snapshot(&argument.path))
                .collect(),
            arguments: statement_expression_span_snapshot(program, call.arguments),
            acknowledgement_synthesized: call.operational_acknowledgement.origin
                == omega_core::semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
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
            target_type: Box::new(type_reference_snapshot(program, cast.target_type)),
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
                .map(|argument| path_snapshot(&argument.path))
                .collect(),
            arguments: expression_span_snapshot(program, call.arguments),
            acknowledgement_synthesized: call.operational_acknowledgement.origin
                == omega_core::semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
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
        ExpressionNode::Mutable(value) => ExpressionSnapshot::Mutable {
            value: Box::new(expression_snapshot(program, *value)),
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
            value: value.to_string(),
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

fn wire_schema_snapshot(
    program: &TypedTrees,
    wire_schema: &crate::wire::WireSchema,
) -> WireSchemaSnapshot {
    WireSchemaSnapshot {
        has_symbol: wire_schema.symbol.is_valid(),
        name: wire_schema.name.to_string(),
        encoding: wire_schema
            .encoding
            .as_ref()
            .map(|encoding| encoding.to_string()),
        members: wire_member_snapshots(program, wire_schema.members),
    }
}

fn wire_member_snapshots(
    program: &TypedTrees,
    members: omega_core::arena::HandleSpan<crate::wire::WireMember>,
) -> Vec<WireMemberSnapshot> {
    program
        .wire_members(members)
        .iter()
        .map(|member| match member {
            crate::wire::WireMember::Field(field) => WireMemberSnapshot::Field {
                number: field.number,
                name: field.name.to_string(),
                type_reference: type_reference_snapshot(program, field.type_reference),
            },
            crate::wire::WireMember::Reserved(reserved) => WireMemberSnapshot::Reserved {
                number: reserved.number,
            },
            crate::wire::WireMember::Version(version) => WireMemberSnapshot::Version {
                name: version.name.to_string(),
                members: wire_member_snapshots(program, version.members),
            },
        })
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
            // Lifetime omitted from the structural snapshot (a borrow-region tag,
            // not part of the type's shape).
            lifetime: _,
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
            length: length.to_string(),
        },
        TypeReferenceNode::Slice { element_type } => TypeReferenceSnapshot::Slice {
            element_type: Box::new(type_reference_snapshot(program, *element_type)),
        },
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
            ..
        } => TypeReferenceSnapshot::Generic {
            base_name: base_name.to_string(),
            lifetime_arguments: lifetime_arguments.iter().map(ToString::to_string).collect(),
            arguments: program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| type_reference_snapshot(program, *argument))
                .collect(),
        },
        TypeReferenceNode::DynamicTrait { name, .. } => TypeReferenceSnapshot::DynamicTrait {
            name: name.to_string(),
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
        TypeConstraintNode::Domain(domain) => TypeConstraintSnapshot::Domain {
            name: domain.name.to_string(),
            arguments: domain
                .arguments
                .iter()
                .map(|argument| type_reference_snapshot(program, *argument))
                .collect(),
            symbol: domain.symbol.arena_index(),
            semantic_id: domain.semantic_id.0,
            predicate_body: domain.predicate_body.as_str(),
            semantic_roles: semantic_roles_snapshot(domain.semantic_roles),
            establishment_routes: domain
                .establishment_routes
                .iter()
                .copied()
                .map(establishment_route_snapshot)
                .collect(),
        },
        TypeConstraintNode::Range { minimum, maximum } => TypeConstraintSnapshot::Range {
            minimum: expression_snapshot(program, *minimum),
            maximum: expression_snapshot(program, *maximum),
        },
        TypeConstraintNode::ArithmeticDomain(domain) => TypeConstraintSnapshot::ArithmeticDomain {
            domain: domain.name().to_owned(),
        },
    }
}

fn semantic_roles_snapshot(
    roles: omega_core::semantics::DomainSemanticRoles,
) -> DomainSemanticRolesSnapshot {
    DomainSemanticRolesSnapshot {
        denotation_dimension: roles.denotation_dimension.map(|semantic| semantic.0),
        arithmetic_policy: roles.arithmetic_policy.map(|semantic| semantic.0),
    }
}

fn establishment_route_snapshot(
    route: omega_core::semantics::DomainEstablishmentRoute,
) -> DomainEstablishmentRouteSnapshot {
    let requirement = route.requirement_symbol();
    DomainEstablishmentRouteSnapshot {
        kind: route.kind_name(),
        source_symbol: route.source_symbol().arena_index(),
        requirement_symbol: requirement.is_valid().then(|| requirement.arena_index()),
    }
}

fn path_snapshot(path: &[Identifier]) -> Vec<String> {
    path.iter().map(ToString::to_string).collect()
}

#[cfg(test)]
mod termination_vocabulary_tests {
    use super::TerminationGuaranteeSnapshot;

    #[test]
    fn termination_guarantee_uses_settled_snapshot_vocabulary() {
        let snapshot = TerminationGuaranteeSnapshot::Terminates {
            premises: vec![3, 5],
        };
        assert_eq!(
            serde_json::to_string(&snapshot).expect("serialize termination guarantee"),
            r#"{"kind":"terminates","premises":[3,5]}"#
        );
    }
}
