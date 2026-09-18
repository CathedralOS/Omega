//! One-field substitution coverage for [`ProductionCompilationManifest`], the
//! canonical package-source/build/target/artifact join a package-aware
//! production report retains.
//!
//! The manifest's producer is the checked-compilation production-subject
//! projection plus `ProductionCompilationManifest::for_terminal`/`for_native`:
//! canonical bytes commit the package root digest and role, the strictly
//! ordered package closure, the strictly ordered dependency edges, the
//! source-consumption commitment, the ordered consumed-source rows, the
//! selected build-machine identity, every `BuildEvaluationUsage` axis, the
//! build-observation identity, the target-profile tag, the native-target axes,
//! and the artifact kind plus artifact identity; the manifest identity is the
//! SHA-256 of those bytes under `OMEGA-PRODUCTION-COMPILATION-MANIFEST-V9`.
//!
//! Three layers see a substitution differently:
//!
//! - `validate` recomputes the canonical bytes and identity over the retained
//!   subject and artifact coordinate, so a substitution that leaves the
//!   containing bytes or identity stale is rejected before any downstream
//!   join runs;
//! - a substitution whose containing bytes and identity are honestly
//!   recomputed is a different self-consistent record: its manifest identity
//!   diverges from the production any deployment journal pins, and the
//!   downstream joins that rebind the changed axis — the artifact/target join
//!   in `matches_native_artifact` and `require_native_physical_evidence`, the
//!   proposal target join and artifact join in report custody, the
//!   receipt/artifact join on a published executable, and the
//!   `derive_*`/`has_same_invocation_usage` source replays over the claimed
//!   subject — reject it;
//! - subject axes the report joins deliberately do not rebind (package
//!   internals, the build-machine identity, invocation usage axes, and the
//!   build-observation identity on an artifact-consistent claim) stay admitted
//!   at the report level as a different self-consistent manifest — the
//!   divergent manifest identity and the source replays are the rejection.
//!
//! `NativeArtifact` and `CanonicalTerminalArtifact` are unforgeable from
//! outside their constructors — `NativeArtifact::from_replayed_parts`
//! revalidates the complete retained authority on construction — so foreign
//! artifact fixtures are honest re-realizations or rejoined replay parts, not
//! edited internals.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use build_evaluation::{BuildEvaluationUsage, BuildObservationSummary};
use compilation_report::{
    CompileOutputKind, CompileReport, FinalRealizationEvidenceError, ProductionArtifactIdentity,
    ProductionCompilationManifest, ProductionCompilationManifestIdentity,
    ProductionCompilationSubject, RetainedTerminalArtifact,
};
use compiler::{
    CheckedCompilation, CheckedCompileRequest, CompileOptions, CompileRequest,
    RequestedCompileProduct, RetainedNativeRealizationRequest, compile, compile_to_checked,
    realize_retained_native_artifact,
};
use native_artifact::{NativeArtifact, NativeArtifactParts};
use package_compilation::{
    BuildDeclarationKind, ConsumedSourceUnit, ConsumedSourceUnitKind, PackageCompilationInputs,
    PackageCompilationSubject, PackageDependencyBinding, PackageDependencyClosure,
    PackageSourceBinding, PackageSourceConsumptionCommitment, derive_consumed_source_units,
    derive_package_compilation_subject, derive_source_consumption_commitment,
};
use semantic_vocabulary::PackageKeyIdentity;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-production-manifest-custody-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create temporary custody tree");
        Self(path)
    }

    fn package(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).expect("create package directory");
        path
    }

    fn write(path: impl AsRef<Path>, source: &str) {
        fs::write(path, source).expect("write Omega package test source");
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero package identity")
}

const ROOT_PACKAGE: u8 = 11;
const DEP_PACKAGE: u8 = 12;
const LEAF_PACKAGE: u8 = 13;
const FOREIGN_ROOT_PACKAGE: u8 = 21;
const FOREIGN_DEP_PACKAGE: u8 = 22;
const FOREIGN_LEAF_PACKAGE: u8 = 23;

/// One three-package chain (`host-app` -> `dep_lib` -> `leaf_ns`) compiled as
/// the production subject. `dirs` keeps the two staged projects apart inside
/// one temporary tree; `leaf_source` is the leaf module's complete text, so
/// the foreign fixture can differ in reachable Terminal semantics (and
/// therefore artifact identity) rather than only in pruned declarations.
fn package_project(
    tree: &TempTree,
    dirs: [&str; 3],
    markers: [u8; 3],
    leaf_source: &str,
) -> (PathBuf, PackageCompilationInputs) {
    let root = tree.package(dirs[0]);
    let dep = tree.package(dirs[1]);
    let leaf = tree.package(dirs[2]);
    TempTree::write(
        root.join("main.omg"),
        "use dep_lib::lib;\ndata Main { }\nmachine Main::main(&mut self) { invoke(); }\n",
    );
    TempTree::write(
        root.join("build.omg"),
        "machine build(builder: &mut Build) {\n    builder.package(\"host-app\");\n    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);\n}\n",
    );
    TempTree::write(
        dep.join("lib.omg"),
        "use leaf_ns::leaf;\npub machine invoke() { noop(); }\n",
    );
    TempTree::write(leaf.join("leaf.omg"), leaf_source);
    let inputs = PackageCompilationInputs::new_package(
        identity(markers[0]),
        vec![
            PackageSourceBinding::new(identity(markers[0]), "host-app", root.clone()),
            PackageSourceBinding::new(identity(markers[1]), "dep-lib", dep),
            PackageSourceBinding::new(identity(markers[2]), "leaf-ns", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(markers[0]), "dep_lib", identity(markers[1])),
            PackageDependencyBinding::new(identity(markers[1]), "leaf_ns", identity(markers[2])),
        ],
    )
    .expect("three-package graph");
    (root.join("main.omg"), inputs)
}

/// Compile one staged package project to a checked compilation.
fn check_project(root_path: &Path, inputs: PackageCompilationInputs) -> CheckedCompilation {
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(root_path, Some("linux_x86_64"))
    })
    .expect("the staged package compiles through package-aware checking")
}

/// Produce the retained Terminal product report for one package project.
fn terminal_product_report(
    root_path: &Path,
    inputs: PackageCompilationInputs,
    build_dir: &Path,
) -> CompileReport {
    compile(
        CompileRequest::new(CompileOptions {
            root_path: root_path.to_owned(),
            build_dir: Some(build_dir.to_owned()),
            target_name: Some("linux_x86_64".to_owned()),
        })
        .with_package_inputs(inputs)
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("the package project retains its Terminal product")
}

/// Realize one retained Terminal product into its native artifact exactly as
/// retained production does.
fn realize_native(retained: RetainedTerminalArtifact) -> NativeArtifact {
    let profile = proof_admission::AdmissionProfile::default();
    let proposal = retained
        .native_realization_proposal()
        .expect("the retained Terminal product carries a native proposal");
    let subsystem = proposal.subsystem();
    let optimization_selections = proposal.post_terminal_optimizations().selections().clone();
    realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
            profile: &profile,
            optimization_selections: &optimization_selections,
            terminal_authority_policy: native_realization::current_terminal_authority_policy(),
            accepted_package_terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy: Some(
                native_realization::current_terminal_authority_permission_policy(),
            ),
            image_request: native_realization::ExecutableImageEmissionRequest::direct(subsystem),
            imports: &[],
        },
    )
    .unwrap_or_else(|(_, diagnostics)| {
        panic!("the retained product realizes natively: {diagnostics:#?}")
    })
    .into_direct()
    .unwrap_or_else(|_| panic!("the flat image request keeps direct artifact custody"))
}

