//! Canonical Terminal module decoding is the compiler's most expensive
//! repeated admission step: `terminal_codec::decode_module` replays the scalar
//! qualification, canonical ordering, structural foundation and module
//! representation validations and then re-encodes the module to reject any
//! non-canonical byte sequence. Stage 00, the backend plan, the object custody
//! and the native artifact each need the same module, and each used to decode
//! the same bytes again to get it.
//!
//! This witness pins zero decodes during retained native realization and runtime
//! execution, including provider installation. The artifact retains its immutable module
//! and lends a sealed view with the corresponding identity. External ingress
//! still decodes; crossing between consumers inside one compile does not.
//!
//! The counter is process-global, so this target holds exactly one test.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct, compile};
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
fn realization_and_execution_borrow_the_retained_terminal_module() {
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
        proposal
            .package_terminal_authority_permission_identities()
            .clone(),
    )
    .expect("package permission policy");
    let mut permission_identities = proposal
        .package_terminal_authority_permission_identities()
        .clone()
        .into_builder();
    let package_permissions = proposal.package_terminal_authority_permissions().to_vec();
    let mut minted_rows = package_permissions;
    for plan in proposal.providers().selection.plans() {
        for row in &plan.rows {
            minted_rows.push(
                provider_admission::TerminalAuthorityPermissionPolicyRow::new(
                    plan.schema.identity_digest(),
                    permission_identities.intern(plan.identities.text(row.requirement_identity)),
                    effects::TerminalAuthorityDisposition::from_classes([]),
                ),
            );
        }
    }
    let permission_rows = minted_rows.into_iter().fold(
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
    );
    let permission_policy = provider_admission::terminal_authority_permission_policy_with_rows(
        permission_rows,
        permission_identities.build(),
    )
    .expect("permission policy");
    let image_request =
        image_emission::ExecutableImageEmissionRequest::direct(proposal.application().subsystem);

    // Everything above is Psi production and proposal retention. The
    // measurement starts here so it attributes only the realization.
    let before = terminal_codec::module_decode_count();
    let product = native_production::compile(
        retained,
        native_production::NativeCompileRequest {
            private_entry_ram: &[],
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

    let before_execution = terminal_codec::module_decode_count();
    // Possessing this same artifact, its selected provider and established
    // root Binding does not delegate containing-domain death to an embedding.
    let denied = compiler::runtime::execute(compiler::runtime::TerminalRuntimeRequest {
        provider_proposal: None,
        artifact: artifact.psi_artifact(),
        target: None,
        provisions: product.runtime_provisions.as_ref(),
        fuel_allowance: 100_000,
        domain_exit: compiler::runtime::TerminalDomainExitDelegation::default(),
        standard_input: None,
        clock: None,
    });
    assert!(
        matches!(
            denied,
            Err(compiler::runtime::TerminalRuntimeError::MissingProvision {
                capability: "process-exit-domain-delegation",
                ..
            })
        ),
        "embedding without host delegation must refuse before execution"
    );
    let observation = compiler::runtime::execute(compiler::runtime::TerminalRuntimeRequest {
        provider_proposal: None,
        artifact: artifact.psi_artifact(),
        target: None,
        provisions: product.runtime_provisions.as_ref(),
        fuel_allowance: 100_000,
        domain_exit: compiler::runtime::TerminalDomainExitDelegation::IsolatedContainingDomain,
        standard_input: None,
        clock: None,
    })
    .expect("retained provider program executes through the product runtime");
    assert!(
        matches!(observation.status,
            terminal_interpreter::TerminalExecutionStatus::ExternalCompletion(ref completion)
            if completion.return_code == 70
        ),
        "unexpected outcome: {:?}",
        observation.status
    );
    assert_eq!(
        terminal_codec::module_decode_count() - before_execution,
        0,
        "retained runtime execution must not reload Terminal bytes"
    );

    exercise_owned_and_borrowed_instances();
}

fn exercise_owned_and_borrowed_instances() {
    use terminal_fuel::TerminalFuelMeter;
    use terminal_interpreter::{
        TerminalExecutionResult, TerminalExecutionStatus, TerminalProgram, TerminalStructuralInputs,
    };

    let root = repo_root()
        .join("tests/omega/pass/optimizer/rollback_to_no_selection_empty_entry/main.omg");
    let prepared = reviewed_repository_fixture_package_inputs(&root, Some("linux_x86_64"))
        .unwrap_or_else(|diagnostics| panic!("empty-entry package inputs: {diagnostics:#?}"));
    let mut request = CompileRequest::new(CompileOptions {
        root_path: prepared.root_path,
        build_dir: Some(
            std::env::temp_dir().join(format!("omega-terminal-storage-{}", std::process::id())),
        ),
        target_name: Some("linux_x86_64".to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    if let Some(package_inputs) = prepared.package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let report = compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("ordinary empty entry produces Terminal");
    let retained = report
        .into_retained_terminal_artifact()
        .expect("retained Terminal");
    let before = terminal_codec::module_decode_count();
    let borrowed = TerminalProgram::borrow(retained.artifact().view());
    let module = retained.artifact().semantic_module();
    // Stand in for the producer's owned module, then track its allocation
    // across publication. Borrowing after a hidden clone is not sufficient.
    let producer_module = module.clone();
    let machine_storage = producer_module.machines.as_ptr();
    let published = terminal_codec::CanonicalTerminalArtifact::from_parts(
        producer_module,
        retained.artifact().optimization().clone(),
        None,
    )
    .expect("owned publication");
    assert_eq!(
        machine_storage,
        published.semantic_module().machines.as_ptr()
    );
    assert_eq!(
        published.semantic_bytes(),
        retained.artifact().semantic_bytes()
    );
    let (transferred_module, _) = published.into_semantics();
    assert_eq!(machine_storage, transferred_module.machines.as_ptr());
    let terminal_psi::TargetMachineCallable::Machine(entry) = module.entry else {
        panic!("this fixture has an ordinary machine entry");
    };
    let entry = module.machine(entry).unwrap();
    assert!(
        std::ptr::eq(borrowed.entry().unwrap().result, &entry.result),
        "execution descriptions must borrow the original declarations"
    );
    let mut first = borrowed
        .instantiate(&[], TerminalStructuralInputs::default())
        .unwrap();
    let mut second = borrowed
        .instantiate(&[], TerminalStructuralInputs::default())
        .unwrap();
    drop(borrowed);
    assert!(matches!(
        first
            .invoke(&mut TerminalFuelMeter::with_allowance(0))
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert!(!second.is_finished());
    assert_eq!(
        second.invoke(&mut TerminalFuelMeter::unbounded()).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        first.invoke(&mut TerminalFuelMeter::unbounded()).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(terminal_codec::module_decode_count() - before, 0);
    drop(first);
    drop(second);

    let envelope = retained.artifact().to_bytes();
    let restored = terminal_codec::CanonicalTerminalArtifact::from_bytes(&envelope)
        .expect("canonical envelope round trip");
    assert_eq!(restored.to_bytes(), envelope);
    assert_eq!(&restored, retained.artifact());
    assert!(
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&envelope[..envelope.len() - 1])
            .is_err()
    );
    drop(restored);
    let before_load = terminal_codec::module_decode_count();
    let owned = TerminalProgram::load_envelope(&envelope).expect("owned external load");
    assert_eq!(terminal_codec::module_decode_count() - before_load, 1);
    drop(envelope);
    drop(retained);
    assert!(
        owned
            .instantiate(
                &[terminal_interpreter::TerminalScalarValue::Boolean(true)],
                TerminalStructuralInputs::default()
            )
            .is_err(),
        "wrong entry arguments must reject"
    );
    let mut instance = owned
        .instantiate(&[], TerminalStructuralInputs::default())
        .unwrap();
    drop(owned);
    assert_eq!(
        instance
            .invoke(&mut TerminalFuelMeter::unbounded())
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(TerminalProgram::load_envelope(b"not a Terminal artifact").is_err());
}
