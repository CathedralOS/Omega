//! Snapshots of items: data, quotients, operators, capabilities,
//! conformances, propositions, proof facts and external bindings.

use crate::item::{
    CapabilityContract, CapabilityContractKind, CapabilityMember, DataMember, ExternalBinding,
    GenericConformanceBound, Item, ProofFact, PropositionBody, SatisfiesClause,
};
use crate::syntax_trees::SyntaxTrees;
use crate::syntax_trees::inspection::snapshot::expression_and_type_snapshots::{
    snapshot_expression_handle, snapshot_identifier, snapshot_identifier_slice,
    snapshot_static_argument, snapshot_type_reference_handle,
};
use crate::syntax_trees::inspection::snapshot::state_and_statement_snapshots::{
    snapshot_state_node, snapshot_state_parameter, snapshot_state_signature,
    snapshot_state_signature_node,
};
use crate::syntax_trees::inspection::snapshot::{
    ExpressionSnapshot, IdentifierSnapshot, StateParameterSnapshot, StateSignatureSnapshot,
    StateSnapshot, StaticArgumentSnapshot, TypeReferenceSnapshot,
};
use serde::Serialize;

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
        #[serde(skip_serializing_if = "Vec::is_empty")]
        trait_lifetime_arguments: Vec<IdentifierSnapshot>,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        spelling: Option<&'static str>,
        is_public: bool,
        bodyless: bool,
        target: Option<IdentifierSnapshot>,
        boundary: bool,
        is_top_level_boundary_requirement: bool,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        lifetime_parameters: Vec<IdentifierSnapshot>,
        type_parameters: Vec<TypeParameterSnapshot>,
        satisfies: Vec<SatisfiesClauseSnapshot>,
        conformance_bounds: Vec<GenericConformanceBoundSnapshot>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        where_facts: Vec<ProofFactSnapshot>,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SatisfiesClauseSnapshot {
    pub trait_name: IdentifierSnapshot,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lifetime_arguments: Vec<IdentifierSnapshot>,
    pub arguments: Vec<TypeReferenceSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement: Option<IdentifierSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<IdentifierSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via: Option<ExternalBindingSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via_expression: Option<Box<ExpressionSnapshot>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GenericConformanceBoundSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binder: Option<IdentifierSnapshot>,
    pub subject: IdentifierSnapshot,
    pub carrier: IdentifierSnapshot,
    pub arguments: Vec<TypeReferenceSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_conformance: Option<StaticArgumentSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExternalBindingSnapshot {
    Syscall { number: i64 },
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
        domain_arguments: Vec<TypeReferenceSnapshot>,
    },
}

pub(crate) fn snapshot_item(syntax_trees: &SyntaxTrees, item: &Item) -> ItemSnapshot {
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
            trait_lifetime_arguments: value
                .trait_lifetime_arguments
                .iter()
                .map(snapshot_identifier)
                .collect(),
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
                language_core::DataSupplyMode::CheckedShape => "checked_shape",
                language_core::DataSupplyMode::BoundaryOpaque => "boundary_opaque",
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
            where_facts: syntax_trees
                .items
                .proof_facts(value.where_facts)
                .iter()
                .map(|fact| snapshot_proof_fact(syntax_trees, fact))
                .collect(),
            name: snapshot_identifier(&value.name),
            attached_data: value.attached_data.as_ref().map(snapshot_identifier),
            spelling: value.spelling.map(|spelling| spelling.symbol()),
            is_public: value.is_public,
            bodyless: value.bodyless,
            target: value.target.as_ref().map(snapshot_identifier),
            boundary: value.boundary,
            is_top_level_boundary_requirement: value.is_top_level_boundary_requirement,
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
    }
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

pub(crate) fn snapshot_type_parameter(
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
            crate::item::TypeParameterKind::Value { type_reference } => (
                "value",
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
    use language_core::{CarryAddress, CarryCpu, CarryHostThread, CarrySuspension, Multiplicity};
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

pub(crate) fn snapshot_capability_contracts(
    syntax_trees: &SyntaxTrees,
    contracts: arena::HandleSpan<CapabilityContract>,
) -> Vec<CapabilityContractSnapshot> {
    syntax_trees
        .items
        .capability_contracts(contracts)
        .iter()
        .map(|contract| snapshot_capability_contract(syntax_trees, contract))
        .collect()
}

pub(crate) fn snapshot_capability_contract(
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
    facts: arena::HandleSpan<ProofFact>,
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
            domain_arguments: syntax_trees
                .type_references
                .type_reference_handles(membership.domain_arguments)
                .iter()
                .map(|argument| snapshot_type_reference_handle(syntax_trees, *argument))
                .collect(),
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

pub(super) fn snapshot_binding_relevance(
    relevance: language_core::BindingRelevance,
) -> &'static str {
    match relevance {
        language_core::BindingRelevance::Relevant => "relevant",
        language_core::BindingRelevance::Erased => "erased",
    }
}

fn snapshot_satisfies_clause(
    syntax_trees: &SyntaxTrees,
    clause: &SatisfiesClause,
) -> SatisfiesClauseSnapshot {
    SatisfiesClauseSnapshot {
        trait_name: snapshot_identifier(&clause.trait_name),
        lifetime_arguments: clause
            .lifetime_arguments
            .iter()
            .map(snapshot_identifier)
            .collect(),
        arguments: syntax_trees
            .type_references
            .type_reference_handles(clause.arguments)
            .iter()
            .map(|handle| snapshot_type_reference_handle(syntax_trees, *handle))
            .collect(),
        requirement: clause.requirement.as_ref().map(snapshot_identifier),
        alias: clause.alias.as_ref().map(snapshot_identifier),
        via: clause.via.as_ref().map(snapshot_external_binding),
        via_expression: clause.via_expression.is_valid().then(|| {
            Box::new(snapshot_expression_handle(
                syntax_trees,
                clause.via_expression,
            ))
        }),
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
        selected_conformance: bound
            .selected_conformance
            .as_ref()
            .map(|argument| snapshot_static_argument(syntax_trees, argument)),
    }
}

fn snapshot_external_binding(binding: &ExternalBinding) -> ExternalBindingSnapshot {
    match binding {
        ExternalBinding::Syscall { number } => ExternalBindingSnapshot::Syscall { number: *number },
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
