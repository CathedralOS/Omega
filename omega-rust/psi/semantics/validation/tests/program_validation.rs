use validation::{
    OpaqueDataPropertyReceipt, OpaquePropertyValidation, validate_specialized_program,
};

#[test]
fn validation_returns_the_operational_and_service_analyses_for_its_exact_program() {
    let program = crate::front_end::typed_program(
        "machine identity(input: u32) -> u32 { input }
         machine invoke(input: u32) -> u32 { identity(input) }",
    );
    let validated = validate_specialized_program(&program, OpaquePropertyValidation::Required(&[]))
        .expect("checked program");
    let operational = validation::infer_operational_may(&program);
    let service_reaches = validation::infer_service_reaches(&program, &operational);
    assert_eq!(validated.operational.machines().len(), 2);
    assert_eq!(validated.operational, operational);
    assert_eq!(validated.service_reaches, service_reaches);
}

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

#[test]
fn preliminary_validation_still_rejects_unsafe_scalar_operations() {
    let program =
        crate::front_end::typed_program("machine narrow(value: u32) -> u8 { value as u8 }");
    for mode in [
        OpaquePropertyValidation::PendingBuildSelection,
        OpaquePropertyValidation::Required(&[]),
    ] {
        let diagnostics = validate_specialized_program(&program, mode)
            .expect_err("pending properties do not disable scalar validation");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("not provably representable"))
        );
    }
}
