use super::{outgoing_product_requests, reachable_source_packages};
use crate::declarations::dependencies::DependencyPurpose;
use crate::lock::{PackageCheckedContext, PackageOccurrenceRoster};
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, PackageSourceClosureLimits,
    resolve_external_local_package_closure,
};
use crate::review::{
    CanonicalPackageReconstructionQuestion, CanonicalPackageReconstructionQuestionLimits,
    SemanticBindingReview, compile_resolved_package_reviews,
};
use package_evidence::ledger::encode_ordinary_package_obligation_ledger;
use package_source::PrimaryGitChoices;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "omega-reconstruction-emission-{}-{stamp}",
            std::process::id()
        )))
    }

    fn package(&self, name: &str, dependencies: &str) {
        let root = self.0.join(name);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("build.omg"), format!(
            "machine build(builder: &mut Build) {{ builder.package(\"{name}\"); {dependencies} }}\n"
        )).unwrap();
        std::fs::write(root.join("main.omg"), "pub machine value() -> u64 { 1 }\n").unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn assert_reference_reachability(subject: &CanonicalSourceClosureSubject) {
    let outgoing = outgoing_product_requests(subject).unwrap();
    assert_eq!(
        outgoing
            .iter()
            .map(|requests| requests.len())
            .sum::<usize>(),
        4
    );
    for source in subject.packages() {
        // Original full-edge traversal is retained here as an independent oracle.
        let mut reference = BTreeSet::new();
        let mut pending = vec![source.key()];
        while let Some(package) = pending.pop() {
            if reference.insert(package) {
                pending.extend(
                    subject
                        .dependency_requests()
                        .iter()
                        .filter(|dependency| {
                            dependency.purpose().is_product() && dependency.requester() == package
                        })
                        .map(|dependency| dependency.selected().key()),
                );
            }
        }
        let actual = reachable_source_packages(subject, &outgoing, source.key()).unwrap();
        assert_eq!(
            actual.len(),
            reference.len(),
            "shared dependencies appear once"
        );
        assert_eq!(
            actual
                .iter()
                .map(|&position| subject.packages()[position].key())
                .collect::<BTreeSet<_>>(),
            reference
        );
        let mut expected_edges = subject
            .dependency_requests()
            .iter()
            .filter(|dependency| {
                dependency.purpose().is_product() && reference.contains(dependency.requester())
            })
            .collect::<Vec<_>>();
        let mut actual_edges = actual
            .iter()
            .flat_map(|&position| outgoing[position])
            .collect::<Vec<_>>();
        let order =
            |left: &&crate::resolution::graph::CanonicalDependencySourceSelection,
             right: &&crate::resolution::graph::CanonicalDependencySourceSelection| {
                left.requester()
                    .cmp(right.requester())
                    .then(left.alias().cmp(right.alias()))
            };
        expected_edges.sort_by(order);
        actual_edges.sort_by(order);
        assert_eq!(
            actual_edges, expected_edges,
            "retain exact requester-local aliases"
        );
    }
}

