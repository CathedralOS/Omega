//! Fixture acceptance layered on ordinary executed package discovery.
//!
//! The application Build coordinator resolves authored declarations and dependencies,
//! retains their completed Builds, and prepares every requested Product target.
//! This harness adds repository test consent to those actual package identities;
//! it neither parses a second dependency graph nor replays Build for checking.
//!
//! Bundled-library entry and dangerous-service bindings are explicit test policy,
//! not production review. `accepted_entry_supplier.txt` selects a declared Product
//! dependency's target contract instead of the bundled default. Explicit assumed
//! contracts require `accepted_assumed_bindings.txt` and exact checked policy rows.

// Shared across test targets through `#[path]`; each target uses the subset
// its fixtures need.
#![allow(dead_code)]

use build_declarations::DependencyPurpose;
use compiler::CheckedCompileRequest;
use diagnostics::Diagnostic;
use package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, PackageCompilationInputs,
    PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use target::TargetProfile;

#[path = "console_acceptance.rs"]
pub mod console_acceptance;
#[path = "dangerous_service_acceptance.rs"]
pub mod dangerous_service_acceptance;
#[path = "process_exit_acceptance.rs"]
pub mod process_exit_acceptance;
#[path = "sealed_fixture_staging.rs"]
pub mod sealed_fixture_staging;

pub fn repo_root() -> PathBuf {
    if let Some(root) = std::env::var_os("OMEGA_CORPUS_ROOT") {
        return PathBuf::from(root);
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compilation/driver")
        .to_path_buf()
}

pub fn fixture_package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32])
        .expect("repository fixture package identity is nonzero")
}

/// The bundled standard library's root.
///
/// Its exact local-source origin identifies the default entry supplier; a package
/// with the same authored name elsewhere receives no implicit fixture consent.
pub fn bundled_standard_library_root() -> PathBuf {
    repo_root().join("source/library/std")
}

/// The reviewed `ProgramEntry` candidate the standard library at
/// `standard_library_root` owns for `target_name`.
///
/// Entry acceptance is decided by the target profile, not by spelling: every
/// hosted profile with a reviewed ProgramEntry candidate binds it here, and a
/// profile without one (macOS x86-64, the cross-platform, unchecked and
/// bootstrap profiles) is `None` rather than falling through a chain of
/// string comparisons. An unknown target name is the vocabulary's diagnostic.
pub fn candidate_program_entry_binding(
    target_name: &str,
    standard_library_root: &Path,
    standard_library: PackageKeyIdentity,
) -> Result<Option<AcceptedSemanticBinding>, Vec<Diagnostic>> {
    // The candidate depends only on these three inputs, and each derivation
    // checks the whole standard library entry: a test that compiles many
    // fixtures (the sample run oracle) would otherwise repeat that check for
    // every one. Compute outside the lock; a racing duplicate is only waste.
    type Key = (String, std::path::PathBuf, PackageKeyIdentity);
    type Candidate = Result<Option<AcceptedSemanticBinding>, Vec<Diagnostic>>;
    static CANDIDATES: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<Key, Candidate>>,
    > = std::sync::OnceLock::new();
    let candidates = CANDIDATES.get_or_init(Default::default);
    let key = (
        target_name.to_owned(),
        standard_library_root.to_path_buf(),
        standard_library,
    );
    if let Some(candidate) = candidates
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&key)
    {
        return candidate.clone();
    }
    let candidate =
        derive_program_entry_binding(target_name, standard_library_root, standard_library);
    candidates
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(key, candidate.clone());
    candidate
}

fn derive_program_entry_binding(
    target_name: &str,
    standard_library_root: &Path,
    standard_library: PackageKeyIdentity,
) -> Result<Option<AcceptedSemanticBinding>, Vec<Diagnostic>> {
    let inputs = PackageCompilationInputs::new_package(
        standard_library,
        vec![PackageSourceBinding::new(
            standard_library,
            "omega_language_std",
            standard_library_root.to_path_buf(),
        )],
        Vec::new(),
    )
    .map_err(|errors| {
        vec![Diagnostic::error(format!(
            "entry fixture inputs: {errors:?}"
        ))]
    })?;
    derive_entry_from_inputs(target_name, inputs)
}

