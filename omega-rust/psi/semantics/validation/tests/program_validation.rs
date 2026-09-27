use validation::{
    OpaqueDataPropertyReceipt, OpaquePropertyValidation, validate_specialized_program,
};

#[test]
fn preliminary_opaque_evidence_cannot_satisfy_final_validation() {
    let program = crate::front_end::typed_program("boundary data Token [copy]; machine entry() {}");
    validate_specialized_program(&program, OpaquePropertyValidation::PendingBuildSelection)
        .expect("build checkpoint may leave copy evidence pending");
    let missing = validate_specialized_program(&program, OpaquePropertyValidation::Required(&[]))
        .expect_err("final validation still requires evidence");
    assert!(missing.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("without an admitted property receipt")
    }));
    validation::validate_program(&program)
        .expect_err("standalone validation also requires evidence");
    let token = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Token")
        .expect("Token")
        .symbol;
    let receipt = OpaqueDataPropertyReceipt::copy(token);
    validate_specialized_program(&program, OpaquePropertyValidation::Required(&[receipt]))
        .expect("exact receipt admits copy");
    validate_specialized_program(
        &program,
        OpaquePropertyValidation::Required(&[receipt, receipt]),
    )
    .expect_err("duplicate receipts remain invalid");
}
