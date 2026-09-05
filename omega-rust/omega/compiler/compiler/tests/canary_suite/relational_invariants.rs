use super::*;

#[path = "../fixture_rosters/relational_invariants.rs"]
pub(super) mod fixture_roster;

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
    let pass =
        pass_canary(fixture_roster::DEPENDENT_RELATIONAL_LOOP_INVARIANT_DYNAMIC_LENGTH_COMPILE);
    compile_canary_without_output(&pass).unwrap_or_else(|diagnostics| {
        panic!(
            "relational loop invariant should prove the indexed access:\n{}",
            render(&diagnostics)
        )
    });

    for &(canary_name, reason) in fixture_roster::HEAD_FACT_FAIL_CANARIES {
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
    for &canary_name in fixture_roster::STABLE_LIMIT_PASS_CANARIES {
        let pass = pass_canary(canary_name);
        compile_canary_without_output(&pass).unwrap_or_else(|diagnostics| {
            panic!(
                "stable relational bounds should compose at the loop head for {canary_name}:\n{}",
                render(&diagnostics)
            )
        });
    }

    for &canary_name in fixture_roster::STABLE_LIMIT_FAIL_CANARIES {
        let diagnostics = compile_canary_without_output(&fail_canary(canary_name))
            .expect_err("missing or stale limit bridge must reject the indexed access");
        let rendered = render(&diagnostics);
        assert!(
            rendered.contains(INDEX_REJECTION),
            "expected stable-limit composition rejection for {canary_name}:\n{rendered}"
        );
    }
}