fn derive_entry_from_inputs(
    target_name: &str,
    inputs: PackageCompilationInputs,
) -> Result<Option<AcceptedSemanticBinding>, Vec<Diagnostic>> {
    let supplier = inputs.root();
    let supplier_root = inputs
        .package_root(supplier)
        .expect("entry supplier source")
        .to_path_buf();
    let profile = TargetProfile::from_canonical_target_name(target_name)
        .map_err(|diagnostic| vec![diagnostic])?;
    let (target_name, role, schema) = match profile {
        TargetProfile::MacosArm64 => (
            "macos_arm64",
            AcceptedSemanticBindingRole::MacosArm64ProgramEntry,
            "MacosApplication",
        ),
        TargetProfile::LinuxX64 => (
            "linux_x86_64",
            AcceptedSemanticBindingRole::LinuxX86_64ProgramEntry,
            "LinuxX86_64Application",
        ),
        TargetProfile::LinuxArm64 => (
            "linux_arm64",
            AcceptedSemanticBindingRole::LinuxArm64ProgramEntry,
            "LinuxArm64Application",
        ),
        TargetProfile::WindowsX64 => (
            "windows_x86_64",
            AcceptedSemanticBindingRole::WindowsX64ProgramEntry,
            "WindowsX86_64Application",
        ),
        TargetProfile::UefiX64 => (
            "uefi_x86_64",
            AcceptedSemanticBindingRole::UefiX64ProgramEntry,
            "UefiApplication",
        ),
        TargetProfile::MacosX64 => (
            "macos_x86_64",
            AcceptedSemanticBindingRole::MacosX64ProgramEntry,
            "MacosX64Application",
        ),
        TargetProfile::CrossPlatformCli
        | TargetProfile::LocalUnchecked
        | TargetProfile::AlphaBootstrap => return Ok(None),
    };
    // Check the package-owned target contract independently on every derivation;
    // application discovery alone cannot authorize the program-entry role.
    let checked = omega_build::check_source(
        &mut compiler::CompilationSession::default(),
        CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(
                &supplier_root.join(format!("targets/{target_name}/entry.omg")),
                Some(target_name),
            )
        },
    )
    .result
    .map_err(|failure| failure.into_diagnostics())?;
    checked
        .execution()
        .candidate_service_binding(
            &checked.providers().boundary_calling_plan_realizations,
            role,
            supplier,
            schema,
        )
        .map(Some)
        .map_err(|diagnostic| vec![diagnostic])
}

/// One fixture's retained execution and target-specific Product plans. Acquisition
/// custody stays alive alongside the compiler until all consumers finish.
pub struct ReviewedFixtureCompilation {
    pub root_path: PathBuf,
    pub package_inputs: Option<PackageCompilationInputs>,
    pub execution: Option<package_compilation::BuildExecutionInputs>,
    pub session: omega_build::BuildSession,
    pub completed: Vec<compiler::CompletedBuild>,
    product: compiler::CompilationProduct,
    plans: Vec<(TargetProfile, compiler::PackageCompilationRequest)>,
    completion: Option<package_manager::operations::ProjectCompilationCompletion>,
}

/// Consume the fixture's explicit consent through the same package-product
/// entrance as accepted-lock compilation. The caller may batch these requests
/// for a target cohort; each target carries its own admitted rows.
pub fn accepted_fixture_product_request(
    prepared: &mut ReviewedFixtureCompilation,
    target: Option<&str>,
) -> Result<compiler::PackageCompilationRequest, Vec<Diagnostic>> {
    let profile = TargetProfile::from_omega_target_name(target).map_err(|error| vec![error])?;
    let position = prepared
        .plans
        .iter()
        .position(|(target, _)| *target == profile)
        .ok_or_else(|| vec![Diagnostic::error("fixture Product target was not prepared")])?;
    let (_, request) = prepared.plans.remove(position);
    Ok(request)
}

