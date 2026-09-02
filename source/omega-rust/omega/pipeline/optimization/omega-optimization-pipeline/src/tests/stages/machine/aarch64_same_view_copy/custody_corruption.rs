#[test]
fn every_generic_publication_custody_field_rejects_after_reauthentication() {
    super::super::post_allocation_custody_corruption::assert_every_field_rejects(
        super::staged_realization(
            omega_optimization_core::Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1,
        ),
    );
    super::super::post_allocation_custody_corruption::assert_every_field_rejects(
        super::staged_realization(
            omega_optimization_core::Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1,
        ),
    );
    super::super::post_allocation_custody_corruption::assert_every_field_rejects(
        super::staged_realization(
            omega_optimization_core::Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1,
        ),
    );
}
