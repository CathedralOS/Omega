// Unit tests for Build-supplied native binding locator checking across selected
// and unselected targets.

use super::*;
use crate::native_bindings::NativeBuildEvaluation;
use build_time_evaluation::native_bindings::NativeBindingSelection;
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use language_semantics::declaration_selection::{NativeBindingProducer, ProductTarget};
use semantic_vocabulary::{PackageKeyIdentity, WorkScheduleIdentity};

const REQUIREMENT: &str = "Clock::now";

fn supply(target: ProductTarget, locator: NativeBindingLocator) -> NativeBuildSupply {
    let package = PackageKeyIdentity::from_digest([3; 32]).unwrap();
    let measurement = EvaluationMeasurementIdentity::TerminalPsi {
        work_schedule: WorkScheduleIdentity::new(1).unwrap(),
        storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
    };
    let limits = BuildEvaluationSponsorLimits::new(100, 100, 8, 8, 100, 100, 100, 100).unwrap();
    let sponsor = BuildEvaluationSponsor::new_with_measurement(limits, measurement).unwrap();
    let usage = *sponsor
        .begin_invocation(measurement, 100)
        .unwrap()
        .snapshot()
        .unwrap()
        .record();
    NativeBuildSupply {
        selection: NativeBindingSelection {
            application_identity: None,
            subject: Default::default(),
            requirement: Default::default(),
            target,
            locator,
            producer: NativeBindingProducer {
                package,
                declaration_path: "build".into(),
                callable_identity: "build::bind_clock".into(),
            },
            source_occurrence_digest: [5; 32],
            supplier_package: package,
            source_span: Default::default(),
        },
        evaluation: Arc::new(NativeBuildEvaluation {
            terminal_code: terminal_psi::TerminalPsiIdentity {
                vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([7; 32]),
            },
            usage,
            observation_digest: [9; 32],
        }),
    }
}

fn prepared(supplies: &[NativeBuildSupply]) -> PreparedNativeBuildBindings {
    PreparedNativeBuildBindings {
        rows: supplies
            .iter()
            .map(|supply| PreparedNativeBuildBinding {
                selection: supply.selection.clone(),
                requirement: Ok(PreparedNativeRequirement {
                    owner: Default::default(),
                    signature: Default::default(),
                    identity: REQUIREMENT.into(),
                    method: "now".into(),
                    surface: None,
                }),
            })
            .collect(),
    }
}

fn form(supplies: &[NativeBuildSupply]) -> Result<Vec<DerivedProviderPlan>, Vec<Diagnostic>> {
    prepared(supplies).form(
        supplies,
        Some(target::TargetProfile::LinuxX64),
        &mut Default::default(),
    )
}

#[test]
fn a_target_that_is_not_selected_still_refuses_its_invalid_locator() {
    let refused = form(&[supply(
        ProductTarget::WindowsX86_64,
        NativeBindingLocator::Syscall { number: 60 },
    )])
    .expect_err("every authored target's locator is checked under that target");
    assert!(
        refused
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not applicable to its target")),
        "{refused:?}"
    );
}

#[test]
fn a_valid_locator_for_a_target_that_is_not_selected_forms_no_candidate() {
    let plans = form(&[supply(
        ProductTarget::LinuxArm64,
        NativeBindingLocator::Syscall { number: 60 },
    )])
    .expect("a valid locator on another target is checked, then left unselected");
    assert!(plans.is_empty());
}
