//! Core Content admits the two resource algebras declared in content.omg.
#[path = "support/program.rs"]
mod program;

const ARBITRARY_ALGEBRA: &str = r#"use {omega}/language/core/content.omg as algebra;
data Region { length: u64; }
domain Region::Owned;
machine Owned::measure(subject: @Region) -> u64
satisfies |algebra|::Content<Region, u64>::project { subject.length }
machine retain(input: Region in Owned) -> Region in Owned { input }
machine main() {}"#;

#[test]
fn a_declaring_package_cannot_choose_an_arbitrary_content_algebra() {
    let (output, report) = program::invoke(ARBITRARY_ALGEBRA, &["run", "--both"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("core Content requires IntervalSet<Space> or CountedQuantity<Unit>"),
        "{report}"
    );
}

#[test]
fn a_user_content_trait_keeps_its_declared_algebra() {
    let source = ARBITRARY_ALGEBRA
        .replace(
            "use {omega}/language/core/content.omg as algebra;",
            "trait Content<Owner, A> { machine project(subject: @Owner) -> A; }",
        )
        .replace("|algebra|::Content", "Content");
    program::assert_silent_return(&source);
}

const FIELD_PROJECTION: &str = r#"use {omega}/language/core/content.omg as algebra;
use {omega}/language/core/nat.omg as nat;
data Bytes {}
data Region { length: u64; }
domain Region::Owned;
machine Owned::measure(subject: @Region) -> |algebra|::CountedQuantity<Bytes>
satisfies |algebra|::Content<Region, |algebra|::CountedQuantity<Bytes>>::project {
    |algebra|::CountedQuantity { magnitude: embed(subject.length) as |nat|::Nat }
}
machine retain(input: Region in Owned) -> Region in Owned { input }
machine main() {}"#;

#[test]
fn a_content_projection_retains_the_selected_unit_and_its_subject_field() {
    let (output, report) =
        program::invoke(FIELD_PROJECTION, &["psi", "inspect", "--machine", "retain"]);
    assert!(output.status.success(), "{report}");
    let domain = report
        .lines()
        .find(|line| line.starts_with("domain ") && line.contains("Owned"))
        .expect(&report);
    assert!(
        domain.contains("kind: CountedQuantity") && domain.contains("Bytes"),
        "{domain}"
    );
    assert!(
        domain.contains(r#"CountedQuantity(RuntimeScalarEmbedding(["length"]))"#),
        "{domain}"
    );
}

#[test]
fn an_arbitrary_helper_call_remains_outside_the_content_projection_fragment() {
    let source = FIELD_PROJECTION
        .replace(
            "machine Owned::measure",
            "machine read(subject: @Region) -> u64 { subject.length }\nmachine Owned::measure",
        )
        .replace("embed(subject.length)", "embed(read(subject))");
    let (output, report) = program::invoke(&source, &["psi", "inspect", "--machine", "retain"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("closed proof-natural fragment"), "{report}");
}

#[test]
fn interval_projection_orders_proof_naturals_before_qualifying_the_resource() {
    let source = FIELD_PROJECTION
        .replace("data Region { length: u64; }", "data Region { base: addr; length: u64; }")
        .replace("CountedQuantity", "IntervalSet")
        .replace("magnitude: embed(subject.length) as |nat|::Nat", "start: embed(subject.base) as |nat|::Nat, end: (embed(subject.base) + embed(subject.length)) as |nat|::Nat");
    let (output, report) = program::invoke(&source, &["psi", "inspect", "--machine", "retain"]);
    assert!(output.status.success(), "{report}");
    let domain = report
        .lines()
        .find(|line| line.starts_with("domain ") && line.contains("Owned"))
        .expect(&report);
    assert!(domain.contains(r#"IntervalSet([(RuntimeScalarEmbedding(["base"]), Add(RuntimeScalarEmbedding(["base"]), RuntimeScalarEmbedding(["length"])))])"#), "{domain}");

    let reversed = source.replace("start: embed(subject.base) as |nat|::Nat, end: (embed(subject.base) + embed(subject.length))",
        "start: (embed(subject.base) + embed(subject.length)) as |nat|::Nat, end: embed(subject.base)");
    let (output, report) = program::invoke(&reversed, &["psi", "inspect", "--machine", "retain"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("content projection") && report.contains("not proved"),
        "{report}"
    );
}

#[test]
fn promising_content_membership_requires_a_qualified_returned_value() {
    let promise = FIELD_PROJECTION.replace(
        "-> Region in Owned { input }",
        "-> Region ensures result in Owned { input }",
    );
    program::assert_silent_return(&promise);
    let forged = promise.replace("retain(input: Region in Owned)", "retain(input: Region)");
    let (output, report) = program::invoke(&forged, &["run", "--both"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("normal completion does not establish its promised runtime membership"),
        "{report}"
    );
}
