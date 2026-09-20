//! Restricted consent uses a supplied host directory, not compiler-owned staging.

use super::SourcePreparationFixture;
use crate::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLock,
    PackageLockRecoveryLimits, PackageLockTarget, PackageOccurrenceRoster,
};
use crate::resolution::graph::CanonicalSourceClosureSubject;
use crate::review::{
    PackagePolicyChangeLimits, PackagePolicyDecision, PackagePolicyDecisionSubject,
    ReviewOnlyRootPolicyDisposition, compare_package_policy_changes,
    resolve_package_policy_decisions, ungranted_restricted_build_requests,
};
use std::fs;

#[test]
fn supplied_host_scope_requires_exact_retained_request_and_occurrence() {
    let fixture = SourcePreparationFixture::new();
    fs::write(
        fixture.0.join("package/build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.package("prepared-package");
    let path: BuildPath = builder.output.resolve("stamp.txt");
    let descriptor: i32 = builder.output.create(path, 420);
    let written: i64 = builder.output.write(descriptor, "x");
    let closed: i32 = builder.output.close(descriptor);
}
"#,
    )
    .unwrap();
    let closure = fixture.closure();
    let target = target::TargetProfile::LinuxX64;
    let exact = closure.for_exact_target(target);
    let source = CanonicalSourceClosureSubject::from_resolved(&exact, Default::default()).unwrap();
    let roster = PackageOccurrenceRoster::derive(&source).unwrap();
    let output = fixture.0.join("supplied-host-output");
    fs::create_dir(&output).unwrap();
    let output = fs::canonicalize(output).unwrap();
    // This directory existed before compilation. Bounded accounting does not
    // turn caller-owned host storage into compiler-owned disposable staging.
    let sponsor = checked_interpreter::FilesystemSponsor::new(&output).unwrap();
    let evaluation_limits = checked_interpreter::BuildEvaluationSponsorLimits::new(
        100_000_000,
        1_048_576,
        65_536,
        4_096,
        1_048_576,
        67_108_864,
        1_048_576,
        67_108_864,
    )
    .unwrap();
    let evaluation = checked_interpreter::BuildEvaluationSponsor::new(evaluation_limits);
    let mut preparation = super::CandidateSourcePreparation::for_closure(&closure);
    let reviews = super::super::package_pass::compile_dependency_closure(
        &exact,
        target::TargetProfile::host_if_supported(),
        &roster,
        &output,
        &sponsor,
        &evaluation,
        &Default::default(),
        None,
        None,
        super::TargetEntryDiscovery::Disabled,
        &mut preparation,
    )
    .expect("compile an honestly supplied host scope")
    .reviews;
    let [review] = reviews.reviews() else {
        panic!("one package occurrence")
    };
    assert!(!review.restricted_build_requests().is_empty());
    let changes = compare_package_policy_changes(
        None,
        &reviews,
        &exact,
        PackagePolicyChangeLimits::default(),
    )
    .unwrap();
    assert!(
        changes.requires_decision(),
        "supplied host storage needs consent"
    );
    let mut choices: Vec<_> = changes
        .packages()
        .iter()
        .flat_map(|package| package.rows())
        .filter(|row| row.requires_decision())
        .map(|row| PackagePolicyDecision {
            subject: PackagePolicyDecisionSubject::Row(row.fingerprint().digest()),
            disposition: ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
        })
        .collect();
    choices.sort_by_key(|decision| decision.subject);
    let resolution =
        resolve_package_policy_decisions(&changes, changes.fingerprint().digest(), &choices)
            .unwrap();
    let history = HistoricalPackagePolicyDecisions::capture_policy(
        &source,
        &changes,
        &resolution,
        HistoricalPackagePolicyLimits::default(),
    )
    .unwrap();
    let accepted = PackageLockTarget::from_policies(
        source,
        &[(review.checked_context(), review.policy())],
        history,
    )
    .unwrap();
    let lock = PackageLock::from_targets(vec![accepted]).unwrap();
    let text = lock.canonical_text().unwrap();
    let recovered = PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap();
    assert!(
        ungranted_restricted_build_requests(recovered.target(target).unwrap(), &reviews)
            .unwrap()
            .is_empty()
    );

    // A structurally valid, manually edited lock cannot grant absent meaning.
    let stripped = lock_text_without_restricted_rows(&text);
    assert_ne!(stripped, text);
    let missing =
        PackageLock::recover_text(&stripped, PackageLockRecoveryLimits::default()).unwrap();
    let gaps =
        ungranted_restricted_build_requests(missing.target(target).unwrap(), &reviews).unwrap();
    assert!(!gaps.is_empty());
    assert!(
        gaps.iter()
            .all(|gap| gap.package() == review.key().identity()
                && gap.purpose() == crate::declarations::DependencyPurpose::Product)
    );

    // Recompile the same source with a larger real host-storage allowance.
    // Source equality and a retained narrower request do not grant that bound.
    let wider_output = fixture.0.join("wider-host-output");
    fs::create_dir(&wider_output).unwrap();
    let wider_output = fs::canonicalize(wider_output).unwrap();
    let mut wider_limits = checked_interpreter::FilesystemSponsorLimits::default();
    wider_limits.maximum_entries += 1;
    let wider_sponsor =
        checked_interpreter::FilesystemSponsor::with_limits(&wider_output, wider_limits).unwrap();
    let wider_reviews = super::super::package_pass::compile_dependency_closure(
        &exact,
        target::TargetProfile::host_if_supported(),
        &roster,
        &wider_output,
        &wider_sponsor,
        &checked_interpreter::BuildEvaluationSponsor::new(evaluation_limits),
        &Default::default(),
        None,
        None,
        super::TargetEntryDiscovery::Disabled,
        &mut preparation,
    )
    .expect("compile a wider supplied host allowance")
    .reviews;
    let wider_gaps =
        ungranted_restricted_build_requests(recovered.target(target).unwrap(), &wider_reviews)
            .unwrap();
    assert_eq!(wider_gaps.len(), gaps.len());
    assert_ne!(wider_gaps[0].request_meaning(), gaps[0].request_meaning());
    let widened = compare_package_policy_changes(
        recovered.target(target),
        &wider_reviews,
        &exact,
        PackagePolicyChangeLimits::default(),
    )
    .unwrap();
    assert!(widened.requires_decision());

    // Explicit fault injection: identical request meaning and package identity
    // must not let another activation purpose borrow this occurrence's consent.
    let mut other_purpose = reviews.clone();
    let review = &mut other_purpose.reviews[0];
    let bundle = review.generated_source_bundle();
    review.generated_source_bundle =
        package_compilation::PackageGeneratedSourceBundle::from_checked(
            bundle.package(),
            crate::declarations::DependencyPurpose::Build,
            bundle.target(),
            bundle.build_execution_profile(),
            bundle.dependency_closure().clone(),
            bundle.source_consumption_commitment(),
            bundle.sources().to_vec(),
        );
    let purpose_gaps =
        ungranted_restricted_build_requests(recovered.target(target).unwrap(), &other_purpose)
            .unwrap();
    assert_eq!(purpose_gaps.len(), gaps.len());
    assert!(
        purpose_gaps
            .iter()
            .all(|gap| gap.purpose() == crate::declarations::DependencyPurpose::Build)
    );
}

