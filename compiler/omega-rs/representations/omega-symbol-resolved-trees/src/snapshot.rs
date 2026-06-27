use crate::SymbolResolvedTrees;
use crate::data::{DataDefinition, DataMember};
use crate::domain::{DomainDefinition, ProofFact};
use crate::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use crate::invariant::InvariantDefinition;
use crate::machine::{Machine, OwnedData};
use crate::operator::OperatorDefinition;
use crate::platform::Platform;
use crate::signature::{StateParameter, StateSignature};
use crate::state::State;
use crate::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
use crate::trait_definition::TraitDefinition;
use crate::types::{TypeConstraint, TypeReference};
use crate::wire::{WireMember, WireSchema};
use omega_core::arena::HandleSpan;
use serde::Serialize;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolResolvedTreesSnapshot {
    pub roots: SymbolResolvedRootsSnapshot,
    pub tables: ResolvedTableSnapshot,
}

impl SymbolResolvedTreesSnapshot {
    pub fn from_symbol_resolved_trees(symbol_resolved_trees: &SymbolResolvedTrees) -> Self {
        Self {
            roots: SymbolResolvedRootsSnapshot {
                data_definitions: symbol_resolved_trees
                    .data_definitions
                    .iter()
                    .map(|data| data_definition_snapshot(symbol_resolved_trees, data))
                    .collect(),
                domain_definitions: symbol_resolved_trees
                    .domain_definitions
                    .iter()
                    .map(|domain| domain_definition_snapshot(symbol_resolved_trees, domain))
                    .collect(),
                invariant_definitions: symbol_resolved_trees
                    .invariant_definitions
                    .iter()
                    .map(|invariant| {
                        invariant_definition_snapshot(symbol_resolved_trees, invariant)
                    })
                    .collect(),
                machines: symbol_resolved_trees
                    .machines
                    .iter()
                    .map(|machine| machine_snapshot(symbol_resolved_trees, machine))
                    .collect(),
                measures: symbol_resolved_trees
                    .measures
                    .iter()
                    .map(|measure| measure_snapshot(symbol_resolved_trees, measure))
                    .collect(),
                operators: symbol_resolved_trees
                    .operators
                    .iter()
                    .map(|operator| operator_snapshot(symbol_resolved_trees, operator))
                    .collect(),
                platforms: symbol_resolved_trees
                    .platforms
                    .iter()
                    .map(|platform| platform_snapshot(symbol_resolved_trees, platform))
                    .collect(),
                traits: symbol_resolved_trees
                    .traits
                    .iter()
                    .map(|trait_definition| {
                        trait_definition_snapshot(symbol_resolved_trees, trait_definition)
                    })
                    .collect(),
                wire_schemas: symbol_resolved_trees
                    .wire_schemas
                    .iter()
                    .map(|wire_schema| wire_schema_snapshot(symbol_resolved_trees, wire_schema))
                    .collect(),
            },
            tables: ResolvedTableSnapshot {
                type_constraint_count: symbol_resolved_trees.tables.types.constraints.len(),
                expression_count: symbol_resolved_trees
                    .tables
                    .bodies
                    .expressions
                    .expression_count(),
                statement_count: symbol_resolved_trees
                    .tables
                    .bodies
                    .statements
                    .statement_count(),
                type_reference_count: symbol_resolved_trees
                    .tables
                    .types
                    .references
                    .type_reference_count(),
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
pub struct SymbolResolvedRootsSnapshot {
    pub data_definitions: Vec<DataDefinitionSnapshot>,
    pub domain_definitions: Vec<DomainDefinitionSnapshot>,
    pub invariant_definitions: Vec<InvariantDefinitionSnapshot>,
    pub machines: Vec<MachineSnapshot>,
    pub measures: Vec<MeasureDefinitionSnapshot>,
    pub operators: Vec<OperatorDefinitionSnapshot>,
    pub platforms: Vec<PlatformSnapshot>,
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
        number: i64,
        name: String,
        type_reference: TypeReferenceSnapshot,
    },
    Reserved {
        number: i64,
    },
    Version {
        name: String,
        members: Vec<WireMemberSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MeasureDefinitionSnapshot {
    pub has_symbol: bool,
    pub name: Vec<String>,
    pub has_parameter: bool,
    pub has_return_type: bool,
    pub lexicographic: bool,
    pub component_count: usize,
}

fn measure_snapshot(
    program: &SymbolResolvedTrees,
    measure: &crate::measure::MeasureDefinition,
) -> MeasureDefinitionSnapshot {
    MeasureDefinitionSnapshot {
        has_symbol: measure.symbol.is_valid(),
        name: program
            .measure_path_members(measure.name)
            .iter()
            .map(ToString::to_string)
            .collect(),
        has_parameter: measure.parameter.is_some(),
        has_return_type: measure.return_type.is_some(),
        lexicographic: measure.lexicographic,
        component_count: program
            .tables
            .bodies
            .expressions
            .expression_handles(measure.body)
            .len(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedTableSnapshot {
    pub type_constraint_count: usize,
    pub expression_count: usize,
    pub statement_count: usize,
    pub type_reference_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OperatorDefinitionSnapshot {
    pub is_boundary: bool,
    pub has_symbol: bool,
    pub name: Vec<String>,
    pub type_parameters: Vec<String>,
    pub parameter_count: usize,
    pub has_return_type: bool,
    pub contract_count: usize,
    pub spelling: Option<&'static str>,
    pub token_count: usize,
}

fn operator_snapshot(
    program: &SymbolResolvedTrees,
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
        type_parameters: program
            .data_type_parameters(operator.type_parameters)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        parameter_count: program.state_parameters(operator.parameters).len(),
        has_return_type: operator.return_type.is_some(),
        contract_count: program.signature_contracts(operator.contracts).len(),
        spelling: operator.spelling.map(|spelling| spelling.symbol()),
        token_count: operator.token_count,
    }
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
        initial_value: Option<ExpressionSnapshot>,
    },
    Variant {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomainDefinitionSnapshot {
    pub name: String,
    pub target_type: TypeReferenceSnapshot,
    pub classifier: Option<ExpressionSnapshot>,
    pub facts: Vec<ProofFactSnapshot>,
    pub operators: Vec<OperatorDefinitionSnapshot>,
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
    pub abi: Option<String>,
    pub type_parameters: Vec<String>,
    pub terminates: bool,
    pub decreases: Vec<ExpressionSnapshot>,
    pub decrease_order: Vec<String>,
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
    pub statements: Vec<StatementSnapshot>,
    pub table_statement_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateSignatureSnapshot {
    pub name: String,
    pub is_default: bool,
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
    Membership {
        value: Box<ExpressionSnapshot>,
        domain: Vec<String>,
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
    Unary {
        operator: &'static str,
        operand: Box<ExpressionSnapshot>,
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
        is_relaxed: bool,
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
        arguments: Vec<TypeReferenceSnapshot>,
    },
    DynamicTrait {
        name: String,
    },
    Named {
        name: String,
    },
    SelfType,
    Unit,
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
    ArithmeticDomain {
        domain: String,
    },
}

fn data_definition_snapshot(
    program: &SymbolResolvedTrees,
    data: &DataDefinition,
) -> DataDefinitionSnapshot {
    DataDefinitionSnapshot {
        name: data.name.to_string(),
        type_parameters: program
            .data_type_parameters(data.type_parameters)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        members: program
            .data_members(data.members)
            .iter()
            .map(|member| data_member_snapshot(program, member))
            .collect(),
    }
}

fn data_member_snapshot(program: &SymbolResolvedTrees, member: &DataMember) -> DataMemberSnapshot {
    match member {
        DataMember::Field(field) => DataMemberSnapshot::Field {
            name: field.name.to_string(),
            type_reference: type_reference_snapshot(program, &field.type_reference),
            initial_value: field
                .initial_value
                .is_valid()
                .then(|| table_expression_snapshot(program, field.initial_value)),
        },
        DataMember::Variant(variant) => DataMemberSnapshot::Variant {
            name: variant.name.to_string(),
        },
    }
}

fn domain_definition_snapshot(
    program: &SymbolResolvedTrees,
    domain: &DomainDefinition,
) -> DomainDefinitionSnapshot {
    DomainDefinitionSnapshot {
        name: domain.name.to_string(),
        target_type: type_reference_snapshot(program, &domain.target_type),
        classifier: domain
            .classifier
            .is_valid()
            .then(|| table_expression_snapshot(program, domain.classifier)),
        facts: domain_fact_snapshots(program, domain.facts),
        operators: program
            .operator_definitions(domain.operators)
            .iter()
            .map(|operator| operator_snapshot(program, operator))
            .collect(),
        body_token_count: domain.body_token_count,
    }
}

fn domain_fact_snapshots(
    program: &SymbolResolvedTrees,
    facts: omega_core::arena::HandleSpan<ProofFact>,
) -> Vec<ProofFactSnapshot> {
    program
        .proof_facts(facts)
        .iter()
        .map(|fact| match fact {
            ProofFact::Expression(expression) => ProofFactSnapshot::Expression {
                value: table_expression_snapshot(program, *expression),
            },
            ProofFact::Membership(membership) => ProofFactSnapshot::Membership {
                value: table_expression_snapshot(program, membership.value),
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
    program: &SymbolResolvedTrees,
    invariant: &InvariantDefinition,
) -> InvariantDefinitionSnapshot {
    InvariantDefinitionSnapshot {
        name: invariant.name.to_string(),
        constraints: program
            .tables
            .types
            .constraints
            .span_or_empty(invariant.constraints)
            .iter()
            .map(|constraint| type_constraint_snapshot(program, constraint))
            .collect(),
    }
}

fn machine_snapshot(program: &SymbolResolvedTrees, machine: &Machine) -> MachineSnapshot {
    MachineSnapshot {
        name: machine.name.to_string(),
        attached_data: machine.attached_data.as_ref().map(ToString::to_string),
        abi: machine.abi.clone(),
        type_parameters: program
            .machine_type_parameters(machine)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        terminates: machine.terminates,
        decreases: program
            .tables
            .bodies
            .expressions
            .expression_handles(machine.decreases)
            .iter()
            .map(|handle| table_expression_snapshot(program, *handle))
            .collect(),
        decrease_order: program
            .machine_decrease_order(machine.decrease_order)
            .iter()
            .map(ToString::to_string)
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
            .machine_contained_objects(machine.contains)
            .iter()
            .map(|contained| ContainedObjectSnapshot {
                name: contained.name.to_string(),
                type_name: contained.type_name.to_string(),
            })
            .collect(),
        owned_data: program
            .machine_owned_data(machine.owned_data)
            .iter()
            .map(|owned| owned_data_snapshot(program, owned))
            .collect(),
        states: program
            .machine_state_handles(machine.states)
            .iter()
            .map(|state| program.machine_state(*state))
            .map(|state| state_snapshot(program, state))
            .collect(),
    }
}

fn owned_data_snapshot(program: &SymbolResolvedTrees, owned: &OwnedData) -> OwnedDataSnapshot {
    OwnedDataSnapshot {
        name: owned.name.to_string(),
        type_reference: type_reference_snapshot(program, &owned.type_reference),
        initial_value: owned
            .initial_value
            .is_valid()
            .then(|| table_expression_snapshot(program, owned.initial_value)),
    }
}

fn platform_snapshot(program: &SymbolResolvedTrees, platform: &Platform) -> PlatformSnapshot {
    PlatformSnapshot {
        name: platform.name.to_string(),
        states: program
            .platform_state_signatures(platform.states)
            .iter()
            .map(|signature| state_signature_snapshot(program, signature))
            .collect(),
    }
}

fn trait_definition_snapshot(
    program: &SymbolResolvedTrees,
    trait_definition: &TraitDefinition,
) -> TraitSnapshot {
    TraitSnapshot {
        name: trait_definition.name.to_string(),
        is_boundary: trait_definition.is_boundary,
        type_parameters: program
            .trait_type_parameters(trait_definition)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        invariants: domain_fact_snapshots(program, trait_definition.invariants),
        requires: program
            .trait_requirements(trait_definition.requires)
            .iter()
            .map(|requirement| requirement.name.to_string())
            .collect(),
        machines: program
            .trait_machine_signatures(trait_definition.machines)
            .iter()
            .map(|signature| state_signature_snapshot(program, signature))
            .collect(),
    }
}

fn state_snapshot(program: &SymbolResolvedTrees, state: &State) -> StateSnapshot {
    StateSnapshot {
        name: state.name.to_string(),
        parameters: program
            .state_parameters(state.parameters)
            .iter()
            .map(|parameter| state_parameter_snapshot(program, parameter))
            .collect(),
        return_type: state
            .return_type
            .as_ref()
            .map(|type_reference| type_reference_snapshot(program, type_reference)),
        statements: program
            .state_statements(state.statements)
            .iter()
            .map(|statement| statement_snapshot(program, statement))
            .collect(),
        table_statement_count: state.statement_nodes.count() as usize,
    }
}

fn state_signature_snapshot(
    program: &SymbolResolvedTrees,
    signature: &StateSignature,
) -> StateSignatureSnapshot {
    StateSignatureSnapshot {
        name: signature.name.to_string(),
        is_default: signature.is_default,
        parameters: program
            .state_parameters(signature.parameters)
            .iter()
            .map(|parameter| state_parameter_snapshot(program, parameter))
            .collect(),
        return_type: signature
            .return_type
            .as_ref()
            .map(|type_reference| type_reference_snapshot(program, type_reference)),
        effects: program
            .signature_effects(signature.effects)
            .iter()
            .map(ToString::to_string)
            .collect(),
        contracts: program
            .signature_contracts(signature.contracts)
            .iter()
            .map(|contract| signature_contract_snapshot(program, contract))
            .collect(),
    }
}

fn signature_contract_snapshot(
    program: &SymbolResolvedTrees,
    contract: &crate::signature::SignatureContract,
) -> SignatureContractSnapshot {
    SignatureContractSnapshot {
        kind: match contract.kind {
            crate::signature::SignatureContractKind::Requires => "requires",
            crate::signature::SignatureContractKind::Ensures => "ensures",
            crate::signature::SignatureContractKind::Boundary => "boundary",
        },
        facts: domain_fact_snapshots(program, contract.facts),
        token_count: contract.token_count,
    }
}

fn state_parameter_snapshot(
    program: &SymbolResolvedTrees,
    parameter: &StateParameter,
) -> StateParameterSnapshot {
    StateParameterSnapshot {
        name: parameter.name.to_string(),
        type_reference: type_reference_snapshot(program, &parameter.type_reference),
        is_mutable: parameter.is_mutable,
        is_self: parameter.is_self,
    }
}

fn statement_snapshot(program: &SymbolResolvedTrees, statement: &Statement) -> StatementSnapshot {
    match statement {
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
            arguments: program
                .tables
                .bodies
                .expressions
                .expression_handles(call.arguments)
                .iter()
                .map(|expression| table_expression_snapshot(program, *expression))
                .collect(),
        },
        Statement::Expression(expression) => StatementSnapshot::Expression {
            value: statement_expression_snapshot(program, *expression),
        },
        Statement::LocalData(local_data) => StatementSnapshot::LocalData {
            name: local_data.name.to_string(),
            type_reference: type_reference_snapshot(program, &local_data.type_reference),
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

fn table_expression_snapshot(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
) -> ExpressionSnapshot {
    let table = &program.tables.bodies.expressions;

    match table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => ExpressionSnapshot::ArrayLiteral {
            values: table
                .expression_handles(*values)
                .iter()
                .map(|value| table_expression_snapshot(program, *value))
                .collect(),
        },
        ExpressionNode::Binary(binary) => ExpressionSnapshot::Binary {
            left: Box::new(table_expression_snapshot(program, binary.left)),
            operator: binary_operator_name(binary.operator),
            right: Box::new(table_expression_snapshot(program, binary.right)),
        },
        ExpressionNode::Boolean(value) => ExpressionSnapshot::Boolean { value: *value },
        ExpressionNode::Cast(cast) => ExpressionSnapshot::Cast {
            value: Box::new(table_expression_snapshot(program, cast.value)),
            target_type: table
                .name_path_members(cast.target_type)
                .iter()
                .map(ToString::to_string)
                .collect(),
        },
        ExpressionNode::Call(call) => ExpressionSnapshot::Call {
            receiver: call
                .receiver
                .is_valid()
                .then(|| Box::new(table_expression_snapshot(program, call.receiver))),
            target: call.target.to_string(),
            arguments: table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| table_expression_snapshot(program, *argument))
                .collect(),
        },
        ExpressionNode::Float(value) => ExpressionSnapshot::Float {
            value: value.to_string(),
        },
        ExpressionNode::Indexed(indexed) => ExpressionSnapshot::Indexed {
            collection: Box::new(table_expression_snapshot(program, indexed.collection)),
            index: Box::new(table_expression_snapshot(program, indexed.index)),
        },
        ExpressionNode::Integer(value) => ExpressionSnapshot::Integer { value: *value },
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
        ExpressionNode::Mutable(value) => ExpressionSnapshot::Mutable {
            value: Box::new(table_expression_snapshot(program, *value)),
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
            value: value.as_str().to_owned(),
        },
        ExpressionNode::Unary(unary) => ExpressionSnapshot::Unary {
            operator: unary.operator.display_name(),
            operand: Box::new(table_expression_snapshot(program, unary.operand)),
        },
    }
}

fn type_reference_snapshot(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> TypeReferenceSnapshot {
    type_reference_snapshot_from_program(program, type_reference)
}

fn wire_schema_snapshot(program: &SymbolResolvedTrees, wire_schema: &WireSchema) -> WireSchemaSnapshot {
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
    program: &SymbolResolvedTrees,
    members: HandleSpan<WireMember>,
) -> Vec<WireMemberSnapshot> {
    program
        .wire_members(members)
        .iter()
        .map(|member| match member {
            WireMember::Field(field) => WireMemberSnapshot::Field {
                number: field.number,
                name: field.name.to_string(),
                type_reference: type_reference_snapshot(program, &field.type_reference),
            },
            WireMember::Reserved(reserved) => WireMemberSnapshot::Reserved {
                number: reserved.number,
            },
            WireMember::Version(version) => WireMemberSnapshot::Version {
                name: version.name.to_string(),
                members: wire_member_snapshots(program, version.members),
            },
        })
        .collect()
}

fn type_reference_snapshot_from_program(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> TypeReferenceSnapshot {
    match type_reference {
        TypeReference::Reference(reference) => TypeReferenceSnapshot::Reference {
            referee: Box::new(type_reference_snapshot_from_program(
                program,
                program.child_type_reference(reference.referee),
            )),
            is_mutable: reference.is_mutable,
            is_relaxed: reference.is_relaxed,
        },
        TypeReference::Constrained(constrained) => TypeReferenceSnapshot::Constrained {
            base_type: Box::new(type_reference_snapshot_from_program(
                program,
                program.child_type_reference(constrained.base_type),
            )),
            constraints: program
                .tables
                .types
                .constraints
                .span_or_empty(constrained.constraints)
                .iter()
                .map(|constraint| type_constraint_snapshot(program, constraint))
                .collect(),
        },
        TypeReference::FixedArray(fixed_array) => TypeReferenceSnapshot::FixedArray {
            element_type: Box::new(type_reference_snapshot_from_program(
                program,
                program.child_type_reference(fixed_array.element_type),
            )),
            length: fixed_array.length.to_string(),
        },
        TypeReference::Slice(slice) => TypeReferenceSnapshot::Slice {
            element_type: Box::new(type_reference_snapshot_from_program(
                program,
                program.child_type_reference(slice.element_type),
            )),
        },
        TypeReference::Generic(generic) => TypeReferenceSnapshot::Generic {
            base_name: generic.base_name.to_string(),
            arguments: program
                .child_type_references(generic.arguments)
                .iter()
                .map(|argument| type_reference_snapshot_from_program(program, argument))
                .collect(),
        },
        TypeReference::DynamicTrait { name, .. } => TypeReferenceSnapshot::DynamicTrait {
            name: name.to_string(),
        },
        TypeReference::Named { name, .. } => TypeReferenceSnapshot::Named {
            name: name.to_string(),
        },
        TypeReference::SelfType { .. } => TypeReferenceSnapshot::SelfType,
        TypeReference::Unit => TypeReferenceSnapshot::Unit,
    }
}

fn type_constraint_snapshot(
    program: &SymbolResolvedTrees,
    constraint: &TypeConstraint,
) -> TypeConstraintSnapshot {
    match constraint {
        TypeConstraint::Named(name) | TypeConstraint::Domain(name) => {
            TypeConstraintSnapshot::Named {
                name: name.to_string(),
            }
        }
        TypeConstraint::Range { minimum, maximum } => TypeConstraintSnapshot::Range {
            minimum: table_expression_snapshot(program, *minimum),
            maximum: table_expression_snapshot(program, *maximum),
        },
        TypeConstraint::ArithmeticDomain(domain) => TypeConstraintSnapshot::ArithmeticDomain {
            domain: domain.name().to_owned(),
        },
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
