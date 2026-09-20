use super::{outgoing_product_requests, reachable_source_packages};
use crate::declarations::dependencies::DependencyPurpose;
use crate::lock::PackageOccurrenceRoster;
use crate::lock::occurrences::purpose_mask_bit;
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
        assert_eq!(
            entry.occurrence_purposes(),
            roster.purposes(entry.package()).unwrap(),
            "each entry's review covers the exact roster occurrences"
        );
        assert_eq!(
            entry.occurrence_purposes(),
            if entry.package().name().as_str() == "tool" {
                &[DependencyPurpose::Build]
            } else {
                &[DependencyPurpose::Product]
            }
            .as_ref()
        );
    }

    let mut reference = b"OMEGA-PACKAGE-RECONSTRUCTION-QUESTION\0".to_vec();
    reference.extend_from_slice(&2u16.to_le_bytes());
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
        let mut frame = Vec::with_capacity(bytes.len() + 1);
        frame.push(
            entry
                .occurrence_purposes()
                .iter()
                .fold(0u8, |mask, purpose| mask | purpose_mask_bit(*purpose)),
        );
        frame.extend_from_slice(&bytes);
        append(&mut reference, &frame);
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
