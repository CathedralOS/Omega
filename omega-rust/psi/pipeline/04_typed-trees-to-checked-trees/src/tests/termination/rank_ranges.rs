use super::lower_typed_trees;
use crate::tests::front_end::{checked_program_result, typed_program};

mod call_components;
mod clamped_calls;
mod entry_reentry;
mod field_arrivals;
mod field_coordinates;
mod field_endpoint_arithmetic;
mod field_endpoint_pins;
mod field_relations;
mod field_steps;
mod identity_measures;
mod increasing_calls;
mod member_subjects;
mod named_states;
mod payloads;
mod projected_relations;
mod relational;
mod slice_length;
mod state_edges;
mod static_fallback;
mod struct_fields;

/// A parameter's `requires` supplies entry membership and a positive floor
/// exactly as a declared refinement does, because every arrival proves it.
/// Complete checking rejects the back-edge whose actual leaves the clause.
fn reject_back_edge(source: &str) {
    let diagnostics = checked_program_result(source).expect_err(source);
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove requires contract for call walk")),
        "{source}\n{diagnostics:#?}"
    );
}

#[test]
fn increasing_view_ranks_distance_not_cursor() {
    for range in ["0..=4", "0..5", "0..=limit", "0..limit"] {
        let source = format!(
            r#"
            machine walk(limit: u32 [5..=5], index: u32 [1..=5])
            terminates by index -> Nat::IncreasingTo(limit) in {range};
            -> u32 {{
                transition index < limit {{
                    true -> walk(limit, index + 1)
                    false -> index
                }}
            }}
        "#
        );
        checked_program_result(&source).expect(range);
    }
}

#[test]
fn changing_view_bound_cannot_reuse_a_pinned_rank_ceiling() {
    let source = r#"
        machine walk(limit: u32, index: u32)
        terminates by index -> Nat::IncreasingTo(limit) in 0..=limit;
        -> u32 {
            transition index < limit {
                true -> walk(limit + 1, index + 1)
                false -> index
            }
        }
    "#;
    let diagnostics = crate::checks::termination::check_machine_termination(&typed_program(source))
        .expect_err("moving bound");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove the `terminates by` ranking")),
        "{diagnostics:#?}"
    );
}
