//! Review the unchanged Windows program under two independent receiver inputs.
//!
//! Ordinary CLI production claims no receiver admission. This test supplies
//! one explicitly, without treating package acceptance or abstract service
//! reach as permission. It deliberately grants all six filesystem classes to
//! each raw requirement owned by the reviewed bundled std package; these are
//! broad filesystem grants, not minimal per-operation permissions.
//! Generic ordinary close gets an empty grant, while namespace-removal close
//! keeps the filesystem grant. The structural process-exit intrinsic and the
//! canonical ProcessExit requirement get termination permission.
//! The positive also grants ProcessOutput to the original trusted
//! WindowsFilesystemTransferIo::write_file requirement, because generic
//! WriteFile can write a console. The original filesystem-only table stays
//! a refusal control. The existing toolchain policy still supplies physical
//! classification independently, so an unsupported leaf must refuse.

use ::provider_admission::provider_admission;
use effects::{
    CompilerIntrinsicExecutionIdentity, PortableFilesystemAuthorityFacet, TerminalAuthorityClass,
    TerminalAuthorityDisposition,
    provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow},
};
use package_manager::{
    PackageCommand, PackageCommandKind, PackageCommandOptions, PackageCommandStatus,
};
use platform_custody::temporary_directory::TemporaryDirectory;
use semantic_vocabulary::PackageKeyIdentity;
use std::path::{Path, PathBuf};

const FIXTURE: &str = "tests/fixtures/pass/filesystem/windows_canonicalize_exit/main.omg";
const TARGET: &str = "windows_x86_64";

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "support/joined_product_execution.rs"]
mod joined_product_execution;

// These are library unit programs, not a replacement Windows provider. Stage
// the complete production helper unchanged so scalar endpoints do not need a
// fabricated four-gigabyte view or an OS handle on the host running the test.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
struct TransferCountProject(PathBuf);

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
impl TransferCountProject {
    fn stage(kind: &str) -> Self {
        let repository = repo_root();
        let projects = repository.join("build/windows-transfer-count");
        std::fs::create_dir_all(&projects).unwrap();
        let root = (0u64..)
            .find_map(|sequence| {
                let root = projects.join(format!("{kind}-{}-{sequence}", std::process::id()));
                match std::fs::create_dir(&root) {
                    Ok(()) => Some(root),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                    Err(error) => panic!("create private transfer project: {error}"),
                }
            })
            .unwrap();
        let project = Self(root);
        copy_tree(
            &repository.join(format!(
                "tests/fixtures/{kind}/filesystem/windows_transfer_count"
            )),
            &project.0,
        );
        std::fs::copy(
            repository.join(
                "source/library/std/targets/windows_x86_64/windows_filesystem_transfer_count.omg",
            ),
            project.0.join("windows_filesystem_transfer_count.omg"),
        )
        .unwrap();
        project
    }