/// Remove every `restricted_build_request` row from each `acceptance` section
/// in canonical lock text, keeping the length framing valid.
fn lock_text_without_restricted_rows(text: &str) -> String {
    const SECTION: &str = "acceptance ";
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0usize;
    while let Some(relative) = text[cursor..].find(SECTION) {
        let header_start = cursor + relative;
        if header_start != 0 && !text[..header_start].ends_with('\n') {
            output.push_str(&text[cursor..header_start + SECTION.len()]);
            cursor = header_start + SECTION.len();
            continue;
        }
        let value_start = header_start + SECTION.len();
        let header_end = value_start + text[value_start..].find('\n').unwrap();
        let Ok(length) = text[value_start..header_end].parse::<usize>() else {
            output.push_str(&text[cursor..value_start]);
            cursor = value_start;
            continue;
        };
        let body_start = header_end + 1;
        let body_end = body_start + length;
        let body = &text[body_start..body_end];
        if !body.starts_with("acceptance_schema ") {
            output.push_str(&text[cursor..body_start]);
            output.push_str(body);
            cursor = body_end;
            continue;
        }
        output.push_str(&text[cursor..header_start]);
        let stripped = acceptance_text_without_restricted_rows(body);
        output.push_str(&format!("acceptance {}\n{stripped}", stripped.len()));
        cursor = body_end;
    }
    output.push_str(&text[cursor..]);
    output
}

/// Rewrite one `acceptance_schema` block dropping restricted rows and fixing
/// its `rows` count. Section bodies are length-framed, so this parses rather
/// than pattern-matching line text inside meanings.
fn acceptance_text_without_restricted_rows(body: &str) -> String {
    let schema_end = body.find('\n').unwrap() + 1;
    let mut output = String::with_capacity(body.len());
    output.push_str(&body[..schema_end]);
    let rows_end = schema_end + body[schema_end..].find('\n').unwrap();
    let count: usize = body[schema_end + "rows ".len()..rows_end].parse().unwrap();
    let mut cursor = rows_end + 1;
    let mut kept = String::new();
    let mut kept_count = 0usize;
    for _ in 0..count {
        let kind_end = cursor + body[cursor..].find('\n').unwrap() + 1;
        let kind = body[cursor + "row ".len()..kind_end].trim_end();
        let key_end = kind_end + body[kind_end..].find('\n').unwrap() + 1;
        let meaning_end = key_end + body[key_end..].find('\n').unwrap() + 1;
        let length: usize = body[key_end + "meaning ".len()..meaning_end]
            .trim_end()
            .parse()
            .unwrap();
        let end = meaning_end + length;
        if kind != "restricted_build_request" {
            kept.push_str(&body[cursor..end]);
            kept_count += 1;
        }
        cursor = end;
    }
    assert_eq!(&body[cursor..], "end_acceptance\n");
    output.push_str(&format!("rows {kept_count}\n"));
    output.push_str(&kept);
    output.push_str(&body[cursor..]);
    output
}