/// Re-encode replayed artifact parts: the Terminal artifact re-derives its
/// sealed sections and manifest rather than cloning retained bytes.
fn replay_native_artifact_parts(parts: &NativeArtifactParts) -> NativeArtifactParts {
    let module = terminal_codec::decode_module(parts.psi_artifact.semantic_bytes())
        .expect("replay Terminal semantics");
    let proof = terminal_codec::decode_proof_bundle(parts.psi_artifact.proof_bytes())
        .expect("replay Terminal proof");
    let debug = parts
        .psi_artifact
        .debug_bytes()
        .map(|bytes| terminal_codec::decode_debug_map(&module, bytes).expect("debug map"));
    NativeArtifactParts {
        target: parts.target,
        psi_artifact: terminal_codec::CanonicalTerminalArtifact::from_parts(
            &module,
            &proof,
            parts.psi_artifact.optimization(),
            debug.as_ref(),
        )
        .expect("reconstruct canonical Terminal artifact"),
        object: parts.object.clone(),
        image: parts.image.clone(),
        selected_provider_closure_report_identity: parts.selected_provider_closure_report_identity,
        selected_provider_closure_digest: parts.selected_provider_closure_digest,
        selected_provider_plans: parts.selected_provider_plans.clone(),
        provider_executions: parts.provider_executions.clone(),
        terminal_authority_policy_identity: parts.terminal_authority_policy_identity,
        terminal_authority_permission_policy_identity: parts
            .terminal_authority_permission_policy_identity,
        terminal_authority_closure_review: parts.terminal_authority_closure_review.clone(),
        boundary_application_coverage: parts.boundary_application_coverage.clone(),
        physical_evidence_scope: parts.physical_evidence_scope.clone(),
        physical_evidence: parts.physical_evidence.clone(),
    }
}

/// The atomic subject parts one `ProductionCompilationSubject::from_checked`
/// joins; each mutation leg substitutes exactly one field.
#[derive(Clone)]
struct SubjectParts {
    package: PackageCompilationSubject,
    machine: String,
    usage: BuildEvaluationUsage,
    observation: BuildObservationSummary,
    profile: target::TargetProfile,
    native: target::NativeTarget,
}

fn mint_subject(parts: &SubjectParts) -> Result<ProductionCompilationSubject, &'static str> {
    ProductionCompilationSubject::from_checked(
        parts.package.clone(),
        parts.machine.clone(),
        parts.usage,
        &parts.observation,
        parts.profile,
        parts.native,
    )
}

/// Rebuild one canonical package closure from substituted parts; rows are
/// sorted into canonical order before the closure's own ordering, reachability,
/// and acyclicity checks run, so a malformed arrangement rejects here.
fn closure_from(
    root: PackageKeyIdentity,
    role: BuildDeclarationKind,
    mut packages: Vec<PackageKeyIdentity>,
    mut dependencies: Vec<PackageDependencyBinding>,
) -> Result<PackageDependencyClosure, &'static str> {
    packages.sort();
    dependencies.sort_by(|left, right| {
        (left.requester(), left.alias()).cmp(&(right.requester(), right.alias()))
    });
    PackageDependencyClosure::from_canonical_parts(root, role, packages, dependencies)
}

/// Rebuild the package subject over substituted parts.
fn package_subject_from(
    root: PackageKeyIdentity,
    closure: PackageDependencyClosure,
    commitment: PackageSourceConsumptionCommitment,
    units: Vec<ConsumedSourceUnit>,
) -> Result<PackageCompilationSubject, &'static str> {
    PackageCompilationSubject::for_test(root, closure, commitment, units)
}

/// One internally consistent sponsored usage block: every session ceiling
/// exceeds the observed invocation totals so `from_checked` admits it.
fn sponsored_usage(usage: BuildEvaluationUsage) -> BuildEvaluationUsage {
    let mut sponsored = usage;
    sponsored.sponsor_schema_version = Some(1);
    sponsored.session_fuel_ceiling = Some(usage.fuel_units + usage.replay_fuel_units + 1);
    sponsored.session_build_log_byte_ceiling =
        Some(usage.build_log_bytes + usage.replay_build_log_bytes + 1);
    sponsored.session_filesystem_attempt_ceiling =
        Some(usage.filesystem_operation_attempts + usage.replay_filesystem_operation_attempts + 1);
    sponsored.session_live_filesystem_handle_ceiling =
        Some(usage.session_peak_live_filesystem_handles + 1);
    sponsored.session_live_cell_ceiling = Some(
        usage
            .peak_live_cells
            .max(usage.replay_peak_live_cells)
            .max(usage.session_peak_live_cells)
            + 1,
    );
    sponsored.session_live_text_byte_ceiling = Some(
        usage
            .peak_live_text_bytes
            .max(usage.replay_peak_live_text_bytes)
            .max(usage.session_peak_live_text_bytes)
            + 1,
    );
    sponsored.session_result_cell_ceiling =
        Some(usage.result_cells + usage.replay_result_cells + 1);
    sponsored.session_result_text_byte_ceiling =
        Some(usage.result_text_bytes + usage.replay_result_text_bytes + 1);
    sponsored.session_peak_live_cells = usage
        .peak_live_cells
        .max(usage.replay_peak_live_cells)
        .max(usage.session_peak_live_cells);
    sponsored.session_peak_live_text_bytes = usage
        .peak_live_text_bytes
        .max(usage.replay_peak_live_text_bytes)
        .max(usage.session_peak_live_text_bytes);
    sponsored
}

/// The whole staged fixture: the authentic package-aware production's checked
/// custody and subject parts, its retained Terminal product report, and one
/// honestly produced foreign production whose coordinates every substitution
/// leg may claim. The retained Terminal report still owns its artifact — the
/// test consumes it into native realization after the Terminal-custody legs.
struct Fixture {
    tree: TempTree,
    root_path: PathBuf,
    source_file_count: usize,
    inputs: PackageCompilationInputs,
    parts: SubjectParts,
    replayed_units: Vec<ConsumedSourceUnit>,
    replayed_package: PackageCompilationSubject,
    /// The retained-Terminal report minted by the real produce path; its
    /// manifest is the authentic `Terminal` coordinate.
    terminal_report: CompileReport,
    foreign_package: PackageCompilationSubject,
    foreign_manifest: ProductionCompilationManifest,
    foreign_terminal_identity: terminal_codec::TerminalArtifactIdentity,
    foreign_native: NativeArtifact,
}

