use super::*;

pub(super) fn guard_terms() -> Vec<CrashPredicateTerm> {
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

pub(super) fn crash_plan(cause: CrashCause, result_type: ScalarType) -> AbstractOperationPlan {
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

pub(super) fn base_plan() -> AbstractOperationPlan {
    crash_plan(CrashCause::Trap, ScalarType::Boolean)
}

pub(super) fn leaf_error(
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

pub(super) fn candidate_error(
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
