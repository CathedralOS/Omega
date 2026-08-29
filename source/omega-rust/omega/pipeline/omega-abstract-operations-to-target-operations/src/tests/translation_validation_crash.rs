use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use omega_target::NativeTarget;
use omega_target_operations::{TargetOperation, TerminalPsiProvenance};
use psi_core::{
    BlockId, ClaimId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId,
    Proposition, ScalarTerm, ScalarType, ServiceId, StructuralDomainId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    CrashCause, CrashPredicateTerm, EntryClaim, SemanticFingerprint, StructuralAccess,
    StructuralMultiplicity, StructuralParameterDeclaration, TerminalPsiIdentity, VocabularyMarker,
};

use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError, StraightLineScalarCrashTranslationError,
    lower_to_target_operations, validate_abstract_to_target_translation,
};

fn guard_terms() -> Vec<CrashPredicateTerm> {
    vec![
        CrashPredicateTerm::new(Proposition::Equal(
            ScalarTerm::boolean(true),
            ScalarTerm::boolean(true),
        )),
        CrashPredicateTerm::new(Proposition::Equal(
            ScalarTerm::boolean(false),
            ScalarTerm::boolean(false),
        )),
    ]
}

fn crash_plan(cause: CrashCause, result_type: ScalarType) -> AbstractOperationPlan {
    let machine = MachineId::new(2_001).unwrap();
    let entry = BlockId::new(2_002).unwrap();
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0xc2; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(2_003).unwrap(),
                scalar_type: result_type,
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![AbstractOperation::Crash {
                psi_edge: EdgeId::new(2_004).unwrap(),
                cause,
                site_guard: guard_terms(),
                frontier_lower_bound: vec![
                    ClaimId::new(2_005).unwrap(),
                    ClaimId::new(2_006).unwrap(),
                ],
            }],
        }],
    }
}

fn base_plan() -> AbstractOperationPlan {
    crash_plan(CrashCause::Trap, ScalarType::Boolean)
}

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineScalarCrashTranslationError {
    let mut source = base_plan();
    let target = lower_to_target_operations(&source, NativeTarget::linux_x64()).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_scalar_crash::validate(
        &source.functions[0],
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineScalarCrashTranslationError {
    let source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut candidate);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineScalarCrash,
        error: AbstractToTargetTranslationFamilyError::StraightLineScalarCrash(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &candidate).unwrap_err()
    else {
        panic!("Crash-family corruption must fail at its independent validator")
    };
    error
}

#[test]
fn validates_exact_scalar_crash_on_every_native_target() {
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for cause in [CrashCause::Trap, CrashCause::Abort] {
            for result_type in [ScalarType::Boolean, integer] {
                let source = crash_plan(cause, result_type);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineScalarCrash(row),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("exact scalar Crash must publish one validated family row")
                };
                assert_eq!(row.machine(), source.entry);
                assert_eq!(row.result_type(), result_type);
                assert_eq!(row.crash_edge(), EdgeId::new(2_004).unwrap());
                assert_eq!(row.cause(), cause);
                assert_eq!(row.site_guard(), guard_terms());
                assert_eq!(
                    row.frontier_lower_bound(),
                    [ClaimId::new(2_005).unwrap(), ClaimId::new(2_006).unwrap()]
                );
            }
        }
    }
}

