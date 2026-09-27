use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

fn reject(source: &str) {
    let diagnostics =
        lower_typed_trees(typed_program(source), &CheckingRequest::settled()).expect_err(source);
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("cannot prove requires")
                && diagnostic.message.contains("step")
        ),
        "{source}\n{diagnostics:#?}"
    );
}

const VARIABLE_STEP: &str = r#"
machine walk(n: u64, step: u64, cap: u64)
requires step > 0 && n <= cap;
terminates by n in 0..=cap;
-> u64 {
    transition n >= step {
        true -> walk(n - step, step, cap)
        false -> n
    }
}
"#;

#[test]
fn different_edges_cannot_mix_incompatible_induction_hypotheses() {
    let source = VARIABLE_STEP.replace("false -> n", "").replace(
        "    }\n}",
        "    }\n    transition n > 0 { true -> walk(n - 1, 0, cap) false -> n }\n}",
    );
    assert!(source.contains("walk(n - 1, 0, cap)"));
    // Rank checking may assume the state's arrival contract. The full
    // checker must reject this separate edge's step=0 actual before that
    // contract can authorize a positive-step hypothesis at reentry.
    reject(&source);
    lower_typed_trees(
        typed_program(&source.replace("walk(n - 1, 0, cap)", "walk(n - 1, step, cap)")),
        &CheckingRequest::settled(),
    )
    .expect("both reentry edges preserve the positive-step requirement");
}
