//! Prefix stores are judged against the premise carriers, not every formal.

use crate::tests::front_end::typed_program;

fn prove(source: &str) {
    crate::checks::termination::check_machine_termination(&typed_program(source))
        .unwrap_or_else(|diagnostics| panic!("termination: {source}\n{diagnostics:#?}"));
}

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

#[test]
fn immutable_locals_forward_endpoint_inputs() {
    // An immutable local names the storage its initializer spelled: forwarding
    // it preserves the endpoint input across the self-edge.
    let source = r#"
machine walk(remaining: u64 [0..=5], slack: u64 [10..=20])
terminates by remaining in 0..=slack;
-> u64 {
    let spare: u64 [10..=20] = slack;
    transition remaining > 0 {
        true -> walk(remaining - 1, spare)
        false -> remaining
    }
}
"#;
    prove(source);
    // Transitive bindings reach the same storage.
    prove(
        &source
            .replace(
                "let spare: u64 [10..=20] = slack;",
                "let spare: u64 [10..=20] = slack;
    let second: u64 [10..=20] = spare;",
            )
            .replace("walk(remaining - 1, spare)", "walk(remaining - 1, second)"),
    );
    // A mutable local can be rebound before the edge and proves nothing.
    reject(&source.replace("let spare:", "let mut spare:"));
    // A local bound to a different value is not the parameter's storage.
    reject(&source.replace("= slack;", "= 15;"));
    // Later shadowing resolves to the binding in effect at the edge.
    reject(&source.replace(
        "let spare: u64 [10..=20] = slack;",
        "let spare: u64 [10..=20] = slack;
    let spare: u64 [10..=20] = 15;",
    ));
}
