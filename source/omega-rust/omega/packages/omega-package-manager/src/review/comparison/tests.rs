use super::commitments::derive_candidate_closure_commitment;
use super::*;
use crate::resolution::{
    PackageSourceClosureLimits, ResolvedPackageSourceClosure,
    resolve_external_local_package_closure,
};
use crate::review::records::PackageReviewEvidence;
use crate::review::{ReviewOnlyCanonicalRow, ReviewOnlySourceConsumptionCommitment};
use omega_package_source::{
    ExternalSourceContext, ImmutableSourceResolution, LocalSourceLimits, PackageKey,
};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
struct TestReview {
    key: PackageKey,
    resolution: ImmutableSourceResolution,
    target: String,
    executable_incident_metadata: [u8; 32],
    source_consumption: ReviewOnlySourceConsumptionCommitment,
    build_observation: Option<[u8; 32]>,
    whole_review: [u8; 32],
    rows: Vec<ReviewOnlyCanonicalRow>,
}

impl PackageReviewEvidence for TestReview {
    fn key(&self) -> &PackageKey {
        &self.key
    }

    fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
    }

    fn projection_identity_matches(&self) -> bool {
        true
    }

    fn target_name(&self) -> &str {
        &self.target
    }

    fn source_consumption_commitment(&self) -> ReviewOnlySourceConsumptionCommitment {
        self.source_consumption
    }

    fn build_observation_commitment(&self) -> Option<[u8; 32]> {
        self.build_observation
    }

    fn whole_review_commitment(&self) -> [u8; 32] {
        self.whole_review
    }

    fn canonical_rows(&self) -> &[ReviewOnlyCanonicalRow] {
        &self.rows
    }
}

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-candidate-closure-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn write_package(root: &Path, name: &str, dependency: Option<&str>) {
    std::fs::create_dir_all(root).expect("create package root");
    let dependency = dependency.map_or_else(String::new, |location| {
        format!("    builder.depend(Source::Path {{\n        location: \"{location}\"\n    }});\n")
    });
    std::fs::write(
            root.join("build.omg"),
            format!(
                "machine build(builder: &mut Build) {{\n    builder.package(\"{name}\");\n{dependency}}}\n"
            ),
        )
        .expect("write package build declaration");
    std::fs::write(root.join("main.omg"), "pub machine value() -> u64 { 1 }\n")
        .expect("write package source");
}

fn commitment(
    closure: &ResolvedPackageSourceClosure,
    reviews: &[TestReview],
) -> ReviewOnlyCandidateClosureCommitment {
    let review_refs = reviews.iter().collect::<Vec<_>>();
    derive_candidate_closure_commitment(closure, &review_refs)
        .expect("derive candidate closure commitment")
}

#[test]
fn candidate_closure_binds_review_evidence_from_every_package() {
    let parent = temp_root("evidence");
    let root = parent.join("root");
    let dependency = parent.join("dependency");
    let cache = temp_root("cache");
    write_package(&dependency, "closure-dependency", None);
    write_package(&root, "closure-root", Some("../dependency"));
    let closure = resolve_external_local_package_closure(
        &root,
        ExternalSourceContext::derive(b"candidate-closure-review-evidence"),
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve two-package source closure");

    let mut reviews = closure
        .graph()
        .packages()
        .iter()
        .enumerate()
        .map(|(index, package)| TestReview {
            key: package.source().key().clone(),
            resolution: package.source().resolution().clone(),
            target: "windows_x64".to_owned(),
            executable_incident_metadata: [1; 32],
            source_consumption: ReviewOnlySourceConsumptionCommitment::from_recovered_digest(
                [2; 32],
            ),
            build_observation: None,
            whole_review: [u8::try_from(index + 3).expect("small fixture index"); 32],
            rows: Vec::new(),
        })
        .collect::<Vec<_>>();
    reviews.sort_by(|left, right| left.key.cmp(&right.key));
    let dependency_index = reviews
        .iter()
        .position(|review| review.key.name().as_str() == "closure-dependency")
        .expect("dependency review");
    let baseline = commitment(&closure, &reviews);

    let mut metadata_only = reviews.clone();
    metadata_only[dependency_index].executable_incident_metadata = [9; 32];
    assert_eq!(commitment(&closure, &metadata_only), baseline);

    for change in 0..4 {
        let mut changed = reviews.clone();
        let review = &mut changed[dependency_index];
        match change {
            0 => review.target = "linux_x64".to_owned(),
            1 => {
                review.source_consumption =
                    ReviewOnlySourceConsumptionCommitment::from_recovered_digest([9; 32])
            }
            2 => review.build_observation = Some([9; 32]),
            3 => review.whole_review = [9; 32],
            _ => unreachable!("four semantic evidence axes"),
        }
        assert_ne!(commitment(&closure, &changed), baseline);
    }

    let _ = std::fs::remove_dir_all(parent);
    let _ = std::fs::remove_dir_all(cache);
}
