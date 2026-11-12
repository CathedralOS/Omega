//! Review the unchanged Windows customer under two independent receiver inputs.
//!
//! Ordinary CLI production makes no receiver-admission claim. This consumer
//! supplies one explicitly, without translating package acceptance or abstract
//! service reach into permission. It deliberately grants all six filesystem
//! classes to each raw requirement owned by the reviewed bundled std package;
//! these are broad filesystem grants, not minimal per-operation permissions.
//! Generic ordinary close gets an empty grant, while namespace-removal close
//! keeps the filesystem grant. The structural process-exit intrinsic and the
//! trusted std Console::exit_process requirement get termination permission.
//! The positive additionally grants ProcessOutput to the original trusted
//! WindowsFilesystemTransferIo::write_file requirement: generic WriteFile can
//! write a console. The original filesystem-only table stays a refusal control.
//! Physical classification stays independently supplied by the existing
//! toolchain policy, so an unsupported leaf must refuse.

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
use terminal_psi_to_abstract_operations::provider_admission;

const FIXTURE: &str = "tests/omega/pass/filesystem/windows_canonicalize_exit/main.omg";
const TARGET: &str = "windows_x86_64";

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
        package_source::SourceLineage::ExternalLocal(
            package_source::ExternalLocalLineage::canonicalize(
                workspace.path().join("source/library/std"),
                package_source::ExternalSourceContext::derive(b"omega-local-project-v1"),
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
    let mut trusted_console_exit_grants = 0;
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
            if !process_exit && plan.schema.declaration.trait_package_identity == Some(trusted_std)
            {
                if permitted.classes() == [TerminalAuthorityClass::ProcessTermination] {
                    trusted_console_exit_grants += 1;
                } else if matches!(
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
        trusted_console_exit_grants > 0,
        "the independently configured std identity matched no Console::exit_process grant"
    );
    eprintln!(
        "trusted std receiver setup: {trusted_filesystem_grants} filesystem grants, {ordinary_close_grants} empty ordinary-close grants, {trusted_console_exit_grants} Console::exit_process grants"
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
    let prepare = |receiver| {
        let mut prepared_native = native_compiler::prepare(
            &retained,
            native_compiler::NativeCompileRequest {
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
            },
        )
        .unwrap_or_else(|(_, diagnostics)| {
            panic!("{FIXTURE} receiver preparation: {diagnostics:#?}")
        });
        let mut accounting = build_time_evaluation::SourceCompletionAccounting::default();
        for completion in prepared_native.scalar_completions() {
            let artifact = completion.artifact();
            let completed = build_time_evaluation::complete_scalar_applications(
                artifact,
                completion.target(),
                None,
                completion.demand(),
                None,
                &mut accounting,
                |scope| {
                    let proposal = completion
                        .provider_proposals()
                        .iter()
                        .find(|proposal| proposal.target == scope)
                        .ok_or_else(|| format!("missing {scope} provider proposal"))?;
                    let providers = proposal
                        .selection
                        .as_ref()
                        .map_err(|reasons| reasons.join("; "))?
                        .admit(artifact.view(), scope)?;
                    let profile = target::TargetProfile::from_canonical_target_name(scope)
                        .map_err(|diagnostic| diagnostic.message)?;
                    Ok(build_time_evaluation::ScalarScopeEvaluation {
                        application_roots: None,
                        import_realizations: None,
                        layout_target: profile
                            .layout_target_facts()
                            .ok_or("scalar evaluation target has no immutable layout catalogue")?,
                        carrier_layout: profile
                            .native_realization()
                            .map(runtime_abi::target_carrier_layout),
                        targets: terminal_installation::admit_target_machine_selections(
                            artifact.view(),
                            scope,
                        ),
                        providers,
                        address_model: Ok(target::TargetSemantics::close_address_model(profile)
                            .map(|model| terminal_installation::ScalarAddressModel {
                                target: scope.to_owned(),
                                carrier: model.carrier(),
                                semantics_version: model.semantics_version(),
                                commitment: model.commitment(),
                            })),
                    })
                },
            )
            .expect("scalar completion");
            completion.complete(completed).expect("one completion");
        }
        prepared_native
    };
    eprintln!("Windows receiver: checking the restricted receiver policy");
    let product = native_compiler::compile(prepare(receiver))
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
    let refusal = match native_compiler::admit_runtime(prepare(filesystem_only)) {
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
    if process_exit {
        return Some(TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::ProcessTermination,
        ]));
    }
    if plan.schema.declaration.trait_package_identity != Some(trusted_std) {
        return None;
    }
    // The independently trusted source declares Console::exit_process. Its
    // Windows implementation is a foreign leaf, rather than the intrinsic
    // above. Grant that authored requirement termination permission, keeping
    // physical import classification independent. Join the exact row to its
    // checked contract; direct leaves have already validated that projection
    // against their separately retained original satisfied schema.
    let method = checked_receiver_method(plan, row);
    if method.requirement_owner_package_identity == Some(trusted_std)
        && method.requirement_owner == "Console"
        && method.name == "exit_process"
    {
        return Some(TerminalAuthorityDisposition::from_classes([
            TerminalAuthorityClass::ProcessTermination,
        ]));
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
