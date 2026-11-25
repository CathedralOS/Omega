//! Resident transport keeps its full type indices, including the selected domains.
#[path = "support/program.rs"]
mod program;

const FORWARD: &str = r#"use {omega}/language/core/extent.omg as extent;
use {omega}/language/core/layout.omg as layout;
use {omega}/language/core/access/access.omg as access;
use {omega}/language/core/reflect/reflect.omg as reflect;

data Word { value: u64; }
domain Word::Placed access WordAccess;
domain Word::Other access WordAccess;
data WordPlacement = Word in Word::Placed;
WordAccess satisfies |layout|::AccessPolicy {
    machine plan(schema: |reflect|::TypeSchema) -> |layout|::AccessPlan reaches none crashes none terminates {
        |access|::new_plan(@schema, |layout|::Issuance::Unrestricted)
    }
}
machine carry(input: @|extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Resident<WordPlacement, Word>)
    -> @|extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Resident<Word in Word::Placed, Word> { input }
machine main() {}
"#;

fn change_result(source: &str) -> String {
    source.replace(
        "Resident<Word in Word::Placed, Word> { input }",
        "Resident<Word in Word::Other, Word> { input }",
    )
}

#[test]
fn a_resident_forward_preserves_its_exact_domain_argument() {
    let inspect = &["psi", "inspect", "--machine", "carry"];
    let (output, report) = program::invoke(FORWARD, inspect);
    assert!(output.status.success(), "{report}");
    assert!(report.contains("psi selected_machine=carry"), "{report}");

    let (output, report) = program::invoke(&change_result(FORWARD), inspect);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("changes its referent type"), "{report}");
}

#[test]
fn an_owned_resident_return_names_both_mismatched_domain_arguments() {
    let owned = FORWARD.replace("@|extent|::Extent", "|extent|::Extent");
    let inspect = &["psi", "inspect", "--machine", "carry"];
    let (output, report) = program::invoke(&owned, inspect);
    assert!(output.status.success(), "{report}");

    let (output, report) = program::invoke(&change_result(&owned), inspect);
    assert!(!output.status.success(), "{report}");
    let (carried, expected) = report
        .lines()
        .find_map(|line| line.split_once("the machine's result declares"))
        .expect(&report);
    assert!(carried.contains("Word::Placed"), "{report}");
    assert!(expected.contains("Word::Other"), "{report}");
}
