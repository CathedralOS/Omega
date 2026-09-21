use super::{
    Lexer, ResolutionRequest, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees,
    resolve,
};
use crate::CheckingRequest;

mod call_components;
mod clamped_calls;
mod computed_field_limits;
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

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolved");
    lower_symbol_resolved_trees(&resolved).expect("typed")
}

fn countdown(range: &str) -> String {
    format!(
        r#"
        machine walk(remaining: u32 [1..=5])
        terminates by remaining -> Nat::Descending in {range};
        -> u32 {{
            transition remaining > 1 {{
                true -> walk(remaining - 1)
                false -> remaining
            }}
        }}
    "#
    )
}

#[test]
fn descending_rank_accepts_proved_nonzero_floor_and_exclusive_ceiling() {
    for range in ["1..=5", "0..=5", "1..6"] {
        lower_typed_trees(typed(&countdown(range)), &CheckingRequest::settled()).expect(range);
    }
}

#[test]
fn descending_rank_rejects_unproved_floor_and_ceiling() {
    for range in ["2..=5", "1..=4", "1..5", "6..=1"] {
        let diagnostics = lower_typed_trees(typed(&countdown(range)), &CheckingRequest::settled())
            .expect_err(range);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
            "{range}: {diagnostics:#?}"
        );
    }
}

#[test]
fn rank_bounds_do_not_excuse_an_out_of_range_backedge() {
    let source = countdown("1..=5").replace("remaining > 1", "remaining > 0");
    assert!(
        lower_typed_trees(typed(&source), &CheckingRequest::settled()).is_err(),
        "the final backedge would deliver zero"
    );
}

#[test]
fn acyclic_body_does_not_ignore_an_authored_rank_range() {
    let source =
        "machine walk(remaining: u32) terminates by remaining in 1..=5; -> u32 { remaining }";
    let diagnostics = lower_typed_trees(typed(source), &CheckingRequest::settled())
        .expect_err("range is not established by an acyclic body");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{diagnostics:#?}"
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
        lower_typed_trees(typed(&source), &CheckingRequest::settled()).expect(range);
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
    let diagnostics = crate::checks::termination::check_machine_termination(&typed(source))
        .expect_err("moving bound");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove the `terminates by` ranking")),
        "{diagnostics:#?}"
    );
}

#[test]
fn mutable_parameters_establish_rank_bounds_only_while_the_prefix_preserves_them() {
    // A mutable parameter still denotes its arrival value while no earlier
    // statement writes its path; the entry query has no prefix at all.
    for source in [
        "machine walk(mut remaining: u32 [1..=5]) terminates by remaining -> Nat::Descending in 1..=5; -> u32 { remaining }",
        "machine walk(mut remaining: u32 [1..=5]) terminates by remaining -> Nat::Descending in 1..=5; -> u32 { transition remaining > 1 { true -> walk(remaining - 1) false -> remaining } }",
    ] {
        crate::checks::termination::check_machine_termination(&typed(source))
            .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
        lower_typed_trees(typed(source), &CheckingRequest::settled())
            .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
    }
    // Live write-frame evidence decides: a prefix store into the ranked
    // path invalidates the arrival premise even when the value stays in
    // range, and the machine must not borrow the arrival constraint.
    let source = "machine walk(mut remaining: u32 [1..=5]) terminates by remaining -> Nat::Descending in 1..=5; -> u32 { remaining = 3; transition remaining > 1 { true -> walk(remaining - 1) false -> remaining } }";
    let diagnostics = crate::checks::termination::check_machine_termination(&typed(source))
        .expect_err("a prefix write invalidates the mutable premise");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{diagnostics:#?}"
    );
    // A wrapping carrier still cannot bound a natural rank.
    let source = "machine walk(remaining: u32 [1..=5] in Wrapping) terminates by remaining -> Nat::Descending in 1..=5; -> u32 { remaining }";
    let diagnostics = crate::checks::termination::check_machine_termination(&typed(source))
        .expect_err("wrapping");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{diagnostics:#?}"
    );
}

#[test]
fn missing_endpoint_custody_cannot_fall_back_to_display_text() {
    let mut program = typed(&countdown("1..=5"));
    // The source-owned endpoint evidence must remain present, regardless of
    // whether the normalized witness still has convincing display strings.
    let machine = program.machines()[0].symbol;
    program
        .ranking_expression_custody
        .iter_mut()
        .find(|custody| custody.machine == machine)
        .expect("custody")
        .rank_range = None;
    let diagnostics =
        lower_typed_trees(program, &CheckingRequest::settled()).expect_err("missing endpoints");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{diagnostics:#?}"
    );
}