fn fixture() -> Fixture {
    let tree = TempTree::new();
    let (root_path, inputs) = package_project(
        &tree,
        ["host-app", "dep-lib", "leaf-ns"],
        [ROOT_PACKAGE, DEP_PACKAGE, LEAF_PACKAGE],
        "pub machine noop() { }\n",
    );
    let checked = check_project(&root_path, inputs.clone());
    let package = checked
        .package_compilation_subject()
        .expect("package-aware check retains its package subject")
        .clone();
    let parts = SubjectParts {
        package,
        machine: checked
            .selected_build_machine_identity()
            .expect("checked package retains its build-machine identity")
            .to_owned(),
        usage: checked
            .build_evaluation_usage()
            .expect("checked package retains build-evaluation usage"),
        observation: checked
            .build_observation_summary()
            .expect("checked package retains build-observation custody")
            .clone(),
        profile: checked
            .selected_target_profile()
            .expect("checked package retains its selected profile"),
        native: checked
            .selected_native_target()
            .expect("checked package retains its selected native target"),
    };
    let subject = mint_subject(&parts).expect("the honest subject mints");
    assert_eq!(
        subject,
        checked
            .production_subject()
            .expect("checked package projects its production subject")
            .expect("package-aware production subject"),
        "the staged parts replay the real production subject"
    );
    let source_file_count = checked.source_file_count();
    let replayed_units = derive_consumed_source_units(checked.terminal_production_trees(), &[])
        .expect("the consumed-source projection replays");
    let replayed_package =
        derive_package_compilation_subject(checked.terminal_production_trees(), &inputs, &[])
            .expect("the package subject replays");
    assert_eq!(replayed_package, parts.package);
    assert_eq!(replayed_units, parts.package.consumed_units());

    let terminal_report =
        terminal_product_report(&root_path, inputs.clone(), &tree.0.join("terminal-build"));
    let authentic_terminal_manifest = terminal_report
        .production_manifest()
        .expect("the Terminal report retains its production manifest");
    assert_eq!(authentic_terminal_manifest.subject(), &subject);

    // The foreign leaf keeps the same `noop` surface but routes it through an
    // extra machine, so its Terminal semantics — and therefore its Terminal
    // and native artifact identities — differ from the authentic artifact.
    let (foreign_root_path, foreign_inputs) = package_project(
        &tree,
        ["foreign-app", "foreign-dep", "foreign-leaf"],
        [
            FOREIGN_ROOT_PACKAGE,
            FOREIGN_DEP_PACKAGE,
            FOREIGN_LEAF_PACKAGE,
        ],
        "machine marker() { }\npub machine noop() { marker(); }\n",
    );
    let foreign_report = terminal_product_report(
        &foreign_root_path,
        foreign_inputs,
        &tree.0.join("foreign-terminal-build"),
    );
    let foreign_manifest = foreign_report
        .production_manifest()
        .expect("the foreign Terminal report retains its production manifest")
        .clone();
    let foreign_terminal_identity = foreign_report
        .artifact()
        .expect("the foreign Terminal report retains its artifact")
        .manifest()
        .identity();
    assert_ne!(
        foreign_terminal_identity,
        terminal_report
            .artifact()
            .expect("the Terminal report retains its artifact")
            .manifest()
            .identity(),
        "the foreign project must produce a distinct Terminal artifact identity"
    );
    let foreign_package = foreign_manifest.subject().package().clone();
    let foreign_native = realize_native(
        foreign_report
            .into_retained_terminal_artifact()
            .expect("the foreign Terminal product report retains its artifact"),
    );

    Fixture {
        tree,
        root_path,
        source_file_count,
        inputs,
        parts,
        replayed_units,
        replayed_package,
        terminal_report,
        foreign_package,
        foreign_manifest,
        foreign_terminal_identity,
        foreign_native,
    }
}

