use super::super::{
    ExternalDecisionReplayError, ExternalDecisionSchemaError, Optimization, OptimizationRunError,
    OptimizationSelections, budget, replay_psi_pipeline, run_psi_pipeline, verified_exact_add_unit,
};

#[test]
fn external_replay_byte_boundary_rejects_exact_duplicate_and_v1_log() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let baseline = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();

    let mut duplicated = baseline.external_decisions().encode();
    const LOG_POINT_COUNT_OFFSET: usize = 8 + 4 + 32 + 7 * 32;
    const LOG_POINTS_OFFSET: usize = LOG_POINT_COUNT_OFFSET + 4;
    let framed_point = duplicated[LOG_POINTS_OFFSET..].to_vec();
    duplicated[LOG_POINT_COUNT_OFFSET..LOG_POINTS_OFFSET].copy_from_slice(&2_u32.to_le_bytes());
    duplicated.extend_from_slice(&framed_point);
    assert!(matches!(
        replay_psi_pipeline(
            verified_exact_add_unit(),
            &selections,
            budget(8),
            &duplicated,
        ),
        Err(OptimizationRunError::ExternalDecisionReplay(
            ExternalDecisionReplayError::Schema(
                ExternalDecisionSchemaError::DuplicateDecisionPoint
            )
        ))
    ));

    let mut v1 = baseline.external_decisions().encode();
    v1[8..12].copy_from_slice(&1_u32.to_le_bytes());
    assert!(matches!(
        replay_psi_pipeline(verified_exact_add_unit(), &selections, budget(8), &v1,),
        Err(OptimizationRunError::ExternalDecisionReplay(
            ExternalDecisionReplayError::Schema(
                ExternalDecisionSchemaError::UnsupportedLogVersion(1)
            )
        ))
    ));
}
