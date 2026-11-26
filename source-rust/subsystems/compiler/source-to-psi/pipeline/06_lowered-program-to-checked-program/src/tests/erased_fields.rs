//! An `[erased]` field of ordinary data. contracts.md, "Explicit erased
//! bindings": the binding "remains in checking and semantic identity but
//! contributes no runtime field, address, read, or cleanup". Stage 05 keeps
//! it out of the record's storage and gives it a Terminal Psi row with
//! erased relevance; the checker finds no leaf for it.

use super::build_purpose::judge_build;
use super::*;

const PASS: &str = include_str!("../../tests/fixtures/erased_field/pass/record_field/main.omg");
const FAIL: &str = include_str!("../../tests/fixtures/erased_field/fail/runtime_read/main.omg");

/// The fixture judged as the Build's entry: `accepted`, or the refusal.
fn verdict(fixture: &str) -> String {
    let source = fixture.replace("machine main()", "machine build_entry()");
    match judge_build(&source) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Rejected {
            violations,
            unproved,
            ..
        }) => {
            if let Some(violation) = violations.first() {
                format!("violation: {}", violation.statement)
            } else if let Some(obligation) = unproved.first() {
                format!("unproved: {}", obligation.statement)
            } else {
                "rejected".to_owned()
            }
        }
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        other => panic!("{other:?}"),
    }
}

/// The record's erased field is declared and supplied but has no storage,
/// so a machine reading its relevant field is judged like any other.
#[test]
fn data_with_an_erased_field_is_stored_without_it() {
    assert_eq!(verdict(PASS), "accepted");
}

/// The twin reads the erased field at runtime, one input apart, and is
/// rejected as a semantic violation rather than as unsupported.
#[test]
fn a_runtime_read_of_an_erased_field_is_refused() {
    assert_eq!(
        FAIL.lines().skip(3).collect::<Vec<_>>(),
        PASS.lines()
            .skip(3)
            .map(|line| line.replace("{ tagged.tail }", "{ tagged.ghost }"))
            .collect::<Vec<_>>()
    );
    let source = FAIL.replace("machine main()", "machine build_entry()");
    let Ok(RouteVerdict::Rejected { violations, .. }) = judge_build(&source) else {
        panic!("expected violation rejection for runtime read of an erased field");
    };
    assert_eq!(violations.len(), 1);
    assert_eq!(
        violations[0].statement,
        "runtime use of an erased binding while reading the body of `pick`"
    );
}

/// An erased scalar field supplies its literal's term to clauses and data
/// `requires`, and mints a checker-named value for formals, while zeroing
/// still refuses because zeroed storage supplies no erased term.
#[test]
fn an_erased_scalar_field_works_in_clauses_and_data_requires() {
    let tagged = "data Tagged [copy] { value: u64; ghost [erased]: u64; }\n";
    assert_eq!(
        verdict(&format!(
            "{tagged}machine pick(tagged: Tagged) -> u64 requires tagged.ghost == 7 {{ tagged.value }}
machine main() {{ let tagged: Tagged = Tagged {{ value: 3, ghost: 7 }}; _ = pick(tagged); }}"
        )),
        "accepted"
    );
    let wrong_ghost = verdict(&format!(
        "{tagged}machine pick(tagged: Tagged) -> u64 requires tagged.ghost == 7 {{ tagged.value }}
machine main() {{ let tagged: Tagged = Tagged {{ value: 3, ghost: 8 }}; _ = pick(tagged); }}"
    ));
    assert!(
        wrong_ghost.starts_with("unproved:"),
        "wrong erased field literal must be unproved, got: {wrong_ghost}"
    );

    let capped = "data Capped [copy] requires value <= cap { value: u64; cap [erased]: u64; }\n";
    assert_eq!(
        verdict(&format!(
            "{capped}machine read_value(c: Capped) -> u64 ensures result <= c.cap {{ c.value }}
machine main() {{ let c: Capped = Capped {{ value: 3, cap: 5 }}; _ = read_value(c); }}"
        )),
        "accepted"
    );
    let violated_cap = verdict(&format!(
        "{capped}machine main() {{ let c: Capped = Capped {{ value: 6, cap: 5 }}; }}"
    ));
    assert!(
        violated_cap.starts_with("unproved:"),
        "violated data requires over erased field must be unproved, got: {violated_cap}"
    );
    let unproved_strict = verdict(&format!(
        "{capped}machine read_value(c: Capped) -> u64 ensures result < c.cap {{ c.value }}
machine main() {{ let c: Capped = Capped {{ value: 3, cap: 5 }}; _ = read_value(c); }}"
    ));
    assert!(
        unproved_strict.starts_with("unproved:"),
        "unproved strict bound over erased field must be unproved, got: {unproved_strict}"
    );

    assert_eq!(
        verdict(&format!("{tagged}machine main() {{ let tagged: Tagged; }}")),
        "statement zero value of data with an erased field while reading the local binding of \
         `build_entry::entry::tagged` while reading the body of `build_entry`"
    );
    assert_eq!(
        verdict(&format!(
            "{tagged}machine main() {{ let tagged: Tagged = Tagged {{ value: 3, .. }}; }}"
        )),
        "expression record literal that leaves an erased field to `..` while reading the local \
         binding of `build_entry::entry::tagged` while reading the body of `build_entry`"
    );
}

/// Copying a record preserves its erased scalar field's term for clauses.
#[test]
fn copying_a_record_preserves_its_erased_scalar_field_term() {
    let source = "
data Capped [copy] requires value <= cap { value: u64; cap [erased]: u64; }
machine check(c: Capped) -> u64 requires c.cap == 10 ensures result <= 10 { c.value }
machine main() {
    let first: Capped = Capped { value: 4, cap: 10 };
    let second: Capped = first;
    _ = check(second);
}
";
    assert_eq!(verdict(source), "accepted");
    let wrong = source.replace("cap: 10", "cap: 9");
    let result = verdict(&wrong);
    assert!(
        result.starts_with("unproved:"),
        "wrong copied erased field must be unproved, got: {result}"
    );
}
