//! Carrier admission fault controls; the source-produced Product inspection
//! fixture exercises the same result through an admitted Terminal Build.

use super::*;
use crate::calls::frames::tests::empty_program;
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use std::sync::Arc;
use terminal_psi::{BoundaryStructuralResultDeclaration, ByteSequenceCarrier, StructuralAccess};

#[test]
fn immutable_host_bytes_retain_accounted_snapshot_until_last_copy_drops() {
    let measurement = EvaluationMeasurementIdentity::TerminalPsi {
        fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
    };
    let sponsor = BuildEvaluationSponsor::new_with_measurement(
        BuildEvaluationSponsorLimits::new(100, 8, 2, 2, 100, 4, 100, 100).unwrap(),
        measurement,
    )
    .unwrap();
    let account = sponsor.begin_invocation(measurement, 100).unwrap();
    let mut input = *b"path";
    let bytes = TerminalByteSequenceValue::copy(&input, Some(&account)).unwrap();
    input[0] = b'X';
    assert_eq!(&input, b"Xath");
    let retained = bytes.clone();
    drop(bytes);
    assert_eq!(retained.0.bytes(), b"path");
    assert_eq!(sponsor.live_payload_bytes(), 4);
    assert!(TerminalByteSequenceValue::copy(b"x", Some(&account)).is_err());
    assert_eq!(sponsor.live_payload_bytes(), 4);
    drop(retained);
    assert_eq!(sponsor.live_payload_bytes(), 0);
    let unaccounted = TerminalByteSequenceValue::copy(b"path", None).unwrap();
    let adopted = unaccounted.0.into_account(Some(&account)).unwrap();
    assert_eq!(adopted.bytes(), b"path");
    assert_eq!(sponsor.live_payload_bytes(), 4);
    assert!(
        TerminalByteSequenceValue::copy(b"x", None)
            .unwrap()
            .0
            .into_account(Some(&account))
            .is_err()
    );
    drop(adopted);
    assert_eq!(sponsor.live_payload_bytes(), 0);
}

#[test]
fn host_byte_results_require_exact_shared_carrier_and_real_storage() {
    for access in [
        Some(StructuralAccess::SharedBorrow),
        Some(StructuralAccess::MutableBorrow),
        None,
    ] {
        for opaque in [false, true] {
            let structural_type = StructuralTypeId::new(1).unwrap();
            let mut program = empty_program();
            Arc::get_mut(&mut program)
                .unwrap()
                .fixture_module_mut()
                .structural_types
                .push(StructuralTypeDeclaration {
                    quotient_carrier: None,
                    id: structural_type,
                    identity: terminal_psi::TerminalIdentityIndex(0),
                    owns_linear_claim: false,
                    copyable: true,
                    no_code_affine_cleanup: true,
                    descriptor_cleanup: true,
                    shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                        access,
                    }),
                    scalar_restriction: None,
                });
            let mut execution = TerminalExecution::start_with_program(
                program,
                &[],
                &[],
                &[],
                &[],
                &[],
                &[],
                &[],
                &[],
                None,
                None,
            )
            .unwrap();
            let declaration = terminal_psi::StructuralOperationResult {
                place: PlaceId::new(1).unwrap(),
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            };
            let expected = BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
                structural_type,
                multiplicity: declaration.multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
            let response = if opaque {
                TerminalEffectResult::Structural(TerminalStructuralValue::minted(
                    7,
                    structural_type,
                    Vec::new(),
                ))
            } else {
                TerminalEffectResult::ByteSequence(
                    TerminalByteSequenceValue::copy(b"path", None).unwrap(),
                )
            };
            let accepted = execution.commit_boundary_result(
                &OperationResult::Structural(declaration.clone()),
                &expected,
                response,
            );
            let valid = access == Some(StructuralAccess::SharedBorrow) && !opaque;
            assert_eq!(
                accepted.is_ok(),
                valid,
                "access={access:?}, opaque={opaque}, result={accepted:?}"
            );
            assert_eq!(
                execution
                    .byte_sequence_values
                    .contains_key(&declaration.place),
                valid
            );
            assert_eq!(
                execution.structural_values.contains_key(&declaration.place),
                valid
            );
            if valid {
                let bytes = execution
                    .byte_sequence_values
                    .get(&declaration.place)
                    .unwrap()
                    .immutable()
                    .unwrap();
                assert_eq!(bytes.bytes(), b"path");
                let repeated = TerminalEffectResult::ByteSequence(
                    TerminalByteSequenceValue::copy(b"other", None).unwrap(),
                );
                assert!(
                    execution
                        .commit_boundary_result(
                            &OperationResult::Structural(declaration.clone()),
                            &expected,
                            repeated
                        )
                        .is_err()
                );
                assert_eq!(
                    execution
                        .byte_sequence_values
                        .get(&declaration.place)
                        .unwrap()
                        .immutable()
                        .unwrap()
                        .bytes(),
                    b"path"
                );
            }
        }
    }
}
