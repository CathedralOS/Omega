//! Snapshots of type references, constraints and wire schemas.

use crate::SymbolResolvedTrees;
use crate::symbol_resolved_trees::inspection::snapshot::ExpressionSnapshot;
use crate::symbol_resolved_trees::inspection::snapshot::declaration_snapshots::snapshot_binding_relevance;
use crate::symbol_resolved_trees::inspection::snapshot::statement_and_expression_snapshots::table_expression_snapshot;
use crate::types::{TypeConstraint, TypeReference};
use crate::wire::{WireMember, WireSchema};
use arena::HandleSpan;
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
        end_inclusive: bool,
    },
    ArithmeticDomain {
        domain: String,
    },
    Domain {
        name: String,
        arguments: Vec<TypeReferenceSnapshot>,
    },
}

pub(crate) fn type_reference_snapshot(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> TypeReferenceSnapshot {
    type_reference_snapshot_from_program(program, type_reference)
}

pub(crate) fn wire_schema_snapshot(
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

pub(crate) fn reference_access_name(access: language_core::ReferenceAccess) -> &'static str {
    match access {
        language_core::ReferenceAccess::Shared => "shared",
        language_core::ReferenceAccess::Mutable => "mutable",
        language_core::ReferenceAccess::WriteOnly => "write_only",
    }
}

pub(crate) fn type_reference_snapshot_from_program(
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
        TypeConstraint::Range {
            minimum,
            maximum,
            end_inclusive,
        } => TypeConstraintSnapshot::Range {
            minimum: table_expression_snapshot(program, *minimum),
            maximum: table_expression_snapshot(program, *maximum),
            end_inclusive: *end_inclusive,
        },
        TypeConstraint::ArithmeticDomain(domain) => TypeConstraintSnapshot::ArithmeticDomain {
            domain: domain.name().to_owned(),
        },
    }
}
