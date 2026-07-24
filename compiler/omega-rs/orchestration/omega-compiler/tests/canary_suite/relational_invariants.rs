use super::*;

const INDEX_REJECTION: &str = "cannot prove index `self.i` is within length 8";

fn render(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn symbolic_head_fact_is_preserved_and_invalidated_precisely() {
    let pass = pass_canary("dependent/relational_loop_invariant_dynamic_length_compile");
    compile_canary_without_output(&pass).unwrap_or_else(|diagnostics| {
        panic!(
            "relational loop invariant should prove the indexed access:\n{}",
            render(&diagnostics)
        )
    });

    for (canary_name, reason) in [
        (
            "dependent/relational_loop_invariant_reassigned_index_rejected",
            "reassigning the index must invalidate the relational loop fact",
        ),
        (
            "dependent/relational_loop_invariant_collection_call_rejected",
            "a collection-overlapping call must block the relational loop fact",
        ),
    ] {
        let diagnostics =
            compile_canary_without_output(&fail_canary(canary_name)).expect_err(reason);
        let rendered = render(&diagnostics);
        assert!(
            rendered.contains(INDEX_REJECTION),
            "expected relational index rejection for {canary_name}:\n{rendered}"
        );
    }
}

#[test]
fn stable_limit_composition_requires_a_live_bridge() {
    for canary_name in [
        "dependent/relational_loop_invariant_stable_limit_compile",
        "dependent/relational_loop_invariant_mixed_strictness_compile",
    ] {
        let pass = pass_canary(canary_name);
        compile_canary_without_output(&pass).unwrap_or_else(|diagnostics| {
            panic!(
                "stable relational bounds should compose at the loop head for {canary_name}:\n{}",
                render(&diagnostics)
            )
        });
    }

    for canary_name in [
        "dependent/relational_loop_invariant_limit_bridge_absent_rejected",
        "dependent/relational_loop_invariant_limit_call_rejected",
        "dependent/relational_loop_invariant_limit_preheader_write_rejected",
        "dependent/relational_loop_invariant_fully_nonstrict_rejected",
    ] {
        let diagnostics = compile_canary_without_output(&fail_canary(canary_name))
            .expect_err("missing or stale limit bridge must reject the indexed access");
        let rendered = render(&diagnostics);
        assert!(
            rendered.contains(INDEX_REJECTION),
            "expected stable-limit composition rejection for {canary_name}:\n{rendered}"
        );
    }
}