/// Compile the complete admitted target cohort, then let package management
/// reconcile its ordered outcomes and close staging before products escape.
pub fn produce_fixture_products(
    prepared: &mut ReviewedFixtureCompilation,
    requests: Vec<compiler::PackageCompilationRequest>,
) -> Result<Vec<(TargetProfile, compiler::CompilationOutput)>, Vec<Diagnostic>> {
    let completion = prepared.completion.take().ok_or_else(|| {
        vec![Diagnostic::error(
            "fixture Product completion already consumed",
        )]
    })?;
    let outputs = omega_build::compile_packages(
        prepared.session.compiler(),
        requests,
        &mut prepared.completed,
    );
    let outcomes = completion.complete(outputs, prepared.session.resources());
    let mut reports = Vec::new();
    let mut failures = Vec::new();
    for outcome in outcomes {
        match outcome.result {
            Ok(output) => reports.push((outcome.target, output)),
            Err(package_manager::operations::ProjectCompilationError::Review(
                package_manager::review::CompileResolvedPackageReviewsError::Compilation {
                    diagnostics,
                    ..
                },
            )) => failures.extend(diagnostics),
            Err(error) => failures.push(Diagnostic::error(error.to_string())),
        }
    }
    if failures.is_empty() {
        Ok(reports)
    } else {
        Err(failures)
    }
}

/// Execute one admitted Product, whether its source was a package or a loose
/// file. Pending execution inputs belong only to the loose-file path.
pub fn produce_fixture(
    prepared: &mut ReviewedFixtureCompilation,
    target: Option<&str>,
) -> Result<compiler::CompilationOutput, Vec<Diagnostic>> {
    if prepared.package_inputs.is_some() {
        let request = accepted_fixture_product_request(prepared, target)?;
        let mut products = produce_fixture_products(prepared, vec![request])?;
        return products.pop().map(|(_, product)| product).ok_or_else(|| {
            vec![Diagnostic::error(
                "fixture Product completion lost its output",
            )]
        });
    }
    let mut request = CheckedCompileRequest::new(&prepared.root_path, target);
    request.build = prepared.execution.take().ok_or_else(|| {
        vec![Diagnostic::error(
            "standalone fixture execution already consumed",
        )]
    })?;
    request.optimization_rollback = prepared.product.optimization_rollback();
    omega_build::produce_source(
        prepared.session.compiler(),
        request,
        prepared.product.clone(),
    )
    .map_err(compiler::CompilationProductError::into_diagnostics)
}

/// Generic authored host-input fixtures: each immediate directory under
/// `build-inputs/` is one named inventory. Read the already sealed fixture
/// staging, not an ambient host path selected by Omega code. This is request
/// setup in the harness, never a shipped CLI fixture recognizer.
pub fn fixture_build_snapshot(
    root_path: &Path,
) -> Result<Option<package_compilation::BuildSnapshotRequest>, Vec<Diagnostic>> {
    let Some(root) = root_path.parent() else {
        return Ok(None);
    };
    let input_root = root.join("build-inputs");
    let listing = match fs::read_dir(&input_root) {
        Ok(listing) => listing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(vec![Diagnostic::error(format!(
                "cannot list fixture build inputs: {error}"
            ))]);
        }
    };
    let mut inputs = std::collections::BTreeMap::new();
    for entry in listing {
        let entry = entry.map_err(|error| {
            vec![Diagnostic::error(format!(
                "cannot read fixture input slot: {error}"
            ))]
        })?;
        if !entry
            .file_type()
            .map_err(|error| vec![Diagnostic::error(error.to_string())])?
            .is_dir()
        {
            return Err(vec![Diagnostic::error(
                "fixture build input slots must be concrete directories",
            )]);
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| {
                vec![Diagnostic::error(
                    "fixture input slot must have a UTF-8 name",
                )]
            })?
            .into_bytes();
        let source = entry.path();
        let source = if source.starts_with(sealed_fixture_staging::staging_session_root()) {
            source
        } else {
            sealed_fixture_staging::stage_sealed_snapshot(&repo_root(), &source)
        };
        let captured =
            package_compilation::capture_package_source_input(&source).map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "cannot capture fixture input slot: {reason}"
                ))]
            })?;
        inputs.insert(name, captured);
    }
    package_compilation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>())
        .with_inputs(inputs)
        .map(Some)
}

