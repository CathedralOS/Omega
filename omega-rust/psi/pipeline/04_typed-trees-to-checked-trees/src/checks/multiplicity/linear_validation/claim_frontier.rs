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
        // An owned `self` receiver is ordinary parameter syntax for the
        // attached data type: its authored `Named` reference spells the
        // machine, so its claims and whole multiplicity enumerate against the
        // attachment. Borrowed receivers spell a `Reference` node and keep
        // their ordinary (claim-free, unrestricted) reading.
        let owned_receiver = parameter.is_self
            && !crate::checks::multiplicity::linear_obligations::type_reference_is_reference(
                program,
                parameter.type_reference,
            );
        let (claims, receiver_multiplicity) = if owned_receiver {
            let (multiplicity, claims) = validation::owned_receiver_claims(program, machine_symbol);
            (
                claims
                    .into_iter()
                    .map(|claim| LinearClaimTemplate {
                        path: claim.path,
                        type_reference: if claim.type_reference.is_valid() {
                            claim.type_reference
                        } else {
                            parameter.type_reference
                        },
                        multiplicity: Multiplicity::Linear,
                        conditional: claim.conditional,
                    })
                    .collect::<Vec<_>>(),
                Some(multiplicity),
            )
        } else {
            (
                linear_claim_frontier(program, parameter.type_reference),
                None,
            )
        };
        // An owned `self` receiver's durable place root is the machine symbol:
        // `normalized_event_place_root` re-roots every `self`/`self.field`
        // event there because post-checked stages drop the receiver parameter
        // from their parameter lists.
        let place_symbol = if owned_receiver {
            machine_symbol
        } else {
            parameter.symbol
        };
        // Whole claim-free affine parameters are live on entry, but establish
        // no linear claim. Track their moves so a transferred parameter cannot
        // be moved again or disposed by the caller at exit.
        if claims.is_empty()
            && if let Some(multiplicity) = receiver_multiplicity {
                multiplicity == Multiplicity::Affine
            } else {
                type_multiplicity(program, parameter.type_reference) == Multiplicity::Affine
                    && matches!(
                        program
                            .type_reference_table
                            .type_reference(parameter.type_reference),
                        TypeReferenceNode::Named { .. }
                            | TypeReferenceNode::Generic { .. }
                            | TypeReferenceNode::FixedArray { .. }
                    )
            }
        {
            places.push(LinearPlace {
                symbol: place_symbol,
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
                symbol: place_symbol,
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
        // A mutable local with no initializer but a zero-constructible record
        // type owns its zeroed shape: the declaration itself establishes the
        // root (the written-target mint keys it to this statement). Fixed
        // arrays stay on the element-wise construction prefix even when their
        // elements are zero-constructible.
        let type_node = program
            .type_reference_table
            .type_reference(local.type_reference);
        let declaration_established = local.initial_value.is_valid()
            || (local.is_mutable
                && matches!(
                    type_node,
                    typed_trees::types::TypeReferenceNode::Named { .. }
                        | typed_trees::types::TypeReferenceNode::Generic { .. }
                )
                && validation::zeroed_structural_parts(program, local.type_reference));
        if claims.is_empty()
            && type_multiplicity(program, local.type_reference) == Multiplicity::Affine
            && declaration_established
            && matches!(
                type_node,
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
