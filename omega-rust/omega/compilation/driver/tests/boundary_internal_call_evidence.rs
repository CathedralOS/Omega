//! End-to-end witness for the D41 installed-provider internal-call arm of
//! native physical evidence: a `boundary requirement` whose checked provider
//! is an in-module machine emits an ordinary resolved internal call, and the
//! retained object call custody — the call-stack row plus the selected
//! provider conformance — binds the boundary occurrence to one exact physical
//! child with `ResolvedInternalCall` relocation disposition.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{
    CompileOptions, CompileRequest, RequestedCompileProduct, RetainedNativeRealizationRequest,
    compile, realize_retained_native_artifact,
};
use fixture_package_inputs::{repo_root, reviewed_repository_fixture_package_inputs};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use target_operations::CallSiteOwner;
use terminal_psi_to_abstract_operations::provider_admission;

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

fn build_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-{name}-{}-{}",
        std::process::id(),
        NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed),
    ))
}

/// The corpus fixture's `boundary requirement` selects an in-module checked
/// provider: `CheckedMath::offset_zero` is realized by
/// `CheckedMathProvider::offset_zero_impl`, an ordinary machine inside the
/// same artifact, so the boundary call emits a resolved internal call rather
/// than a settlement row or a normalized foreign call. Physical evidence must
/// bind both boundary occurrences — the provider call and the hosted exit —
/// or name the exact one it cannot.
#[test]
fn installed_provider_internal_call_retains_complete_physical_evidence() {
    let main = repo_root()
        .join("tests/omega/pass/providers/checked_boundary_requirement_dispatch_exit/main.omg");
    let package_inputs = reviewed_repository_fixture_package_inputs(&main, Some("linux_x86_64"))
        .unwrap_or_else(|diagnostics| {
            panic!("checked boundary fixture package inputs: {diagnostics:#?}")
        });
    let root = build_dir("boundary-internal-call");
    let _ = fs::remove_dir_all(&root);
    let mut request = CompileRequest::new(CompileOptions {
        root_path: main,
        build_dir: Some(root),
        target_name: Some("linux_x86_64".to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let retained = compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| {
            panic!("checked boundary fixture must reach Terminal: {diagnostics:#?}")
        })
        .into_retained_terminal_artifact()
        .expect("checked boundary fixture retains its native proposal");
    let proposal = retained
        .native_realization_proposal()
        .expect("checked boundary fixture native proposal");
    let package_policy = provider_admission::terminal_authority_permission_policy_with_rows(
        proposal.package_terminal_authority_permissions().to_vec(),
    )
    .expect("package permission policy");
    let permission_policy = provider_admission::terminal_authority_permission_policy_with_rows(
        proposal
            .package_terminal_authority_permissions()
            .iter()
            .cloned()
            .chain(
                proposal
                    .providers()
                    .selection
                    .plans()
                    .iter()
                    .flat_map(|plan| {
                        plan.rows.iter().map(|row| {
                            provider_admission::TerminalAuthorityPermissionPolicyRow::new(
                                plan.schema.identity_digest(),
                                row.requirement_identity.clone(),
                                effects::TerminalAuthorityDisposition::from_classes([]),
                            )
                        })
                    }),
            )
            .fold(
                Vec::new(),
                |mut rows: Vec<provider_admission::TerminalAuthorityPermissionPolicyRow>, row| {
                    if !rows.iter().any(|seen| {
                        seen.service_schema() == row.service_schema()
                            && seen.requirement_identity() == row.requirement_identity()
                    }) {
                        rows.push(row);
                    }
                    rows
                },
            ),
    )
    .expect("permission policy");
    let image_request =
        image_emission::ExecutableImageEmissionRequest::direct(proposal.application().subsystem);
    let replay_image_request = image_request.clone();
    let product = realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: provider_admission::terminal_authority_policy_with_rows(
                Vec::new(),
            )
            .expect("empty terminal authority policy"),
            accepted_package_terminal_authority_permission_policy: package_policy,
            terminal_authority_permission_policy: Some(permission_policy),
            image_request,
            imports: &[],
        },
    )
    .unwrap_or_else(|(_, diagnostics)| {
        panic!("checked boundary fixture must realize its native artifact: {diagnostics:#?}")
    });
    let native_artifact::RequestedNativeArtifact::Direct(artifact) = product else {
        panic!("checked boundary fixture emits a direct native artifact")
    };
    artifact
        .validate()
        .unwrap_or_else(|error| panic!("checked boundary artifact replay: {error}"));

    let evidence = artifact
        .physical_evidence()
        .expect("installed provider internal call binds complete physical evidence");
    assert_eq!(evidence.children().len(), 2);
    let internal_call_children = evidence
        .children()
        .iter()
        .filter(|child| {
            matches!(
                child.relocation(),
                native_artifact::PhysicalRelocationDisposition::ResolvedInternalCall
            )
        })
        .collect::<Vec<_>>();
    let [call_child] = internal_call_children.as_slice() else {
        panic!("one boundary child must carry the resolved internal call")
    };
    let native_artifact::PhysicalChildParent::BoundaryTraitSettlement(settlement) =
        call_child.parent()
    else {
        panic!("the internal-call boundary child keeps its settlement parent")
    };
    let native_artifact::BoundaryTraitSettlementRole::InstalledProviderInternalCall {
        boundary,
        provider_identity,
        candidate_identity,
        call,
    } = settlement.role()
    else {
        panic!("the internal-call boundary child names its exact role")
    };
    assert_eq!(*boundary, settlement.occurrence().boundary());
    assert!(
        !provider_identity.is_empty() && !candidate_identity.is_empty(),
        "the internal-call role retains the selected provider and candidate identities"
    );
    let native_artifact::InstalledProviderInternalCallCustody::Unit {
        owner,
        target,
        call_field_offset,
        ..
    } = call
    else {
        panic!("the Unit caller retains unit call-stack custody")
    };
    assert_eq!(
        *owner,
        CallSiteOwner::Operation(settlement.occurrence().operation()),
        "the retained call row names the exact boundary operation"
    );
    assert_ne!(
        *target,
        settlement.occurrence().machine(),
        "the retained call targets the selected provider machine, not its caller"
    );
    assert!(call_child.machine_span().byte_count() > 0);
    assert!(call_child.object_span().byte_count() > 0);
    assert!(call_child.final_image_span().byte_count() > 0);
    assert!(
        *call_field_offset >= call_child.object_span().offset()
            && *call_field_offset + 4
                <= call_child.object_span().offset() + call_child.object_span().byte_count(),
        "the retained call field sits inside the child's object span"
    );
    let hosted_child = evidence
        .children()
        .iter()
        .find(|child| {
            matches!(
                child.relocation(),
                native_artifact::PhysicalRelocationDisposition::DirectInstructionBytes
            )
        })
        .expect("the hosted exit boundary keeps its settlement child");
    let native_artifact::PhysicalChildParent::BoundaryTraitSettlement(hosted) =
        hosted_child.parent()
    else {
        panic!("the hosted-exit boundary child keeps its settlement parent")
    };
    assert!(
        matches!(
            hosted.role(),
            native_artifact::BoundaryTraitSettlementRole::CompilerBuiltin { .. }
        ),
        "the hosted exit boundary keeps its builtin role"
    );

    // The backend owns final image construction and custody replay, even
    // when called without the compiler's realization coordinator.
    let expected_identity = artifact.identity();
    let reproduced = native_artifact::RequestedNativeArtifact::emit(emission_request_parts(
        artifact,
        replay_image_request.clone(),
    ))
    .expect("backend re-emission preserves the complete native join")
    .into_direct()
    .expect("the same object and image request remain direct custody");
    assert_eq!(reproduced.identity(), expected_identity);
    let mut substituted = emission_request_parts(reproduced, replay_image_request);
    substituted.target = target::NativeTarget::windows_x64();
    let diagnostics = native_artifact::RequestedNativeArtifact::emit(substituted)
        .expect_err("the backend must replay, not trust the supplied target");
    // Physical replay encounters the unsupported hosted target before the
    // later object/image target equality check.
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("hosted builtin physical child requires a canonical supported target")),
        "{diagnostics:?}",
    );
}

fn emission_request_parts(
    artifact: native_artifact::NativeArtifact,
    image: image_emission::ExecutableImageEmissionRequest,
) -> native_artifact::NativeArtifactEmissionParts<image_emission::ExecutableImageEmissionRequest> {
    let parts = artifact.into_parts();
    native_artifact::NativeArtifactEmissionParts {
        target: parts.target,
        psi_artifact: parts.psi_artifact,
        object: parts.object,
        image,
        selected_provider_closure_report_identity: parts.selected_provider_closure_report_identity,
        selected_provider_closure_digest: parts.selected_provider_closure_digest,
        selected_provider_plans: parts.selected_provider_plans,
        provider_executions: parts.provider_executions,
        terminal_authority_policy_identity: parts.terminal_authority_policy_identity,
        terminal_authority_permission_policy_identity: parts
            .terminal_authority_permission_policy_identity,
        terminal_authority_closure_review: parts.terminal_authority_closure_review,
        boundary_application_coverage: parts.boundary_application_coverage,
        physical_evidence_scope: parts.physical_evidence_scope,
    }
}
