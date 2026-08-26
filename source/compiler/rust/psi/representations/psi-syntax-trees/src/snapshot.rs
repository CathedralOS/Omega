use crate::expression::{
    BinaryOperator, ExpressionNode, TableCallExpression, TableStructLiteralField,
};
use crate::identifier::Identifier;
use crate::item::{
    CapabilityContract, CapabilityContractKind, CapabilityMember, DataMember, ExternalBinding,
    GenericConformanceBound, Item, ProofFact, PropositionBody, SatisfiesClause, StateParameterNode,
    StateSignature, WireDataMember,
};
use crate::statement::{
    AssemblyFactKind, StatementNode, TransitionGuardNode, TransitionTargetNode,
};
use crate::syntax_trees::SyntaxTrees;
use crate::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceNode};
use psi_diagnostics::PhaseSnapshot;
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
#[serde(untagged)]
pub enum StaticArgumentSnapshot {
    Path(Vec<IdentifierSnapshot>),
    Application {
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
pub enum ItemSnapshot {
    Capability {
        name: IdentifierSnapshot,
        members: Vec<CapabilityMemberSnapshot>,
    },
    Conformance {
        #[serde(skip_serializing_if = "is_false")]
        is_public: bool,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        lifetime_parameters: Vec<IdentifierSnapshot>,
        type_parameters: Vec<TypeParameterSnapshot>,
        #[serde(skip_serializing_if = "Option::is_none")]
        type_name: Option<IdentifierSnapshot>,
        #[serde(skip_serializing_if = "is_false")]
        subjectless: bool,
        trait_name: IdentifierSnapshot,
        trait_arguments: Vec<TypeReferenceSnapshot>,
        #[serde(skip_serializing_if = "Option::is_none")]
        alias: Option<IdentifierSnapshot>,
        body: ConformanceBodySnapshot,
    },
    Const {
        scope: IdentifierSnapshot,
        name: IdentifierSnapshot,
        is_public: bool,
        type_reference: TypeReferenceSnapshot,
        value: Box<ExpressionSnapshot>,
    },
    Data {
        name: IdentifierSnapshot,
        is_public: bool,
        supply: &'static str,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        lifetime_parameters: Vec<IdentifierSnapshot>,
        type_parameters: Vec<TypeParameterSnapshot>,
        #[serde(skip_serializing_if = "Option::is_none")]
        generic_instance: Option<TypeReferenceSnapshot>,
        properties: DataPropertiesSnapshot,
        #[serde(skip_serializing_if = "Option::is_none")]
        quotient: Option<QuotientSnapshot>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        where_facts: Vec<ProofFactSnapshot>,
        members: Vec<DataMemberSnapshot>,
    },
    Domain {
        name: IdentifierSnapshot,
        type_parameters: Vec<TypeParameterSnapshot>,
        target_type: TypeReferenceSnapshot,
        index_arguments: Vec<TypeReferenceSnapshot>,
        is_public: bool,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        alias: Vec<Vec<IdentifierSnapshot>>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        authored_routes: Vec<Vec<IdentifierSnapshot>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        classification: Option<&'static str>,
        predicate_body: &'static str,
        facts: Vec<ProofFactSnapshot>,
        operators: Vec<OperatorSnapshot>,
        semantic_clause_token_count: usize,
    },
    Measure {
        name: Vec<IdentifierSnapshot>,
        parameter: Option<StateParameterSnapshot>,
        return_type: TypeReferenceSnapshot,
        lexicographic: bool,
        body: Vec<ExpressionSnapshot>,
        token_count: usize,
    },
    Operator {
        operator: OperatorSnapshot,
    },
    Module {
        path: Vec<IdentifierSnapshot>,
    },
    Package {
        path: Vec<IdentifierSnapshot>,
    },
    Proposition {
        name: IdentifierSnapshot,
        is_public: bool,
        type_parameters: Vec<TypeParameterSnapshot>,
        parameters: Vec<StateParameterSnapshot>,
        body: PropositionBodySnapshot,
    },
    Use {
        path: Vec<IdentifierSnapshot>,
    },
    Machine {
        name: IdentifierSnapshot,
        attached_data: Option<IdentifierSnapshot>,
        is_public: bool,
        bodyless: bool,
        target: Option<IdentifierSnapshot>,
        boundary: bool,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        lifetime_parameters: Vec<IdentifierSnapshot>,
        type_parameters: Vec<TypeParameterSnapshot>,
        satisfies: Vec<SatisfiesClauseSnapshot>,
        conformance_bounds: Vec<GenericConformanceBoundSnapshot>,
        terminates_guarantee: bool,
        ranking_subjects: Vec<ExpressionSnapshot>,
        ranking_view: Vec<IdentifierSnapshot>,
        ranking_view_arguments: Vec<ExpressionSnapshot>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ranking_range: Option<Box<ExpressionSnapshot>>,
        #[serde(skip_serializing_if = "is_false")]
        service_reach_is_installation_bound: bool,
        service_reaches: Vec<IdentifierSnapshot>,
        invokes: Vec<IdentifierSnapshot>,
        suspends: bool,
        blocks: bool,
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
        is_public: bool,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        lifetime_parameters: Vec<IdentifierSnapshot>,
        type_parameters: Vec<TypeParameterSnapshot>,
        conformance_bounds: Vec<GenericConformanceBoundSnapshot>,
        parents: Vec<TypeReferenceSnapshot>,
        requires: Vec<IdentifierSnapshot>,
        machines: Vec<StateSignatureSnapshot>,
    },
    Target {
        name: IdentifierSnapshot,
        host: Option<TargetHostSnapshot>,
        boundary_policies: Vec<BoundaryPolicySnapshot>,
    },
    WireData {
        name: IdentifierSnapshot,
        is_public: bool,
        encoding: Option<IdentifierSnapshot>,
        members: Vec<WireDataMemberSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SatisfiesClauseSnapshot {
    pub trait_name: IdentifierSnapshot,
    pub arguments: Vec<TypeReferenceSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement: Option<IdentifierSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<IdentifierSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via: Option<ExternalBindingSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GenericConformanceBoundSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binder: Option<IdentifierSnapshot>,
    pub subject: IdentifierSnapshot,
    pub carrier: IdentifierSnapshot,
    pub arguments: Vec<TypeReferenceSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conformance: Option<IdentifierSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExternalBindingSnapshot {
    Syscall { number: i64 },
    DllImport { module: String, symbol: String },
    CompilerIntrinsic,
    VtableSlot { index: i64 },
    VtableField { field: IdentifierSnapshot },
    TableFunction { field: IdentifierSnapshot },
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConformanceBodySnapshot {
    AttachedRequirementMachines,
    Closed {
        members: Vec<ConformanceMemberSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConformanceMemberSnapshot {
    Machine {
        declaration: Box<ItemSnapshot>,
    },
    TraitDefault {
        declaring_trait: IdentifierSnapshot,
        requirement_ordinal: usize,
        declaration: Box<ItemSnapshot>,
    },
    Reference {
        declaring_trait: IdentifierSnapshot,
        requirement: IdentifierSnapshot,
        target: Vec<IdentifierSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PropositionBodySnapshot {
    Primitive,
    Witness {
        evidence: TypeReferenceSnapshot,
    },
    Transparent {
        proposition: Box<ExpressionSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypeParameterSnapshot {
    pub name: IdentifierSnapshot,
    pub kind: &'static str,
    pub const_type: Option<TypeReferenceSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine_contract: Option<StateSignatureSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine_requirement: Option<Vec<IdentifierSnapshot>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposition_contract: Option<PropositionParameterSignatureSnapshot>,
    pub bounds: DataPropertiesSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PropositionParameterSignatureSnapshot {
    pub name: IdentifierSnapshot,
    pub parameters: Vec<StateParameterSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DataPropertiesSnapshot {
    pub multiplicity: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carry: Option<CarryPolicySnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuotientSnapshot {
    pub carrier: TypeReferenceSnapshot,
    pub relation: Vec<IdentifierSnapshot>,
    pub equivalence: Option<QuotientEquivalenceSelectionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuotientEquivalenceSelectionSnapshot {
    pub relation: Vec<IdentifierSnapshot>,
    pub trait_name: IdentifierSnapshot,
    pub trait_arguments: Vec<TypeReferenceSnapshot>,
    pub conformance_name: IdentifierSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CarryPolicySnapshot {
    pub suspension: &'static str,
    pub cpu: &'static str,
    pub thread: &'static str,
    pub address: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OperatorSnapshot {
    pub is_public: bool,
    pub is_boundary: bool,
    pub name: Vec<IdentifierSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_parameters: Vec<IdentifierSnapshot>,
    pub type_parameters: Vec<TypeParameterSnapshot>,
    pub parameters: Vec<StateParameterSnapshot>,
    pub return_type: TypeReferenceSnapshot,
    pub contracts: Vec<CapabilityContractSnapshot>,
    pub spelling: Option<&'static str>,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataMemberSnapshot {
    Field {
        identity: Option<u64>,
        name: IdentifierSnapshot,
        relevance: &'static str,
        type_reference: TypeReferenceSnapshot,
    },
    Variant {
        identity: Option<u64>,
        name: IdentifierSnapshot,
        payload: Vec<DataPayloadFieldSnapshot>,
        retired_payload_identities: Vec<u64>,
    },
    Retired {
        identity: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DataPayloadFieldSnapshot {
    pub identity: Option<u64>,
    pub name: IdentifierSnapshot,
    pub relevance: &'static str,
    pub type_reference: TypeReferenceSnapshot,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<IdentifierSnapshot>,
    pub facts: Vec<ProofFactSnapshot>,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CapabilityContractKindSnapshot {
    Ensures,
    EnsuresForResultCase {
        result_case: Vec<IdentifierSnapshot>,
    },
    Requires,
    Crashes {
        cause: &'static str,
    },
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
pub enum WireDataMemberSnapshot {
    Field {
        number: u64,
        name: IdentifierSnapshot,
        relevance: &'static str,
        type_reference: TypeReferenceSnapshot,
    },
    Reserved {
        number: u64,
    },
    Version {
        name: IdentifierSnapshot,
        members: Vec<WireDataMemberSnapshot>,
    },
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
    ConstExpression {
        expression: ExpressionSnapshot,
    },
    DynamicTrait {
        name: IdentifierSnapshot,
        #[serde(skip_serializing_if = "Option::is_none")]
        conformance: Option<IdentifierSnapshot>,
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
pub enum FixedArrayLengthSnapshot {
    Literal { value: usize },
    ConstParameter { name: IdentifierSnapshot },
    ConstCall { name: IdentifierSnapshot },
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
    ArithmeticDomain {
        domain: String,
    },
    Domain {
        name: IdentifierSnapshot,
        arguments: Vec<TypeReferenceSnapshot>,
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
    Float {
        text: String,
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
    },
    Range {
        start: Option<Box<ExpressionSnapshot>>,
        end: Option<Box<ExpressionSnapshot>>,
        end_inclusive: bool,
    },
    SelfValue,
    StructLiteral {
        type_name: IdentifierSnapshot,
        #[serde(skip_serializing_if = "Option::is_none")]
        case_name: Option<IdentifierSnapshot>,
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
        Item::Conformance(value) => ItemSnapshot::Conformance {
            is_public: value.is_public,
            lifetime_parameters: value
                .lifetime_parameters
                .iter()
                .map(snapshot_identifier)
                .collect(),
            type_parameters: syntax_trees
                .items
                .type_parameters(value.type_parameters)
                .iter()
                .map(|parameter| snapshot_type_parameter(syntax_trees, parameter))
                .collect(),
            type_name: match &value.subject {
                crate::item::ConformanceSubject::Carrier(type_name) => {
                    Some(snapshot_identifier(type_name))
                }
                crate::item::ConformanceSubject::Subjectless => None,
            },
            subjectless: matches!(value.subject, crate::item::ConformanceSubject::Subjectless),
            trait_name: snapshot_identifier(&value.trait_name),
            trait_arguments: syntax_trees
                .type_references
                .type_reference_handles(value.trait_arguments)
                .iter()
                .map(|argument| snapshot_type_reference_handle(syntax_trees, *argument))
                .collect(),
            alias: value.alias.as_ref().map(snapshot_identifier),
            body: match value.body {
                crate::item::ConformanceBody::AttachedRequirementMachines => {
                    ConformanceBodySnapshot::AttachedRequirementMachines
                }
                crate::item::ConformanceBody::Closed { members } => {
                    ConformanceBodySnapshot::Closed {
                        members: syntax_trees
                            .items
                            .conformance_members(members)
                            .iter()
                            .map(|member| match member {
                                crate::item::ConformanceMember::Machine(machine) => {
                                    ConformanceMemberSnapshot::Machine {
                                        declaration: Box::new(snapshot_item(
                                            syntax_trees,
                                            &Item::Machine(machine.clone()),
                                        )),
                                    }
                                }
                                crate::item::ConformanceMember::TraitDefault {
                                    declaring_trait,
                                    requirement_ordinal,
                                    machine,
                                } => ConformanceMemberSnapshot::TraitDefault {
                                    declaring_trait: snapshot_identifier(declaring_trait),
                                    requirement_ordinal: *requirement_ordinal,
                                    declaration: Box::new(snapshot_item(
                                        syntax_trees,
                                        &Item::Machine(machine.clone()),
                                    )),
                                },
                                crate::item::ConformanceMember::Reference {
                                    declaring_trait,
                                    requirement,
                                    target,
                                } => ConformanceMemberSnapshot::Reference {
                                    declaring_trait: snapshot_identifier(declaring_trait),
                                    requirement: snapshot_identifier(requirement),
                                    target: snapshot_identifier_slice(
                                        syntax_trees.items.identifier_path_members(*target),
                                    ),
                                },
                            })
                            .collect(),
                    }
                }
            },
        },
        Item::Const(value) => ItemSnapshot::Const {
            scope: snapshot_identifier(&value.scope),
            name: snapshot_identifier(&value.name),
            is_public: value.is_public,
            type_reference: snapshot_type_reference_handle(syntax_trees, value.type_reference),
            value: Box::new(snapshot_expression_handle(syntax_trees, value.value)),
        },
        Item::Data(value) => ItemSnapshot::Data {
            name: snapshot_identifier(&value.name),
            is_public: value.is_public,
            supply: match value.supply_mode {
                psi_language_core::DataSupplyMode::CheckedShape => "checked_shape",
                psi_language_core::DataSupplyMode::BoundaryOpaque => "boundary_opaque",
            },
            lifetime_parameters: value
                .lifetime_parameters
                .iter()
                .map(snapshot_identifier)
                .collect(),
            type_parameters: syntax_trees
                .items
                .type_parameters(value.type_parameters)
                .iter()
                .map(|parameter| snapshot_type_parameter(syntax_trees, parameter))
                .collect(),
            generic_instance: value
                .generic_instance
                .map(|origin| snapshot_type_reference_handle(syntax_trees, origin)),
            properties: snapshot_data_properties(value.properties),
            quotient: value.quotient.as_ref().map(|quotient| QuotientSnapshot {
                carrier: snapshot_type_reference_handle(syntax_trees, quotient.carrier),
                relation: snapshot_identifier_slice(
                    syntax_trees
                        .items
                        .identifier_path_members(quotient.relation),
                ),
                equivalence: quotient.equivalence.as_ref().map(|selection| {
                    QuotientEquivalenceSelectionSnapshot {
                        relation: snapshot_identifier_slice(
                            syntax_trees
                                .items
                                .identifier_path_members(selection.relation),
                        ),
                        trait_name: snapshot_identifier(&selection.trait_name),
                        trait_arguments: syntax_trees
                            .type_references
                            .type_reference_handles(selection.trait_arguments)
                            .iter()
                            .map(|argument| snapshot_type_reference_handle(syntax_trees, *argument))
                            .collect(),
                        conformance_name: snapshot_identifier(&selection.conformance_name),
                    }
                }),
            }),
            where_facts: snapshot_proof_facts(syntax_trees, value.where_facts),
            members: syntax_trees
                .items
                .data_members(value.members)
                .iter()
                .map(|member| snapshot_data_member(syntax_trees, member))
                .collect(),
        },
        Item::Domain(value) => ItemSnapshot::Domain {
            name: snapshot_identifier(&value.name),
            type_parameters: syntax_trees
                .items
                .type_parameters(value.type_parameters)
                .iter()
                .map(|parameter| snapshot_type_parameter(syntax_trees, parameter))
                .collect(),
            target_type: snapshot_type_reference_handle(syntax_trees, value.target_type),
            index_arguments: syntax_trees
                .type_references
                .type_reference_handles(value.index_arguments)
                .iter()
                .map(|argument| snapshot_type_reference_handle(syntax_trees, *argument))
                .collect(),
            is_public: value.is_public,
            alias: value
                .alias
                .as_ref()
                .map(|alias| {
                    alias
                        .constituents
                        .iter()
                        .map(|constituent| {
                            snapshot_identifier_slice(
                                syntax_trees.items.identifier_path_members(*constituent),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            authored_routes: value
                .authored_routes
                .iter()
                .map(|route| snapshot_identifier_slice(route))
                .collect(),
            classification: value.classification.map(|value| value.as_str()),
            predicate_body: value.predicate_body.as_str(),
            facts: snapshot_proof_facts(syntax_trees, value.facts),
            operators: syntax_trees
                .items
                .operators(value.operators)
                .iter()
                .map(|operator| snapshot_operator(syntax_trees, operator))
                .collect(),
            semantic_clause_token_count: value.semantic_clause_token_count,
        },
        Item::Measure(value) => ItemSnapshot::Measure {
            name: snapshot_identifier_slice(syntax_trees.items.identifier_path_members(value.name)),
            parameter: value.parameter.is_valid().then(|| {
                snapshot_state_parameter(
                    syntax_trees,
                    syntax_trees.items.state_parameter(value.parameter),
                )
            }),
            return_type: snapshot_type_reference_handle(syntax_trees, value.return_type),
            lexicographic: value.lexicographic,
            body: syntax_trees
                .expressions
                .expression_handles(value.body)
                .iter()
                .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                .collect(),
            token_count: value.token_count,
        },
        Item::Operator(value) => ItemSnapshot::Operator {
            operator: snapshot_operator(syntax_trees, value),
        },
        Item::Module(value) => ItemSnapshot::Module {
            path: snapshot_identifier_slice(syntax_trees.items.identifier_path_members(value.path)),
        },
        Item::Package(value) => ItemSnapshot::Package {
            path: snapshot_identifier_slice(syntax_trees.items.identifier_path_members(value.path)),
        },
        Item::Proposition(value) => ItemSnapshot::Proposition {
            name: snapshot_identifier(&value.name),
            is_public: value.is_public,
            type_parameters: syntax_trees
                .items
                .type_parameters(value.type_parameters)
                .iter()
                .map(|parameter| snapshot_type_parameter(syntax_trees, parameter))
                .collect(),
            parameters: syntax_trees
                .items
                .state_parameters(value.parameters)
                .iter()
                .map(|parameter| {
                    snapshot_state_parameter(
                        syntax_trees,
                        syntax_trees.items.state_parameter(*parameter),
                    )
                })
                .collect(),
            body: match value.body {
                PropositionBody::Primitive => PropositionBodySnapshot::Primitive,
                PropositionBody::Witness { evidence } => PropositionBodySnapshot::Witness {
                    evidence: snapshot_type_reference_handle(syntax_trees, evidence),
                },
                PropositionBody::Transparent { proposition } => {
                    PropositionBodySnapshot::Transparent {
                        proposition: Box::new(snapshot_expression_handle(
                            syntax_trees,
                            proposition,
                        )),
                    }
                }
            },
        },
        Item::Use(value) => ItemSnapshot::Use {
            path: snapshot_identifier_slice(syntax_trees.items.identifier_path_members(value.path)),
        },
        Item::Machine(value) => ItemSnapshot::Machine {
            name: snapshot_identifier(&value.name),
            attached_data: value.attached_data.as_ref().map(snapshot_identifier),
            is_public: value.is_public,
            bodyless: value.bodyless,
            target: value.target.as_ref().map(snapshot_identifier),
            boundary: value.boundary,
            lifetime_parameters: value
                .lifetime_parameters
                .iter()
                .map(snapshot_identifier)
                .collect(),
            type_parameters: syntax_trees
                .items
                .type_parameters(value.type_parameters)
                .iter()
                .map(|parameter| snapshot_type_parameter(syntax_trees, parameter))
                .collect(),
            satisfies: syntax_trees
                .items
                .satisfies_clauses(value.satisfies)
                .iter()
                .map(|clause| snapshot_satisfies_clause(syntax_trees, clause))
                .collect(),
            conformance_bounds: value
                .conformance_bounds
                .iter()
                .map(|bound| snapshot_generic_conformance_bound(syntax_trees, bound))
                .collect(),
            terminates_guarantee: value.terminates_guarantee,
            ranking_subjects: syntax_trees
                .expressions
                .expression_handles(value.ranking_subjects)
                .iter()
                .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                .collect(),
            ranking_view: snapshot_identifier_slice(
                syntax_trees
                    .items
                    .identifier_path_members(value.ranking_view),
            ),
            ranking_view_arguments: syntax_trees
                .expressions
                .expression_handles(value.ranking_view_arguments)
                .iter()
                .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                .collect(),
            ranking_range: value.ranking_range.is_valid().then(|| {
                Box::new(snapshot_expression_handle(
                    syntax_trees,
                    value.ranking_range,
                ))
            }),
            service_reach_is_installation_bound: value.service_reach_is_installation_bound,
            service_reaches: snapshot_identifier_slice(
                syntax_trees
                    .items
                    .identifier_path_members(value.service_reaches),
            ),
            invokes: snapshot_identifier_slice(
                syntax_trees.items.identifier_path_members(value.invokes),
            ),
            suspends: value.suspends,
            blocks: value.blocks,
            contracts: snapshot_capability_contracts(syntax_trees, value.contracts),
            states: syntax_trees
                .items
                .state_handles(value.states)
                .iter()
                .map(|handle| snapshot_state_node(syntax_trees, syntax_trees.items.state(*handle)))
                .collect(),
        },
        Item::Trait(value) => ItemSnapshot::Trait {
            name: snapshot_identifier(&value.name),
            is_boundary: value.is_boundary,
            is_public: value.is_public,
            lifetime_parameters: value
                .lifetime_parameters
                .iter()
                .map(snapshot_identifier)
                .collect(),
            type_parameters: syntax_trees
                .items
                .type_parameters(value.type_parameters)
                .iter()
                .map(|parameter| snapshot_type_parameter(syntax_trees, parameter))
                .collect(),
            conformance_bounds: value
                .conformance_bounds
                .iter()
                .map(|bound| snapshot_generic_conformance_bound(syntax_trees, bound))
                .collect(),
            parents: syntax_trees
                .type_references
                .type_reference_handles(value.parents)
                .iter()
                .map(|parent| snapshot_type_reference_handle(syntax_trees, *parent))
                .collect(),
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
        Item::WireData(value) => ItemSnapshot::WireData {
            name: snapshot_identifier(&value.name),
            is_public: value.is_public,
            encoding: value.encoding.as_ref().map(snapshot_identifier),
            members: snapshot_wire_data_members(syntax_trees, value.members),
        },
    }
}

fn snapshot_wire_data_members(
    syntax_trees: &SyntaxTrees,
    members: psi_arena::HandleSpan<WireDataMember>,
) -> Vec<WireDataMemberSnapshot> {
    syntax_trees
        .items
        .wire_data_members(members)
        .iter()
        .map(|member| match member {
            WireDataMember::Field(field) => WireDataMemberSnapshot::Field {
                number: field.number,
                name: snapshot_identifier(&field.name),
                relevance: snapshot_binding_relevance(field.relevance),
                type_reference: snapshot_type_reference_handle(syntax_trees, field.type_reference),
            },
            WireDataMember::Reserved(reserved) => WireDataMemberSnapshot::Reserved {
                number: reserved.number,
            },
            WireDataMember::Version(version) => WireDataMemberSnapshot::Version {
                name: snapshot_identifier(&version.name),
                members: snapshot_wire_data_members(syntax_trees, version.members),
            },
        })
        .collect()
}

fn snapshot_operator(
    syntax_trees: &SyntaxTrees,
    operator: &crate::item::OperatorDefinition,
) -> OperatorSnapshot {
    OperatorSnapshot {
        is_public: operator.is_public,
        is_boundary: operator.is_boundary,
        name: snapshot_identifier_slice(syntax_trees.items.identifier_path_members(operator.name)),
        lifetime_parameters: operator
            .lifetime_parameters
            .iter()
            .map(snapshot_identifier)
            .collect(),
        type_parameters: syntax_trees
            .items
            .type_parameters(operator.type_parameters)
            .iter()
            .map(|parameter| snapshot_type_parameter(syntax_trees, parameter))
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
        token_count: operator.token_count,
    }
}

fn snapshot_type_parameter(
    syntax_trees: &SyntaxTrees,
    parameter: &crate::item::TypeParameter,
) -> TypeParameterSnapshot {
    let (kind, const_type, machine_contract, machine_requirement, proposition_contract) =
        match &parameter.kind {
            crate::item::TypeParameterKind::Type => ("type", None, None, None, None),
            crate::item::TypeParameterKind::Const { type_reference } => (
                "const",
                Some(snapshot_type_reference_handle(
                    syntax_trees,
                    *type_reference,
                )),
                None,
                None,
                None,
            ),
            crate::item::TypeParameterKind::Machine { contract } => match contract {
                Some(crate::item::MachineParameterContract::RequirementIdentity) => {
                    ("machine_requirement", None, None, None, None)
                }
                Some(crate::item::MachineParameterContract::Structural(signature)) => (
                    "machine",
                    None,
                    Some(snapshot_state_signature(syntax_trees, signature)),
                    None,
                    None,
                ),
                Some(crate::item::MachineParameterContract::Nominal { requirement }) => (
                    "machine",
                    None,
                    None,
                    Some(snapshot_identifier_slice(
                        syntax_trees.items.identifier_path_members(*requirement),
                    )),
                    None,
                ),
                None => ("machine", None, None, None, None),
            },
            crate::item::TypeParameterKind::Proposition { contract } => (
                "proposition",
                None,
                None,
                None,
                contract
                    .as_ref()
                    .map(|contract| PropositionParameterSignatureSnapshot {
                        name: snapshot_identifier(&contract.name),
                        parameters: syntax_trees
                            .items
                            .state_parameters(contract.parameters)
                            .iter()
                            .map(|handle| {
                                snapshot_state_parameter(
                                    syntax_trees,
                                    syntax_trees.items.state_parameter(*handle),
                                )
                            })
                            .collect(),
                    }),
            ),
        };
    TypeParameterSnapshot {
        name: snapshot_identifier(&parameter.name),
        kind,
        const_type,
        machine_contract,
        machine_requirement,
        proposition_contract,
        bounds: snapshot_data_properties(parameter.bounds),
    }
}

fn snapshot_data_properties(properties: crate::item::DataProperties) -> DataPropertiesSnapshot {
    use psi_language_core::{
        CarryAddress, CarryCpu, CarryHostThread, CarrySuspension, Multiplicity,
    };
    DataPropertiesSnapshot {
        multiplicity: match properties.multiplicity {
            Multiplicity::Unrestricted => "unrestricted",
            Multiplicity::Affine => "affine",
            Multiplicity::Linear => "linear",
        },
        carry: properties.carry.map(|carry| CarryPolicySnapshot {
            suspension: match carry.suspension {
                CarrySuspension::Forbidden => "forbidden",
                CarrySuspension::Allowed => "allowed",
            },
            cpu: match carry.cpu {
                CarryCpu::Origin => "same",
                CarryCpu::Any => "any",
            },
            thread: match carry.host_thread {
                CarryHostThread::Origin => "same",
                CarryHostThread::Any => "any",
            },
            address: match carry.address {
                CarryAddress::Stable => "stable",
                CarryAddress::Movable => "movable",
            },
        }),
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
    contracts: psi_arena::HandleSpan<CapabilityContract>,
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
            CapabilityContractKind::EnsuresForResultCase { result_case } => {
                CapabilityContractKindSnapshot::EnsuresForResultCase {
                    result_case: syntax_trees
                        .items
                        .identifier_path_members(*result_case)
                        .iter()
                        .map(snapshot_identifier)
                        .collect(),
                }
            }
            CapabilityContractKind::Requires => CapabilityContractKindSnapshot::Requires,
            CapabilityContractKind::Crashes { cause } => CapabilityContractKindSnapshot::Crashes {
                cause: match cause {
                    crate::item::CrashCause::Trap => "Trap",
                    crate::item::CrashCause::Abort => "Abort",
                },
            },
        },
        binding: contract.binding.as_ref().map(snapshot_identifier),
        facts: snapshot_proof_facts(syntax_trees, contract.facts),
        token_count: contract.token_count,
    }
}

fn snapshot_proof_facts(
    syntax_trees: &SyntaxTrees,
    facts: psi_arena::HandleSpan<ProofFact>,
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
            identity: field.identity,
            name: snapshot_identifier(&field.name),
            relevance: snapshot_binding_relevance(field.relevance),
            type_reference: snapshot_type_reference_handle(syntax_trees, field.type_reference),
        },
        DataMember::Variant(variant) => DataMemberSnapshot::Variant {
            identity: variant.identity,
            name: snapshot_identifier(&variant.name),
            payload: syntax_trees
                .items
                .data_payload_fields(variant.payload)
                .iter()
                .map(|field| DataPayloadFieldSnapshot {
                    identity: field.identity,
                    name: snapshot_identifier(&field.name),
                    relevance: snapshot_binding_relevance(field.relevance),
                    type_reference: snapshot_type_reference_handle(
                        syntax_trees,
                        field.type_reference,
                    ),
                })
                .collect(),
            retired_payload_identities: variant.retired_payload_identities.clone(),
        },
        DataMember::Retired(identity) => DataMemberSnapshot::Retired {
            identity: *identity,
        },
    }
}

fn snapshot_binding_relevance(relevance: psi_language_core::BindingRelevance) -> &'static str {
    match relevance {
        psi_language_core::BindingRelevance::Relevant => "relevant",
        psi_language_core::BindingRelevance::Erased => "erased",
    }
}

fn snapshot_satisfies_clause(
    syntax_trees: &SyntaxTrees,
    clause: &SatisfiesClause,
) -> SatisfiesClauseSnapshot {
    SatisfiesClauseSnapshot {
        trait_name: snapshot_identifier(&clause.trait_name),
        arguments: syntax_trees
            .type_references
            .type_reference_handles(clause.arguments)
            .iter()
            .map(|handle| snapshot_type_reference_handle(syntax_trees, *handle))
            .collect(),
        requirement: clause.requirement.as_ref().map(snapshot_identifier),
        alias: clause.alias.as_ref().map(snapshot_identifier),
        via: clause.via.as_ref().map(snapshot_external_binding),
    }
}

fn snapshot_generic_conformance_bound(
    syntax_trees: &SyntaxTrees,
    bound: &GenericConformanceBound,
) -> GenericConformanceBoundSnapshot {
    GenericConformanceBoundSnapshot {
        binder: bound.binder.as_ref().map(snapshot_identifier),
        subject: snapshot_identifier(&bound.subject),
        carrier: snapshot_identifier(&bound.carrier),
        arguments: syntax_trees
            .type_references
            .type_reference_handles(bound.arguments)
            .iter()
            .map(|handle| snapshot_type_reference_handle(syntax_trees, *handle))
            .collect(),
        conformance: bound.conformance.as_ref().map(snapshot_identifier),
    }
}

fn snapshot_external_binding(binding: &ExternalBinding) -> ExternalBindingSnapshot {
    match binding {
        ExternalBinding::Syscall { number } => ExternalBindingSnapshot::Syscall { number: *number },
        ExternalBinding::DllImport { module, symbol } => ExternalBindingSnapshot::DllImport {
            module: module.clone(),
            symbol: symbol.clone(),
        },
        ExternalBinding::CompilerIntrinsic => ExternalBindingSnapshot::CompilerIntrinsic,
        ExternalBinding::VtableSlot { index } => {
            ExternalBindingSnapshot::VtableSlot { index: *index }
        }
        ExternalBinding::VtableField { field } => ExternalBindingSnapshot::VtableField {
            field: snapshot_identifier(field),
        },
        ExternalBinding::TableFunction { field } => ExternalBindingSnapshot::TableFunction {
            field: snapshot_identifier(field),
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

fn snapshot_state_signature(
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

fn snapshot_state_signature_node(
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
                .map(snapshot_static_argument)
                .collect(),
            arguments: syntax_trees
                .statements
                .expression_handles(call.arguments)
                .iter()
                .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
                .collect(),
            evidence_arguments: snapshot_identifier_slice(&call.evidence_arguments),
            acknowledgement_synthesized: call.operational_acknowledgement.origin
                == psi_language_core::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
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

fn reference_access_name(access: psi_language_core::ReferenceAccess) -> &'static str {
    match access {
        psi_language_core::ReferenceAccess::Shared => "shared",
        psi_language_core::ReferenceAccess::Mutable => "mutable",
        psi_language_core::ReferenceAccess::WriteOnly => "write_only",
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
            access,
            lifetime,
        } => TypeReferenceSnapshot::Reference {
            referee: Box::new(snapshot_type_reference_handle(syntax_trees, *referee)),
            access: reference_access_name(*access),
            lifetime: lifetime.as_ref().map(snapshot_identifier),
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
            length: snapshot_fixed_array_length(length),
        },
        TypeReferenceNode::Slice { element_type } => TypeReferenceSnapshot::Slice {
            element_type: Box::new(snapshot_type_reference_handle(syntax_trees, *element_type)),
        },
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } => TypeReferenceSnapshot::Generic {
            base_name: snapshot_identifier(base_name),
            lifetime_arguments: lifetime_arguments.iter().map(snapshot_identifier).collect(),
            arguments: syntax_trees
                .type_references
                .type_reference_handles(*arguments)
                .iter()
                .map(|handle| snapshot_type_reference_handle(syntax_trees, *handle))
                .collect(),
        },
        TypeReferenceNode::ConstExpression(expression) => TypeReferenceSnapshot::ConstExpression {
            expression: snapshot_expression_handle(syntax_trees, *expression),
        },
        TypeReferenceNode::DynamicTrait { name, conformance } => {
            TypeReferenceSnapshot::DynamicTrait {
                name: snapshot_identifier(name),
                conformance: conformance.as_ref().map(snapshot_identifier),
            }
        }
        TypeReferenceNode::Named(name) => TypeReferenceSnapshot::Named {
            name: snapshot_identifier(name),
        },
        TypeReferenceNode::SelfType => TypeReferenceSnapshot::SelfType,
        TypeReferenceNode::Unit => TypeReferenceSnapshot::Unit,
    }
}

fn snapshot_fixed_array_length(length: &FixedArrayLength) -> FixedArrayLengthSnapshot {
    match length {
        FixedArrayLength::Literal(value) => FixedArrayLengthSnapshot::Literal { value: *value },
        FixedArrayLength::ConstParameter(name) => FixedArrayLengthSnapshot::ConstParameter {
            name: snapshot_identifier(name),
        },
        FixedArrayLength::ConstCall(name) => FixedArrayLengthSnapshot::ConstCall {
            name: snapshot_identifier(name),
        },
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
        TypeConstraintNode::Domain(domain) => TypeConstraintSnapshot::Domain {
            name: snapshot_identifier(&domain.name),
            arguments: syntax_trees
                .type_references
                .type_reference_handles(domain.arguments)
                .iter()
                .map(|argument| snapshot_type_reference_handle(syntax_trees, *argument))
                .collect(),
        },
        TypeConstraintNode::Range { minimum, maximum } => TypeConstraintSnapshot::Range {
            minimum: snapshot_expression_handle(syntax_trees, *minimum),
            maximum: snapshot_expression_handle(syntax_trees, *maximum),
        },
        TypeConstraintNode::ArithmeticDomain(domain) => TypeConstraintSnapshot::ArithmeticDomain {
            domain: domain.name().to_owned(),
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
            operator: snapshot_binary_operator(binary.operator),
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
                psi_language_core::cast_form::CastForm::Value => "value",
                psi_language_core::cast_form::CastForm::RecastShared => "recast_shared",
                psi_language_core::cast_form::CastForm::RecastMutable => "recast_mutable",
            },
            semantic_domain: snapshot_identifier_slice(
                syntax_trees
                    .expressions
                    .identifier_path_members(cast.semantic_domain),
            ),
            semantic_domain_arguments: syntax_trees
                .type_references
                .type_reference_handles(cast.semantic_domain_arguments)
                .iter()
                .map(|argument| snapshot_type_reference_handle(syntax_trees, *argument))
                .collect(),
        },
        ExpressionNode::Call(call) => snapshot_call_expression(syntax_trees, call),
        ExpressionNode::Float(value) => ExpressionSnapshot::Float {
            text: value.as_str().to_owned(),
        },
        ExpressionNode::Indexed(indexed) => ExpressionSnapshot::Indexed {
            collection: Box::new(snapshot_expression_handle(syntax_trees, indexed.collection)),
            index: Box::new(snapshot_expression_handle(syntax_trees, indexed.index)),
        },
        ExpressionNode::Integer(value) => ExpressionSnapshot::Integer {
            text: value.text().to_owned(),
        },
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
            case_variant: member.case_variant.as_ref().map(snapshot_identifier),
        },
        ExpressionNode::Borrow(value) => ExpressionSnapshot::Borrow {
            access: reference_access_name(value.access),
            value: Box::new(snapshot_expression_handle(syntax_trees, value.target)),
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
            end_inclusive: range.end_inclusive,
        },
        ExpressionNode::SelfValue => ExpressionSnapshot::SelfValue,
        ExpressionNode::StructLiteral(value) => ExpressionSnapshot::StructLiteral {
            type_name: snapshot_identifier(&value.type_name),
            case_name: value.case_name.as_ref().map(snapshot_identifier),
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
        machine_arguments: call
            .machine_arguments
            .iter()
            .map(snapshot_static_argument)
            .collect(),
        arguments: syntax_trees
            .expressions
            .expression_handles(call.arguments)
            .iter()
            .map(|handle| snapshot_expression_handle(syntax_trees, *handle))
            .collect(),
        evidence_arguments: snapshot_identifier_slice(&call.evidence_arguments),
        acknowledgement_synthesized: call.operational_acknowledgement.origin
            == psi_language_core::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
        acknowledges_suspend: call.operational_acknowledgement.acknowledges_suspend,
        acknowledges_block: call.operational_acknowledgement.acknowledges_block,
    }
}

fn snapshot_static_argument(
    argument: &crate::expression::StaticMachineArgument,
) -> StaticArgumentSnapshot {
    if let Some(literal) = &argument.const_literal {
        StaticArgumentSnapshot::Const(literal.text().to_owned())
    } else if let Some(projection) = &argument.evidence_projection {
        StaticArgumentSnapshot::EvidenceProjection {
            term: snapshot_identifier(&projection.term),
            member: snapshot_identifier(&projection.member),
        }
    } else if let Some(application) = &argument.application {
        StaticArgumentSnapshot::Application {
            path: snapshot_identifier_slice(&argument.path),
            lifetime_arguments: snapshot_identifier_slice(&application.lifetime_arguments),
            arguments: application
                .arguments
                .iter()
                .map(snapshot_static_argument)
                .collect(),
        }
    } else {
        StaticArgumentSnapshot::Path(snapshot_identifier_slice(&argument.path))
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