/// The authored package graph plus this repository's test acceptance: the
/// bundled library's target entry candidate when present, then the services the
/// preliminary checked graph requires, including root-owned services. The
/// package inputs are `None` for a fixture that stays standalone; its source
/// is staged either way, so one route compiles every fixture.
pub fn reviewed_repository_fixture_package_inputs(
    authored_root_path: &Path,
    target_name: Option<&str>,
) -> Result<ReviewedFixtureCompilation, Vec<Diagnostic>> {
    let profile =
        TargetProfile::from_omega_target_name(target_name).map_err(|error| vec![error])?;
    let discovered = discover_fixture(
        authored_root_path,
        package_compilation::BuildExecutionInputs {
            snapshot: fixture_build_snapshot(authored_root_path)?,
            ..Default::default()
        },
    )?;
    review_fixture(discovered, &[profile], compiler::CompilationProduct::Check)
}

/// Executed source discovery, before the caller chooses Product targets.
/// Package custody and settled activations stay together across that choice.
pub struct DiscoveredFixture {
    pub authored_root_path: PathBuf,
    root_path: PathBuf,
    prepared: Option<package_manager::operations::PreparedLocalProject>,
    execution: Option<package_compilation::BuildExecutionInputs>,
    snapshot: Option<package_compilation::BuildSnapshotRequest>,
    execution_profile: TargetProfile,
    session: omega_build::BuildSession,
    completed: Vec<compiler::CompletedBuild>,
    pub targets: Vec<String>,
}

pub fn discover_fixture(
    authored_root_path: &Path,
    execution: package_compilation::BuildExecutionInputs,
) -> Result<DiscoveredFixture, Vec<Diagnostic>> {
    let authored_project_root = authored_root_path.parent().expect("fixture project root");
    let directory =
        sealed_fixture_staging::fresh_staging_build_dir(&repo_root(), authored_project_root);
    let mut session = omega_build::BuildSession::create(&directory)
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    let preparation = package_manager::operations::begin_local_project_preparation(
        authored_root_path,
        &TargetProfile::ALL,
        true,
    )
    .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    let Some(preparation) = preparation else {
        let staged =
            sealed_fixture_staging::stage_sealed_snapshot(&repo_root(), authored_project_root);
        return Ok(DiscoveredFixture {
            authored_root_path: authored_root_path.to_path_buf(),
            root_path: staged.join(authored_root_path.file_name().expect("fixture source name")),
            prepared: None,
            execution: Some(execution),
            snapshot: None,
            execution_profile: TargetProfile::from_omega_target_name(None)
                .map_err(|error| vec![error])?,
            session,
            completed: Vec::new(),
            targets: Vec::new(),
        });
    };
    let snapshot = execution.snapshot.clone();
    let resolved = omega_build::resolve_project_sources(
        &mut session,
        omega_build::ProjectSourceRequest {
            preparation,
            execution,
            collect_timings: false,
        },
    )
    .map_err(|error| match error {
        omega_build::ProjectSourceResolutionError::Compilation(error) => error.into_diagnostics(),
        other => vec![Diagnostic::error(other.to_string())],
    })?;
    let root_path = resolved.prepared.entry_path().to_path_buf();
    let mut completed = Vec::with_capacity(resolved.builds.len());
    for build in resolved.builds {
        let inputs = build.package_inputs().clone();
        completed.push(
            build
                .settle(session.compiler(), resolved.execution_profile, inputs)
                .map_err(|error| error.into_diagnostics())?,
        );
    }
    // The source graph identifies its root independently of completion order.
    let root = completed
        .iter()
        .find(|build| build.package_inputs().root() == resolved.prepared.root_package())
        .ok_or_else(|| vec![Diagnostic::error("fixture discovery lost its root Build")])?;
    let targets = root
        .bound_program_targets()?
        .into_iter()
        .map(|profile| profile.target_name().to_owned())
        .collect();
    Ok(DiscoveredFixture {
        authored_root_path: authored_root_path.to_path_buf(),
        root_path,
        prepared: Some(resolved.prepared),
        execution: None,
        snapshot,
        execution_profile: resolved.execution_profile,
        session,
        completed,
        targets,
    })
}