    fn main(&self) -> PathBuf {
        self.0.join("main.omg")
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
impl Drop for TransferCountProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn windows_transfer_count_runs_in_native_and_psi() {
    let project = TransferCountProject::stage("run");
    let discovered =
        fixture_package_inputs::discover_fixture(&project.main(), Default::default()).unwrap();
    let mut prepared = fixture_package_inputs::review_fixture(
        discovered,
        &[target::TargetProfile::LinuxX64],
        compiler::CompilationProduct::Native(products::NativeProductRequest {
            retain_native_inputs: false,
            retain_native_stage_inputs: false,
            installation_assumptions: Vec::new(),
            image: Default::default(),
            private_entry_ram: Vec::new(),
            terminal_authority_policy: provider_admission::current_terminal_authority_policy(),
            terminal_authority_permission_policy: None,
            optimization_rollback: Default::default(),
        }),
    )
    .unwrap();
    let produced = fixture_package_inputs::produce_fixture(&mut prepared, Some("linux_x86_64"))
        .unwrap_or_else(|diagnostics| panic!("transfer endpoint production: {diagnostics:#?}"));
    let report = orchestration::complete_product(produced)
        .and_then(compiler::CompilationOutput::into_report)
        .unwrap_or_else(|error| panic!("transfer endpoint completion: {error:?}"));
    let output = TemporaryDirectory::create("windows-transfer-count-execution", false).unwrap();
    joined_product_execution::execute_report(report, &output.path().join("native"));
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn windows_transfer_count_refuses_original_request_beyond_available() {
    let project = TransferCountProject::stage("fail");
    let discovered =
        fixture_package_inputs::discover_fixture(&project.main(), Default::default()).unwrap();
    let reviewed = fixture_package_inputs::review_fixture(
        discovered,
        &[target::TargetProfile::LinuxX64],
        compiler::CompilationProduct::Check,
    );
    // Ordinary package review may already check this application body. The
    // same exact capacity refusal is valid there or in the final source check;
    // every setup error and every unrelated diagnostic still fails below.
    let diagnostics = match reviewed {
        Err(diagnostics) => diagnostics,
        Ok(mut prepared) => match fixture_package_inputs::check_fixture(
            &mut prepared,
            "linux_x86_64",
            Default::default(),
        ) {
            Ok(_) => {
                panic!("clamping must not admit an original request beyond its available bytes")
            }
            Err(diagnostics) => diagnostics,
        },
    };
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    let refusal = &diagnostics[0];
    assert!(refusal.is_error());
    assert!(
        refusal.message.starts_with(
            "cannot prove requires contract for call windows_transfer_count from main:",
        ) && refusal.message.contains("4294967296 <= 4294967295")
            && refusal.message.contains("requested <= available"),
        "{diagnostics:#?}"
    );
    let location = refusal.resolved_location.as_ref().unwrap();
    assert!(location.path.ends_with("main.omg"));
    assert_eq!((location.line, location.column), (8, 22));
}

#[test]
fn windows_transfer_count_checks_both_native_consumers() {
    // Source-check the actual Windows providers without invoking Windows I/O.
    // Both raw-family calls must reach the private bounded-view consumers;
    // scalar helper execution alone cannot establish those call contracts.
    let workspace = TemporaryDirectory::create("windows-transfer-consumers", false).unwrap();
    let root = repo_root();
    copy_tree(
        &root.join("source/library"),
        &workspace.path().join("source/library"),
    );
    let project = workspace.path().join("transfer-consumers");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(
        project.join("main.omg"),
        r#"use omega::language::core::FilesystemContentRead;
use omega::language::core::FilesystemContentWrite;
use omega_language_std::WindowsFilesystemHost;

machine main() reaches FilesystemContentRead + FilesystemContentWrite {
    let mut bytes: [u8; 5];
    let read: i64 = block WindowsFilesystemHost::read_file(1, &mut bytes[..], 5);
    let written: i64 = block WindowsFilesystemHost::write_file(1, &bytes[..]);
}
"#,
    )
    .unwrap();
    std::fs::write(
        project.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    Build::application(builder, "windows_transfer_consumers");
    let std_registration: DependencyRegistration = Build::dependency(builder, dependency::from_source(Source::Path {
        location: "../source/library/std"
    }));
    Build::bind(builder, windows_x86_64::ProgramEntry, main);
}
"#,
    )
    .unwrap();
    let profile = target::TargetProfile::WindowsX64;
    // A fresh project uses the documented unlocked development route.
    // The ordinary package build still checks the selected providers and imports.
    let mut outcomes = orchestration::build(orchestration::BuildRequest {
        stage_validators: Default::default(),
        root_path: project.join("main.omg"),
        build_directory: Some(project.join("build/terminal")),
        targets: orchestration::BuildTargets::Single(profile),
        product: orchestration::ProjectProduct::Terminal,
        timings: false,
        offline: true,
        verify: false,
        require_package_project: true,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })
    .expect("ordinary Windows transfer consumer build");
    assert_eq!(outcomes.len(), 1);
    let report = outcomes
        .pop()
        .unwrap()
        .result
        .unwrap_or_else(|error| panic!("Windows transfer consumer compilation: {error}"))
        .into_report()
        .expect("Windows transfer Terminal report")
        .report;
    let retained = report
        .into_retained_terminal_artifact()
        .expect("source-produced Windows transfer artifact");
    let proposal = retained.native_realization_proposal().unwrap();
    let mut read = false;
    let mut write = false;
    for plan in proposal.providers().selection.plans() {
        for row in &plan.rows {
            let method = checked_receiver_method(plan, row);
            if method.requirement_owner == "WindowsFilesystemTransferIo" {
                read |= method.name == "read_file";
                write |= method.name == "write_file";
            }
        }
    }
    assert!(
        read && write,
        "both actual raw transfer imports were reached"
    );
}

#[test]
fn windows_canonicalize_receiver() {
    // The ordinary application route must establish project acceptance. The
    // fixture helper's synthetic Console bindings are not a substitute for it.
    let workspace = TemporaryDirectory::create("windows-filesystem-receiver", false)
        .expect("private project workspace");
    let root = repo_root();
    copy_tree(
        &root.join("source/library"),
        &workspace.path().join("source/library"),
    );
    let project = workspace
        .path()
        .join(FIXTURE)
        .parent()
        .unwrap()
        .to_path_buf();
    copy_tree(root.join(FIXTURE).parent().unwrap(), &project);
    let profile = target::TargetProfile::WindowsX64;
    eprintln!("Windows receiver: starting ordinary project review");
    accept_project(&project, profile);
    eprintln!("Windows receiver: ordinary project review completed");
    // The receiver selects its trusted source identity independently: this
    // private std path, expected authored package name and explicit local
    // context version. A foreign owner receives no fallback grant by name.
    let trusted_std = package_manager::declarations::PackageKey::new(
        package_manager::declarations::PackageName::parse("omega_language_std")
            .expect("receiver's expected std package name"),
        source_acquisition::SourceLineage::ExternalLocal(
            source_acquisition::ExternalLocalLineage::canonicalize(
                workspace.path().join("source/library/std"),
                source_acquisition::ExternalSourceContext::derive(b"omega-local-project-v1"),
            )
            .expect("receiver's exact canonical trusted std source"),
        ),
    )
    .identity();
    // Reuse the public application owner of source discovery, accepted
    // package admission, Product compilation and completion. In particular,
    // there is no fixture-injected Console provider or second scheduler here.
    eprintln!("Windows receiver: producing the Windows Terminal program");
    let mut outcomes = orchestration::build(orchestration::BuildRequest {
        stage_validators: Default::default(),
        root_path: project.join("main.omg"),
        build_directory: Some(project.join("build/terminal")),
        targets: orchestration::BuildTargets::Single(profile),
        product: orchestration::ProjectProduct::Terminal,
        timings: false,
        offline: true,
        verify: false,
        require_package_project: true,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })
    .unwrap_or_else(|error| panic!("{FIXTURE} ordinary project build: {error}"));
    assert_eq!(outcomes.len(), 1, "one exact Windows target");
    let report = outcomes
        .pop()
        .unwrap()
        .result
        .unwrap_or_else(|error| panic!("{FIXTURE} ordinary Terminal production: {error}"))
        .into_report()
        .unwrap_or_else(|error| panic!("{FIXTURE} Terminal report: {error}"))
        .report;
    let retained = report
        .into_retained_terminal_artifact()
        .expect("source-produced native proposal");
    eprintln!("Windows receiver: Terminal program retained");
    let proposal = retained
        .native_realization_proposal()
        .expect("native proposal");
    // Package custody is an input to ordinary production. Its accepted rows
    // stay separate from the new, independently authored receiver table.
    let package_policy = provider_admission::terminal_authority_permission_policy_with_rows(
        proposal.package_terminal_authority_permissions().to_vec(),
        proposal
            .package_terminal_authority_permission_identities()
            .clone(),
    )
    .expect("accepted package permissions");
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let mut permissions = Vec::new();
    let mut output_requirements = Vec::new();
    let mut trusted_filesystem_grants = 0;
    let mut ordinary_close_grants = 0;
    let mut canonical_process_exit_grants = 0;
    let mut grant = |plan: &ProviderPlan, process_exit: bool| {
        for row in &plan.rows {
            if !process_exit
                && !matches!(
                    row.binding,
                    ProviderBinding::Import { .. } | ProviderBinding::Syscall { .. }
                )
            {
                continue;
            }
            let Some(permitted) = receiver_grant(plan, row, trusted_std, process_exit) else {
                continue;
            };
            if permitted.classes() == [TerminalAuthorityClass::ProcessTermination] {
                canonical_process_exit_grants += 1;
            } else if !process_exit
                && plan.schema.declaration.trait_package_identity == Some(trusted_std)
            {
                if matches!(
                    row.method.as_str(),
                    "c_close_handle" | "close_handle" | "find_close"
                ) {
                    assert!(permitted.classes().is_empty(), "ordinary close permission");
                    ordinary_close_grants += 1;
                } else {
                    assert_eq!(permitted.classes().len(), 6, "broad filesystem permission");
                    trusted_filesystem_grants += 1;
                }
            }
            let permission = provider_admission::TerminalAuthorityPermissionPolicyRow::new(
                plan.schema.identity_digest(),
                identities.intern(plan.identities.text(row.requirement_identity)),
                permitted,
            );
            if !process_exit && trusted_write_file_requirement(plan, row, trusted_std) {
                let requirement = (
                    permission.service_schema(),
                    permission.requirement_identity(),
                );
                if !output_requirements.contains(&requirement) {
                    output_requirements.push(requirement);
                }
            }
            if !permissions.contains(&permission) {
                permissions.push(permission);
            }
        }
    };
    for plan in proposal.providers().selection.plans() {
        grant(plan, false);
    }
    for leaf in &proposal.providers().direct_boundary_leaves {
        if leaf.plan.target == TARGET {
            // DirectBoundaryInput::finish projects one original row and
            // validates its callable identity. Recheck that public invariant
            // before granting the intrinsic's sole requirement; an entire
            // unprojected provider plan must never inherit exit permission.
            leaf.validate_calling_custody()
                .expect("exact direct callable custody");
            assert_eq!(leaf.plan.rows.len(), 1, "one direct callable requirement");
            grant(
                &leaf.plan,
                matches!(
                    leaf.execution,
                    backend_plan::DirectBoundaryExecution::Intrinsic(Some(
                        CompilerIntrinsicExecutionIdentity::HostedExitProcessI32
                    ))
                ),
            );
        }
    }
    assert!(
        trusted_filesystem_grants > 0,
        "the independently configured std identity matched no filesystem grants"
    );
    assert!(
        ordinary_close_grants > 0,
        "the independently configured std identity matched no empty ordinary-close grant"
    );
    assert!(
        canonical_process_exit_grants > 0,
        "the independently configured std identity matched no ProcessExit::exit_process grant"
    );
    eprintln!(
        "trusted std receiver setup: {trusted_filesystem_grants} filesystem grants, {ordinary_close_grants} empty ordinary-close grants, {canonical_process_exit_grants} ProcessExit::exit_process grants"
    );
    assert_eq!(
        output_requirements.len(),
        1,
        "receiver setup must join the original trusted write_file requirement exactly once"
    );
    let identities = identities.build();
    let filesystem_only = provider_admission::terminal_authority_permission_policy_with_rows(
        permissions.clone(),
        identities.clone(),
    )
    .expect("original independent filesystem-only receiver permissions");
    // This second table is authored by this integration test, independently of
    // physical classification and package acceptance. Only the exact original
    // trusted requirement gets ProcessOutput; the empty close grant stays empty.
    let positive_permissions = permissions
        .into_iter()
        .map(|row| {
            if output_requirements.contains(&(row.service_schema(), row.requirement_identity())) {
                assert_eq!(row.permitted().classes().len(), 6);
                provider_admission::TerminalAuthorityPermissionPolicyRow::new(
                    row.service_schema(),
                    row.requirement_identity(),
                    TerminalAuthorityDisposition::from_classes(
                        row.permitted()
                            .classes()
                            .iter()
                            .copied()
                            .chain([TerminalAuthorityClass::ProcessOutput]),
                    ),
                )
            } else {
                row
            }
        })
        .collect();
    let receiver = provider_admission::terminal_authority_permission_policy_with_rows(
        positive_permissions,
        identities,
    )
    .expect("independent exact receiver permissions");
    let receiver_identity = receiver.identity();
    eprintln!("windows filesystem receiver policy: {receiver_identity:?}");
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    eprintln!("Windows receiver: preparing restricted native admission");
    let mut accounting = source_compiler::evaluation::SourceCompletionAccounting::default();
    let scalars = build_settings::scalar_completion::complete_native(&retained, &mut accounting)
        .expect("scalar completion");
    let request = |receiver| native_compiler::NativeCompileRequest {
        retain_native_stage_inputs: false,
        private_entry_ram: &[],
        installation_assumptions: &[],
        optimization_selections: &optimizations,
        terminal_authority_policy: provider_admission::current_terminal_authority_policy(),
        accepted_package_terminal_authority_permission_policy: package_policy.clone(),
        terminal_authority_permission_policy: Some(receiver),
        image_request: image_emission::ExecutableImageEmissionRequest::direct(
            proposal.application().subsystem,
        ),
        imports: &[],
        components: &[],
        scalars: scalars.as_ref(),
    };
    eprintln!("Windows receiver: checking the restricted receiver policy");
    let product = native_compiler::compile(&retained, request(receiver))
        .unwrap_or_else(|(_, diagnostics)| panic!("{FIXTURE} must realize: {diagnostics:#?}"));

    let native_artifact::RequestedNativeArtifact::Direct(artifact) = product.artifact else {
        panic!("{FIXTURE} emits its ordinary Windows image")
    };
    artifact
        .validate()
        .expect("receiver-admitted native artifact replay");
    assert_eq!(
        artifact.terminal_authority_permission_policy_identity(),
        Some(receiver_identity),
        "production without an independent receiver verdict cannot satisfy this request"
    );
    eprintln!("Windows receiver: checking the filesystem-only refusal control");
    // Reuse the same accepted retained Terminal input. Runtime admission shares
    // the provider/receiver review and stops before image production; this is
    // a policy refusal control, not a second source review or runtime claim.
    let refusal = match native_compiler::admit_runtime(&retained, request(filesystem_only)) {
        Err((_, diagnostics)) => diagnostics,
        Ok(_) => panic!("filesystem-only input must refuse console-capable WriteFile"),
    };
    assert!(
        refusal.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("WindowsFilesystemTransferIo::write_file")
                && diagnostic
                    .message
                    .contains("ExercisedAuthorityNotPermitted")
                && diagnostic.message.contains("ProcessOutput")
        }),
        "the control must refuse the actual WriteFile class mismatch: {refusal:#?}"
    );
}

