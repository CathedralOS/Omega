//! Canonical Terminal module decoding is the compiler's most expensive
//! repeated admission step: `terminal_codec::decode_module` replays the scalar
//! qualification, canonical ordering, structural foundation and module
//! representation validations and then re-encodes the module to reject any
//! non-canonical byte sequence. Stage 00, the backend plan, the object custody
//! and the native artifact each need the same module, and each used to decode
//! the same bytes again to get it.
//!
//! This witness pins zero decodes during retained native realization,
//! including provider installation. The artifact retains its immutable module
//! and lends a sealed view with the corresponding identity. External ingress
//! still decodes; crossing between consumers inside one compile does not.
//!
//! The counter is process-global, so this target holds exactly one test.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{
    CompileOptions, CompileRequest, RequestedCompileProduct, RetainedNativeRealizationRequest,
    compile, realize_retained_native_artifact,
};
use fixture_package_inputs::{repo_root, reviewed_repository_fixture_package_inputs};
use std::fs;
use std::path::PathBuf;
use terminal_psi_to_abstract_operations::provider_admission;

/// The fixture realizes a checked `boundary requirement` whose provider is an
/// in-module machine, so the realization runs the whole native admission:
/// canonical replay, entry settlement, service establishment, provider
/// installation, object custody, physical evidence and image emission. Every
/// one of those used to decode the artifact's semantic section for itself.
const FIXTURE: &str =
    "tests/omega/pass/providers/checked_boundary_requirement_dispatch_exit/main.omg";

#[test]
fn native_realization_borrows_the_retained_terminal_module() {
    let target_name = "linux_x86_64";
    let main = repo_root().join(FIXTURE);
    let prepared = reviewed_repository_fixture_package_inputs(&main, Some(target_name))
        .unwrap_or_else(|diagnostics| panic!("{FIXTURE} package inputs: {diagnostics:#?}"));
    let root: PathBuf =
        std::env::temp_dir().join(format!("omega-terminal-decode-once-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    // The root package is bound to the sealed snapshot staged for this
    // fixture, so the compilation reads its root from the snapshot too.
    let mut request = CompileRequest::new(CompileOptions {
        root_path: prepared.root_path,
        build_dir: Some(root),
        target_name: Some(target_name.to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    if let Some(package_inputs) = prepared.package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let retained = compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| panic!("{FIXTURE} must reach Terminal: {diagnostics:#?}"))
        .into_retained_terminal_artifact()
        .unwrap_or_else(|| panic!("{FIXTURE} retains its native proposal"));
    let proposal = retained
        .native_realization_proposal()
        .unwrap_or_else(|| panic!("{FIXTURE} native proposal"));
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

    // Everything above is Psi production and proposal retention. The
    // measurement starts here so it attributes only the realization.
    let before = terminal_codec::module_decode_count();
    let product = realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
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
        panic!("{FIXTURE} must realize its native artifact: {diagnostics:#?}")
    });
    let realization_decodes = terminal_codec::module_decode_count() - before;
    let native_artifact::RequestedNativeArtifact::Direct(artifact) = product.artifact else {
        panic!("{FIXTURE} emits a direct native artifact")
    };

    // Replaying the finished artifact interrogates its retained Terminal
    // module again — image replay, physical evidence, proof sidecar — and
    // decodes nothing, because the artifact already holds that decoding.
    let before_replay = terminal_codec::module_decode_count();
    artifact
        .validate()
        .unwrap_or_else(|error| panic!("{FIXTURE} artifact replay: {error}"));
    assert_eq!(
        terminal_codec::module_decode_count() - before_replay,
        0,
        "replaying a realized artifact must read its retained module, not decode its bytes again"
    );

    assert_eq!(
        realization_decodes, 0,
        "native realization, including provider installation, must borrow the retained artifact \
         rather than decode its semantic bytes again"
    );
}
