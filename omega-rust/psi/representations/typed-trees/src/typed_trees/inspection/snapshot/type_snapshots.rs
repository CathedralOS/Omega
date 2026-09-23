//! Snapshots of type references, constraints, open index normalizations and
//! wire schemas.

use crate::TypedTrees;
use crate::name::Identifier;
use crate::typed_trees::inspection::snapshot::declaration_snapshots::{
    establishment_route_snapshot, semantic_roles_snapshot, snapshot_binding_relevance,
};
use crate::typed_trees::inspection::snapshot::statement_and_expression_snapshots::expression_snapshot;
use crate::typed_trees::inspection::snapshot::{
    DomainEstablishmentRouteSnapshot, DomainSemanticRolesSnapshot, ExpressionSnapshot,
};
use crate::types::{
    DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use serde::Serialize;

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
        #[serde(skip_serializing_if = "Option::is_none")]
        normalization: Option<OpenIndexNormalizationSnapshot>,
    },
    DynamicTrait {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        conformance: Option<String>,
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
pub struct OpenIndexNormalizationSnapshot {
    pub index_type: Box<TypeReferenceSnapshot>,
    pub normalizer_version: u32,
    pub operations: Vec<OpenIndexOperationSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpenIndexOperationSnapshot {
    pub expression: ExpressionSnapshot,
    pub spelling: &'static str,
    pub operation_contract_identity: String,
    pub provider: String,
    pub algebra_trait: String,
    pub algebra_requirement: String,
    pub algebra_alias: Option<String>,
    pub commutativity_licensed: bool,
    pub associativity_licensed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeConstraintSnapshot {
    Named {
        name: String,
    },
    Domain {
        name: String,
        subject: DomainConstraintSubjectSnapshot,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        arguments: Vec<TypeReferenceSnapshot>,
        symbol: u32,
        semantic_id: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        classification: Option<&'static str>,
        predicate_body: &'static str,
        semantic_roles: DomainSemanticRolesSnapshot,
        establishment_routes: Vec<DomainEstablishmentRouteSnapshot>,
    },
    Range {
        minimum: ExpressionSnapshot,
        maximum: ExpressionSnapshot,
        end_inclusive: bool,
    },
    ArithmeticDomain {
        domain: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DomainConstraintSubjectSnapshot {
    Declared,
    Carry { permission: &'static str },
    Value { domain: &'static str },
    OmegaLayout { grammar: &'static str },
}

pub(crate) fn wire_schema_snapshot(
    program: &TypedTrees,
    wire_schema: &crate::wire::WireSchema,
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
    program: &TypedTrees,
    members: arena::HandleSpan<crate::wire::WireMember>,
) -> Vec<WireMemberSnapshot> {
    program
        .wire_members(members)
        .iter()
        .map(|member| match member {
            crate::wire::WireMember::Field(field) => WireMemberSnapshot::Field {
                number: field.number,
                name: field.name.to_string(),
                relevance: snapshot_binding_relevance(field.relevance),
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

pub(crate) fn reference_access_name(access: language_core::ReferenceAccess) -> &'static str {
    match access {
        language_core::ReferenceAccess::Shared => "shared",
        language_core::ReferenceAccess::Mutable => "mutable",
        language_core::ReferenceAccess::WriteOnly => "write_only",
    }
}

pub(crate) fn type_reference_snapshot_option(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceSnapshot> {
    type_reference
        .is_valid()
        .then(|| type_reference_snapshot(program, type_reference))
}

pub(crate) fn type_reference_snapshot(
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
            access,
            // Lifetime omitted from the structural snapshot (a borrow-region tag,
            // not part of the type's shape).
            lifetime: _,
        } => TypeReferenceSnapshot::Reference {
            referee: Box::new(type_reference_snapshot(program, *referee)),
            access: reference_access_name(*access),
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
        TypeReferenceNode::ConstExpression(expression) => TypeReferenceSnapshot::ConstExpression {
            expression: expression_snapshot(program, *expression),
            normalization: program
                .open_index_normalizations
                .iter()
                .find(|normalization| normalization.expression == *expression)
                .map(|normalization| OpenIndexNormalizationSnapshot {
                    index_type: Box::new(type_reference_snapshot(
                        program,
                        normalization.index_type,
                    )),
                    normalizer_version: normalization.normalizer_version,
                    operations: normalization
                        .operations
                        .iter()
                        .map(|operation| OpenIndexOperationSnapshot {
                            expression: expression_snapshot(program, operation.expression),
                            spelling: operation.spelling.symbol(),
                            operation_contract_identity: operation
                                .operation_contract_identity
                                .clone(),
                            provider: program.symbols.display_path(operation.provider, "::"),
                            algebra_trait: program
                                .symbols
                                .display_path(operation.algebra_trait, "::"),
                            algebra_requirement: operation.algebra_requirement.clone(),
                            algebra_alias: operation.algebra_alias.clone(),
                            commutativity_licensed: operation.commutativity_licensed,
                            associativity_licensed: operation.associativity_licensed,
                        })
                        .collect(),
                }),
        },
        TypeReferenceNode::DynamicTrait {
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
        TypeReferenceNode::Named { name, .. } => TypeReferenceSnapshot::Named {
            name: name.to_string(),
        },
        TypeReferenceNode::Unit => TypeReferenceSnapshot::Unit,
    }
}

pub(crate) fn type_constraint_snapshot(
    program: &TypedTrees,
    constraint: &TypeConstraintNode,
) -> TypeConstraintSnapshot {
    match constraint {
        TypeConstraintNode::Named(name) => TypeConstraintSnapshot::Named {
            name: name.to_string(),
        },
        TypeConstraintNode::Domain(domain) => TypeConstraintSnapshot::Domain {
            name: domain.name.to_string(),
            subject: match domain.subject {
                DomainConstraintSubject::Declared => DomainConstraintSubjectSnapshot::Declared,
                DomainConstraintSubject::Carry(permission) => {
                    DomainConstraintSubjectSnapshot::Carry {
                        permission: permission.name(),
                    }
                }
                DomainConstraintSubject::Value(value_domain) => {
                    DomainConstraintSubjectSnapshot::Value {
                        domain: value_domain.name(),
                    }
                }
                DomainConstraintSubject::OmegaLayout { grammar } => {
                    DomainConstraintSubjectSnapshot::OmegaLayout {
                        grammar: grammar.as_str(),
                    }
                }
            },
            arguments: domain
                .arguments
                .iter()
                .map(|argument| type_reference_snapshot(program, *argument))
                .collect(),
            symbol: domain.symbol.arena_index(),
            semantic_id: domain.semantic_id.0,
            classification: domain.classification.map(|value| value.as_str()),
            predicate_body: domain.predicate_body.as_str(),
            semantic_roles: semantic_roles_snapshot(domain.semantic_roles),
            establishment_routes: domain
                .establishment_routes
                .iter()
                .copied()
                .map(establishment_route_snapshot)
                .collect(),
        },
        TypeConstraintNode::Range {
            minimum,
            maximum,
            end_inclusive,
        } => TypeConstraintSnapshot::Range {
            minimum: expression_snapshot(program, *minimum),
            maximum: expression_snapshot(program, *maximum),
            end_inclusive: *end_inclusive,
        },
        TypeConstraintNode::ArithmeticDomain(domain) => TypeConstraintSnapshot::ArithmeticDomain {
            domain: domain.name().to_owned(),
        },
    }
}

pub(crate) fn path_snapshot(path: &[Identifier]) -> Vec<String> {
    path.iter().map(ToString::to_string).collect()
}

#[cfg(test)]
mod termination_vocabulary_tests {
    use crate::snapshot::ProgressPremiseSnapshot;
    use crate::snapshot::TerminationGuaranteeSnapshot;

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
