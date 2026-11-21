//! A saved program is validated against the exact target and requested emission.

use compiler::stage_validators::{
    NativeValidationError, StageValidationResult, StageValidatorRequest,
};
use compiler::{
    Artifact, CompileRequest, CompilerRequestOutcome, NativeTarget, Program,
    TargetProviderInstallation,
};
use psi_authorization::{HostProducerTrust, authorize_terminal_artifact};
use std::sync::Arc;
use target::TargetProfile;
use terminal_codec::CanonicalTerminalArtifact;

#[test]
fn saved_psi_requests_do_not_store_the_parser_tables_inline() {
    let parsed = size_of::<source_compiler::ParsedStageSources>();
    assert!(size_of::<Program<'_>>() < parsed);
    assert!(size_of::<CompileRequest<'_>>() < parsed);
}

#[test]
fn final_native_image_passes_checks_and_runtime_admission_obeys_applicability() {
    let bytes = include_bytes!(
        "../../../../tests/fixtures/calls/run/scalar_contract_route_exit/linux_x86_64.psi"
    );
    let artifact = Arc::new(CanonicalTerminalArtifact::from_bytes(bytes).unwrap());
    let trust = HostProducerTrust::accept("published scalar contract route fixture");
    let profile = TargetProfile::LinuxX64;
    let settings = Default::default();
    let target = || NativeTarget {
        profile,
        installation: TargetProviderInstallation::new(
            artifact.manifest().identity(),
            profile,
            Default::default(),
        ),
        requested_boundaries: vec![],
        application: Default::default(),
        optimizations: &settings,
        behavior_exclusions: &[],
        x86_scalar_fma_provider: None,
    };
    let program = || Program::Psi {
        program: authorize_terminal_artifact(Arc::clone(&artifact), Some(&trust)).unwrap(),
        target: target(),
    };
    let native = CompileRequest::Native {
        program: program(),
        entry_memory: &[],
    };
    let named =
        StageValidatorRequest::from_exact_names(["FixedFrameCorrespondence"], false).unwrap();
    let compiler::CompileOutput {
        artifact: product,
        validations: rows,
    } = compiler::compile(
        native,
        compiler::CompileOptions {
            validators: Some(&named),
            ..Default::default()
        },
    )
    .unwrap();
    let Artifact::Native(product) = product else {
        panic!("expected the requested native product")
    };
    assert_eq!(
        product
            .retained_native_artifact()
            .unwrap()
            .psi_artifact()
            .manifest()
            .identity(),
        artifact.manifest().identity()
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].result, StageValidationResult::Passed);
    assert_eq!(rows[0].target.as_deref(), Some(profile.target_name()));

    let runtime = || CompileRequest::Runtime {
        program: program(),
        entry_memory: &[],
    };
    let all = StageValidatorRequest::from_exact_names([], true).unwrap();
    let compiler::CompileOutput {
        artifact: admitted,
        validations: rows,
    } = compiler::compile(
        runtime(),
        compiler::CompileOptions {
            validators: Some(&all),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(admitted, Artifact::Runtime(_)));
    assert_eq!(rows.len(), 3);
    assert!(
        rows.iter()
            .all(|row| row.target.as_deref() == Some(profile.target_name())
                && row.result
                    == StageValidationResult::Inapplicable(
                        "runtime admission stops before target lowering"
                    ))
    );

    let Err(error) = compiler::compile(
        runtime(),
        compiler::CompileOptions {
            validators: Some(&named),
            ..Default::default()
        },
    ) else {
        panic!("runtime admission returned a product for an unavailable named check")
    };
    assert!(matches!(error, NativeValidationError::Unavailable(_)));
    assert_eq!(
        CompilerRequestOutcome::of_stage_validation(&error).halt_value(),
        1
    );
}
