use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

mod computed_copies;

const ALTERNATING: &str = r#"
machine walk(remaining: u32 [0..=5])
terminates by remaining in 0..=5;
-> u32 {
    transition remaining > 2 {
        true -> first(remaining)
        false -> second(remaining)
    }
    state first(pending: u32 [0..=5]) {
        transition pending > 0 {
            true -> second(pending - 1)
            false -> pending
        }
    }
    state second(left: u32 [0..=5]) {
        transition left > 0 {
            true -> first(left - 1)
            false -> left
        }
    }
}
"#;

fn prove(source: &str) {
    crate::checks::termination::check_machine_termination(&typed_program(source))
        .unwrap_or_else(|diagnostics| panic!("termination: {source}\n{diagnostics:#?}"));
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("complete checking: {source}\n{diagnostics:#?}"));
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
fn acyclic_computed_arrivals_need_membership_but_not_strict_decrease() {
    let source = r#"
    machine walk(remaining: u32 [0..=5])
    terminates by remaining in 0..=5;
    -> u32 {
        transition remaining > 2 {
            true -> prepare(remaining)
            false -> finish(remaining)
        }
        state prepare(pending: u32 [0..=5]) {
            transition pending < 5 {
                true -> finish(pending + 1)
                false -> pending
            }
        }
        state finish(result: u32 [0..=5]) { result }
    }
    "#;
    prove(source);
    reject(&source.replace("pending < 5", "pending <= 5"));
}

#[test]
fn computed_arrivals_transport_the_unique_authored_rank_subject() {
    let source = ALTERNATING.replace(
        "transition remaining > 2 {\n        true -> first(remaining)\n        false -> second(remaining)\n    }",
        "transition { _ -> first(remaining) }",
    );
    prove(&source);
    for argument in ["remaining + 0", "(remaining - 0) * 1"] {
        prove(&source.replace("first(remaining)", &format!("first({argument})")));
    }
    // Repeated occurrences are one dependency in the rank proof. General
    // exact-arithmetic checking independently owes each intermediate bound;
    // this assertion does not claim complete checking of those expressions.
    for argument in [
        "(2 * remaining) - remaining",
        "(remaining + remaining) - remaining",
    ] {
        crate::checks::termination::check_machine_termination(&typed_program(
            &source.replace("first(remaining)", &format!("first({argument})")),
        ))
        .expect("one current parameter despite repeated occurrences");
    }
    prove(&source.replace(
        "transition { _ -> first(remaining) }",
        "transition remaining > 0 { true -> first(remaining - 1) false -> remaining }",
    ));
    reject(&source.replace("first(remaining)", "first(remaining - 1)"));
    reject(&source.replace("second(pending - 1)", "second(pending)"));
    reject(&source.replace("second(pending - 1)", "second(pending - 2)"));
    reject(&format!(
        "operator + u32::add(left: u32, right: u32) -> u32; {}",
        source.replace("first(remaining)", "first(remaining + 0)")
    ));
}

#[test]
fn auxiliary_dependencies_do_not_replace_the_authored_rank_subject() {
    let source = ALTERNATING
        .replace("remaining: u32 [0..=5]", "remaining: u32 [0..=5], other: u32 [0..=5]")
        .replace("transition remaining > 2 {\n        true -> first(remaining)\n        false -> second(remaining)\n    }",
            "transition { _ -> first(remaining + (other - other)) }");
    prove(&source);
    reject(&source.replace("remaining + (other - other)", "0"));
    reject(&source.replace("remaining + (other - other)", "other"));
    reject(&source.replace("remaining + (other - other)", "remaining - other"));

    // Arithmetic over two auxiliary slots names no entry role. The payload
    // slot stays premise-free while the rank slot keeps its exact mapping.
    prove(
        r#"
        machine walk(remaining: u32 [0..=5], payload: u32 [0..=5])
        terminates by remaining in 0..=5;
        -> u32 {
            transition { _ -> prepare(remaining, payload, payload) }
            state prepare(pending: u32 [0..=5], left: u32 [0..=5], right: u32 [0..=5]) {
                transition { _ -> finish(pending, left + (right - right)) }
            }
            state finish(result: u32 [0..=5], spare: u32 [0..=5]) { result }
        }
    "#,
    );
}

#[test]
fn a_late_conflicting_identity_arrival_cannot_reuse_a_processed_mapping() {
    let source = r#"
    machine walk(remaining: u32 [0..=5], other: u32 [0..=5])
    terminates by remaining in 0..=5;
    -> u32 {
        transition remaining > 2 {
            true -> first(remaining, other)
            false -> second(remaining, other)
        }
        state first(pending: u32 [0..=5], spare: u32 [0..=5]) {
            transition { _ -> finish(pending, spare) }
        }
        state second(left: u32 [0..=5], extra: u32 [0..=5]) {
            transition { _ -> finish(extra, left) }
        }
        state finish(result: u32 [0..=5], unused: u32 [0..=5]) { result }
    }
    "#;
    reject(source);
    let computed = source
        .replace("first(remaining, other)", "first(remaining + 0, other + 0)")
        .replace(
            "second(remaining, other)",
            "second(remaining + 0, other + 0)",
        )
        .replace("finish(pending, spare)", "finish(pending + 0, spare + 0)")
        .replace("finish(extra, left)", "finish(extra + 0, left + 0)");
    reject(&computed);
}

#[test]
fn every_parallel_edge_occurrence_still_requires_its_own_proof() {
    reject(
        &ALTERNATING
            .replace("true -> second(pending - 1)", "true -> second(pending)")
            .replace(
                "transition pending > 0 {",
                "transition pending > 1 { true -> second(pending - 1) } transition pending > 0 {",
            ),
    );
}
