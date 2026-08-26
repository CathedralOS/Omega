use crate::SymbolResolvedTrees;
use crate::data::{DataDefinition, DataMember};
use crate::domain::{DomainDefinition, ProofFact};
use crate::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use crate::machine::{Machine, OwnedData};
use crate::operator::OperatorDefinition;
use crate::proposition::{PropositionBinderKind, PropositionBody, PropositionDefinition};
use crate::signature::{StateParameter, StateSignature};
use crate::state::State;
use crate::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
use crate::trait_definition::TraitDefinition;
use crate::types::{TypeConstraint, TypeReference};
use crate::wire::{WireMember, WireSchema};
use psi_arena::HandleSpan;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum StaticArgumentSnapshot {
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

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolResolvedTreesSnapshot {
    pub roots: SymbolResolvedRootsSnapshot,
    pub tables: ResolvedTableSnapshot,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub evidence_forwardings: Vec<EvidenceForwardingSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenceForwardingSnapshot {
    pub machine_symbol: u32,
    pub state_symbol: u32,
    pub statement_index: usize,
    pub target: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_conformance: Option<u32>,
}

impl SymbolResolvedTreesSnapshot {
    pub fn from_symbol_resolved_trees(symbol_resolved_trees: &SymbolResolvedTrees) -> Self {
        Self {
            roots: SymbolResolvedRootsSnapshot {
                const_declarations: symbol_resolved_trees
                    .const_declarations
                    .iter()
                    .map(|declaration| ConstDeclarationSnapshot {
                        has_symbol: declaration.symbol.is_valid(),
                        name: symbol_resolved_trees
                            .symbols
                            .display_path(declaration.symbol, "::"),
                        is_public: declaration.is_public,
                        declared_type: type_reference_snapshot(
                            symbol_resolved_trees,
                            &declaration.declared_type,
                        ),
                        canonical_value_encoding: declaration.canonical_value_encoding.clone(),
                    })
                    .collect(),
                conformances: symbol_resolved_trees
                    .conformances
                    .iter()
                    .map(|conformance| conformance_snapshot(symbol_resolved_trees, conformance))
                    .collect(),
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
                propositions: symbol_resolved_trees
                    .propositions
                    .iter()
                    .map(|proposition| proposition_snapshot(symbol_resolved_trees, proposition))
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
            evidence_forwardings: symbol_resolved_trees
                .evidence_forwardings
                .iter()
                .map(|forwarding| EvidenceForwardingSnapshot {
                    machine_symbol: forwarding.machine_symbol.arena_index(),
                    state_symbol: forwarding.state_symbol.arena_index(),
                    statement_index: forwarding.statement_index,
                    target: forwarding.target.to_string(),
                    source: forwarding.source.to_string(),
                    source_conformance: forwarding
                        .source_conformance
                        .map(|symbol| symbol.arena_index()),
                })
                .collect(),
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
    pub const_declarations: Vec<ConstDeclarationSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conformances: Vec<ConformanceSnapshot>,
    pub data_definitions: Vec<DataDefinitionSnapshot>,
    pub domain_definitions: Vec<DomainDefinitionSnapshot>,
    pub machines: Vec<MachineSnapshot>,
    pub measures: Vec<MeasureDefinitionSnapshot>,
    pub operators: Vec<OperatorDefinitionSnapshot>,
    pub propositions: Vec<PropositionSnapshot>,
    pub traits: Vec<TraitSnapshot>,
    pub wire_schemas: Vec<WireSchemaSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConformanceSnapshot {
    pub has_symbol: bool,
    pub name: String,
    pub is_public: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_parameters: Vec<String>,
    pub type_parameters: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_symbol: Option<u32>,
    pub trait_name: String,
    pub trait_symbol: u32,
    pub arguments: Vec<TypeReferenceSnapshot>,
    pub implementation: &'static str,
    pub rows: Vec<ConformanceRowSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConformanceRowSnapshot {
    pub declaring_trait: u32,
    pub requirement: u32,
    pub realization_machine: u32,
    pub realization_state: u32,
    pub source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConstDeclarationSnapshot {
    pub has_symbol: bool,
    pub name: String,
    pub is_public: bool,
    pub declared_type: TypeReferenceSnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_value_encoding: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WireSchemaSnapshot {
    pub has_symbol: bool,
    pub name: String,
    pub is_public: bool,
    pub encoding: Option<String>,
    pub members: Vec<WireMemberSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WireMemberSnapshot {
    Field {
        number: u64,
        name: String,
        relevance: &'static str,
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
    pub is_public: bool,
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
    program: &SymbolResolvedTrees,
    operator: &OperatorDefinition,
) -> OperatorDefinitionSnapshot {
    OperatorDefinitionSnapshot {
        is_public: operator.is_public,
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
    pub is_public: bool,
    pub supply: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_parameters: Vec<String>,
    pub type_parameters: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generic_instance: Option<TypeReferenceSnapshot>,
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
    pub equivalence: Option<QuotientEquivalenceSelectionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuotientEquivalenceSelectionSnapshot {
    pub relation: Vec<String>,
    pub relation_symbol: u32,
    pub trait_name: String,
    pub trait_symbol: u32,
    pub trait_arguments: Vec<TypeReferenceSnapshot>,
    pub conformance_name: String,
    pub conformance_symbol: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataMemberSnapshot {
    Field {
        identity: Option<u64>,
        name: String,
        relevance: &'static str,
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
    pub relevance: &'static str,
    pub type_reference: TypeReferenceSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomainDefinitionSnapshot {
    pub name: String,
    pub target_type: TypeReferenceSnapshot,
    pub is_public: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub alias: Vec<DomainAliasConstituentSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub authored_routes: Vec<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification: Option<&'static str>,
    pub predicate_body: &'static str,
    pub semantic_id: u32,
    pub semantic_roles: DomainSemanticRolesSnapshot,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub establishment_routes: Vec<DomainEstablishmentRouteSnapshot>,
    pub facts: Vec<ProofFactSnapshot>,
    pub operators: Vec<OperatorDefinitionSnapshot>,
    pub semantic_clause_token_count: usize,
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
pub struct MachineSnapshot {
    pub name: String,
    pub attached_data: Option<String>,
    pub is_public: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_parameters: Vec<String>,
    pub type_parameters: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conformance_bounds: Vec<GenericConformanceBoundSnapshot>,
    pub supply: MachineSupplySnapshot,
    pub body_is_present: bool,
    pub termination: TerminationInterfaceSnapshot,
    pub ranking_subjects: Vec<ExpressionSnapshot>,
    pub ranking_view: Vec<String>,
    pub invokes: Vec<String>,
    pub service_reach: Vec<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub service_reach_is_installation_bound: bool,
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: Vec<SignatureContractSnapshot>,
    pub owned_data: Vec<OwnedDataSnapshot>,
    pub states: Vec<StateSnapshot>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GenericConformanceBoundSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binder_symbol: Option<u32>,
    pub subject: String,
    pub subject_symbol: u32,
    pub carrier: String,
    pub carrier_symbol: u32,
    pub arguments: Vec<TypeReferenceSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conformance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conformance_symbol: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MachineSupplySnapshot {
    CheckedBody,
    Requirement,
    Boundary,
    Accepted,
    ExternalRealization {
        binding: u32,
        mechanism: &'static str,
    },
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
    Terminates {
        premises: Vec<ProgressPremiseSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProgressPremiseSnapshot {
    pub profile: u32,
    pub subject_root: u32,
    pub subject_projections: Vec<u32>,
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
    pub is_public: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_parameters: Vec<String>,
    pub type_parameters: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conformance_bounds: Vec<GenericConformanceBoundSnapshot>,
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
    pub table_statement_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateSignatureSnapshot {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spelling: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_parameters: Vec<String>,
    pub type_parameters: Vec<String>,
    pub is_default: bool,
    pub parameters: Vec<StateParameterSnapshot>,
    pub return_type: Option<TypeReferenceSnapshot>,
    pub invokes: Vec<String>,
    pub service_reach: Vec<String>,
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: Vec<SignatureContractSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignatureContractSnapshot {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crash_cause: Option<&'static str>,
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
pub struct PropositionSnapshot {
    pub has_symbol: bool,
    pub name: String,
    pub is_public: bool,
    pub binders: Vec<PropositionBinderSnapshot>,
    pub parameters: Vec<StateParameterSnapshot>,
    pub body: PropositionBodySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PropositionBinderSnapshot {
    pub has_symbol: bool,
    pub name: String,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub const_type: Option<TypeReferenceSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PropositionBodySnapshot {
    Primitive,
    Witness { evidence: TypeReferenceSnapshot },
    Transparent { proposition: ExpressionSnapshot },
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
    },
    Transition {
        target: TransitionTargetSnapshot,
        continuation: Option<TransitionTargetSnapshot>,
        guard: TransitionGuardSnapshot,
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
        /// Canonical literal spelling (see `psi_numerics::literals::IntegerLiteral`) --
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeReferenceSnapshot {
    Reference {
        referee: Box<TypeReferenceSnapshot>,
        access: &'static str,
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
    ConstExpression {
        expression: ExpressionSnapshot,
    },
    DynamicTrait {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        conformance: Option<String>,
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
    Domain {
        name: String,
        arguments: Vec<TypeReferenceSnapshot>,
    },
}

fn data_definition_snapshot(
    program: &SymbolResolvedTrees,
    data: &DataDefinition,
) -> DataDefinitionSnapshot {
    DataDefinitionSnapshot {
        name: data.name.to_string(),
        is_public: data.is_public,
        supply: match data.supply_mode {
            psi_language_semantics::DataSupplyMode::CheckedShape => "checked_shape",
            psi_language_semantics::DataSupplyMode::BoundaryOpaque => "boundary_opaque",
        }
        .to_owned(),
        lifetime_parameters: data
            .lifetime_parameters
            .iter()
            .map(ToString::to_string)
            .collect(),
        type_parameters: program
            .data_type_parameters(data.type_parameters)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        generic_instance: data
            .generic_instance
            .as_ref()
            .map(|origin| type_reference_snapshot(program, origin)),
        quotient: data
            .quotient
            .as_ref()
            .map(|quotient| QuotientDefinitionSnapshot {
                carrier: type_reference_snapshot(program, &quotient.carrier),
                relation: quotient.relation.iter().map(ToString::to_string).collect(),
                relation_symbol: quotient.relation_symbol.arena_index(),
                equivalence: quotient.equivalence.as_ref().map(|selection| {
                    QuotientEquivalenceSelectionSnapshot {
                        relation: selection.relation.iter().map(ToString::to_string).collect(),
                        relation_symbol: selection.relation_symbol.arena_index(),
                        trait_name: selection.trait_name.to_string(),
                        trait_symbol: selection.trait_symbol.arena_index(),
                        trait_arguments: program
                            .child_type_references(selection.trait_arguments)
                            .iter()
                            .map(|argument| type_reference_snapshot(program, argument))
                            .collect(),
                        conformance_name: selection.conformance_name.to_string(),
                        conformance_symbol: selection.conformance_symbol.arena_index(),
                    }
                }),
            }),
        retired_identities: data.retired_identities.clone(),
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
            identity: field.identity,
            name: field.name.to_string(),
            relevance: snapshot_binding_relevance(field.relevance),
            type_reference: type_reference_snapshot(program, &field.type_reference),
        },
        DataMember::Variant(variant) => DataMemberSnapshot::Variant {
            identity: variant.identity,
            name: variant.name.to_string(),
            payload: program
                .data_payload_fields(variant.payload)
                .iter()
                .map(|field| DataPayloadFieldSnapshot {
                    identity: field.identity,
                    name: field.name.to_string(),
                    relevance: snapshot_binding_relevance(field.relevance),
                    type_reference: type_reference_snapshot(program, &field.type_reference),
                })
                .collect(),
            retired_payload_identities: variant.retired_payload_identities.clone(),
        },
    }
}

fn snapshot_binding_relevance(relevance: psi_language_core::BindingRelevance) -> &'static str {
    match relevance {
        psi_language_core::BindingRelevance::Relevant => "relevant",
        psi_language_core::BindingRelevance::Erased => "erased",
    }
}

fn proposition_snapshot(
    program: &SymbolResolvedTrees,
    proposition: &PropositionDefinition,
) -> PropositionSnapshot {
    PropositionSnapshot {
        has_symbol: proposition.symbol.is_valid(),
        name: proposition.name.to_string(),
        is_public: proposition.is_public,
        binders: program
            .tables
            .declarations
            .proposition_binders
            .span_or_empty(proposition.binders)
            .iter()
            .map(|binder| {
                let (kind, const_type) = match &binder.kind {
                    PropositionBinderKind::Type => ("type", None),
                    PropositionBinderKind::Const { type_reference } => (
                        "const",
                        Some(type_reference_snapshot(program, type_reference)),
                    ),
                    PropositionBinderKind::Machine => ("machine", None),
                };
                PropositionBinderSnapshot {
                    has_symbol: binder.symbol.is_valid(),
                    name: binder.name.to_string(),
                    kind,
                    const_type,
                }
            })
            .collect(),
        parameters: program
            .state_parameters(proposition.parameters)
            .iter()
            .map(|parameter| state_parameter_snapshot(program, parameter))
            .collect(),
        body: match &proposition.body {
            PropositionBody::Primitive => PropositionBodySnapshot::Primitive,
            PropositionBody::Witness { evidence } => PropositionBodySnapshot::Witness {
                evidence: type_reference_snapshot(program, evidence),
            },
            PropositionBody::Transparent { proposition } => PropositionBodySnapshot::Transparent {
                proposition: table_expression_snapshot(program, *proposition),
            },
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
        authored_routes: domain
            .authored_routes
            .iter()
            .map(|route| route.iter().map(ToString::to_string).collect())
            .collect(),
        classification: domain.classification.map(|value| value.as_str()),
        predicate_body: domain.predicate_body.as_str(),
        semantic_id: domain.semantic_id.0,
        semantic_roles: DomainSemanticRolesSnapshot {
            denotation_dimension: domain
                .semantic_roles
                .denotation_dimension
                .map(|semantic| semantic.0),
            arithmetic_policy: domain
                .semantic_roles
                .arithmetic_policy
                .map(|semantic| semantic.0),
        },
        establishment_routes: domain
            .establishment_routes
            .iter()
            .copied()
            .map(establishment_route_snapshot)
            .collect(),
        facts: domain_fact_snapshots(program, domain.facts),
        operators: program
            .operator_definitions(domain.operators)
            .iter()
            .map(|operator| operator_snapshot(program, operator))
            .collect(),
        semantic_clause_token_count: domain.semantic_clause_token_count,
    }
}

fn establishment_route_snapshot(
    route: psi_language_semantics::DomainEstablishmentRoute,
) -> DomainEstablishmentRouteSnapshot {
    let requirement = route.requirement_symbol();
    DomainEstablishmentRouteSnapshot {
        kind: route.kind_name(),
        source_symbol: route.source_symbol().arena_index(),
        requirement_symbol: requirement.is_valid().then(|| requirement.arena_index()),
    }
}

fn domain_fact_snapshots(
    program: &SymbolResolvedTrees,
    facts: psi_arena::HandleSpan<ProofFact>,
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

fn machine_snapshot(program: &SymbolResolvedTrees, machine: &Machine) -> MachineSnapshot {
    MachineSnapshot {
        name: machine.name.to_string(),
        attached_data: machine.attached_data.as_ref().map(ToString::to_string),
        is_public: machine.is_public,
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
        conformance_bounds: machine
            .conformance_bounds
            .iter()
            .map(|bound| GenericConformanceBoundSnapshot {
                binder: bound.binder_name.as_ref().map(ToString::to_string),
                binder_symbol: bound.binder.map(|symbol| symbol.arena_index()),
                subject: bound.subject_name.to_string(),
                subject_symbol: bound.subject.arena_index(),
                carrier: bound.carrier_name.to_string(),
                carrier_symbol: bound.carrier.arena_index(),
                arguments: program
                    .child_type_references(bound.arguments)
                    .iter()
                    .map(|argument| type_reference_snapshot(program, argument))
                    .collect(),
                conformance: bound.conformance_name.as_ref().map(ToString::to_string),
                conformance_symbol: bound.conformance.map(|symbol| symbol.arena_index()),
            })
            .collect(),
        supply: machine_supply_snapshot(machine.supply_mode),
        body_is_present: machine.body_is_present,
        termination: termination_interface_snapshot(&machine.termination_plan.interface),
        ranking_subjects: program
            .tables
            .bodies
            .expressions
            .expression_handles(machine.ranking_subjects)
            .iter()
            .map(|handle| table_expression_snapshot(program, *handle))
            .collect(),
        ranking_view: program
            .machine_ranking_view(machine.ranking_view)
            .iter()
            .map(ToString::to_string)
            .collect(),
        invokes: program
            .machine_invokes(machine)
            .iter()
            .map(ToString::to_string)
            .collect(),
        service_reach: service_reach_names(program, machine.service_reach_row),
        service_reach_is_installation_bound: machine.service_reach_is_installation_bound,
        suspends: machine.suspends,
        blocks: machine.blocks,
        contracts: program
            .machine_contracts(machine)
            .iter()
            .map(|contract| signature_contract_snapshot(program, contract))
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

fn machine_supply_snapshot(
    supply: psi_language_semantics::MachineSupplyMode,
) -> MachineSupplySnapshot {
    use psi_language_semantics::MachineSupplyMode;
    match supply {
        MachineSupplyMode::CheckedBody => MachineSupplySnapshot::CheckedBody,
        MachineSupplyMode::Requirement => MachineSupplySnapshot::Requirement,
        MachineSupplyMode::Boundary => MachineSupplySnapshot::Boundary,
        MachineSupplyMode::Accepted => MachineSupplySnapshot::Accepted,
        MachineSupplyMode::ExternalRealization { binding, mechanism } => {
            MachineSupplySnapshot::ExternalRealization {
                binding: binding.0,
                mechanism: mechanism.as_str(),
            }
        }
    }
}

fn termination_interface_snapshot(
    interface: &psi_language_semantics::TerminationInterface,
) -> TerminationInterfaceSnapshot {
    use psi_language_semantics::{TerminationGuarantee, TerminationInterface};
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
                    premises: premises
                        .iter()
                        .map(|premise| ProgressPremiseSnapshot {
                            profile: premise.profile.0,
                            subject_root: premise.subject.root.arena_index(),
                            subject_projections: premise
                                .subject
                                .projections
                                .iter()
                                .map(|symbol| symbol.arena_index())
                                .collect(),
                        })
                        .collect(),
                },
            }
        }
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

fn trait_definition_snapshot(
    program: &SymbolResolvedTrees,
    trait_definition: &TraitDefinition,
) -> TraitSnapshot {
    TraitSnapshot {
        name: trait_definition.name.to_string(),
        is_boundary: trait_definition.is_boundary,
        is_public: trait_definition.is_public,
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
        conformance_bounds: trait_definition
            .conformance_bounds
            .iter()
            .map(|bound| GenericConformanceBoundSnapshot {
                binder: bound.binder_name.as_ref().map(ToString::to_string),
                binder_symbol: bound.binder.map(|symbol| symbol.arena_index()),
                subject: bound.subject_name.to_string(),
                subject_symbol: bound.subject.arena_index(),
                carrier: bound.carrier_name.to_string(),
                carrier_symbol: bound.carrier.arena_index(),
                arguments: program
                    .child_type_references(bound.arguments)
                    .iter()
                    .map(|argument| type_reference_snapshot(program, argument))
                    .collect(),
                conformance: bound.conformance_name.as_ref().map(ToString::to_string),
                conformance_symbol: bound.conformance.map(|symbol| symbol.arena_index()),
            })
            .collect(),
        requires: program
            .trait_requirements(trait_definition.requires)
            .iter()
            .map(|requirement| {
                let arguments = program.child_type_references(requirement.arguments);
                if requirement.lifetime_arguments.is_empty() && arguments.is_empty() {
                    requirement.name.to_string()
                } else {
                    let lifetime_arguments = requirement
                        .lifetime_arguments
                        .iter()
                        .map(|lifetime| format!("'{lifetime}"));
                    let type_arguments = arguments.iter().map(|argument| argument.display_name());
                    format!(
                        "{}<{}>",
                        requirement.name,
                        lifetime_arguments
                            .chain(type_arguments)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            })
            .collect(),
        machines: program
            .trait_machine_signatures(trait_definition.machines)
            .iter()
            .map(|signature| state_signature_snapshot(program, signature))
            .collect(),
    }
}

fn conformance_snapshot(
    program: &SymbolResolvedTrees,
    conformance: &crate::trait_definition::Conformance,
) -> ConformanceSnapshot {
    let (implementation, rows) = match &conformance.implementation {
        crate::trait_definition::ConformanceImplementation::AttachedRequirementMachines => {
            ("attached_requirement_machines", Vec::new())
        }
        crate::trait_definition::ConformanceImplementation::Closed { rows } => (
            "closed",
            rows.iter()
                .map(|row| ConformanceRowSnapshot {
                    declaring_trait: row.declaring_trait.arena_index(),
                    requirement: row.requirement.arena_index(),
                    realization_machine: row.realization_machine.arena_index(),
                    realization_state: row.realization_state.arena_index(),
                    source: match row.source {
                        crate::trait_definition::ConformanceRowSource::Inline => "inline",
                        crate::trait_definition::ConformanceRowSource::Reference => "reference",
                        crate::trait_definition::ConformanceRowSource::TraitDefault => {
                            "trait_default"
                        }
                    },
                })
                .collect(),
        ),
    };
    ConformanceSnapshot {
        has_symbol: conformance.symbol.is_valid(),
        name: conformance.alias.as_ref().map_or_else(
            || program.symbols.display_path(conformance.symbol, "::"),
            ToString::to_string,
        ),
        is_public: conformance.is_public,
        lifetime_parameters: conformance
            .lifetime_parameters
            .iter()
            .map(ToString::to_string)
            .collect(),
        type_parameters: program
            .data_type_parameters(conformance.type_parameters)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        subject: conformance.carrier_name().map(ToString::to_string),
        subject_symbol: conformance
            .carrier_symbol
            .is_valid()
            .then(|| conformance.carrier_symbol.arena_index()),
        trait_name: conformance.trait_name.to_string(),
        trait_symbol: conformance.trait_symbol.arena_index(),
        arguments: program
            .child_type_references(conformance.arguments)
            .iter()
            .map(|argument| type_reference_snapshot(program, argument))
            .collect(),
        implementation,
        rows,
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
        contracts: program
            .signature_contracts(state.contracts)
            .iter()
            .map(|contract| signature_contract_snapshot(program, contract))
            .collect(),
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
        spelling: signature.spelling.map(|spelling| spelling.symbol()),
        lifetime_parameters: signature
            .lifetime_parameters
            .iter()
            .map(ToString::to_string)
            .collect(),
        type_parameters: program
            .data_type_parameters(signature.type_parameters)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
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
        invokes: program
            .signature_invokes(signature.invokes)
            .iter()
            .map(ToString::to_string)
            .collect(),
        service_reach: service_reach_names(program, signature.service_reach_row),
        suspends: signature.suspends,
        blocks: signature.blocks,
        contracts: program
            .signature_contracts(signature.contracts)
            .iter()
            .map(|contract| signature_contract_snapshot(program, contract))
            .collect(),
    }
}

fn service_reach_names(
    program: &SymbolResolvedTrees,
    row: psi_language_semantics::ServiceReachRowId,
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
    program: &SymbolResolvedTrees,
    contract: &crate::signature::SignatureContract,
) -> SignatureContractSnapshot {
    let (kind, crash_cause) = match &contract.kind {
        crate::signature::SignatureContractKind::Requires => ("requires", None),
        crate::signature::SignatureContractKind::Ensures => ("ensures", None),
        crate::signature::SignatureContractKind::Crashes { cause } => {
            return SignatureContractSnapshot {
                kind: "crashes",
                binding: contract.binding.as_ref().map(ToString::to_string),
                crash_cause: Some(match cause {
                    crate::signature::CrashCause::Trap => "Trap",
                    crate::signature::CrashCause::Abort => "Abort",
                }),
                facts: domain_fact_snapshots(program, contract.facts),
                token_count: contract.token_count,
            };
        }
    };
    SignatureContractSnapshot {
        kind,
        binding: contract.binding.as_ref().map(ToString::to_string),
        crash_cause,
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
                .map(snapshot_static_argument)
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
                == psi_language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
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
                .map(snapshot_static_argument)
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
                == psi_language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
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

fn snapshot_static_argument(
    argument: &crate::expression::StaticMachineArgument,
) -> StaticArgumentSnapshot {
    if let Some(literal) = &argument.const_literal {
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
                .map(snapshot_static_argument)
                .collect(),
        }
    } else {
        StaticArgumentSnapshot::Path(diagnostic_name_span_snapshot(&argument.path))
    }
}

fn type_reference_snapshot(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> TypeReferenceSnapshot {
    type_reference_snapshot_from_program(program, type_reference)
}

fn wire_schema_snapshot(
    program: &SymbolResolvedTrees,
    wire_schema: &WireSchema,
) -> WireSchemaSnapshot {
    WireSchemaSnapshot {
        has_symbol: wire_schema.symbol.is_valid(),
        name: wire_schema.name.to_string(),
        is_public: wire_schema.is_public,
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
                relevance: snapshot_binding_relevance(field.relevance),
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

fn reference_access_name(access: psi_language_core::ReferenceAccess) -> &'static str {
    match access {
        psi_language_core::ReferenceAccess::Shared => "shared",
        psi_language_core::ReferenceAccess::Mutable => "mutable",
        psi_language_core::ReferenceAccess::WriteOnly => "write_only",
    }
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
            access: reference_access_name(reference.access),
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
            lifetime_arguments: generic
                .lifetime_arguments
                .iter()
                .map(ToString::to_string)
                .collect(),
            arguments: program
                .child_type_references(generic.arguments)
                .iter()
                .map(|argument| type_reference_snapshot_from_program(program, argument))
                .collect(),
        },
        TypeReference::ConstExpression(expression) => TypeReferenceSnapshot::ConstExpression {
            expression: table_expression_snapshot(program, *expression),
        },
        TypeReference::DynamicTrait {
            name,
            conformance_carrier,
            conformance_name,
            ..
        } => TypeReferenceSnapshot::DynamicTrait {
            name: name.to_string(),
            conformance: conformance_carrier
                .as_ref()
                .zip(conformance_name.as_ref())
                .map(|(carrier, selection)| format!("{carrier}::{selection}")),
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
        TypeConstraint::Named(name) => TypeConstraintSnapshot::Named {
            name: name.to_string(),
        },
        TypeConstraint::Domain(domain) => TypeConstraintSnapshot::Domain {
            name: domain.name.to_string(),
            arguments: program
                .tables
                .declarations
                .child_type_references
                .span_or_empty(domain.arguments)
                .iter()
                .map(|argument| type_reference_snapshot(program, argument))
                .collect(),
        },
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

#[cfg(test)]
mod termination_vocabulary_tests {
    use super::{ProgressPremiseSnapshot, TerminationGuaranteeSnapshot};

    #[test]
    fn termination_guarantee_uses_settled_snapshot_vocabulary() {
        let snapshot = TerminationGuaranteeSnapshot::Terminates {
            premises: vec![ProgressPremiseSnapshot {
                profile: 3,
                subject_root: 5,
                subject_projections: vec![7],
            }],
        };
        assert_eq!(
            serde_json::to_string(&snapshot).expect("serialize termination guarantee"),
            r#"{"kind":"terminates","premises":[{"profile":3,"subject_root":5,"subject_projections":[7]}]}"#
        );
    }
}
