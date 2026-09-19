//! Snapshots of the declarations: constants, operators, propositions, data
//! and quotient definitions, domains and proof facts.

use crate::TypedTrees;
use crate::data::{DataDefinition, DataMember};
use crate::domain::{DomainDefinition, ProofFact};
use crate::mathematical::{MathematicalBody, MathematicalType};
use crate::operator::OperatorDefinition;
use crate::proposition::{
    PropositionBinderKind, PropositionBody, PropositionDefinition, PropositionFormula,
};
use crate::typed_trees::inspection::snapshot::machine_snapshots::state_parameter_snapshot;
use crate::typed_trees::inspection::snapshot::statement_and_expression_snapshots::expression_snapshot;
use crate::typed_trees::inspection::snapshot::type_snapshots::type_reference_snapshot;
use crate::typed_trees::inspection::snapshot::{
    ExpressionSnapshot, StateParameterSnapshot, TypeReferenceSnapshot,
};
use serde::Serialize;

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

pub(crate) fn operator_snapshot(
    program: &TypedTrees,
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
    Transparent { formula: PropositionFormulaSnapshot },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PropositionFormulaSnapshot {
    Application {
        proposition_symbol: u32,
        name: String,
        binder_arguments: Vec<Vec<String>>,
        arguments: Vec<ExpressionSnapshot>,
    },
    BooleanExpression {
        expression: ExpressionSnapshot,
    },
}

pub(crate) fn proposition_snapshot(
    program: &TypedTrees,
    proposition: &PropositionDefinition,
) -> PropositionSnapshot {
    PropositionSnapshot {
        has_symbol: proposition.symbol.is_valid(),
        name: proposition.name.to_string(),
        is_public: proposition.is_public,
        binders: program
            .proposition_binders(proposition)
            .iter()
            .map(|binder| {
                let (kind, const_type) = match binder.kind {
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
            .proposition_parameters(proposition)
            .iter()
            .map(|parameter| state_parameter_snapshot(program, parameter))
            .collect(),
        body: match &proposition.body {
            PropositionBody::Primitive => PropositionBodySnapshot::Primitive,
            PropositionBody::Witness { evidence } => PropositionBodySnapshot::Witness {
                evidence: type_reference_snapshot(program, *evidence),
            },
            PropositionBody::Transparent { proposition } => PropositionBodySnapshot::Transparent {
                formula: match proposition {
                    PropositionFormula::Application(application) => {
                        proposition_formula_application_snapshot(program, application)
                    }
                    PropositionFormula::BooleanExpression(expression) => {
                        PropositionFormulaSnapshot::BooleanExpression {
                            expression: expression_snapshot(program, *expression),
                        }
                    }
                },
            },
        },
    }
}

fn proposition_formula_application_snapshot(
    program: &TypedTrees,
    application: &crate::proposition::PropositionApplication,
) -> PropositionFormulaSnapshot {
    PropositionFormulaSnapshot::Application {
        proposition_symbol: application.proposition.arena_index(),
        name: application.name.to_string(),
        binder_arguments: application
            .binder_arguments
            .iter()
            .map(|argument| vec![argument.display_name()])
            .collect(),
        arguments: program
            .expression_table
            .expression_handles(application.arguments)
            .iter()
            .map(|argument| expression_snapshot(program, *argument))
            .collect(),
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub type_parameters: Vec<DomainTypeParameterSnapshot>,
    pub target_type: TypeReferenceSnapshot,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub index_arguments: Vec<TypeReferenceSnapshot>,
    pub is_public: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub alias: Vec<DomainAliasConstituentSnapshot>,
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
        #[serde(skip_serializing_if = "Vec::is_empty")]
        domain_arguments: Vec<TypeReferenceSnapshot>,
    },
    Proposition {
        proposition_symbol: u32,
        name: String,
        binder_arguments: Vec<Vec<String>>,
        arguments: Vec<ExpressionSnapshot>,
    },
}

pub(crate) fn data_definition_snapshot(
    program: &TypedTrees,
    data: &DataDefinition,
) -> DataDefinitionSnapshot {
    DataDefinitionSnapshot {
        name: data.name.to_string(),
        is_public: data.is_public,
        supply: match data.supply_mode {
            language_semantics::DataSupplyMode::CheckedShape => "checked_shape",
            language_semantics::DataSupplyMode::BoundaryOpaque => "boundary_opaque",
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
        generic_instance: data
            .generic_instance
            .map(|origin| type_reference_snapshot(program, origin)),
        quotient: data
            .quotient
            .as_ref()
            .map(|quotient| QuotientDefinitionSnapshot {
                carrier: type_reference_snapshot(program, quotient.carrier),
                relation: quotient.relation.iter().map(ToString::to_string).collect(),
                relation_symbol: quotient.relation_symbol.arena_index(),
                equivalence: quotient.equivalence.as_ref().map(|selection| {
                    QuotientEquivalenceSelectionSnapshot {
                        relation: selection.relation.iter().map(ToString::to_string).collect(),
                        relation_symbol: selection.relation_symbol.arena_index(),
                        trait_name: selection.trait_name.to_string(),
                        trait_symbol: selection.trait_symbol.arena_index(),
                        trait_arguments: program
                            .type_reference_table
                            .type_reference_handles(selection.trait_arguments)
                            .iter()
                            .map(|argument| type_reference_snapshot(program, *argument))
                            .collect(),
                        conformance_name: selection.conformance_name.to_string(),
                        conformance_symbol: selection.conformance_symbol.arena_index(),
                    }
                }),
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
            relevance: snapshot_binding_relevance(field.relevance),
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
                    relevance: snapshot_binding_relevance(field.relevance),
                    type_reference: type_reference_snapshot(program, field.type_reference),
                })
                .collect(),
            retired_payload_identities: variant.retired_payload_identities.clone(),
        },
    }
}

pub(crate) fn snapshot_binding_relevance(
    relevance: language_core::BindingRelevance,
) -> &'static str {
    match relevance {
        language_core::BindingRelevance::Relevant => "relevant",
        language_core::BindingRelevance::Erased => "erased",
    }
}

pub(crate) fn domain_definition_snapshot(
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
                    crate::data::TypeParameterKind::Value { .. } => "value",
                    crate::data::TypeParameterKind::Machine { .. } => "machine",
                    crate::data::TypeParameterKind::Proposition { .. } => "proposition",
                },
                const_type: match parameter.kind {
                    crate::data::TypeParameterKind::Const { type_reference }
                    | crate::data::TypeParameterKind::Value { type_reference } => {
                        Some(type_reference_snapshot(program, type_reference))
                    }
                    crate::data::TypeParameterKind::Type
                    | crate::data::TypeParameterKind::Machine { .. }
                    | crate::data::TypeParameterKind::Proposition { .. } => None,
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
        classification: domain.classification.map(|value| value.as_str()),
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
        semantic_clause_token_count: domain.semantic_clause_token_count,
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
                domain_arguments: program
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments)
                    .iter()
                    .map(|argument| type_reference_snapshot(program, *argument))
                    .collect(),
            },
            ProofFact::Proposition(application) => ProofFactSnapshot::Proposition {
                proposition_symbol: application.proposition.arena_index(),
                name: application.name.to_string(),
                binder_arguments: application
                    .binder_arguments
                    .iter()
                    .map(|argument| vec![argument.display_name()])
                    .collect(),
                arguments: program
                    .expression_table
                    .expression_handles(application.arguments)
                    .iter()
                    .map(|argument| expression_snapshot(program, *argument))
                    .collect(),
            },
        })
        .collect()
}

