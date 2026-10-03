use crate::support::*;
use compiler::CheckedCompileRequest;

const SOURCE: &str = r#"
domain [u8; 5]::Utf8 requires valid_utf8(self);
domain [u8; 16]::Utf8 requires valid_utf8(self);
data Entry { name: [u8; 5] in Utf8; }
data Catalog { entry: Entry; label: [u8; 16] in Utf8; }
data Main { catalog: Catalog; }
machine Main::set_entry(&mut self)
ensures self.catalog.entry.name in Utf8
{ self.catalog.entry.name = "Alpha"; }
machine Main::build_label(&mut self)
ensures self.catalog.label in Utf8
{ self.catalog.label = "[" + self.catalog.entry.name + "]"; }
"#;

#[test]
fn exit_membership_discharge_replays_custody_and_keeps_persisted_rows_inert() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write("main.omg", SOURCE);
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { Build::package(builder, "review_fixture"); }"#,
    );
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
    .expect("nested UTF-8 membership is checked at normal exits");
    let review = package_evidence::ledger::reconstruct_package_review(&checked)
        .expect("fresh package review replays exact exit membership");
    assert_eq!(
        review
            .projection
            .contract_entailment_open_obligations()
            .len(),
        2
    );
    assert_eq!(
        review
            .projection
            .contract_entailment_exit_membership_discharges()
            .len(),
        2
    );
    assert!(
        review
            .results
            .open_contract_entailment_obligations()
            .is_empty()
    );
    assert_eq!(
        review
            .results
            .contract_entailment_exit_membership_discharges()
            .len(),
        2
    );
    for discharge in review
        .results
        .contract_entailment_exit_membership_discharges()
    {
        assert_eq!(
            discharge.status(),
            OrdinaryPackageObligationStatus::Discharged
        );
        assert_eq!(discharge.covered_exits().len(), 1);
        assert_eq!(
            discharge.evidence_row().kind(),
            PackageReviewCanonicalRowKind::ContractEntailmentExitMembershipDischarge
        );
    }
    let recovered = decode_ordinary_package_obligation_ledger(
        &encode_ordinary_package_obligation_ledger(&review.ledger)
            .expect("encode membership evidence"),
    )
    .expect("recover inert membership evidence");
    validate_ordinary_package_obligation_ledger(&recovered, &checked)
        .expect("persisted membership evidence rejoins fresh custody");
    let inert = package_evidence::ledger::ordinary_package_obligation_results_from_projection(
        &recovered,
        &review.projection,
    )
    .expect("projection-only recovery retains open questions");
    assert_eq!(inert.open_contract_entailment_obligations().len(), 2);
    assert!(
        inert
            .contract_entailment_exit_membership_discharges()
            .is_empty()
    );

    let rows_without_evidence = review
        .canonical_rows
        .iter()
        .filter(|row| {
            row.kind() != PackageReviewCanonicalRowKind::ContractEntailmentExitMembershipDischarge
        })
        .cloned()
        .collect::<Vec<_>>();
    let missing = ordinary_package_obligation_ledger_from_compiler_rows(
        checked
            .custody
            .sources()
            .dependency_closure()
            .cloned()
            .expect("package closure"),
        &rows_without_evidence,
    )
    .expect("missing evidence is representable as an inert ledger");
    validate_ordinary_package_obligation_ledger(&missing, &checked)
        .expect_err("missing discharge evidence must fail exact replay");

    let mut changed = checked.clone();
    changed.facts.flow.control.exits = Default::default();
    let diagnostics = project_checked_package_review(&changed)
        .expect_err("supplied empty exit roster cannot borrow original compilation's proof");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("immutable checked compilation custody")),
        "{diagnostics:#?}"
    );
}

#[test]
fn exit_membership_cannot_recover_after_an_alias_invalidates_the_subject() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write("main.omg", &SOURCE.replace(
        "self.catalog.entry.name = \"Alpha\";",
        "self.catalog.entry.name = \"Alpha\"; let alias: &mut [u8; 5] = &mut self.catalog.entry.name; alias[0] = 255;",
    ));
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { Build::package(builder, "review_fixture"); }"#,
    );
    let diagnostics = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
    .expect_err("an invalidating alias write cannot earn exit membership evidence");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
        "{diagnostics:#?}"
    );
}

const SCALAR_POST_STATE_SOURCE: &str = r#"
data Vec { items: [i32; 4]; length: u64 in Wrapping; }
machine Vec::clear(&mut self) ensures self.length < self.items.len { self.length = 0; }
machine Vec::push(&mut self, value: i32 in Wrapping)
requires self.length < self.items.len
ensures self.length > 0; self.length - 1 < self.items.len
{
    let slots: &mut [i32] = self.items.as_mut_slice();
    let index: u64 in Wrapping = self.length;
    let raw_value: i32 = value as i32;
    slots[index] = raw_value;
    self.length = self.length + 1;
}
"#;

