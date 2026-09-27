use validation::{
    OpaqueDataPropertyReceipt, OpaquePropertyValidation, validate_specialized_program,
};

#[test]
fn duplicate_declarations_preserve_call_free_local_range_facts() {
    for (duplicate, value, intervening, range_error) in [
        (false, 7, "let other: u64 = 0;", false),
        (true, 7, "let other: u64 = 0;", false),
        (false, 300, "let other: u64 = 0;", true),
        (true, 300, "let other: u64 = 0;", true),
        (true, 7, "let other: u64 = opaque();", true),
        (true, 7, "opaque_unit();", true),
    ] {
        let declarations = if duplicate {
            "data Duplicate {} data Duplicate {}"
        } else {
            "data Duplicate {}"
        };
        let source = format!(
            r#"
            {declarations}
            machine opaque() -> u64 {{ 0 }}
            machine opaque_unit() {{}}
            machine entry() -> bool {{
                let value: u64 = {value};
                {intervening}
                transition {{ _ -> narrow(value) }}
                state narrow(value: u64 [0..=254]) -> bool {{ true }}
            }}
        "#
        );
        let program = crate::front_end::typed_program(&source);
        let diagnostics = validation::validate_program(&program)
            .err()
            .unwrap_or_default();
        let messages = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            messages
                .iter()
                .any(|message| message.contains("duplicate data")),
            duplicate,
            "{source}\n{messages:?}"
        );
        assert_eq!(
            messages
                .iter()
                .any(|message| message.contains("not provably within its declared range")),
            range_error,
            "{source}\n{messages:?}"
        );
        assert_eq!(
            messages.len(),
            usize::from(duplicate) + usize::from(range_error),
            "{source}\n{messages:?}"
        );
    }
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
