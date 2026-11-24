//! Generic cases retain active payloads without inventing contents in empty cases.

#[path = "support/build_fixture.rs"]
mod build_fixture;
mod support;

fn fixture(tier: &str) -> std::path::PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/ownership")
        .join(tier)
        .join("generic_case_payloads")
}

#[test]
fn generic_case_payloads_survive_construction_and_calls_in_both_engines() {
    build_fixture::assert_silent_return(&fixture("run"));
}

#[test]
fn an_empty_generic_result_cannot_discard_its_owned_argument() {
    let source =
        include_str!("fixtures/ownership/fail/generic_case_payloads/main.omg");
    let declaration = source.find("machine observe<T>").unwrap();
    let result = declaration + source[declaration..].find("{ Outcome::Missing }").unwrap() + 2;
    let expected_span = format!("bytes {}..{}:", result, result + "Outcome::Missing".len());
    let (output, report) = build_fixture::check(&fixture("fail"));
    assert!(!output.status.success(), "{report}");
    assert!(report.contains(&expected_span), "{report}");
    assert!(
        report.contains(
            "normal edge cannot implicitly dispose storage without an authorized disposition"
        ),
        "{report}"
    );
}
