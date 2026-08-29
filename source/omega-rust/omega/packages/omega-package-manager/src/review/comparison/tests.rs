use super::commitments::{derive_candidate_closure_commitment, derive_candidate_graph_commitment};
use super::*;
use crate::identity::PackageKey;
use crate::manifest::BuildDeclarationKind;
use crate::resolution::{
    PackageSourceClosureLimits, ResolvedPackageClosure, ResolvedPackageSourceClosure,
    resolve_external_local_package_closure,
};
use crate::review::records::PackageReviewEvidence;
use crate::review::{ReviewOnlyCanonicalRow, ReviewOnlySourceConsumptionCommitment};
use omega_package_source::{ExternalSourceContext, ImmutableSourceResolution, LocalSourceLimits};
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
        omega_target::TargetProfile::CrossPlatformCli,
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

#[test]
fn candidate_closure_binds_the_selected_target_profile() {
    let root = temp_root("target-profile");
    let cache = temp_root("target-profile-cache");
    write_package(&root, "profile-probe", None);
    let source_context = ExternalSourceContext::derive(b"candidate-closure-target-profile");
    let windows = resolve_external_local_package_closure(
        &root,
        source_context.clone(),
        omega_target::TargetProfile::WindowsX64,
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve Windows closure");
    let linux = resolve_external_local_package_closure(
        &root,
        source_context,
        omega_target::TargetProfile::LinuxX64,
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve Linux closure");
    assert_eq!(windows.graph(), linux.graph());

    let reviews = windows
        .graph()
        .packages()
        .iter()
        .map(|package| TestReview {
            key: package.source().key().clone(),
            resolution: package.source().resolution().clone(),
            target: "same-compiler-target".to_owned(),
            executable_incident_metadata: [1; 32],
            source_consumption: ReviewOnlySourceConsumptionCommitment::from_recovered_digest(
                [2; 32],
            ),
            build_observation: None,
            whole_review: [3; 32],
            rows: Vec::new(),
        })
        .collect::<Vec<_>>();

    assert_ne!(
        commitment(&windows, &reviews),
        commitment(&linux, &reviews),
        "review identity must bind the package closure's selected profile"
    );

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
fn candidate_closure_and_directional_review_bind_the_exact_root_role() {
    let root = temp_root("root-role");
    let cache = temp_root("root-role-cache");
    write_package(&root, "role-probe", None);
    let closure = resolve_external_local_package_closure(
        &root,
        ExternalSourceContext::derive(b"candidate-closure-root-role"),
        omega_target::TargetProfile::CrossPlatformCli,
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve one-package source closure");
    let package_graph = closure.graph().clone();
    let application_graph = ResolvedPackageClosure::new(
        package_graph.root().clone(),
        BuildDeclarationKind::Application,
        package_graph.packages().to_vec(),
    )
    .expect("same package graph may select an application root");
    let reviews = package_graph
        .packages()
        .iter()
        .map(|package| TestReview {
            key: package.source().key().clone(),
            resolution: package.source().resolution().clone(),
            target: "windows_x64".to_owned(),
            executable_incident_metadata: [1; 32],
            source_consumption: ReviewOnlySourceConsumptionCommitment::from_recovered_digest(
                [2; 32],
            ),
            build_observation: None,
            whole_review: [3; 32],
            rows: Vec::new(),
        })
        .collect::<Vec<_>>();
    let review_refs = reviews.iter().collect::<Vec<_>>();

    assert_ne!(
        derive_candidate_graph_commitment(&package_graph, &review_refs)
            .expect("commit package-root graph"),
        derive_candidate_graph_commitment(&application_graph, &review_refs)
            .expect("commit application-root graph"),
        "candidate closure identity must bind root role"
    );
    assert_eq!(
        compare_review_only_root_role_graphs(&package_graph, &package_graph)
            .expect("same-role comparison"),
        None
    );
    let dependency_break = compare_review_only_root_role_graphs(&package_graph, &application_graph)
        .expect("package-to-application comparison")
        .expect("role changed");
    assert_eq!(
        dependency_break.broken_contract(),
        ReviewOnlyRootRoleContract::DependencyCompatibility
    );
    assert_eq!(
        dependency_break.baseline_role(),
        BuildDeclarationKind::Package
    );
    assert_eq!(
        dependency_break.candidate_role(),
        BuildDeclarationKind::Application
    );
    assert!(dependency_break.is_blocking());

    let activation_break = compare_review_only_root_role_graphs(&application_graph, &package_graph)
        .expect("application-to-package comparison")
        .expect("role changed");
    assert_eq!(
        activation_break.broken_contract(),
        ReviewOnlyRootRoleContract::ApplicationActivation
    );

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(cache);
}