pub(crate) fn semantic_roles_snapshot(
    roles: language_semantics::DomainSemanticRoles,
) -> DomainSemanticRolesSnapshot {
    DomainSemanticRolesSnapshot {
        denotation_dimension: roles.denotation_dimension.map(|semantic| semantic.0),
        arithmetic_policy: roles.arithmetic_policy.map(|semantic| semantic.0),
    }
}

pub(crate) fn establishment_route_snapshot(
    route: language_semantics::DomainEstablishmentRoute,
) -> DomainEstablishmentRouteSnapshot {
    let requirement = route.requirement_symbol();
    DomainEstablishmentRouteSnapshot {
        kind: route.kind_name(),
        source_symbol: route.source_symbol().arena_index(),
        requirement_symbol: requirement.is_valid().then(|| requirement.arena_index()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MathematicalDefinitionSnapshot {
    pub has_symbol: bool,
    pub name: String,
    pub is_public: bool,
    pub binders: Vec<MathematicalBinderSnapshot>,
    pub parameters: Vec<MathematicalParameterSnapshot>,
    pub result: MathematicalTypeSnapshot,
    pub body: MathematicalBodySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MathematicalBinderSnapshot {
    pub has_symbol: bool,
    pub name: String,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carrier: Option<TypeReferenceSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MathematicalParameterSnapshot {
    pub has_symbol: bool,
    pub name: String,
    pub relevance: &'static str,
    pub ty: MathematicalTypeSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MathematicalTypeSnapshot {
    Ordinary {
        reference: TypeReferenceSnapshot,
    },
    Arrow {
        #[serde(skip_serializing_if = "Option::is_none")]
        binder: Option<String>,
        domain: Box<MathematicalTypeSnapshot>,
        codomain: Box<MathematicalTypeSnapshot>,
    },
    Application {
        callee: Box<MathematicalTypeSnapshot>,
        arguments: Vec<ExpressionSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MathematicalBodySnapshot {
    Assumption,
    Definition { term: ExpressionSnapshot },
}

fn mathematical_type_snapshot(
    program: &TypedTrees,
    handle: crate::mathematical::MathematicalTypeHandle,
) -> MathematicalTypeSnapshot {
    match program.mathematical_type(handle) {
        MathematicalType::Ordinary(type_reference) => MathematicalTypeSnapshot::Ordinary {
            reference: type_reference_snapshot(program, *type_reference),
        },
        MathematicalType::Arrow {
            binder,
            domain,
            codomain,
        } => MathematicalTypeSnapshot::Arrow {
            binder: binder.as_ref().map(|binder| binder.to_string()),
            domain: Box::new(mathematical_type_snapshot(program, *domain)),
            codomain: Box::new(mathematical_type_snapshot(program, *codomain)),
        },
        MathematicalType::Application { callee, arguments } => {
            MathematicalTypeSnapshot::Application {
                callee: Box::new(mathematical_type_snapshot(program, *callee)),
                arguments: program
                    .expression_table
                    .expression_handles(*arguments)
                    .iter()
                    .map(|argument| expression_snapshot(program, *argument))
                    .collect(),
            }
        }
    }
}

fn mathematical_binder_snapshot(
    program: &TypedTrees,
    parameter: &crate::data::TypeParameter,
) -> MathematicalBinderSnapshot {
    let (kind, carrier) = match &parameter.kind {
        crate::data::TypeParameterKind::Type => ("type", None),
        crate::data::TypeParameterKind::Const { type_reference }
        | crate::data::TypeParameterKind::Value { type_reference } => {
            let kind = match &parameter.kind {
                crate::data::TypeParameterKind::Const { .. } => "const",
                _ => "value",
            };
            (
                kind,
                Some(type_reference_snapshot(program, *type_reference)),
            )
        }
        crate::data::TypeParameterKind::Machine { .. } => ("machine", None),
        crate::data::TypeParameterKind::Proposition { .. } => ("proposition", None),
    };
    MathematicalBinderSnapshot {
        has_symbol: parameter.symbol.is_valid(),
        name: parameter.name.to_string(),
        kind,
        carrier,
    }
}

pub(crate) fn mathematical_definition_snapshot(
    program: &TypedTrees,
    definition: &crate::mathematical::MathematicalDefinition,
) -> MathematicalDefinitionSnapshot {
    MathematicalDefinitionSnapshot {
        has_symbol: definition.symbol.is_valid(),
        name: definition.name.to_string(),
        is_public: definition.is_public,
        binders: program
            .data_type_parameters
            .span_or_empty(definition.binders)
            .iter()
            .map(|binder| mathematical_binder_snapshot(program, binder))
            .collect(),
        parameters: program
            .mathematical_parameters(definition.parameters)
            .iter()
            .map(|parameter| MathematicalParameterSnapshot {
                has_symbol: parameter.symbol.is_valid(),
                name: parameter.name.to_string(),
                relevance: snapshot_binding_relevance(parameter.relevance),
                ty: mathematical_type_snapshot(program, parameter.ty),
            })
            .collect(),
        result: mathematical_type_snapshot(program, definition.result),
        body: match &definition.body {
            MathematicalBody::Assumption => MathematicalBodySnapshot::Assumption,
            MathematicalBody::Definition(term) => MathematicalBodySnapshot::Definition {
                term: expression_snapshot(program, *term),
            },
        },
    }
}
