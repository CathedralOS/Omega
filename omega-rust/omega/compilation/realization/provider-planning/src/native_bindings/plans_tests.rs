// Locator normalization retains authored targets and refuses unsupported
// mechanisms before any provider selection or admission.

use super::*;
use crate::native_bindings::NativeBuildEvaluation;
use build_time_evaluation::native_bindings::NativeBindingSelection;
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use language_semantics::declaration_selection::{NativeBindingProducer, ProductTarget};
use semantic_vocabulary::{PackageKeyIdentity, WorkScheduleIdentity};
use std::sync::Arc;

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
            application: None,
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

#[test]
fn a_target_that_is_not_selected_still_refuses_its_invalid_locator() {
    let refused = normalize_native_binding(
        &supply(
            ProductTarget::WindowsX86_64,
            NativeBindingLocator::Syscall { number: 60 },
        ),
        REQUIREMENT,
        &mut Default::default(),
    )
    .expect_err("every authored target's locator is checked under that target");
    assert!(
        refused.contains("not applicable to its target"),
        "{refused:?}"
    );
}

#[test]
fn a_native_locator_receipt_retains_its_authored_target() {
    let binding = normalize_native_binding(
        &supply(
            ProductTarget::LinuxArm64,
            NativeBindingLocator::Syscall { number: 60 },
        ),
        REQUIREMENT,
        &mut Default::default(),
    )
    .expect("an authored target's valid locator retains its evaluation receipt");
    let crate::evaluated_via_bindings::EvaluatedNativeBinding::Syscall(binding) = binding else {
        panic!("the syscall locator must retain its mechanism");
    };
    assert_eq!(binding.target(), target::TargetProfile::LinuxArm64);
}