#[test]
fn scalar_post_state_guarantees_close_by_exit_evidence_at_every_exit() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write("main.omg", SCALAR_POST_STATE_SOURCE);
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { Build::package(builder, "review_fixture"); }"#,
    );
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
    .expect("post-state field guarantees are checked at normal exits");
    let review = package_evidence::ledger::reconstruct_package_review(&checked)
        .expect("fresh package review replays exact scalar exit evidence");
    // `clear` and both facts `push` lists stand down: the entailment engine
    // reads neither post-state storage nor these transition-shaped bodies.
    // Three open questions, each closed by its own exit evidence rather than
    // left open or approved.
    let open = review.projection.contract_entailment_open_obligations();
    assert_eq!(open.len(), 3, "{open:#?}");
    assert!(
        open.iter().all(|obligation| matches!(
            obligation.reason(),
            PackageReviewContractEntailmentOpenReason::OutsideEntailmentLanguage
                | PackageReviewContractEntailmentOpenReason::UnrecognizedInductiveBody
        )),
        "{open:#?}"
    );
    assert_eq!(
        review
            .projection
            .contract_entailment_exit_membership_discharges()
            .len(),
        3
    );
    assert!(
        review
            .results
            .open_contract_entailment_obligations()
            .is_empty()
    );
    let discharges = review
        .results
        .contract_entailment_exit_membership_discharges();
    assert_eq!(discharges.len(), 3);
    for discharge in discharges {
        assert_eq!(
            discharge.status(),
            OrdinaryPackageObligationStatus::Discharged
        );
        assert_eq!(discharge.covered_exits().len(), 1);
        assert_eq!(
            discharge.evidence_row().kind(),
            PackageReviewCanonicalRowKind::ContractEntailmentExitMembershipDischarge
        );
    }

    // Recovered rows stay inert questions until fresh reconstruction
    // reproduces the evidence, and a ledger missing the evidence fails replay.
    let recovered = decode_ordinary_package_obligation_ledger(
        &encode_ordinary_package_obligation_ledger(&review.ledger)
            .expect("encode scalar exit evidence"),
    )
    .expect("recover inert scalar exit evidence");
    validate_ordinary_package_obligation_ledger(&recovered, &checked)
        .expect("persisted scalar exit evidence rejoins fresh custody");
    let inert = package_evidence::ledger::ordinary_package_obligation_results_from_projection(
        &recovered,
        &review.projection,
    )
    .expect("projection-only recovery retains open questions");
    assert_eq!(inert.open_contract_entailment_obligations().len(), 3);
    assert!(
        inert
            .contract_entailment_exit_membership_discharges()
            .is_empty()
    );
    let rows_without_evidence = review
        .canonical_rows
        .iter()
        .filter(|row| {
            row.kind() != PackageReviewCanonicalRowKind::ContractEntailmentExitMembershipDischarge
        })
        .cloned()
        .collect::<Vec<_>>();
    let missing = ordinary_package_obligation_ledger_from_compiler_rows(
        checked
            .custody
            .sources()
            .dependency_closure()
            .cloned()
            .expect("package closure"),
        &rows_without_evidence,
    )
    .expect("missing evidence is representable as an inert ledger");
    validate_ordinary_package_obligation_ledger(&missing, &checked)
        .expect_err("missing scalar exit evidence must fail exact replay");
}

#[test]
fn a_guarantee_its_requires_discharges_gains_no_exit_evidence() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"machine retain(value: u64) -> u64
requires
    value >= 1
ensures
    value >= 1
{
    let retained: u64 = value;
    retained
}
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { Build::package(builder, "review_fixture"); }"#,
    );
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
    .expect("a restated requirement checks");
    let review = package_evidence::ledger::reconstruct_package_review(&checked)
        .expect("fresh package review replays the assumption discharge");
    assert_eq!(
        review
            .projection
            .contract_entailment_open_obligations()
            .len(),
        1
    );
    assert_eq!(
        review
            .results
            .contract_entailment_assumption_discharges()
            .len(),
        1
    );
    assert!(
        review
            .results
            .contract_entailment_exit_membership_discharges()
            .is_empty(),
        "exit evidence never doubles an assumption discharge"
    );
    assert!(
        review
            .results
            .open_contract_entailment_obligations()
            .is_empty()
    );
}
