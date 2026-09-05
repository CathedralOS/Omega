//! R1 in-callee ordering facts: a dependent-ranged state parameter
//! (`i: u32 [0..=self.count]`) mints `i <= self.count` into the DBM at
//! state entry -- the caller proved it at every transition/call (the
//! proof-plan atom), so inside the state it composes with everything the
//! guard-seeded orderings already serve (via_ordering index bounds,
//! two-pointer chains). Staleness rides the existing machinery: a write
//! to EITHER side forgets the pair (`forget_orderings` drops on both
//! ends), exactly as guard-seeded orderings behave.
//!
//! Only offsets `k <= 0` mint (i <= F + k implies i <= F); a positive
//! offset's weaker truth would over-claim. The substituted literal range
//! (types.rs) independently carries the tight bound either way.

use super::facts::RangeFacts;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::state::State;

pub(in crate::checks::ranges) fn seed_dependent_param_orderings(
    program: &TypedTrees,
    facts: &mut RangeFacts<'_>,
    machine: &Machine,
    state: &State,
) {
    for parameter in program.state_parameters(state) {
        if !parameter.type_reference.is_valid() {
            continue;
        }
        // Sibling-length class (`index: u64 [0..items.len]`): a STRICT bound
        // (offset <= -1) makes the param a VALID INDEX of the sibling --
        // exactly the unknown-length prover's `prove_index` fact. The
        // caller proved it at every entry (the sibling-length atom); slices
        // are immutable views, so no write fence is needed on the length.
        if let Some(sibling) = sibling_len_of_type_reference(program, parameter.type_reference) {
            if sibling.offset <= -1 {
                facts.prove_index(
                    sibling.sibling.as_str().to_owned(),
                    parameter.name.as_str().to_owned(),
                );
            }
            continue;
        }
        let Some((field, offset)) =
            dependent_maximum_of_type_reference(program, parameter.type_reference)
        else {
            continue;
        };
        if offset > 0 {
            continue;
        }
        // The atom is only mintable when the machine's own attached-data
        // field is the named one -- the same binding the declaration gate
        // validated. (A trait-signature state has no attached data; skip.)
        if machine.attached_data.is_none() {
            continue;
        }
        facts.prove_at_most(
            parameter.name.as_str().to_owned(),
            format!("self.{}", field.as_str()),
        );
    }
}

/// The dependent maximum (field, offset) of a declared type's Range
/// constraint under Exact shells (the recognizer class; mirrors the
/// substitution gates in types.rs).
fn dependent_maximum_of_type_reference(
    program: &TypedTrees,
    handle: typed_trees::types::TypeReferenceHandle,
) -> Option<(typed_trees::name::Identifier, i64)> {
    use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            dependent_maximum_of_type_reference(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = program.type_reference_table.constraints(*constraints);
            if constraints.iter().any(|constraint| {
                matches!(
                    constraint,
                    TypeConstraintNode::ArithmeticDomain(domain)
                        if *domain != numerics::arithmetic::ArithmeticDomain::Exact
                )
            }) {
                return None;
            }
            constraints
                .iter()
                .find_map(|constraint| match constraint {
                    TypeConstraintNode::Range { maximum, .. } => {
                        let symbolic = typed_trees::dependent_ranges::symbolic_max_bound(
                            &program.expression_table,
                            *maximum,
                        )?;
                        Some((symbolic.field, symbolic.offset))
                    }
                    _ => None,
                })
                .or_else(|| dependent_maximum_of_type_reference(program, *base_type))
        }
        _ => None,
    }
}

/// The sibling-length bound of a declared type's Range constraint under
/// Exact shells (the recognizer class).
fn sibling_len_of_type_reference(
    program: &TypedTrees,
    handle: typed_trees::types::TypeReferenceHandle,
) -> Option<typed_trees::dependent_ranges::SiblingLenBound> {
    use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            sibling_len_of_type_reference(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = program.type_reference_table.constraints(*constraints);
            if constraints.iter().any(|constraint| {
                matches!(
                    constraint,
                    TypeConstraintNode::ArithmeticDomain(domain)
                        if *domain != numerics::arithmetic::ArithmeticDomain::Exact
                )
            }) {
                return None;
            }
            constraints
                .iter()
                .find_map(|constraint| match constraint {
                    TypeConstraintNode::Range { maximum, .. } => {
                        typed_trees::dependent_ranges::sibling_len_bound(
                            &program.expression_table,
                            *maximum,
                        )
                    }
                    _ => None,
                })
                .or_else(|| sibling_len_of_type_reference(program, *base_type))
        }
        _ => None,
    }
}
