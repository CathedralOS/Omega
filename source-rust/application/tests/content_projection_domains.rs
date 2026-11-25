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