fn trusted_write_file_requirement(
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    trusted_std: PackageKeyIdentity,
) -> bool {
    if plan.schema.declaration.trait_package_identity != Some(trusted_std) {
        return false;
    }
    let method = checked_receiver_method(plan, row);
    method.requirement_owner_package_identity == Some(trusted_std)
        && method.requirement_owner == "WindowsFilesystemTransferIo"
        && method.name == "write_file"
}

fn checked_receiver_method<'a>(
    plan: &'a ProviderPlan,
    row: &ProviderPlanRow,
) -> &'a effects::provider_plan::ServiceMethod {
    let requirement = plan.identities.text(row.requirement_identity);
    let mut methods = plan
        .schema
        .declaration
        .methods
        .iter()
        .filter(|method| plan.schema.identity_text(method.requirement_identity) == requirement);
    let method = methods
        .next()
        .expect("receiver row has its exact checked contract");
    assert!(methods.next().is_none(), "one exact receiver contract");
    method
}

fn receiver_grant(
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    trusted_std: PackageKeyIdentity,
    process_exit: bool,
) -> Option<TerminalAuthorityDisposition> {
    let method = checked_receiver_method(plan, row);
    // Completion was independently admitted for the exact canonical
    // requirement. This receiver grants that checked row termination
    // permission; Console I/O contributes no such grant.
    if process_exit
        || method.completion
            == semantic_vocabulary::BoundaryCompletion::External(
                semantic_vocabulary::ExternalCompletionKind::ProcessExit,
            )
    {
        return Some(TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::ProcessTermination,
        ]));
    }
    if plan.schema.declaration.trait_package_identity != Some(trusted_std) {
        return None;
    }
    // This changes the receiver's permission, never the mechanism's physical
    // classification. The exact schema digest includes declaration ownership
    // and checked contracts; names only locate the policy's ordinary-close row.
    if matches!(
        row.method.as_str(),
        "c_close_handle" | "close_handle" | "find_close"
    ) {
        Some(TerminalAuthorityDisposition::from_classes([]))
    } else {
        Some(TerminalAuthorityDisposition::from_filesystem_facets(
            PortableFilesystemAuthorityFacet::ALL,
        ))
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate belongs to the repository")
        .to_path_buf()
}

