//! A constant keeps the domains its declared type puts on each element, but
//! it grants no authority (`wiki/spec/language/constants.md`). The evaluated
//! value holds only the elements' fields, so materialization rebuilds each
//! qualified element as `value as +D`, and the cast reader owes that domain's
//! ordinary issuer and predicate checks. A domain anyone may issue survives
//! the constant in both engines; one whose plan names an issuer is refused
//! at the constant, while the same `make()` called directly still runs.

#[path = "support/build_fixture.rs"]
mod build_fixture;
#[path = "support/program.rs"]
mod program;
mod support;




#[test]
fn a_constant_keeps_a_domain_anyone_may_issue_on_each_element() {
    build_fixture::assert_silent_return(
        &support::repository_root()
            .join("source-rust/application/tests/fixtures/domains/run/constant_qualified_elements"),
    );
}

#[test]
fn a_constant_cannot_reissue_a_domain_its_plan_restricts() {
    let (output, report) = build_fixture::check(
        &support::repository_root()
            .join("source-rust/application/tests/fixtures/domains/fail/constant_issued_elements"),
    );
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains(
            "constant `ISSUED` evaluation: a constant grants no authority, and \
             `Ticket::Stamped` restricts who may issue it"
        ),
        "{report}"
    );
}

#[test]
fn a_core_quantity_conversion_preserves_its_unit_and_scale() {
    program::assert_silent_return(include_str!(
        "fixtures/domains/run/core_quantity_conversion/main.omg"
    ));
}

#[test]
fn a_meter_result_cannot_initialize_seconds() {
    let (output, report) = program::invoke(
        include_str!("fixtures/domains/fail/core_quantity_wrong_unit/main.omg"),
        &["--check"],
    );
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("Units::METER)") && report.contains("Units::SECOND)"), "{report}");
}