#[test]
fn parameterized_scalar_crash_remains_explicitly_uncovered() {
    let mut source = base_plan();
    source.functions[0].parameters.push(AbstractParameter {
        value: ValueId::new(2_100).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    assert_eq!(
        receipt.function_roster()[0].translation(),
        &AbstractToTargetFunctionTranslationDisposition::Uncovered
    );
}

#[test]
fn crash_source_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(2_101).unwrap(),
                scalar_type: ScalarType::Boolean,
            });
        }),
        StraightLineScalarCrashTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(2_102).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(2_103).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(2_104).unwrap()],
                });
        }),
        StraightLineScalarCrashTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| function.result = AbstractFunctionResult::Unit),
        StraightLineScalarCrashTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(2_105).unwrap(),
                input: PlaceId::new(2_106).unwrap(),
                path: Vec::new(),
            });
        }),
        StraightLineScalarCrashTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(2_107).unwrap());
        }),
        StraightLineScalarCrashTranslationError::SourcePublishedServices
    );
    for mutate in [
        |function: &mut AbstractFunction| function.block_entries.clear(),
        |function: &mut AbstractFunction| {
            function.block_entries[0].block = BlockId::new(2_108).unwrap()
        },
        |function: &mut AbstractFunction| {
            function.block_entries[0]
                .parameters
                .push(AbstractParameter {
                    value: ValueId::new(2_109).unwrap(),
                    scalar_type: ScalarType::Boolean,
                })
        },
        |function: &mut AbstractFunction| function.block_entries[0].operation_offset = 1,
    ] {
        assert_eq!(
            leaf_error(mutate),
            StraightLineScalarCrashTranslationError::SourceBlockRoster
        );
    }
    assert_eq!(
        leaf_error(|function| function.operations.clear()),
        StraightLineScalarCrashTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function.operations.insert(
                0,
                AbstractOperation::BooleanConstant {
                    psi_operation: OperationId::new(2_110).unwrap(),
                    result: ValueId::new(2_111).unwrap(),
                    value: true,
                },
            );
        }),
        StraightLineScalarCrashTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function.operations[0] = AbstractOperation::Return {
                psi_edge: EdgeId::new(2_004).unwrap(),
                result: ValueId::new(2_003).unwrap(),
                value: ValueId::new(2_112).unwrap(),
                scalar_type: ScalarType::Boolean,
                cleanup_actions: Vec::new(),
            };
        }),
        StraightLineScalarCrashTranslationError::SourceOperationRoster
    );
}

#[test]
fn crash_candidate_and_provenance_corruption_fails_closed() {
    for provenance in [
        TerminalPsiProvenance::default(),
        TerminalPsiProvenance {
            operations: vec![OperationId::new(2_113).unwrap()],
            edges: vec![EdgeId::new(2_004).unwrap()],
        },
        TerminalPsiProvenance {
            operations: Vec::new(),
            edges: vec![EdgeId::new(2_114).unwrap()],
        },
        TerminalPsiProvenance {
            operations: Vec::new(),
            edges: vec![EdgeId::new(2_004).unwrap(), EdgeId::new(2_115).unwrap()],
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| candidate.functions[0].provenance = provenance),
            StraightLineScalarCrashTranslationError::TargetProvenance
        );
    }
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].operation = TargetOperation::ReturnBooleanImmediate {
                psi_edge: EdgeId::new(2_004).unwrap(),
                source_value: ValueId::new(2_003).unwrap(),
                value: false,
            };
        }),
        StraightLineScalarCrashTranslationError::TargetOperation
    );
    for mutate in [
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash { psi_edge, .. } = operation else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(2_116).unwrap();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash { cause, .. } = operation else {
                unreachable!()
            };
            *cause = CrashCause::Abort;
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash { site_guard, .. } = operation else {
                unreachable!()
            };
            site_guard.pop();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash { site_guard, .. } = operation else {
                unreachable!()
            };
            site_guard.swap(0, 1);
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash { site_guard, .. } = operation else {
                unreachable!()
            };
            site_guard.push(CrashPredicateTerm::new(Proposition::Truth));
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash { site_guard, .. } = operation else {
                unreachable!()
            };
            site_guard[0] = CrashPredicateTerm::new(Proposition::Falsehood);
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash {
                frontier_lower_bound,
                ..
            } = operation
            else {
                unreachable!()
            };
            frontier_lower_bound.pop();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash {
                frontier_lower_bound,
                ..
            } = operation
            else {
                unreachable!()
            };
            frontier_lower_bound.swap(0, 1);
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash {
                frontier_lower_bound,
                ..
            } = operation
            else {
                unreachable!()
            };
            frontier_lower_bound.push(ClaimId::new(2_117).unwrap());
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::Crash {
                frontier_lower_bound,
                ..
            } = operation
            else {
                unreachable!()
            };
            frontier_lower_bound[0] = ClaimId::new(2_118).unwrap();
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| mutate(&mut candidate.functions[0].operation)),
            StraightLineScalarCrashTranslationError::TargetOperation
        );
    }
}
