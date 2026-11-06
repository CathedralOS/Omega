//! Ask a separate filesystem receiver to admit the unchanged Windows customer.
//!
//! Ordinary CLI production makes no receiver-admission claim. This consumer
//! supplies one explicitly, without translating package acceptance or abstract
//! service reach into permission. It deliberately grants all six filesystem
//! classes to each raw requirement owned by the reviewed bundled std package;
//! these are broad filesystem grants, not minimal per-operation permissions.
//! Generic ordinary close gets an empty grant, while namespace-removal close
//! keeps the filesystem grant. Only the structural process-exit intrinsic gets
//! termination permission. Physical classification stays independently supplied
//! by the existing toolchain policy, so an unsupported leaf must refuse.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct};
use effects::{
    CompilerIntrinsicExecutionIdentity, PortableFilesystemAuthorityFacet, TerminalAuthorityClass,
    TerminalAuthorityDisposition,
    provider_plan::{ProviderBinding, ProviderPlan},
};
use fixture_package_inputs::{repo_root, reviewed_repository_fixture_package_inputs};
use semantic_vocabulary::PackageKeyIdentity;
use terminal_psi_to_abstract_operations::provider_admission;

const FIXTURE: &str = "tests/omega/pass/filesystem/windows_canonicalize_exit/main.omg";
const TARGET: &str = "windows_x86_64";

#[test]
fn windows_canonicalize_receiver() {
    let main = repo_root().join(FIXTURE);
    let mut prepared = reviewed_repository_fixture_package_inputs(&main, Some(TARGET))
        .unwrap_or_else(|diagnostics| panic!("{FIXTURE} package review: {diagnostics:#?}"));
    // The shared fixture reviewer selects this entry supplier only by the
    // canonical bundled-library source root. Its retained owner identifies the
    // receiver's trusted std declarations; a same-named offered package cannot
    // acquire these grants. Permission classes below are authored independently.
    let trusted_std = prepared
        .package_inputs
        .as_ref()
        .expect("the customer has its authored std dependency")
        .accepted_semantic_bindings()
        .find(|binding| {
            binding.role()
                == package_compilation::AcceptedSemanticBindingRole::WindowsX64ProgramEntry
        })
        .expect("reviewed bundled Windows entry supplier")
        .package();
    let request = CompileRequest::new(CompileOptions {
        root_path: prepared.root_path.clone(),
        build_dir: Some(std::env::temp_dir().join(format!(
            "omega-windows-filesystem-receiver-{}",
            std::process::id()
        ))),
        target_name: Some(TARGET.to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact)
    .with_package_inputs(
        prepared
            .package_inputs
            .take()
            .expect("reviewed package inputs"),
    );
    let retained = orchestration::compile_sources(prepared.session.compiler(), request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| panic!("{FIXTURE} Terminal production: {diagnostics:#?}"))
        .into_retained_terminal_artifact()
        .expect("source-produced native proposal");
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
            let Some(permitted) = receiver_grant(plan, &row.method, trusted_std, process_exit)
            else {
                continue;
            };
            let permission = provider_admission::TerminalAuthorityPermissionPolicyRow::new(
                plan.schema.identity_digest(),
                identities.intern(plan.identities.text(row.requirement_identity)),
                permitted,
            );
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
    let receiver = provider_admission::terminal_authority_permission_policy_with_rows(
        permissions,
        identities.build(),
    )
    .expect("independent exact receiver permissions");
    let receiver_identity = receiver.identity();
    eprintln!("windows filesystem receiver policy: {receiver_identity:?}");
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let mut prepared_native = native_production::prepare(
        &retained,
        native_production::NativeCompileRequest {
            private_entry_ram: &[],
            optimization_selections: &optimizations,
            terminal_authority_policy: provider_admission::current_terminal_authority_policy(),
            accepted_package_terminal_authority_permission_policy: package_policy,
            terminal_authority_permission_policy: Some(receiver),
            image_request: image_emission::ExecutableImageEmissionRequest::direct(
                proposal.application().subsystem,
            ),
            imports: &[],
            components: &[],
        },
    )
    .unwrap_or_else(|(_, diagnostics)| panic!("{FIXTURE} receiver preparation: {diagnostics:#?}"));
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
                    address_model: Ok(target::TargetSemantics::close_address_model(profile).map(
                        |model| terminal_installation::ScalarAddressModel {
                            target: scope.to_owned(),
                            carrier: model.carrier(),
                            semantics_version: model.semantics_version(),
                            commitment: model.commitment(),
                        },
                    )),
                })
            },
        )
        .expect("scalar completion");
        completion.complete(completed).expect("one completion");
    }
    let product = native_production::compile(prepared_native)
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
}

fn receiver_grant(
    plan: &ProviderPlan,
    method: &str,
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
    // This changes the receiver's permission, never the mechanism's physical
    // classification. The exact schema digest includes declaration ownership
    // and checked contracts; names only locate the policy's ordinary-close row.
    if matches!(method, "c_close_handle" | "close_handle" | "find_close") {
        Some(TerminalAuthorityDisposition::from_classes([]))
    } else {
        Some(TerminalAuthorityDisposition::from_filesystem_facets(
            PortableFilesystemAuthorityFacet::ALL,
        ))
    }
}
