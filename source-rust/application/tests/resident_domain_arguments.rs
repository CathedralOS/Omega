//! Resident transport keeps its full type indices, including the selected domains.
#[path = "support/program.rs"]
mod program;

const DOMAINS: &str = r#"use {omega}/language/core/extent.omg as extent;
use {omega}/language/core/layout.omg as layout;
use {omega}/language/core/access/access.omg as access;
use {omega}/language/core/reflect/reflect.omg as reflect;
use {omega}/language/core/run.omg as run;

data Word { value: u64; }
domain Word::Placed access WordAccess;
domain Word::Other access WordAccess;
data WordPlacement = Word in Word::Placed;
WordAccess satisfies |layout|::AccessPolicy {
    machine plan(schema: |reflect|::TypeSchema) -> |layout|::AccessPlan reaches none crashes none terminates {
        |access|::new_plan(@schema, |layout|::Issuance::Unrestricted)
    }
}
"#;

const RESIDENT_RETURN: &str = r#"machine carry(input: @|extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Resident<WordPlacement, Word>)
    -> @|extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Resident<Word in Word::Placed, Word> { input }"#;

fn inspect(declaration: &str) -> (std::process::Output, String) {
    let source = format!("{DOMAINS}\n{declaration}\nmachine main() {{}}");
    program::invoke(&source, &["psi", "inspect", "--machine", "carry"])
}

fn change_result(source: &str) -> String {
    source.replace(
        "Resident<Word in Word::Placed, Word> { input }",
        "Resident<Word in Word::Other, Word> { input }",
    )
}

#[test]
fn a_resident_forward_preserves_its_exact_domain_argument() {
    let (output, report) = inspect(RESIDENT_RETURN);
    assert!(output.status.success(), "{report}");
    assert!(report.contains("psi selected_machine=carry"), "{report}");

    let (output, report) = inspect(&change_result(RESIDENT_RETURN));
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("changes its referent type"), "{report}");
}

#[test]
fn an_owned_resident_return_names_both_mismatched_domain_arguments() {
    let owned = RESIDENT_RETURN.replace("@|extent|::Extent", "|extent|::Extent");
    let (output, report) = inspect(&owned);
    assert!(output.status.success(), "{report}");

    let (output, report) = inspect(&change_result(&owned));
    assert!(!output.status.success(), "{report}");
    let (carried, expected) = report
        .lines()
        .find_map(|line| line.split_once("the machine's result declares"))
        .expect(&report);
    assert!(carried.contains("Word::Placed"), "{report}");
    assert!(expected.contains("Word::Other"), "{report}");
}

#[test]
fn an_owned_run_keeps_its_domain_index_when_transferred() {
    let carry = |index| {
        format!(
            "machine carry(input: |run|::Run<WordPlacement, Word>) -> |run|::Run<{index}, Word> {{ input }}"
        )
    };
    let (output, report) = inspect(&carry("Word in Word::Placed"));
    assert!(output.status.success(), "{report}");
    assert!(report.contains("psi selected_machine=carry"), "{report}");

    let (output, report) = inspect(&carry("Word in Word::Other"));
    assert!(!output.status.success(), "{report}");
    let (expected, carried) = report
        .lines()
        .find_map(|line| line.split_once("but this value carries"))
        .expect(&report);
    assert!(
        expected.contains("Run") && expected.contains("Word::Other"),
        "{report}"
    );
    assert!(carried.contains("Word::Placed"), "{report}");
}

#[test]
fn a_resident_view_checks_its_lent_extent_before_asking_for_custody() {
    let declaration = |placement, schema| {
        format!(
            "machine carry(input: @mutable |extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Resident<{placement}, {schema}>) -> @mutable Word in Word::Placed {{\n    |access|::view_resident_mut::<WordPlacement, Word, Word in Word::Placed>(input)\n}}"
        )
    };
    let (output, report) = inspect(&declaration("Word in Word::Placed", "Word"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report
            .contains("resident view needs evaluated layout, content, profile and policy custody"),
        "{report}"
    );

    for (placement, schema, carried_type) in [
        ("Word in Word::Other", "Word", "Word::Other"),
        ("Word in Word::Placed", "u64", "u64"),
    ] {
        let (output, report) = inspect(&declaration(placement, schema));
        assert!(!output.status.success(), "{report}");
        let (carried, expected) = report
            .lines()
            .find_map(|line| line.split_once("the parameter it is passed for declares"))
            .expect(&report);
        assert!(
            carried.contains("Resident") && carried.contains(carried_type),
            "{report}"
        );
        assert!(
            expected.contains("Resident")
                && (expected.contains("Word::Placed") || expected.contains("WordPlacement"))
                && expected.contains(", Word)"),
            "{report}"
        );
        assert!(report.contains("main.omg:18:"), "{report}");
    }
}

#[test]
fn a_resident_view_preserves_the_placement_even_with_a_shared_policy() {
    let (output, report) = inspect(
        r#"machine carry(input: @mutable |extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Resident<WordPlacement, Word>) -> @mutable Word in Word::Other {
    |access|::view_resident_mut::<WordPlacement, Word, Word in Word::Other>(input)
}"#,
    );
    assert!(!output.status.success(), "{report}");
    let (placement, view) = report
        .lines()
        .find_map(|line| line.split_once("but the requested view carries"))
        .expect(&report);
    assert!(placement.contains("Word::Placed"), "{report}");
    assert!(view.contains("Word::Other"), "{report}");
}

#[test]
fn a_resident_view_requires_a_qualified_index_of_its_value_type() {
    for (index, schema, expected) in [
        ("Word", "Word", "must name a domain-qualified type"),
        (
            "WordPlacement",
            "u16",
            "does not qualify the supplied value type `u16`",
        ),
    ] {
        let body = format!(
            "machine carry(input: @mutable |extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Resident<{index}, {schema}>) -> @mutable Word in Word::Placed {{\n    |access|::view_resident_mut::<{index}, {schema}, Word in Word::Placed>(input)\n}}"
        );
        let (output, report) = inspect(&body);
        assert!(!output.status.success(), "{report}");
        assert!(report.contains(expected), "{report}");
    }
}
