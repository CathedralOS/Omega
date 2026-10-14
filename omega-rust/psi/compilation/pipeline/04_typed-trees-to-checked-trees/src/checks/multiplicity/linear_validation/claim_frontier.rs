//! Initial linear places and the linear claim frontier.

use crate::checks::multiplicity::linear_obligations::{LinearClaimTemplate, LinearPlace};
use crate::checks::multiplicity::linear_validation::permission_production::established_provenance;
use crate::checks::multiplicity::type_multiplicity::{data_field_name, type_multiplicity};
use language_semantics::{Multiplicity, PermissionEventSource};
use symbols::SymbolHandle;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn initial_linear_places(
    program: &dyn ::validation::CheckingInput,
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
        let owned_receiver = false
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
                        program.type_reference_table.type_reference(
                            validation::peel_declared_constraints(
                                program,
                                parameter.type_reference,
                            )
                        ),
                        TypeReferenceNode::Named { .. }
                            | TypeReferenceNode::Generic { .. }
                            | TypeReferenceNode::FixedArray { .. }
                    )
            }
        {
            places.push(LinearPlace {
                root: facts::PlaceRoot::Symbol(place_symbol),
                name: parameter.name.as_str().to_owned(),
                path: Vec::new(),
                multiplicity: Multiplicity::Affine,
                claim_identity: None,
                // Claim-free does not mean unestablished: selected cleanup
                // must retain this exact incoming storage occurrence too.
                provenance: Some(established_provenance(
                    machine_symbol,
                    state_symbol,
                    PermissionEventSource::StateEntry,
                )),
                live: true,
                ever_established: true,
                zero_image: false,
                conditional: false,
                case_excluded: false,
            });
        }
        for claim in claims {
            places.push(LinearPlace {
                root: facts::PlaceRoot::Symbol(place_symbol),
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
                zero_image: false,
                conditional: claim.conditional,
                case_excluded: false,
            });
        }
    }
    for statement in program.statement_table.statements(state.statement_nodes) {
        let Some(local) = statement.as_local_data() else {
            continue;
        };
        let claims = linear_claim_frontier(program, local.type_reference);
        for claim in &claims {
            places.push(LinearPlace {
                root: facts::PlaceRoot::Symbol(local.symbol),
                name: claim_place_name(program, local.name.as_str(), &claim.path),
                path: claim.path.clone(),
                multiplicity: claim.multiplicity,
                claim_identity: None,
                provenance: None,
                live: false,
                ever_established: false,
                zero_image: false,
                conditional: claim.conditional,
                case_excluded: false,
            });
        }
        // An explicitly initialized affine destination owns one whole value,
        // independently of which validated expression produced it. The ordinary
        // LocalData write establishes this root; later replacement settles the
        // old value before establishing the new one, including mutable locals.
        // A local with no initializer owns its declared shape: the
        // declaration itself establishes the root (the written-target mint
        // keys it to this statement). Member operations fill each leaf;
        // subsequent element stores replace established contents rather
        // than minting the root.
        // Qualifications change evidence, not whether this affine carrier
        // owns storage. Reserve the same occurrence beneath constraints so
        // construction, selected moves and cleanup retain their normal
        // establishment/transfer events even with an empty linear frontier.
        let type_node =
            program
                .type_reference_table
                .type_reference(validation::peel_declared_constraints(
                    program,
                    local.type_reference,
                ));
        let declaration_established = local.initial_value.is_valid()
            || matches!(
                type_node,
                typed_trees::types::TypeReferenceNode::Named { .. }
                    | typed_trees::types::TypeReferenceNode::Generic { .. }
                    | typed_trees::types::TypeReferenceNode::FixedArray { .. }
            );
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
                root: facts::PlaceRoot::Symbol(local.symbol),
                name: local.name.as_str().to_owned(),
                path: Vec::new(),
                multiplicity: Multiplicity::Affine,
                claim_identity: None,
                provenance: None,
                live: false,
                ever_established: false,
                zero_image: false,
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
    program: &dyn ::validation::CheckingInput,
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

/// Reserve custody at a real producer root. No claim exists before the value
/// completes; permission production preserves transferred identities or mints
/// the genuinely new claims at that completion.
pub(crate) fn reserve_owner_places(
    program: &dyn ::validation::CheckingInput,
    root: facts::PlaceRoot,
    type_reference: TypeReferenceHandle,
    places: &mut Vec<LinearPlace>,
) {
    if places.iter().any(|place| place.root == root) {
        return;
    }
    let name = program.normalized_type_identity_text(type_reference);
    let claims = linear_claim_frontier(program, type_reference);
    if claims.is_empty()
        && validation::reference_result_custody::binding_multiplicity(program, type_reference)
            == language_semantics::Multiplicity::Affine
    {
        places.push(LinearPlace {
            root,
            name,
            path: Vec::new(),
            multiplicity: Multiplicity::Affine,
            claim_identity: None,
            provenance: None,
            live: false,
            ever_established: false,
            zero_image: false,
            conditional: false,
            case_excluded: false,
        });
    } else {
        for claim in claims {
            places.push(LinearPlace {
                root,
                name: claim_place_name(program, &name, &claim.path),
                path: claim.path,
                multiplicity: claim.multiplicity,
                claim_identity: None,
                provenance: None,
                live: false,
                ever_established: false,
                zero_image: false,
                conditional: claim.conditional,
                case_excluded: false,
            });
        }
    }
}

fn claim_place_name(
    program: &dyn ::validation::CheckingInput,
    root: &str,
    path: &[facts::PlaceSegment],
) -> String {
    let mut name = root.to_owned();
    for segment in path {
        match segment {
            facts::PlaceSegment::Case { variant } => {
                let case = program
                    .variant_owner(*variant)
                    .map(|(_, candidate)| candidate.name.as_str());
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
