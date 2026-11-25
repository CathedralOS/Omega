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

/// The twin reads the erased field at runtime, one input apart.
#[test]
fn a_runtime_read_of_an_erased_field_is_refused() {
    assert_eq!(
        FAIL.lines().skip(3).collect::<Vec<_>>(),
        PASS.lines()
            .skip(3)
            .map(|line| line.replace("{ tagged.tail }", "{ tagged.ghost }"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        verdict(FAIL),
        "expression runtime use of an erased binding while reading the body of `pick`"
    );
}

/// An erased field can be neither read by a clause nor left to zero, since
/// Terminal Psi keeps no value for it and zeroed storage supplies no term.
#[test]
fn an_erased_field_is_refused_where_its_term_would_be_needed() {
    let data = "data Tagged [copy] { value: u64; ghost [erased]: u64; }\n";
    assert_eq!(
        verdict(&format!(
            "{data}machine pick(tagged: Tagged) -> u64 requires tagged.ghost == 7 {{ tagged.value }}
machine main() {{ let tagged: Tagged = Tagged {{ value: 3, ghost: 7 }}; _ = pick(tagged); }}"
        )),
        "expression read of an erased field of data while reading the signature of `pick`"
    );
    assert_eq!(
        verdict(&format!("{data}machine main() {{ let tagged: Tagged; }}")),
        "statement zero value of data with an erased field while reading the local binding of \
         `build_entry::entry::tagged` while reading the body of `build_entry`"
    );
    assert_eq!(
        verdict(&format!(
            "{data}machine main() {{ let tagged: Tagged = Tagged {{ value: 3, .. }}; }}"
        )),
        "expression record literal that leaves an erased field to `..` while reading the local \
         binding of `build_entry::entry::tagged` while reading the body of `build_entry`"
    );
}