/// Admit the chosen products only after Build returns. Native requests enter
/// native admission here; they are never upgraded from a checking permission.
pub fn review_fixture(
    discovered: DiscoveredFixture,
    profiles: &[TargetProfile],
    product: compiler::CompilationProduct,
) -> Result<ReviewedFixtureCompilation, Vec<Diagnostic>> {
    let DiscoveredFixture {
        authored_root_path,
        root_path,
        prepared,
        execution,
        snapshot,
        execution_profile,
        mut session,
        mut completed,
        ..
    } = discovered;
    let Some(prepared) = prepared else {
        return Ok(ReviewedFixtureCompilation {
            root_path,
            package_inputs: None,
            execution,
            session,
            completed,
            product,
            plans: Vec::new(),
            completion: None,
        });
    };
    let authored_project_root = authored_root_path.parent().expect("fixture project root");
    let standard_library_root = bundled_standard_library_root()
        .canonicalize()
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    let standard_library = prepared.package_at_local_root(&standard_library_root);
    let admitted = package_manager::operations::admit_project_products(
        package_manager::operations::ProjectCompilationAdmissionRequest {
            prepared,
            target_profiles: profiles,
            snapshot: snapshot.as_ref(),
            verify: false,
            timings: false,
            product: product.clone(),
        },
        session.resources(),
        execution_profile,
    );
    let failures = admitted
        .completion
        .failures()
        .map(|(target, error)| {
            Diagnostic::error(format!(
                "fixture {} admission: {error}",
                target.target_name()
            ))
        })
        .collect::<Vec<_>>();
    if !failures.is_empty() {
        return Err(failures);
    }
    let mut plans = Vec::with_capacity(admitted.requests.len());
    for mut request in admitted.requests {
        let Some(compiler::PackageSourceRequest::Product(root)) = request.occurrences.last() else {
            return Err(vec![Diagnostic::error(
                "fixture admission lost its retained Product root",
            )]);
        };
        let profile = root.target;
        let mut inputs = root.inputs.clone();
        let mut bindings = Vec::new();
        let suppliers = accepted_entry_suppliers(authored_project_root);
        for supplier in &suppliers {
            bindings.push(retained_supplier_entry_binding(
                profile.target_name(),
                supplier,
                &inputs,
                &completed,
            )?);
        }
        if suppliers.is_empty()
            && let Some(owner) = standard_library
        {
            bindings.extend(candidate_program_entry_binding(
                profile.target_name(),
                &standard_library_root,
                owner,
            )?);
        }
        inputs = inputs
            .with_accepted_semantic_bindings(bindings.clone())
            .map_err(|errors| {
                vec![Diagnostic::error(format!(
                    "entry fixture acceptance: {errors:?}"
                ))]
            })?;
        let mut discovery = clone_fixture_occurrences(&request.occurrences)?;
        let Some(compiler::PackageSourceRequest::Product(root)) = discovery.last_mut() else {
            return Err(vec![Diagnostic::error(
                "fixture discovery lost its Product root",
            )]);
        };
        root.inputs = inputs.clone().with_semantic_role_discovery(true);
        root.options.permit_unsettled_fused_service_fields = true;
        let build = omega_build::prepare_package(session.compiler(), discovery, &mut completed)
            .map_err(|error| error.failure.into_diagnostics())?;
        let preliminary = session
            .compiler()
            .check_package_build(build)
            .map_err(|error| error.into_diagnostics())?;
        for (owner, _) in inputs.packages() {
            let required =
                dangerous_service_acceptance::required_dangerous_services(&preliminary, owner);
            bindings.extend(dangerous_service_bindings(&preliminary, owner, &required)?);
        }
        let Some(compiler::PackageSourceRequest::Product(root)) = request.occurrences.last_mut()
        else {
            return Err(vec![Diagnostic::error(
                "fixture acceptance lost its Product root",
            )]);
        };
        root.inputs = inputs
            .with_accepted_semantic_bindings(bindings)
            .map_err(|errors| {
                vec![Diagnostic::error(format!(
                    "fixture semantic acceptance: {errors:?}"
                ))]
            })?;
        request.terminal_permissions =
            assumed_fixture_permissions(authored_project_root, profile, &preliminary)?;
        plans.push((profile, request));
    }
    let first = plans
        .first()
        .ok_or_else(|| vec![Diagnostic::error("fixture has no admitted target")])?;
    let package_inputs = first
        .1
        .occurrences
        .last()
        .and_then(compiler::PackageSourceRequest::package_inputs)
        .cloned();
    Ok(ReviewedFixtureCompilation {
        root_path,
        package_inputs,
        execution: None,
        session,
        completed,
        product,
        plans,
        completion: Some(admitted.completion),
    })
}

