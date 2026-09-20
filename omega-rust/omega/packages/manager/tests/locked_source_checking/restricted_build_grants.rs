//! The execution-time grant join: fresh checking may consume a compile's
//! generated sources only when the consuming project's retained acceptance
//! rows grant every projected restricted build request.

use super::generated::generated_workspace;
use super::{
    LockedSourceRecoveryOptions, PackageLock, PackageLockRecoveryLimits, TARGET, Tree,
    capture_lock, check_locked_sources, fs,
};
use package_manager::declarations::DependencyPurpose;
use package_manager::operations::CheckLockedSourcesError;
use package_manager::review::{
    SemanticBindingReview, compile_resolved_package_reviews, ungranted_restricted_build_requests,
};

#[test]
fn retained_acceptance_grants_the_projected_restricted_build_requests() {
    let tree = Tree::new();
    let storage = tree.storage("old-cache");
    let closure = generated_workspace(&tree, &storage);
    let (lock, request) = capture_lock(&closure, &tree.path("old-build"));
    fs::remove_dir_all(tree.path("old-build")).unwrap();
    let storage = tree.storage("new-cache");
    let checked = check_locked_sources(
        &lock,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
        &tree.path("fresh-build"),
    )
    .expect("locked checking honors the restricted requests it retained");
    let producer = checked
        .reviews()
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "generated-table")
        .expect("the generated producer reviewed");
    assert!(
        !producer.restricted_build_requests().is_empty(),
        "the fixture must project restricted build requests for this test to matter"
    );
    assert!(
        ungranted_restricted_build_requests(checked.accepted(), checked.reviews())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn retained_acceptance_missing_a_restricted_row_rejects_locked_checking() {
    let tree = Tree::new();
    let storage = tree.storage("old-cache");
    let closure = generated_workspace(&tree, &storage);
    let (lock, request) = capture_lock(&closure, &tree.path("old-build"));
    fs::remove_dir_all(tree.path("old-build")).unwrap();

    // The lock stores decision meaning, not credentials: remove the producer's
    // retained restricted-request rows while leaving a structurally valid lock,
    // as a manual edit of omega.lock would.
    let original = lock.canonical_text().unwrap();
    let tampered = lock_text_without_restricted_rows(&original);
    assert_ne!(
        tampered, original,
        "the captured lock must retain restricted build request rows"
    );
    let lock = PackageLock::recover_text(&tampered, PackageLockRecoveryLimits::default())
        .expect("a lock missing rows still recovers");

    // The same gap is visible directly on the join against fresh reviews.
    let target = closure.for_exact_target(TARGET);
    let reviews = compile_resolved_package_reviews(
        &target,
        &tree.path("review-build"),
        SemanticBindingReview::Explicit(&[]),
    )
    .expect("fresh reviews for the join");
    let ungranted =
        ungranted_restricted_build_requests(lock.target(TARGET).unwrap(), &reviews).unwrap();
    assert!(!ungranted.is_empty());
    for gap in &ungranted {
        assert_eq!(gap.purpose(), DependencyPurpose::Product);
        assert_eq!(
            gap.package(),
            reviews
                .reviews()
                .iter()
                .find(|review| review.key().name().as_str() == "generated-table")
                .unwrap()
                .key()
                .identity()
        );
    }

    let storage = tree.storage("new-cache");
    let error = check_locked_sources(
        &lock,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
        &tree.path("rejected-build"),
    )
    .err()
    .expect("locked checking must reject projected requests the lock never granted");
    let CheckLockedSourcesError::UngrantedRestrictedBuild(ref gaps) = error else {
        panic!("unexpected rejection: {error}");
    };
    assert_eq!(gaps.len(), ungranted.len());
    assert!(error.to_string().contains("restricted build authority"));
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
