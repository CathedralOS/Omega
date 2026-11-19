//! A build that has settled its final request audits that exact emission.

use compiler::stage_validators::{
    NativeValidationError, StageValidationResult, StageValidatorRequest,
};
use compiler::{
    Artifact, CompileRequest, CompilerRequestOutcome, NativeTarget, Program,
    TargetProviderInstallation,
};
use diagnostics::Diagnostic;
use project_build::{BuildHost, NextStage, build_with_validators};
use psi_authorization::{HostProducerTrust, authorize_terminal_artifact};
use std::sync::Arc;
use target::TargetProfile;
use terminal_codec::CanonicalTerminalArtifact;

struct FinalStage<'a>(Option<CompileRequest<'a>>);

impl BuildHost for FinalStage<'_> {
    type Plan = ();
    type Grants = ();
    type Services = ();

    fn core_plan(&mut self) -> Result<(), Vec<Diagnostic>> {
        Ok(())
    }

    fn admit(&mut self, (): (), (): ()) -> Result<NextStage<'_, (), ()>, Vec<Diagnostic>> {
        Ok(NextStage::Emit(self.0.take().expect("one final emission")))
    }

    fn interpret_plan(&mut self, _: compiler::PublishedPsi, (): ()) -> Result<(), Vec<Diagnostic>> {
        panic!("a final emission must not be executed as a planning program")
    }
}

#[test]
fn final_native_image_passes_checks_and_runtime_admission_obeys_applicability() {
    let bytes = include_bytes!(
        "../../../../../tests/fixtures/run/calls/scalar_contract_route_exit/linux_x86_64.psi"
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
    let program =
        || Program::Psi(authorize_terminal_artifact(Arc::clone(&artifact), Some(&trust)).unwrap());
    let native = CompileRequest::Native {
        program: program(),
        target: target(),
        entry_memory: &[],
    };
    let named =
        StageValidatorRequest::from_exact_names(["FixedFrameCorrespondence"], false).unwrap();
    let (product, rows) =
        build_with_validators(&mut FinalStage(Some(native)), (), 0, &named).unwrap();
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
        target: target(),
        entry_memory: &[],
    };
    let all = StageValidatorRequest::from_exact_names([], true).unwrap();
    let (admitted, rows) =
        build_with_validators(&mut FinalStage(Some(runtime())), (), 0, &all).unwrap();
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

    let Err(error) = build_with_validators(&mut FinalStage(Some(runtime())), (), 0, &named) else {
        panic!("runtime admission returned a product for an unavailable named check")
    };
    assert!(matches!(error, NativeValidationError::Unavailable(_)));
    assert_eq!(
        CompilerRequestOutcome::of_stage_validation(&error).halt_value(),
        1
    );
}