#[test]
fn shared_product_graph_and_single_emission_preserve_reference_bytes_and_limits() {
    let fixture = Fixture::new();
    fixture.package(
        "root",
        concat!(
            "builder.depend(Source::Path { location: \"../left\" });",
            "builder.depend_as(\"right_branch\", Source::Path { location: \"../right\" });",
            "builder.build_depend_as(\"left\", Source::Path { location: \"../tool\" });",
        ),
    );
    fixture.package(
        "left",
        "builder.depend(Source::Path { location: \"../shared\" });",
    );
    fixture.package(
        "right",
        "builder.depend_as(\"shared_override\", Source::Path { location: \"../shared\" });",
    );
    fixture.package("shared", "");
    fixture.package("tool", "");
    let storage = SourceResolverStorage::for_hardened_base(
        fixture.0.join("cache"),
        PrimaryGitChoices::default(),
    )
    .unwrap();
    let closure = resolve_external_local_package_closure(
        fixture.0.join("root"),
        ExternalSourceContext::derive(b"reconstruction-emission"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .unwrap();
    let target = closure.for_exact_target(target::TargetProfile::WindowsX64);
    let limits = CanonicalPackageReconstructionQuestionLimits::default();
    let subject =
        CanonicalSourceClosureSubject::from_resolved(&target, limits.source_closure).unwrap();
    assert_eq!(subject.packages().len(), 5);
    assert_eq!(subject.dependency_requests().len(), 5);
    assert_reference_reachability(&subject);
    let reviews = compile_resolved_package_reviews(
        &target,
        &fixture.0.join("build"),
        SemanticBindingReview::Discover,
    )
    .unwrap();
    let question = CanonicalPackageReconstructionQuestion::from_resolved_and_reviews(
        &target, &reviews, limits,
    )
    .unwrap();

    let roster = PackageOccurrenceRoster::derive(&subject).unwrap();
    for entry in question.entries() {
        assert!(roster.is_occurrence(entry.package(), entry.context().purpose()));
        assert_eq!(
            entry.context().purpose(),
            if entry.package().name().as_str() == "tool" {
                DependencyPurpose::Build
            } else {
                DependencyPurpose::Product
            }
        );
    }

    let mut reference = b"OMEGA-PACKAGE-RECONSTRUCTION-QUESTION\0".to_vec();
    reference.extend_from_slice(&3u16.to_le_bytes());
    let append = |output: &mut Vec<u8>, bytes: &[u8]| {
        output.extend_from_slice(&u32::try_from(bytes.len()).unwrap().to_le_bytes());
        output.extend_from_slice(bytes);
    };
    append(&mut reference, subject.canonical_bytes());
    reference.extend_from_slice(
        &u32::try_from(question.entries().len())
            .unwrap()
            .to_le_bytes(),
    );
    let mut total_bytes = 0;
    let mut maximum_bytes = 0;
    for entry in question.entries() {
        let bytes = encode_ordinary_package_obligation_ledger(entry.obligations()).unwrap();
        total_bytes += bytes.len();
        maximum_bytes = maximum_bytes.max(bytes.len());
        reference.extend_from_slice(
            &(if entry.context().purpose().is_product() {
                0u16
            } else {
                1u16
            })
            .to_le_bytes(),
        );
        append(
            &mut reference,
            entry.context().target().identity().as_str().as_bytes(),
        );
        append(
            &mut reference,
            entry
                .context()
                .build_execution_profile()
                .map_or(&[][..], |profile| profile.identity().as_str().as_bytes()),
        );
        append(&mut reference, &bytes);
    }
    assert_eq!(question.canonical_bytes(), reference);
    let exact = CanonicalPackageReconstructionQuestionLimits {
        maximum_record_bytes: reference.len(),
        maximum_packages: question.entries().len(),
        maximum_ledger_bytes: maximum_bytes,
        maximum_total_ledger_bytes: total_bytes,
        ..limits
    };
    assert_eq!(
        CanonicalPackageReconstructionQuestion::from_resolved_and_reviews(&target, &reviews, exact)
            .unwrap(),
        question
    );
    assert_eq!(
        CanonicalPackageReconstructionQuestion::recover(&reference, exact).unwrap(),
        question
    );
    for insufficient in [
        CanonicalPackageReconstructionQuestionLimits {
            maximum_record_bytes: reference.len() - 1,
            ..exact
        },
        CanonicalPackageReconstructionQuestionLimits {
            maximum_packages: question.entries().len() - 1,
            ..exact
        },
        CanonicalPackageReconstructionQuestionLimits {
            maximum_ledger_bytes: maximum_bytes - 1,
            ..exact
        },
        CanonicalPackageReconstructionQuestionLimits {
            maximum_total_ledger_bytes: total_bytes - 1,
            ..exact
        },
    ] {
        assert!(
            CanonicalPackageReconstructionQuestion::from_resolved_and_reviews(
                &target,
                &reviews,
                insufficient
            )
            .is_err()
        );
        assert!(CanonicalPackageReconstructionQuestion::recover(&reference, insufficient).is_err());
    }
    let mut trailing = reference.clone();
    trailing.push(0);
    assert!(CanonicalPackageReconstructionQuestion::recover(&trailing, limits).is_err());
    assert!(
        CanonicalPackageReconstructionQuestion::recover(&reference[..reference.len() - 1], limits)
            .is_err()
    );
}

#[test]
fn dual_role_questions_keep_exact_context_and_reject_substitutions() {
    let Some(execution) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let product = if execution == target::TargetProfile::WindowsX64 {
        target::TargetProfile::LinuxX64
    } else {
        target::TargetProfile::WindowsX64
    };
    let fixture = Fixture::new();
    fixture.package(
        "root",
        concat!(
            "builder.depend_as(\"shared_product\", Source::Path { location: \"../shared\" });",
            "builder.build_depend_as(\"shared_build\", Source::Path { location: \"../shared\" });",
        ),
    );
    fixture.package(
        "shared",
        "builder.depend(Source::Path { location: \"../leaf\" });",
    );
    fixture.package("leaf", "");
    let storage = SourceResolverStorage::for_hardened_base(
        fixture.0.join("cache"),
        PrimaryGitChoices::default(),
    )
    .unwrap();
    let closure = resolve_external_local_package_closure(
        fixture.0.join("root"),
        ExternalSourceContext::derive(b"dual-reconstruction"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .unwrap();
    let target = closure.for_exact_target(product);
    let reviews = compile_resolved_package_reviews(
        &target,
        &fixture.0.join("build"),
        SemanticBindingReview::Discover,
    )
    .unwrap();
    let limits = CanonicalPackageReconstructionQuestionLimits::default();
    let question = CanonicalPackageReconstructionQuestion::from_resolved_and_reviews(
        &target, &reviews, limits,
    )
    .unwrap();
    assert_eq!(question.entries().len(), 5);
    assert_eq!(question.target_name(), product.target_name());
    assert_eq!(
        CanonicalPackageReconstructionQuestion::recover(question.canonical_bytes(), limits)
            .unwrap(),
        question
    );
    let shared = question
        .entries()
        .iter()
        .filter(|entry| entry.package().name().as_str() == "shared")
        .collect::<Vec<_>>();
    assert_eq!(shared.len(), 2);
    assert_eq!(
        shared[0].context(),
        PackageCheckedContext::new(DependencyPurpose::Product, product, Some(execution))
    );
    assert_eq!(
        shared[1].context(),
        PackageCheckedContext::new(DependencyPurpose::Build, execution, Some(execution))
    );
    let composed =
        crate::review::LocallyComposedPackageObligationResults::from_resolved_and_reviews(
            &target, &reviews, limits,
        )
        .unwrap();
    for (entry, result) in question.entries().iter().zip(composed.entries()) {
        assert_eq!(entry.package(), result.package());
        assert_eq!(entry.context(), result.context());
    }
    let changes = crate::review::compare_package_policy_changes(
        None,
        &reviews,
        &target,
        crate::review::PackagePolicyChangeLimits::default(),
    )
    .unwrap();
    assert!(!changes.requires_decision());
    let decisions = crate::review::resolve_package_policy_decisions(
        &changes,
        changes.fingerprint().digest(),
        &[],
    )
    .unwrap();
    let history = crate::lock::HistoricalPackagePolicyDecisions::capture_policy(
        question.source_closure(),
        &changes,
        &decisions,
        crate::lock::HistoricalPackagePolicyLimits::default(),
    )
    .unwrap();
    let policies = question
        .entries()
        .iter()
        .map(|entry| {
            let review = reviews
                .reviews()
                .iter()
                .find(|review| {
                    review.key() == entry.package() && review.checked_context() == entry.context()
                })
                .unwrap();
            (entry.context(), review.policy())
        })
        .collect::<Vec<_>>();
    let accepted = crate::lock::PackageLockTarget::from_policies(
        question.source_closure().clone(),
        &policies,
        history,
    )
    .unwrap();
    let bound = crate::review::bind_fresh_package_root_policy(
        &target,
        &reviews,
        limits,
        crate::review::ReviewOnlyCapabilityConflictLimits::default(),
        Some(&accepted),
    )
    .unwrap();
    assert_eq!(bound.obligations(), &composed);
    assert!(!bound.policy_changes().requires_decision());

    let reject = |entries| {
        assert!(
            CanonicalPackageReconstructionQuestion::finish(
                question.source_closure().clone(),
                entries,
                limits
            )
            .is_err()
        )
    };
    let mut missing = question.entries().to_vec();
    missing.pop();
    reject(missing);
    let mut duplicate = question.entries().to_vec();
    duplicate.push(duplicate[0].clone());
    reject(duplicate);
    let build_position = question
        .entries()
        .iter()
        .position(|entry| entry.context().purpose() == DependencyPurpose::Build)
        .unwrap();
    let mut wrong_purpose = question.entries().to_vec();
    wrong_purpose[build_position].context =
        PackageCheckedContext::new(DependencyPurpose::Product, execution, Some(execution));
    reject(wrong_purpose);
    let mut missing_profile = question.entries().to_vec();
    missing_profile[build_position].context =
        PackageCheckedContext::new(DependencyPurpose::Build, execution, None);
    reject(missing_profile);
    let mut wrong_target = question.entries().to_vec();
    wrong_target[build_position].context =
        PackageCheckedContext::new(DependencyPurpose::Build, product, Some(execution));
    reject(wrong_target);
    let mut substituted_ledger = question.entries().to_vec();
    let product_position = question
        .entries()
        .iter()
        .position(|entry| {
            entry.package() == &substituted_ledger[build_position].package
                && entry.context().purpose() == DependencyPurpose::Product
        })
        .unwrap();
    substituted_ledger[build_position].obligations =
        substituted_ledger[product_position].obligations.clone();
    reject(substituted_ledger);
    let version_offset = b"OMEGA-PACKAGE-RECONSTRUCTION-QUESTION\0".len();
    for version in [1u16, 2u16] {
        let mut old = question.canonical_bytes().to_vec();
        old[version_offset..version_offset + 2].copy_from_slice(&version.to_le_bytes());
        assert!(CanonicalPackageReconstructionQuestion::recover(&old, limits).is_err());
    }
}
