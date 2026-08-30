use crate::support::*;

#[test]
fn review_projects_exact_outcome_specific_guarantees() {
    let compile = |source: &str| {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write(
            "build.omg",
            r#"target windows_x86_64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x86_64"),
            package_inputs(&package.0),
        )
        .expect("outcome-specific package fixture should check")
    };
    let source = |selector: &str, groups: &str| {
        format!(
            r#"pub trait Evidence {{
    machine witness();
}}
pub proposition ready() evidence Evidence;
pub data Outcome {{ case Success; case Failure; }}
pub machine choose(flag: bool) -> Outcome
requires input_proof: ready()
ensures
{groups}
{{
    {selector} = input_proof;
    Outcome::Success
}}
"#,
        )
    };
    let ordered_groups = r#"    Outcome::Success -> {
        selected: ready();
        true;
    }
ensures
    Outcome::Failure -> {
        true;
    }"#;
    let reordered_groups = r#"    Outcome::Failure -> {
        true;
    }
ensures
    Outcome::Success -> {
        true;
        selected: ready();
    }"#;
    let moved_group = r#"    Outcome::Success -> {
        selected: ready();
    }
ensures
    Outcome::Failure -> {
        true;
        true;
    }"#;

    let checked = compile(&source("selected", ordered_groups));
    let review = project_checked_package_review(&checked)
        .expect("checked outcome-specific carriers should rejoin review rows");
    let choose = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("choose"))
        .expect("public choose callable");
    let guarded = choose
        .contracts()
        .iter()
        .filter(|contract| contract.result_case().is_some())
        .collect::<Vec<_>>();
    assert_eq!(guarded.len(), 3);
    assert!(
        guarded
            .iter()
            .all(|contract| contract.kind() == PackageReviewContractKind::Ensures)
    );
    assert!(guarded.iter().all(|contract| {
        contract
            .result_case()
            .is_some_and(|guard| guard.result_data().path() == "Outcome")
    }));
    let selected = guarded
        .iter()
        .find(|contract| contract.binding() == Some("selected"))
        .expect("named guarded guarantee");
    assert_eq!(selected.evidence_lane_position(), Some(0));
    assert!(
        selected
            .result_case()
            .is_some_and(|guard| guard.result_case().path().contains("Success"))
    );

    let reordered = project_checked_package_review(&compile(&source("selected", reordered_groups)))
        .expect("reordered guarded rows should project");
    assert_eq!(
        review.canonical_review_bytes().unwrap(),
        reordered.canonical_review_bytes().unwrap(),
        "group and row ordering must not affect guarded contract identity",
    );

    let moved = project_checked_package_review(&compile(&source("selected", moved_group)))
        .expect("moved guarded fact should project");
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        moved.canonical_review_bytes().unwrap(),
        "moving a guarantee to another result case must change review identity",
    );
    let renamed_source = source("approved", &ordered_groups.replace("selected", "approved"));
    let renamed = project_checked_package_review(&compile(&renamed_source))
        .expect("renamed guarded selector should project");
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        renamed.canonical_review_bytes().unwrap(),
        "renaming a public guarded selector must change review identity",
    );

    let mut missing = checked.clone();
    missing.facts.proof.outcome_specific_guarantees.clear();
    let diagnostics = project_checked_package_review(&missing)
        .expect_err("missing guarded carriers must reject review");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("0 exact checked carrier rows; expected one")
    }));

    let mut duplicate = checked.clone();
    let duplicate_row = duplicate
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .next()
        .map(|(_, row)| row.clone())
        .expect("one guarded carrier to duplicate");
    duplicate
        .facts
        .proof
        .outcome_specific_guarantees
        .append(duplicate_row);
    let diagnostics = project_checked_package_review(&duplicate)
        .expect_err("duplicate guarded carriers must reject review");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("2 exact checked carrier rows; expected one")
    }));

    let mut mismatched = checked;
    let carrier = mismatched
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("one guarded carrier to alter");
    mismatched
        .facts
        .proof
        .outcome_specific_guarantees
        .get_mut(carrier)
        .public_selector = Some("spoofed-selector".to_owned());
    let diagnostics = project_checked_package_review(&mismatched)
        .expect_err("mismatched guarded carriers must reject review");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("0 exact checked carrier rows; expected one")
    }));
}

#[test]
fn claim_free_boundary_supply_does_not_collapse_into_an_accepted_claim() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write("main.omg", "boundary machine host_ping();\n");
    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("claim-free boundary fixture should check");
    let review =
        project_checked_package_review(&checked).expect("claim-free boundary review should close");
    let boundary = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "host_ping")
        .expect("claim-free boundary row");
    assert_eq!(boundary.supply(), PackageReviewCallableSupply::Boundary);
    assert!(boundary.contracts().is_empty());
    assert_eq!(
        boundary.checked_service_reach(),
        &PackageReviewCheckedServiceReach::NoCheckedBody
    );
    assert!(review.dangerous_authority_slack().is_empty());
    assert!(
        review
            .canonical_rows()
            .expect("claim-free boundary rows")
            .iter()
            .all(|row| row.kind() != PackageReviewCanonicalRowKind::AcceptedClaim),
        "claim-free boundary supply must not emit accepted-claim evidence"
    );
}