// Preserve the customer's relative Build dependency and every source byte.
// Only the private copy receives package-review and compiler output files.
fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("private source directory");
    for entry in std::fs::read_dir(source).expect("source inventory") {
        let entry = entry.expect("source entry");
        let kind = entry.file_type().expect("source file kind");
        let output = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &output);
        } else {
            assert!(kind.is_file(), "source inputs must be ordinary files");
            std::fs::copy(entry.path(), output).expect("unchanged source copy");
        }
    }
}

fn accept_project(project: &Path, target: target::TargetProfile) {
    let mut options = PackageCommandOptions {
        project_root: project.to_path_buf(),
        targets: vec![target],
        offline: true,
        build_inputs: None,
    };
    // Match explicit CLI review: Update returns to the user before a later
    // Resume invocation reads the saved proposal and authored decisions. Each
    // command owns its compiler and resource grant until disposal; this test
    // does not exercise the CLI's same-invocation automatic acceptance option.
    let workspace = project.join("build/package-manager");
    let mut session = orchestration::BuildSession::create_retained(&workspace)
        .expect("ordinary update invocation");
    let outcome = orchestration::execute_package_command(
        PackageCommand::Update {
            packages: Vec::new(),
            revision: None,
        },
        options.clone(),
        &mut session,
    );
    let mut outcome = session.dispose(outcome).expect("ordinary project update");
    // This harness supplies repository-test review consent through the same
    // editable decisions as the CLI. It neither constructs accepted bindings
    // nor turns those project decisions into the receiver's permission table.
    for _ in 0..4 {
        if outcome.status == PackageCommandStatus::Published {
            return;
        }
        assert_eq!(outcome.status, PackageCommandStatus::ReviewRequired);
        for path in &outcome.review_paths {
            let document = std::fs::read_to_string(path).expect("pending review");
            let mut accepted = String::new();
            for line in document.lines() {
                match line
                    .strip_suffix(" pending")
                    .filter(|_| line.starts_with("decision "))
                {
                    Some(subject) => {
                        accepted.push_str(subject);
                        accepted.push_str(" accept");
                    }
                    None => accepted.push_str(line),
                }
                accepted.push('\n');
            }
            std::fs::write(path, accepted).expect("authored test review consent");
        }
        options.targets.clear();
        options.build_inputs = None;
        let mut session = orchestration::BuildSession::create_retained(&workspace)
            .expect("ordinary resume invocation");
        let resumed = orchestration::execute_package_command(
            PackageCommand::Resume {
                kind: PackageCommandKind::Update,
            },
            options.clone(),
            &mut session,
        );
        outcome = session.dispose(resumed).expect("ordinary project resume");
    }
    assert_eq!(
        outcome.status,
        PackageCommandStatus::Published,
        "bounded review completion"
    );
}
