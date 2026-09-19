//! Initial linear places and the linear claim frontier.

use crate::checks::multiplicity::linear_obligations::{LinearClaimTemplate, LinearPlace};
use crate::checks::multiplicity::linear_validation::permission_production::established_provenance;
use crate::checks::multiplicity::type_multiplicity::{data_field_name, type_multiplicity};
use language_semantics::{Multiplicity, PermissionEventSource};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn initial_linear_places(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Vec<LinearPlace> {
    let mut places = Vec::new();
    for parameter in program.state_parameters(state) {
        // A by-value `self` parameter is the language's terminal-consumer
        // form. The caller owns the consumption judgment.
        if parameter.is_self {
            continue;
        }
        let claims = linear_claim_frontier(program, parameter.type_reference);
        // Whole claim-free affine parameters are live on entry, but establish
        // no linear claim. Track their moves so a transferred parameter cannot
        // be moved again or disposed by the caller at exit.
        if claims.is_empty()
            && type_multiplicity(program, parameter.type_reference) == Multiplicity::Affine
            && matches!(
                program
                    .type_reference_table
                    .type_reference(parameter.type_reference),
                TypeReferenceNode::Named { .. }
                    | TypeReferenceNode::Generic { .. }
                    | TypeReferenceNode::FixedArray { .. }
            )
        {
            places.push(LinearPlace {
                symbol: parameter.symbol,
                name: parameter.name.as_str().to_owned(),
                path: Vec::new(),
                multiplicity: Multiplicity::Affine,
                claim_identity: None,
                provenance: None,
                live: true,
                ever_established: true,
                conditional: false,
                case_excluded: false,
            });
        }
        for claim in claims {
            places.push(LinearPlace {
                symbol: parameter.symbol,
                name: claim_place_name(program, parameter.name.as_str(), &claim.path),
                path: claim.path,
                multiplicity: claim.multiplicity,
                claim_identity: None,
                provenance: Some(established_provenance(
                    machine_symbol,
                    state_symbol,
                    PermissionEventSource::StateEntry,
                )),
                live: true,
                ever_established: true,
                conditional: claim.conditional,
                case_excluded: false,
            });
        }
    }
    for statement in program.statement_table.statements(state.statement_nodes) {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        let claims = linear_claim_frontier(program, local.type_reference);
        for claim in &claims {
            places.push(LinearPlace {
                symbol: local.symbol,
                name: claim_place_name(program, local.name.as_str(), &claim.path),
                path: claim.path.clone(),
                multiplicity: claim.multiplicity,
                claim_identity: None,
                provenance: None,
                live: false,
                ever_established: false,
                conditional: claim.conditional,
                case_excluded: false,
            });
        }
        // An explicitly initialized affine destination owns one whole value,
        // independently of which validated expression produced it. The ordinary
        // LocalData write establishes this root; later replacement settles the
        // old value before establishing the new one, including mutable locals.
        // Absent initializers remain outside this roster: partial zero-fill
        // construction needs its own initialization judgment.
        if claims.is_empty()
            && type_multiplicity(program, local.type_reference) == Multiplicity::Affine
            && local.initial_value.is_valid()
            && matches!(
                program
                    .type_reference_table
                    .type_reference(local.type_reference),
                typed_trees::types::TypeReferenceNode::Named { .. }
                    | typed_trees::types::TypeReferenceNode::Generic { .. }
                    | typed_trees::types::TypeReferenceNode::FixedArray { .. }
            )
        {
            places.push(LinearPlace {
                symbol: local.symbol,
                name: local.name.as_str().to_owned(),
                path: Vec::new(),
                multiplicity: Multiplicity::Affine,
                claim_identity: None,
                provenance: None,
                live: false,
                ever_established: false,
                conditional: false,
                case_excluded: false,
            });
        }
    }
    places
}

/// The exact linear claim frontier of a type, enumerated by the shared
/// validation walker so the checker's place roster and the lowering replay
/// cannot diverge. Templates keep the checker's fields; every frontier claim
/// is linear by construction and `conditional` rides the case-segment test.
pub(crate) fn linear_claim_frontier(
    program: &typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<LinearClaimTemplate> {
    validation::linear_claim_frontier(program, type_reference)
        .into_iter()
        .map(|claim| LinearClaimTemplate {
            path: claim.path,
            type_reference: claim.type_reference,
            multiplicity: Multiplicity::Linear,
            conditional: claim.conditional,
        })
        .collect()
}

fn claim_place_name(
    program: &typed_trees::TypedTrees,
    root: &str,
    path: &[facts::PlaceSegment],
) -> String {
    let mut name = root.to_owned();
    for segment in path {
        match segment {
            facts::PlaceSegment::Case { variant } => {
                let case = program.data_definitions().iter().find_map(|definition| {
                    program.data_members(definition).iter().find_map(|member| {
                        let typed_trees::data::DataMember::Variant(candidate) = member else {
                            return None;
                        };
                        (candidate.symbol == *variant).then_some(candidate.name.as_str())
                    })
                });
                name.push_str("::");
                name.push_str(case.unwrap_or("<case>"));
            }
            facts::PlaceSegment::Field { symbol } => {
                let field = data_field_name(program, *symbol);
                name.push('.');
                name.push_str(field.unwrap_or("<field>"));
            }
            facts::PlaceSegment::FixedIndex { index } => {
                name.push('[');
                name.push_str(&index.to_string());
                name.push(']');
            }
            facts::PlaceSegment::FixedRange { start, end } => {
                name.push('[');
                name.push_str(&start.to_string());
                name.push_str("..");
                name.push_str(&end.to_string());
                name.push(']');
            }
            facts::PlaceSegment::Index { .. } => name.push_str("[<index>]"),
        }
    }
    name
}
