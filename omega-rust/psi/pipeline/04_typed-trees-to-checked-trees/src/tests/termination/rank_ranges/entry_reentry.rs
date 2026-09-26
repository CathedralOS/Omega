use crate::tests::front_end::typed_program;

fn reject(source: &str) {
    let diagnostics = crate::checks::termination::check_machine_termination(&typed_program(source))
        .expect_err(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
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
    reject(&VARIABLE_STEP.replace("false -> n", "").replace(
        "    }\n}",
        "    }\n    transition n > 0 { true -> walk(n - 1, 0, cap) false -> n }\n}",
    ));
}
