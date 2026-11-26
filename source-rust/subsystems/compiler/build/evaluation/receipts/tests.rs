//! Construction, identity and storage of a completed Build evaluation.
use super::{DependencyPreparations, NativeBuildEvaluation};
use semantic_vocabulary::PackageKeyIdentity;
use std::sync::Arc;

fn program(byte: u8) -> psi_program_identity::TerminalPsiIdentity {
    psi_program_identity::TerminalPsiIdentity {
        vocabulary_marker: psi_program_identity::VocabularyMarker::CURRENT,
        program_fingerprint: psi_program_identity::SemanticFingerprint::from_bytes([byte; 32]),
    }
}

fn single_stage_receipt() -> NativeBuildEvaluation {
    let measurement = evaluation_contracts::EvaluationMeasurementIdentity::current();
    let sponsor = evaluation_accounting::BuildEvaluationSponsor::new_with_measurement(
        evaluation_contracts::BuildEvaluationSponsorLimits::new(
            100, 100, 100, 100, 100, 100, 100, 100,
        )
        .unwrap(),
        measurement,
    )
    .unwrap();
    let usage = *sponsor
        .begin_invocation(measurement, 100)
        .unwrap()
        .snapshot()
        .unwrap()
        .record();
    NativeBuildEvaluation::from_observation_digest(program(9), usage, [8; 32]).unwrap()
}

#[test]
fn every_receipt_records_its_dependency_preparations_and_checks_usage() {
    let receipt = single_stage_receipt();
    let bytes = record_storage::encode(&receipt, Default::default()).unwrap();
    assert_eq!(
        record_storage::decode::<NativeBuildEvaluation>(&bytes, Default::default()).unwrap(),
        receipt
    );
    let mut invalid = receipt.clone();
    invalid.usage.usage_schema_version = 0;
    let invalid = record_storage::encode(&invalid, Default::default()).unwrap();
    assert!(record_storage::decode::<NativeBuildEvaluation>(&invalid, Default::default()).is_err());
    let incomplete = record_storage::encode(
        &(
            receipt.terminal_code,
            receipt.usage,
            receipt.observation_digest,
        ),
        Default::default(),
    )
    .unwrap();
    assert!(
        record_storage::decode::<NativeBuildEvaluation>(&incomplete, Default::default()).is_err()
    );
}

#[test]
fn preparing_a_product_binds_the_child_build_without_selecting_a_product_handoff() {
    let single = single_stage_receipt();
    let receipt = |child| {
        let mut preparations = DependencyPreparations::default();
        preparations.prepared(
            source_provenance::DependencyScope::Product,
            0,
            PackageKeyIdentity::from_digest([1; 32]).unwrap(),
            [2; 32],
            child,
            None,
        );
        NativeBuildEvaluation::from_execution(
            single.terminal_code,
            Arc::new(preparations),
            single.usage,
            single.observation_digest,
        )
        .unwrap()
    };
    let actual = receipt(Some([3; 32]));
    for other in [receipt(Some([4; 32])), receipt(None)] {
        assert_ne!(actual.identity_digest(), other.identity_digest());
    }
}

#[test]
fn receipt_identity_distinguishes_the_checked_program_and_its_preparations() {
    let plain = single_stage_receipt();
    let receipt = |code, dependency, sources, generated, reverse: bool| {
        let mut preparations = DependencyPreparations::default();
        let prepare = |preparations: &mut DependencyPreparations, ordinal, package| {
            preparations.prepared(
                source_provenance::DependencyScope::Build,
                ordinal,
                PackageKeyIdentity::from_digest([package; 32]).unwrap(),
                [sources; 32],
                Some([3; 32]),
                Some([generated; 32]),
            );
        };
        if reverse {
            prepare(&mut preparations, 1, 7);
        }
        prepare(&mut preparations, 0, dependency);
        if !reverse {
            prepare(&mut preparations, 1, 7);
        }
        NativeBuildEvaluation::from_execution(
            program(code),
            Arc::new(preparations),
            plain.usage,
            plain.observation_digest,
        )
        .unwrap()
    };
    let actual = receipt(9, 3, 4, 5, false);
    for other in [
        receipt(2, 3, 4, 5, false),
        receipt(9, 4, 4, 5, false),
        receipt(9, 3, 5, 5, false),
        receipt(9, 3, 4, 6, false),
        receipt(9, 3, 4, 5, true),
        plain.clone(),
    ] {
        assert_ne!(actual.identity_digest(), other.identity_digest());
    }
    let stored = record_storage::encode(&actual, Default::default()).unwrap();
    assert_eq!(
        record_storage::decode::<NativeBuildEvaluation>(&stored, Default::default()).unwrap(),
        actual
    );
}