#[test]
fn production_compilation_manifest_rejects_every_one_field_substitution() {
    let mut fixture = fixture();
    let parts = fixture.parts.clone();
    let subject = mint_subject(&parts).expect("the authentic subject mints");
    let authentic_terminal = fixture
        .terminal_report
        .production_manifest()
        .expect("the Terminal report retains its production manifest")
        .clone();
    let terminal_artifact_identity = fixture
        .terminal_report
        .artifact()
        .expect("the Terminal report retains its artifact")
        .manifest()
        .identity();

    assert!(
        fixture
            .terminal_report
            .has_consistent_executable_publication_custody()
    );
    assert!(authentic_terminal.validate());
    assert_eq!(authentic_terminal.subject(), &subject);
    assert_eq!(
        authentic_terminal.artifact(),
        ProductionArtifactIdentity::Terminal(terminal_artifact_identity)
    );
    assert!(
        authentic_terminal
            .matches_terminal_artifact(fixture.terminal_report.artifact().expect("artifact"))
    );
    assert!(fixture.foreign_manifest.validate());
    assert_ne!(
        fixture.foreign_manifest.identity(),
        authentic_terminal.identity()
    );
    assert_eq!(
        fixture
            .terminal_report
            .require_package_native_physical_evidence(),
        Err(FinalRealizationEvidenceError::RetainedNativeArtifactRequired),
        "a Terminal product report makes no native-evidence claim"
    );

    let subject_for = |mutate: &dyn Fn(&mut SubjectParts)| {
        let mut changed = parts.clone();
        mutate(&mut changed);
        mint_subject(&changed)
    };

    // Every representable mutated subject variant, collected once and driven
    // through the Terminal-report legs, the native-artifact joins, and the
    // retained-native report custody.
    let mut variants: Vec<(&'static str, ProductionCompilationSubject)> = Vec::new();
    let mut admits = |name: &'static str, subject: ProductionCompilationSubject| {
        variants.push((name, subject));
    };

    // == selected_build_machine_identity ==
    admits(
        "selected build-machine identity",
        subject_for(&|p| p.machine = "foreign-build-machine".to_owned())
            .expect("a different build machine is representable"),
    );
    assert_eq!(
        subject_for(&|p| p.machine = String::new()).unwrap_err(),
        "production compilation subject has an empty build-machine identity",
        "an empty build-machine identity is unrepresentable"
    );

    // == build_evaluation_usage axes: the invocation-accounting fields ==
    let usage_fields: [(&'static str, fn(&mut BuildEvaluationUsage)); 17] = [
        ("usage_schema_version", |u| u.usage_schema_version += 1),
        ("step_schedule_marker", |u| u.step_schedule_marker += 1),
        ("invocation_fuel_ceiling", |u| {
            u.invocation_fuel_ceiling += 1
        }),
        ("fuel_units", |u| u.fuel_units += 1),
        ("replay_fuel_units", |u| u.replay_fuel_units += 1),
        ("build_log_bytes", |u| u.build_log_bytes += 1),
        ("replay_build_log_bytes", |u| u.replay_build_log_bytes += 1),
        ("filesystem_operation_attempts", |u| {
            u.filesystem_operation_attempts += 1
        }),
        ("replay_filesystem_operation_attempts", |u| {
            u.replay_filesystem_operation_attempts += 1
        }),
        ("peak_live_cells", |u| u.peak_live_cells += 1),
        ("replay_peak_live_cells", |u| u.replay_peak_live_cells += 1),
        ("peak_live_text_bytes", |u| u.peak_live_text_bytes += 1),
        ("replay_peak_live_text_bytes", |u| {
            u.replay_peak_live_text_bytes += 1
        }),
        ("result_cells", |u| u.result_cells += 1),
        ("replay_result_cells", |u| u.replay_result_cells += 1),
        ("result_text_bytes", |u| u.result_text_bytes += 1),
        ("replay_result_text_bytes", |u| {
            u.replay_result_text_bytes += 1
        }),
    ];
    assert!(
        parts.usage.invocation_fuel_ceiling > parts.usage.fuel_units
            && parts.usage.invocation_fuel_ceiling > parts.usage.replay_fuel_units,
        "the fixture's usage leaves headroom below its invocation fuel ceiling"
    );
    for (name, mutate) in usage_fields {
        let mut usage = parts.usage;
        mutate(&mut usage);
        let changed = mint_subject(&SubjectParts {
            usage,
            ..parts.clone()
        })
        .unwrap_or_else(|rejection| panic!("usage axis {name} must be representable: {rejection}"));
        assert!(
            !parts.usage.has_same_invocation_usage(usage),
            "usage axis {name}: the invocation-usage replay sees the substitution"
        );
        admits(name, changed);
    }
    let unsustainable: [(&'static str, fn(&mut BuildEvaluationUsage)); 3] = [
        (
            "zero invocation fuel ceiling",
            |u: &mut BuildEvaluationUsage| u.invocation_fuel_ceiling = 0,
        ),
        ("fuel above the invocation ceiling", |u| {
            u.fuel_units = u.invocation_fuel_ceiling + 1
        }),
        ("replay fuel above the invocation ceiling", |u| {
            u.replay_fuel_units = u.invocation_fuel_ceiling + 1
        }),
    ];
    for (name, mutate) in unsustainable {
        let mut usage = parts.usage;
        mutate(&mut usage);
        assert!(
            mint_subject(&SubjectParts {
                usage,
                ..parts.clone()
            })
            .is_err(),
            "{name}: the unsustainable accounting is unrepresentable"
        );
    }

    // The sponsor/session axes are committed in canonical bytes even though
    // `has_same_invocation_usage` deliberately excludes them: a package review
    // may run inside a shared sponsored session while production runs alone.
    let partial_sponsor: [(&'static str, fn(&mut BuildEvaluationUsage)); 12] = [
        ("sponsor_schema_version", |u: &mut BuildEvaluationUsage| {
            u.sponsor_schema_version = Some(1)
        }),
        ("session_fuel_ceiling", |u| {
            u.session_fuel_ceiling = Some(u.fuel_units + u.replay_fuel_units + 1)
        }),
        ("session_build_log_byte_ceiling", |u| {
            u.session_build_log_byte_ceiling =
                Some(u.build_log_bytes + u.replay_build_log_bytes + 1)
        }),
        ("session_filesystem_attempt_ceiling", |u| {
            u.session_filesystem_attempt_ceiling =
                Some(u.filesystem_operation_attempts + u.replay_filesystem_operation_attempts + 1)
        }),
        ("session_live_filesystem_handle_ceiling", |u| {
            u.session_live_filesystem_handle_ceiling =
                Some(u.session_peak_live_filesystem_handles + 1)
        }),
        ("session_live_cell_ceiling", |u| {
            u.session_live_cell_ceiling = Some(1)
        }),
        ("session_live_text_byte_ceiling", |u| {
            u.session_live_text_byte_ceiling = Some(1)
        }),
        ("session_result_cell_ceiling", |u| {
            u.session_result_cell_ceiling = Some(1)
        }),
        ("session_result_text_byte_ceiling", |u| {
            u.session_result_text_byte_ceiling = Some(1)
        }),
        ("session_peak_live_filesystem_handles", |u| {
            u.session_peak_live_filesystem_handles = 1
        }),
        ("session_peak_live_cells", |u| u.session_peak_live_cells = 1),
        ("session_peak_live_text_bytes", |u| {
            u.session_peak_live_text_bytes = 1
        }),
    ];
    for (name, mutate) in partial_sponsor {
        let mut usage = parts.usage;
        mutate(&mut usage);
        assert!(
            mint_subject(&SubjectParts {
                usage,
                ..parts.clone()
            })
            .is_err(),
            "{name}: a partial sponsor block is unrepresentable"
        );
    }
    let sponsored = sponsored_usage(parts.usage);
    let sponsored_subject = mint_subject(&SubjectParts {
        usage: sponsored,
        ..parts.clone()
    })
    .expect("the complete sponsor block is representable");
    assert!(
        parts.usage.has_same_invocation_usage(sponsored),
        "the sponsor block leaves invocation accounting untouched"
    );
    admits("complete sponsored session block", sponsored_subject);
    // Inside the sponsored block each individual axis still substitutes.
    let sponsored_axes: [(&'static str, fn(&mut BuildEvaluationUsage)); 3] = [
        (
            "sponsored session fuel ceiling",
            |u: &mut BuildEvaluationUsage| {
                u.session_fuel_ceiling = u.session_fuel_ceiling.map(|v| v + 1)
            },
        ),
        ("sponsored session live-cell ceiling", |u| {
            u.session_live_cell_ceiling = u.session_live_cell_ceiling.map(|v| v + 1)
        }),
        ("sponsored session live-cell peak", |u| {
            u.session_peak_live_cells += 1
        }),
    ];
    for (name, mutate) in sponsored_axes {
        let mut usage = sponsored;
        mutate(&mut usage);
        let changed = mint_subject(&SubjectParts {
            usage,
            ..parts.clone()
        })
        .unwrap_or_else(|rejection| {
            panic!("sponsored usage axis {name} must be representable: {rejection}")
        });
        assert!(
            parts.usage.has_same_invocation_usage(usage),
            "{name}: the invocation join excludes sponsor axes by design"
        );
        admits(name, changed);
    }
    let torn_sponsor: [(&'static str, fn(&mut BuildEvaluationUsage)); 2] = [
        (
            "sponsored schema dropped",
            |u: &mut BuildEvaluationUsage| u.sponsor_schema_version = None,
        ),
        ("sponsored session fuel ceiling dropped", |u| {
            u.session_fuel_ceiling = None
        }),
    ];
    for (name, mutate) in torn_sponsor {
        let mut usage = sponsored;
        mutate(&mut usage);
        assert!(
            mint_subject(&SubjectParts {
                usage,
                ..parts.clone()
            })
            .is_err(),
            "{name}: a torn sponsor block is unrepresentable"
        );
    }

    // == build_observation_identity ==
    let foreign_observation =
        build_evaluation::test_support::replayable_unknown_descriptor_summary();
    assert_ne!(
        foreign_observation.identity(),
        parts.observation.identity(),
        "the foreign observation carries a distinct identity"
    );
    admits(
        "build-observation identity",
        subject_for(&|p| p.observation = foreign_observation.clone())
            .expect("a different build observation is representable"),
    );

    // == target_profile / native_target ==
    for (name, profile, native) in [
        (
            "target profile only",
            target::TargetProfile::MacosArm64,
            parts.native,
        ),
        (
            "native target only",
            parts.profile,
            target::NativeTarget::macos_arm64(),
        ),
    ] {
        assert!(
            subject_for(&|p| {
                p.profile = profile;
                p.native = native;
            })
            .is_err(),
            "{name}: a target profile and native target that disagree are unrepresentable"
        );
    }
    let native_axes: [(&'static str, fn(&mut target::NativeTarget)); 4] = [
        ("native architecture", |n: &mut target::NativeTarget| {
            n.architecture = target::Architecture::Aarch64
        }),
        ("native object format", |n: &mut target::NativeTarget| {
            n.object_format = target::ObjectFormat::MachO
        }),
        ("native pointer size", |n| n.pointer_size = 4),
        ("native pointer alignment", |n| n.pointer_alignment = 4),
    ];
    for (name, mutate) in native_axes {
        let mut native = parts.native;
        mutate(&mut native);
        assert!(
            mint_subject(&SubjectParts {
                native,
                ..parts.clone()
            })
            .is_err(),
            "{name}: a native target disagreeing with the profile is unrepresentable"
        );
    }
    admits(
        "target profile and native target together",
        subject_for(&|p| {
            p.profile = target::TargetProfile::MacosArm64;
            p.native = target::NativeTarget::macos_arm64();
        })
        .expect("a consistent foreign target pair is representable"),
    );

    // == package compilation subject ==
    admits(
        "whole package subject",
        subject_for(&|p| p.package = fixture.foreign_package.clone())
            .expect("the honest foreign package subject is representable"),
    );
    let closure = parts.package.dependency_closure().clone();
    let units = parts.package.consumed_units().to_vec();
    let commitment = parts.package.source_consumption_commitment();
    let root = parts.package.root();

    // root package identity: the subject's root and the closure's root must
    // agree, so the honest variant retargets the closure and rekeys the unit
    // rows to the substituted root.
    let foreign_root = identity(31);
    let mut rekeyed: Vec<ConsumedSourceUnit> = units
        .iter()
        .map(|unit| {
            ConsumedSourceUnit::for_test(
                unit.kind(),
                unit.package().map(|package| {
                    if package == root {
                        foreign_root
                    } else {
                        package
                    }
                }),
                unit.toolchain_namespace().map(str::to_owned),
                unit.relative_path().to_vec(),
                unit.byte_count(),
                unit.content_digest(),
            )
            .expect("rekeyed unit")
        })
        .collect();
    // The package coordinate participates in row ordering: rekeying the root
    // moves the root's rows, so the canonical order must be re-established.
    rekeyed.sort();
    let foreign_root_closure = closure_from(
        foreign_root,
        closure.root_role(),
        closure
            .packages()
            .iter()
            .map(|package| {
                if *package == root {
                    foreign_root
                } else {
                    *package
                }
            })
            .collect(),
        closure
            .dependencies()
            .iter()
            .map(|edge| {
                PackageDependencyBinding::new(
                    if edge.requester() == root {
                        foreign_root
                    } else {
                        edge.requester()
                    },
                    edge.alias(),
                    edge.target(),
                )
            })
            .collect(),
    )
    .expect("the retargeted closure is canonical");
    admits(
        "root package identity",
        mint_subject(&SubjectParts {
            package: package_subject_from(foreign_root, foreign_root_closure, commitment, rekeyed)
                .expect("the rekeyed package subject is canonical"),
            ..parts.clone()
        })
        .expect("the substituted root is representable"),
    );
    assert_eq!(
        package_subject_from(foreign_root, closure.clone(), commitment, units.clone()).unwrap_err(),
        "package compilation subject root disagrees with its dependency closure",
        "a root that disagrees with its closure is unrepresentable"
    );

    // root role
    admits(
        "root role application",
        mint_subject(&SubjectParts {
            package: package_subject_from(
                root,
                closure_from(
                    root,
                    BuildDeclarationKind::Application,
                    closure.packages().to_vec(),
                    closure.dependencies().to_vec(),
                )
                .expect("the application-role closure is canonical"),
                commitment,
                units.clone(),
            )
            .expect("the application-role package subject is canonical"),
            ..parts.clone()
        })
        .expect("the application role is representable"),
    );
    assert_eq!(
        closure_from(
            root,
            BuildDeclarationKind::Workspace,
            closure.packages().to_vec(),
            closure.dependencies().to_vec(),
        )
        .unwrap_err(),
        "package dependency closure root has workspace role",
        "a workspace root is unrepresentable"
    );

    // package roster: drop, duplicate, reorder, substitute.
    let leaf = identity(LEAF_PACKAGE);
    let without_leaf_packages: Vec<PackageKeyIdentity> = closure
        .packages()
        .iter()
        .copied()
        .filter(|package| *package != leaf)
        .collect();
    let without_leaf_edges: Vec<PackageDependencyBinding> = closure
        .dependencies()
        .iter()
        .filter(|edge| edge.requester() != leaf && edge.target() != leaf)
        .cloned()
        .collect();
    let without_leaf_units: Vec<ConsumedSourceUnit> = units
        .iter()
        .filter(|unit| unit.package() != Some(leaf))
        .cloned()
        .collect();
    admits(
        "dropped leaf package",
        mint_subject(&SubjectParts {
            package: package_subject_from(
                root,
                closure_from(
                    root,
                    closure.root_role(),
                    without_leaf_packages,
                    without_leaf_edges,
                )
                .expect("the smaller closure is canonical"),
                commitment,
                without_leaf_units,
            )
            .expect("the smaller package subject is canonical"),
            ..parts.clone()
        })
        .expect("a dropped package is representable"),
    );
    for (name, packages, dependencies) in [
        (
            "dropped dependency package",
            closure
                .packages()
                .iter()
                .copied()
                .filter(|package| *package != identity(DEP_PACKAGE))
                .collect::<Vec<_>>(),
            closure.dependencies().to_vec(),
        ),
        (
            "dropped dependency edge",
            closure.packages().to_vec(),
            closure
                .dependencies()
                .iter()
                .filter(|edge| edge.target() != leaf)
                .cloned()
                .collect::<Vec<_>>(),
        ),
        (
            "duplicated package row",
            {
                let mut packages = closure.packages().to_vec();
                packages.push(leaf);
                packages
            },
            closure.dependencies().to_vec(),
        ),
        (
            "open dependency edge",
            closure.packages().to_vec(),
            vec![PackageDependencyBinding::new(root, "dep_lib", identity(77))],
        ),
    ] {
        assert!(
            closure_from(root, closure.root_role(), packages, dependencies).is_err(),
            "{name}: the malformed closure is unrepresentable"
        );
    }
    // Strict canonical order rejects a permuted roster and permuted edges.
    let mut permuted_packages = closure.packages().to_vec();
    permuted_packages.swap(0, 1);
    assert!(
        PackageDependencyClosure::from_canonical_parts(
            root,
            closure.root_role(),
            permuted_packages,
            closure.dependencies().to_vec(),
        )
        .is_err(),
        "a reordered package roster is unrepresentable"
    );
    let mut permuted_edges = closure.dependencies().to_vec();
    permuted_edges.swap(0, 1);
    assert!(
        PackageDependencyClosure::from_canonical_parts(
            root,
            closure.root_role(),
            closure.packages().to_vec(),
            permuted_edges,
        )
        .is_err(),
        "a reordered edge roster is unrepresentable"
    );

    // dependency edges: alias, requester, target.
    for (name, edge) in [
        (
            "noncanonical dependency alias",
            PackageDependencyBinding::new(root, "Dep-Lib", identity(DEP_PACKAGE)),
        ),
        (
            "dependency cycle",
            PackageDependencyBinding::new(leaf, "dep_lib", identity(DEP_PACKAGE)),
        ),
    ] {
        let mut edges = closure.dependencies().to_vec();
        edges[0] = edge;
        assert!(
            closure_from(
                root,
                closure.root_role(),
                closure.packages().to_vec(),
                edges,
            )
            .is_err(),
            "{name}: the malformed edge is unrepresentable"
        );
    }
    let mut alias_edges = closure.dependencies().to_vec();
    alias_edges[0] = PackageDependencyBinding::new(root, "other_dep", identity(DEP_PACKAGE));
    admits(
        "dependency alias",
        mint_subject(&SubjectParts {
            package: package_subject_from(
                root,
                closure_from(
                    root,
                    closure.root_role(),
                    closure.packages().to_vec(),
                    alias_edges,
                )
                .expect("the renamed alias is canonical"),
                commitment,
                units.clone(),
            )
            .expect("the renamed-alias package subject is canonical"),
            ..parts.clone()
        })
        .expect("a renamed dependency alias is representable"),
    );

    // source-consumption commitment: the claimed commitment must equal the
    // replay over the claimed units and inputs.
    let foreign_commitment = PackageSourceConsumptionCommitment::for_test([0xEE; 32]);
    assert_ne!(
        foreign_commitment.digest(),
        commitment.digest(),
        "the forged commitment differs"
    );
    admits(
        "source-consumption commitment",
        mint_subject(&SubjectParts {
            package: package_subject_from(root, closure.clone(), foreign_commitment, units.clone())
                .expect("the foreign commitment is canonical"),
            ..parts.clone()
        })
        .expect("a foreign commitment is representable"),
    );
    admits(
        "foreign source-consumption commitment",
        mint_subject(&SubjectParts {
            package: package_subject_from(
                root,
                closure.clone(),
                fixture.foreign_package.source_consumption_commitment(),
                units.clone(),
            )
            .expect("the authentic foreign commitment is canonical"),
            ..parts.clone()
        })
        .expect("an honestly produced foreign commitment is representable"),
    );

    // consumed-source rows: every field of one authored row, then roster
    // cardinality and order.
    let authored_index = units
        .iter()
        .position(|unit| {
            unit.kind() == ConsumedSourceUnitKind::PackageAuthored
                && unit.package() == Some(root)
                && unit.relative_path().last().map(String::as_str) == Some("main.omg")
        })
        .expect("the fixture consumes the authored root main.omg row");
    let authored = units[authored_index].clone();
    let mut unit_mutations: Vec<(&'static str, Result<ConsumedSourceUnit, &'static str>)> = vec![
        (
            "unit kind",
            ConsumedSourceUnit::for_test(
                ConsumedSourceUnitKind::PackageGenerated,
                authored.package(),
                authored.toolchain_namespace().map(str::to_owned),
                authored.relative_path().to_vec(),
                authored.byte_count(),
                authored.content_digest(),
            ),
        ),
        (
            "unit package identity",
            ConsumedSourceUnit::for_test(
                authored.kind(),
                Some(identity(DEP_PACKAGE)),
                authored.toolchain_namespace().map(str::to_owned),
                authored.relative_path().to_vec(),
                authored.byte_count(),
                authored.content_digest(),
            ),
        ),
        (
            "unit relative path",
            ConsumedSourceUnit::for_test(
                authored.kind(),
                authored.package(),
                authored.toolchain_namespace().map(str::to_owned),
                vec!["renamed.omg".to_owned()],
                authored.byte_count(),
                authored.content_digest(),
            ),
        ),
        (
            "unit byte count",
            ConsumedSourceUnit::for_test(
                authored.kind(),
                authored.package(),
                authored.toolchain_namespace().map(str::to_owned),
                authored.relative_path().to_vec(),
                authored.byte_count() + 1,
                authored.content_digest(),
            ),
        ),
        (
            "unit content digest",
            ConsumedSourceUnit::for_test(
                authored.kind(),
                authored.package(),
                authored.toolchain_namespace().map(str::to_owned),
                authored.relative_path().to_vec(),
                authored.byte_count(),
                [0xAB; 32],
            ),
        ),
        (
            "unit toolchain namespace on an authored row",
            ConsumedSourceUnit::for_test(
                authored.kind(),
                authored.package(),
                Some("foreign-namespace".to_owned()),
                authored.relative_path().to_vec(),
                authored.byte_count(),
                authored.content_digest(),
            ),
        ),
        (
            "unit package outside the closure",
            ConsumedSourceUnit::for_test(
                authored.kind(),
                Some(identity(99)),
                authored.toolchain_namespace().map(str::to_owned),
                authored.relative_path().to_vec(),
                authored.byte_count(),
                authored.content_digest(),
            ),
        ),
        (
            "unit empty relative path",
            ConsumedSourceUnit::for_test(
                authored.kind(),
                authored.package(),
                authored.toolchain_namespace().map(str::to_owned),
                Vec::new(),
                authored.byte_count(),
                authored.content_digest(),
            ),
        ),
        (
            "unit empty path component",
            ConsumedSourceUnit::for_test(
                authored.kind(),
                authored.package(),
                authored.toolchain_namespace().map(str::to_owned),
                vec![String::new()],
                authored.byte_count(),
                authored.content_digest(),
            ),
        ),
    ];
    let toolchain_unit = ConsumedSourceUnit::for_test(
        ConsumedSourceUnitKind::ToolchainOwned,
        None,
        Some("omega-language-std".to_owned()),
        vec!["std.omg".to_owned()],
        7,
        [5; 32],
    )
    .expect("the toolchain row shape is canonical");
    unit_mutations.push((
        "toolchain unit namespace",
        ConsumedSourceUnit::for_test(
            toolchain_unit.kind(),
            toolchain_unit.package(),
            Some("foreign-toolchain".to_owned()),
            toolchain_unit.relative_path().to_vec(),
            toolchain_unit.byte_count(),
            toolchain_unit.content_digest(),
        ),
    ));
    unit_mutations.push((
        "toolchain unit kind virtual",
        ConsumedSourceUnit::for_test(
            ConsumedSourceUnitKind::ToolchainVirtual,
            toolchain_unit.package(),
            toolchain_unit.toolchain_namespace().map(str::to_owned),
            toolchain_unit.relative_path().to_vec(),
            toolchain_unit.byte_count(),
            toolchain_unit.content_digest(),
        ),
    ));
    for (name, changed_unit) in unit_mutations {
        match changed_unit {
            Ok(unit) => {
                let mut claimed = units.clone();
                claimed[authored_index] = unit;
                claimed.sort();
                match package_subject_from(root, closure.clone(), commitment, claimed.clone()) {
                    Ok(package) => {
                        admits(
                            name,
                            mint_subject(&SubjectParts {
                                package,
                                ..parts.clone()
                            })
                            .expect("the unit substitution is representable"),
                        );
                        assert_ne!(
                            derive_source_consumption_commitment(&claimed, &fixture.inputs)
                                .expect("the claimed units still replay to a commitment")
                                .digest(),
                            commitment.digest(),
                            "{name}: the claimed units replay to a different source-consumption commitment"
                        );
                    }
                    Err(rejection) => assert_eq!(
                        rejection,
                        "consumed source unit names a package outside the dependency closure",
                        "{name}: the closure join is the only representable row-level rejection"
                    ),
                }
            }
            Err(rejection) => {
                assert!(
                    !rejection.is_empty(),
                    "{name}: the row-level rejection names the cause"
                );
            }
        }
    }
    // roster legs: drop, duplicate, reorder, and append a toolchain row.
    let mut dropped = units.clone();
    dropped.remove(authored_index);
    admits(
        "dropped consumed-source row",
        mint_subject(&SubjectParts {
            package: package_subject_from(root, closure.clone(), commitment, dropped)
                .expect("the shorter roster is canonical"),
            ..parts.clone()
        })
        .expect("a dropped row is representable"),
    );
    assert!(
        package_subject_from(root, closure.clone(), commitment, Vec::new()).is_err(),
        "an empty consumed-source roster is unrepresentable"
    );
    let mut duplicated = units.clone();
    duplicated.push(authored.clone());
    duplicated.sort();
    assert!(
        package_subject_from(root, closure.clone(), commitment, duplicated).is_err(),
        "a duplicated consumed-source row is unrepresentable"
    );
    let mut reordered = units.clone();
    reordered.swap(0, 1);
    assert!(
        package_subject_from(root, closure.clone(), commitment, reordered).is_err(),
        "a reordered consumed-source roster is unrepresentable"
    );
    let mut appended = units.clone();
    appended.push(toolchain_unit.clone());
    appended.sort();
    admits(
        "appended toolchain row",
        mint_subject(&SubjectParts {
            package: package_subject_from(root, closure.clone(), commitment, appended)
                .expect("the extended roster is canonical"),
            ..parts.clone()
        })
        .expect("an appended row is representable"),
    );

    // == Terminal-report custody legs: the proposal target join and the
    // terminal-artifact identity join ==
    let proposal_matches = |candidate: &ProductionCompilationSubject| {
        candidate.target_profile() == parts.profile && candidate.native_target() == parts.native
    };
    for (name, changed_subject) in &variants {
        let swapped = ProductionCompilationManifest::for_test(
            changed_subject.clone(),
            ProductionArtifactIdentity::Terminal(terminal_artifact_identity),
        );
        assert!(swapped.validate(), "{name}: the honest mint validates");
        assert_ne!(
            swapped.identity(),
            authentic_terminal.identity(),
            "{name}: the substituted manifest identity diverges"
        );
        *fixture.terminal_report.production_manifest_mut_for_test() = Some(swapped);
        assert_eq!(
            fixture
                .terminal_report
                .has_consistent_executable_publication_custody(),
            proposal_matches(changed_subject),
            "{name}: Terminal-report custody replays the proposal target join"
        );
    }
    *fixture.terminal_report.production_manifest_mut_for_test() = Some(authentic_terminal.clone());
    assert!(
        fixture
            .terminal_report
            .has_consistent_executable_publication_custody()
    );

    // A manifest claiming the foreign terminal artifact is a different honest
    // record that the retained product's join rejects.
    *fixture.terminal_report.production_manifest_mut_for_test() =
        Some(fixture.foreign_manifest.clone());
    assert!(
        !fixture
            .terminal_report
            .has_consistent_executable_publication_custody()
    );
    // A manifest whose stored identity went stale never reaches the join.
    let stale = ProductionCompilationManifest::for_test_parts(
        subject.clone(),
        ProductionArtifactIdentity::Terminal(terminal_artifact_identity),
        authentic_terminal.canonical_bytes().to_vec(),
        ProductionCompilationManifestIdentity::for_test([0x5C; 32]),
    );
    assert!(!stale.validate());
    *fixture.terminal_report.production_manifest_mut_for_test() = Some(stale);
    assert!(
        !fixture
            .terminal_report
            .has_consistent_executable_publication_custody()
    );
    *fixture.terminal_report.production_manifest_mut_for_test() = Some(authentic_terminal.clone());
    assert!(
        fixture
            .terminal_report
            .has_consistent_executable_publication_custody()
    );
    // A manifest claiming the Native coordinate cannot join a Terminal report.
    let wrong_kind = ProductionCompilationManifest::for_test(
        subject.clone(),
        ProductionArtifactIdentity::Native(fixture.foreign_native.identity()),
    );
    *fixture.terminal_report.production_manifest_mut_for_test() = Some(wrong_kind);
    assert!(
        !fixture
            .terminal_report
            .has_consistent_executable_publication_custody()
    );
    *fixture.terminal_report.production_manifest_mut_for_test() = Some(authentic_terminal.clone());

    // == consume the Terminal report into the native artifact ==
    let retained = fixture
        .terminal_report
        .into_retained_terminal_artifact()
        .expect("the Terminal report retains its artifact");
    let artifact = realize_native(retained);
    let artifact_identity = artifact.identity();
    let artifact_target = artifact.target();
    let artifact_parts = artifact.into_parts();
    // The artifact type is unforgeable: replayed parts revalidate the complete
    // retained authority, so a forged physical-evidence row rejects before the
    // manifest join ever runs.
    let mut forged_evidence = replay_native_artifact_parts(&artifact_parts);
    forged_evidence.physical_evidence = None;
    assert!(
        NativeArtifact::from_replayed_parts(forged_evidence).is_err(),
        "a replayed artifact cannot drop its derived physical evidence"
    );
    let artifact =
        NativeArtifact::from_replayed_parts(replay_native_artifact_parts(&artifact_parts))
            .expect("the replayed artifact parts rejoin");
    assert_eq!(artifact.identity(), artifact_identity);
    assert_eq!(artifact.target(), artifact_target);

    let mut native_report = CompileReport::from_retained_native_artifact(
        fixture.root_path.clone(),
        fixture.source_file_count,
        NativeArtifact::from_replayed_parts(replay_native_artifact_parts(&artifact_parts))
            .expect("the report artifact copy rejoins"),
        None,
        Some(subject.clone()),
    )
    .expect("the retained native artifact assembles its production report");
    let authentic_native = native_report
        .production_manifest()
        .expect("the native report retains its production manifest")
        .clone();
    assert!(native_report.has_consistent_executable_publication_custody());
    assert!(authentic_native.validate());
    assert_eq!(
        authentic_native.artifact(),
        ProductionArtifactIdentity::Native(artifact_identity)
    );
    assert!(
        authentic_native
            .require_native_physical_evidence(&artifact)
            .is_ok(),
        "the authentic manifest borrows the artifact's exact physical evidence"
    );
    assert!(
        native_report
            .require_package_native_physical_evidence()
            .is_ok()
    );
    // A manifest-less retained-native report is a standalone production: its
    // custody holds, but it makes no package evidence claim.
    let standalone_native = CompileReport::from_retained_native_artifact(
        fixture.root_path.clone(),
        fixture.source_file_count,
        NativeArtifact::from_replayed_parts(replay_native_artifact_parts(&artifact_parts))
            .expect("the standalone artifact copy rejoins"),
        None,
        None,
    )
    .expect("the manifest-less report is consistent");
    assert_eq!(
        standalone_native.require_package_native_physical_evidence(),
        Err(FinalRealizationEvidenceError::PackageProductionManifestRequired)
    );

    // == every subject variant: stale and honestly recomputed legs ==
    for (name, changed_subject) in &variants {
        // A stale record keeps the authentic canonical bytes and identity over
        // the claimed subject: canonical replay rejects it.
        let stale = ProductionCompilationManifest::for_test_parts(
            changed_subject.clone(),
            authentic_native.artifact(),
            authentic_native.canonical_bytes().to_vec(),
            authentic_native.identity(),
        );
        assert!(
            !stale.validate(),
            "{name}: the stale containing identity fails canonical replay"
        );
        assert_eq!(
            stale
                .require_native_physical_evidence(&artifact)
                .unwrap_err(),
            FinalRealizationEvidenceError::InvalidProductionManifest,
            "{name}: the evidence join fails closed on the stale record"
        );

        // Honestly recomputed: a different self-consistent manifest.
        let honest = ProductionCompilationManifest::for_test(
            changed_subject.clone(),
            authentic_native.artifact(),
        );
        assert!(honest.validate(), "{name}: the honest mint validates");
        assert_eq!(honest.subject(), changed_subject);
        assert_ne!(
            honest.identity(),
            authentic_native.identity(),
            "{name}: the manifest identity diverges"
        );
        assert_ne!(
            honest.canonical_bytes(),
            authentic_native.canonical_bytes(),
            "{name}: the canonical bytes diverge"
        );
        let target_matches = changed_subject.native_target() == artifact_target;
        assert_eq!(
            honest.matches_native_artifact(&artifact),
            target_matches,
            "{name}: the native-artifact join replays the claimed target"
        );
        if target_matches {
            assert!(
                honest.require_native_physical_evidence(&artifact).is_ok(),
                "{name}: an artifact-consistent foreign manifest borrows the same evidence"
            );
        } else {
            assert_eq!(
                honest
                    .require_native_physical_evidence(&artifact)
                    .unwrap_err(),
                FinalRealizationEvidenceError::NativeTargetMismatch,
                "{name}: the evidence join rejects a foreign target"
            );
        }
        *native_report.production_manifest_mut_for_test() = Some(honest);
        assert_eq!(
            native_report.has_consistent_executable_publication_custody(),
            target_matches,
            "{name}: native-report custody replays the artifact join"
        );
        *native_report.production_manifest_mut_for_test() = Some(authentic_native.clone());
        // The fixture's derivation replay over the retained checked
        // projection produces `replayed_package`/`replayed_units`, so a
        // claimed package subject that differs is the substitution that
        // replay pins. Where the claimed unit roster itself moved, the
        // source-consumption commitment replayed over the same inputs cannot
        // reproduce the claimed commitment.
        if changed_subject.package() != &fixture.replayed_package
            && changed_subject.package().consumed_units() != fixture.replayed_units
        {
            let replayed_commitment = derive_source_consumption_commitment(
                changed_subject.package().consumed_units(),
                &fixture.inputs,
            );
            assert_ne!(
                replayed_commitment
                    .map(|commitment| commitment.digest())
                    .ok(),
                Some(
                    changed_subject
                        .package()
                        .source_consumption_commitment()
                        .digest()
                ),
                "{name}: the claimed unit roster replays to a different source-consumption commitment"
            );
        }
    }
    assert!(native_report.has_consistent_executable_publication_custody());
    assert!(
        native_report
            .require_package_native_physical_evidence()
            .is_ok()
    );

    // == artifact coordinate axes ==
    // artifact identity: the honestly produced foreign native artifact.
    let foreign_native_manifest =
        ProductionCompilationManifest::for_native(subject.clone(), &fixture.foreign_native)
            .expect("the foreign artifact mints its own honest manifest");
    assert!(foreign_native_manifest.validate());
    assert_ne!(
        foreign_native_manifest.identity(),
        authentic_native.identity()
    );
    assert!(!foreign_native_manifest.matches_native_artifact(&artifact));
    assert!(foreign_native_manifest.matches_native_artifact(&fixture.foreign_native));
    assert_eq!(
        foreign_native_manifest
            .require_native_physical_evidence(&artifact)
            .unwrap_err(),
        FinalRealizationEvidenceError::NativeArtifactMismatch,
    );
    *native_report.production_manifest_mut_for_test() = Some(foreign_native_manifest);
    assert!(!native_report.has_consistent_executable_publication_custody());
    *native_report.production_manifest_mut_for_test() = Some(authentic_native.clone());

    // artifact kind: the same subject claiming a Terminal coordinate.
    for (name, coordinate) in [
        (
            "authentic terminal artifact",
            ProductionArtifactIdentity::Terminal(terminal_artifact_identity),
        ),
        (
            "foreign terminal artifact",
            ProductionArtifactIdentity::Terminal(fixture.foreign_terminal_identity),
        ),
    ] {
        let kind_substituted = ProductionCompilationManifest::for_test(subject.clone(), coordinate);
        assert!(
            kind_substituted.validate(),
            "{name}: the honest mint validates"
        );
        assert!(
            !kind_substituted.matches_native_artifact(&artifact),
            "{name}: a Terminal coordinate never joins a native artifact"
        );
        assert_eq!(
            kind_substituted
                .require_native_physical_evidence(&artifact)
                .unwrap_err(),
            FinalRealizationEvidenceError::NativeArtifactMismatch,
            "{name}: the evidence join rejects a kind substitution"
        );
        *native_report.production_manifest_mut_for_test() = Some(kind_substituted);
        assert!(
            !native_report.has_consistent_executable_publication_custody(),
            "{name}: native-report custody rejects a kind substitution"
        );
    }
    *native_report.production_manifest_mut_for_test() = Some(authentic_native.clone());

    // target mismatch: a consistent foreign-target subject cannot mint through
    // for_native, and the minted-via-for_test record fails the artifact join.
    let foreign_target_subject = subject_for(&|p| {
        p.profile = target::TargetProfile::MacosArm64;
        p.native = target::NativeTarget::macos_arm64();
    })
    .expect("the consistent foreign target pair is representable");
    assert_eq!(
        ProductionCompilationManifest::for_native(foreign_target_subject.clone(), &artifact)
            .unwrap_err(),
        "production manifest subject target disagrees with native artifact",
        "the mint refuses a subject/artifact target disagreement"
    );
    let target_drifted = ProductionCompilationManifest::for_test(
        foreign_target_subject,
        ProductionArtifactIdentity::Native(artifact_identity),
    );
    assert!(target_drifted.validate());
    assert!(!target_drifted.matches_native_artifact(&artifact));
    assert_eq!(
        target_drifted
            .require_native_physical_evidence(&artifact)
            .unwrap_err(),
        FinalRealizationEvidenceError::NativeTargetMismatch,
    );
    *native_report.production_manifest_mut_for_test() = Some(target_drifted);
    assert!(!native_report.has_consistent_executable_publication_custody());
    *native_report.production_manifest_mut_for_test() = Some(authentic_native.clone());

    // == stored canonical bytes and identity ==
    for (name, mutated_bytes) in [
        ("truncated", {
            let mut bytes = authentic_native.canonical_bytes().to_vec();
            bytes.pop();
            bytes
        }),
        ("appended", {
            let mut bytes = authentic_native.canonical_bytes().to_vec();
            bytes.push(0);
            bytes
        }),
        ("flipped", {
            let mut bytes = authentic_native.canonical_bytes().to_vec();
            let last = bytes.len() - 1;
            bytes[last] ^= 0xFF;
            bytes
        }),
        ("empty", Vec::new()),
    ] {
        let forged = ProductionCompilationManifest::for_test_parts(
            subject.clone(),
            authentic_native.artifact(),
            mutated_bytes,
            authentic_native.identity(),
        );
        assert!(
            !forged.validate(),
            "{name}: substituted canonical bytes fail canonical replay"
        );
        assert_eq!(
            forged
                .require_native_physical_evidence(&artifact)
                .unwrap_err(),
            FinalRealizationEvidenceError::InvalidProductionManifest,
            "{name}: the evidence join fails closed"
        );
    }
    for (name, claimed_identity) in [
        (
            "forged",
            ProductionCompilationManifestIdentity::for_test([0x77; 32]),
        ),
        ("foreign", fixture.foreign_manifest.identity()),
    ] {
        let forged = ProductionCompilationManifest::for_test_parts(
            subject.clone(),
            authentic_native.artifact(),
            authentic_native.canonical_bytes().to_vec(),
            claimed_identity,
        );
        assert!(
            !forged.validate(),
            "{name}: a substituted containing identity fails canonical replay"
        );
        *native_report.production_manifest_mut_for_test() = Some(forged);
        assert!(
            !native_report.has_consistent_executable_publication_custody(),
            "{name}: report custody rejects the stale record"
        );
        *native_report.production_manifest_mut_for_test() = Some(authentic_native.clone());
    }
    assert!(native_report.has_consistent_executable_publication_custody());

    // The authentic Terminal manifest never joins the native artifact, and
    // the authentic native manifest never joins the Terminal artifact: the
    // artifact kind is part of the coordinate. The artifact's retained
    // canonical Terminal artifact is the same authentic artifact the
    // Terminal manifest joined.
    assert!(!authentic_terminal.matches_native_artifact(&artifact));
    assert!(authentic_terminal.matches_terminal_artifact(artifact.psi_artifact()));
    assert!(!authentic_native.matches_terminal_artifact(artifact.psi_artifact()));
    assert!(
        !authentic_native.matches_terminal_artifact(fixture.foreign_native.psi_artifact()),
        "a foreign Terminal artifact identity never joins the native manifest"
    );

    // == published executable custody ==
    let published = native_report
        .publish_retained_native_artifact(&fixture.tree.0.join("published"))
        .expect("the validated retained native product publishes");
    assert_eq!(published.output_kind(), CompileOutputKind::NativeExecutable);
    assert!(published.has_consistent_executable_publication_custody());
    assert!(
        published.checked_native_executable_path().is_some(),
        "the published report exposes its checked executable path"
    );
    let mut published = published;
    // The executable join binds the receipt's retained artifact identity: a
    // Terminal-kind manifest, a foreign-artifact manifest, or a stale record
    // all reject, while a subject substitution over the same artifact is a
    // different self-consistent record the join admits.
    *published.production_manifest_mut_for_test() = Some(authentic_terminal.clone());
    assert!(!published.has_consistent_executable_publication_custody());
    *published.production_manifest_mut_for_test() = Some(fixture.foreign_manifest.clone());
    assert!(!published.has_consistent_executable_publication_custody());
    let foreign_native_manifest =
        ProductionCompilationManifest::for_native(subject.clone(), &fixture.foreign_native)
            .expect("the foreign artifact mints");
    *published.production_manifest_mut_for_test() = Some(foreign_native_manifest);
    assert!(!published.has_consistent_executable_publication_custody());
    let stale = ProductionCompilationManifest::for_test_parts(
        subject.clone(),
        ProductionArtifactIdentity::Native(artifact_identity),
        authentic_native.canonical_bytes().to_vec(),
        ProductionCompilationManifestIdentity::for_test([0x9D; 32]),
    );
    *published.production_manifest_mut_for_test() = Some(stale);
    assert!(!published.has_consistent_executable_publication_custody());
    let subject_substituted = ProductionCompilationManifest::for_native(
        subject_for(&|p| p.machine = "published-foreign-machine".to_owned())
            .expect("the machine substitution is representable"),
        &artifact,
    )
    .expect("the subject substitution mints over the same artifact");
    *published.production_manifest_mut_for_test() = Some(subject_substituted);
    assert!(
        published.has_consistent_executable_publication_custody(),
        "a subject substitution over the published artifact is a different self-consistent record"
    );
    *published.production_manifest_mut_for_test() = Some(authentic_native.clone());
    assert!(published.has_consistent_executable_publication_custody());
}
