use super::{
    Lexer, lower_symbol_resolved_trees, lower_syntax_trees, lower_typed_trees, parse_syntax_trees,
};

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = lower_syntax_trees(&syntax).expect("resolved");
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
        lower_typed_trees(typed(&countdown(range))).expect(range);
    }
}

#[test]
fn descending_rank_rejects_unproved_floor_and_ceiling() {
    for range in ["2..=5", "1..=4", "1..5", "6..=1"] {
        let diagnostics = lower_typed_trees(typed(&countdown(range))).expect_err(range);
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
        lower_typed_trees(typed(&source)).is_err(),
        "the final backedge would deliver zero"
    );
}

#[test]
fn acyclic_body_does_not_ignore_an_authored_rank_range() {
    let source =
        "machine walk(remaining: u32) terminates by remaining in 1..=5; -> u32 { remaining }";
    let diagnostics =
        lower_typed_trees(typed(source)).expect_err("range is not established by an acyclic body");
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
        lower_typed_trees(typed(&source)).expect(range);
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
fn mutable_and_wrapping_declarations_do_not_establish_rank_bounds() {
    for parameter in [
        "mut remaining: u32 [1..=5]",
        "remaining: u32 [1..=5] in Wrapping",
    ] {
        let source = format!(
            "machine walk({parameter}) terminates by remaining -> Nat::Descending in 1..=5; -> u32 {{ remaining }}"
        );
        let diagnostics = crate::checks::termination::check_machine_termination(&typed(&source))
            .expect_err(parameter);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
            "{diagnostics:#?}"
        );
    }
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
    let diagnostics = lower_typed_trees(program).expect_err("missing endpoints");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{diagnostics:#?}"
    );
}