fn clone_fixture_occurrences(
    occurrences: &[compiler::PackageSourceRequest],
) -> Result<Vec<compiler::PackageSourceRequest>, Vec<Diagnostic>> {
    occurrences
        .iter()
        .map(|occurrence| match occurrence {
            compiler::PackageSourceRequest::Product(request) => {
                Ok(compiler::PackageSourceRequest::Product(request.clone()))
            }
            compiler::PackageSourceRequest::Execute(_) => Err(vec![Diagnostic::error(
                "resolved fixture Product admission requested another Build execution",
            )]),
        })
        .collect()
}

fn assumed_fixture_permissions(
    authored_project_root: &Path,
    profile: TargetProfile,
    preliminary: &compiler::CheckedCompilation,
) -> Result<Option<compiler::TerminalPermissionAcceptance>, Vec<Diagnostic>> {
    let consent = authored_project_root.join("accepted_assumed_bindings.txt");
    let permissions = if consent.is_file() {
        let text = fs::read_to_string(&consent)
            .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        if text.trim() != "Accept this fixture's exact checked root assumed binding contract rows."
        {
            return Err(vec![Diagnostic::error(
                "invalid explicit assumed-binding fixture consent",
            )]);
        }
        let package = preliminary.sources().package_identity().ok_or_else(|| {
            vec![Diagnostic::error(
                "assumed-binding fixture consent requires a root package",
            )]
        })?;
        let policy =
            package_evidence::project_checked_package_policy(preliminary, profile, package)?;
        let mut rows: Vec<_> =
            policy
                .acceptance_rows_with_limits(
                    package_evidence::record::PackagePolicyRowLimits::default(),
                )
                .map_err(|error| vec![Diagnostic::error(error.to_string())])?
                .0
                .into_iter()
                .filter(|row| {
                    matches!(
                        row.kind(),
                        package_evidence::record::PackagePolicyRowKind::AssumedBindingContract
                            | package_evidence::record::PackagePolicyRowKind::TerminalPermission
                    )
                })
                .map(|row| {
                    (
                        optimization_core::digest_encoder::content_identity(row.key_bytes()),
                        row.into_canonical_text(),
                    )
                })
                .collect();
        if preliminary
            .execution()
            .selected_provider_plan_facts
            .assumed_bindings()
            .is_empty()
        {
            return Err(vec![Diagnostic::error(
                "assumed-binding fixture consent names no assumed contracts",
            )]);
        }
        rows.sort_unstable_by_key(|(key, _)| *key);
        Some(
            package_evidence::permission_acceptance::TerminalPermissionAcceptance {
                schema_version: package_evidence::record::PACKAGE_POLICY_ROW_VERSION,
                package,
                target: profile,
                rows,
            },
        )
    } else {
        None
    };

    Ok(permissions)
}

/// The explicit source-tree oracle consumes the same retained Product plan.
/// It does not accept another execution request after Build has returned.
pub fn check_fixture(
    prepared: &mut ReviewedFixtureCompilation,
    target: &str,
    rollback: compiler::OptimizationRollback,
) -> Result<compiler::CheckedCompilation, Vec<Diagnostic>> {
    if prepared.package_inputs.is_some() {
        let profile =
            TargetProfile::from_canonical_target_name(target).map_err(|error| vec![error])?;
        let (_, plan) = prepared
            .plans
            .iter()
            .find(|(candidate, _)| *candidate == profile)
            .ok_or_else(|| vec![Diagnostic::error("fixture Product target was not prepared")])?;
        let mut occurrences = clone_fixture_occurrences(&plan.occurrences)?;
        for occurrence in &mut occurrences {
            occurrence.set_optimization_rollback(rollback.clone());
        }
        let build = omega_build::prepare_package(
            prepared.session.compiler(),
            occurrences,
            &mut prepared.completed,
        )
        .map_err(|error| error.failure.into_diagnostics())?;
        prepared
            .session
            .compiler()
            .check_package_build(build)
            .map_err(|error| error.into_diagnostics())
    } else {
        let mut request = CheckedCompileRequest::new(&prepared.root_path, Some(target));
        request.build = prepared.execution.take().ok_or_else(|| {
            vec![Diagnostic::error(
                "standalone fixture execution already consumed",
            )]
        })?;
        request.optimization_rollback = rollback;
        omega_build::check_source(prepared.session.compiler(), request)
            .result
            .map_err(|error| error.into_diagnostics())
    }
}

fn retained_supplier_entry_binding(
    target: &str,
    supplier_name: &str,
    root: &PackageCompilationInputs,
    completed: &[compiler::CompletedBuild],
) -> Result<AcceptedSemanticBinding, Vec<Diagnostic>> {
    let supplier = root
        .direct_dependencies(DependencyPurpose::Product)
        .find(|(alias, identity)| {
            *alias == supplier_name || root.package_name(*identity) == Some(supplier_name)
        })
        .map(|(_, identity)| identity)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "accepted entry supplier `{supplier_name}` is not a declared Product dependency"
            ))]
        })?;
    let inputs = completed
        .iter()
        .find(|build| build.package_inputs().root() == supplier)
        .ok_or_else(|| vec![Diagnostic::error("entry supplier has no settled source")])?
        .package_inputs()
        .clone();
    derive_entry_from_inputs(target, inputs)?.ok_or_else(|| {
        vec![Diagnostic::error(
            "entry supplier offers no target contract",
        )]
    })
}

/// The package names a fixture's `accepted_entry_supplier.txt` declares, one
/// per line; blank lines and `//` comments are inert. The file is optional:
/// absent means the bundled library's reviewed candidate is the accepted one.
fn accepted_entry_suppliers(authored_project_root: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(authored_project_root.join("accepted_entry_supplier.txt"))
    else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .map(str::to_owned)
        .collect()
}

/// The dangerous-service rows the preliminary checked graph admits, every one
/// derived from and replayed against that graph.
fn dangerous_service_bindings(
    preliminary: &compiler::CheckedCompilation,
    owner: PackageKeyIdentity,
    required: &dangerous_service_acceptance::RequiredDangerousServices,
) -> Result<Vec<AcceptedSemanticBinding>, Vec<Diagnostic>> {
    let mut bindings = Vec::new();
    if required.filesystem {
        bindings.push(
            preliminary
                .execution()
                .candidate_service_binding(
                    &preliminary.providers().boundary_calling_plan_realizations,
                    AcceptedSemanticBindingRole::FilesystemHostService,
                    owner,
                    "FilesystemHost",
                )
                .map_err(|diagnostic| vec![diagnostic])?,
        );
    }
    if required.time {
        bindings.push(
            preliminary
                .execution()
                .candidate_service_binding(
                    &preliminary.providers().boundary_calling_plan_realizations,
                    AcceptedSemanticBindingRole::TimeHostService,
                    owner,
                    "TimeHost",
                )
                .map_err(|diagnostic| vec![diagnostic])?,
        );
    }
    if let Some(console) = required.console {
        bindings.push(console_acceptance::candidate_console_exit_binding(
            preliminary,
            owner,
            console.output,
            console.input,
        )?);
    }
    if required.process_exit {
        bindings.push(process_exit_acceptance::candidate_process_exit_binding(
            preliminary,
            owner,
        )?);
    }
    Ok(bindings)
}
