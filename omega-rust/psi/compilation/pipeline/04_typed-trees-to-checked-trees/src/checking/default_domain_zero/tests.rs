use super::refine_default_domain_zero;

#[test]
fn zero_refinement_uses_the_landed_format_and_complete_predicate() {
    for (predicate, expected_gated) in [
        ("value >= 0.0 && value <= 1.0", false),
        ("value > 0.0", true),
        ("value == 1e-50", false),
        ("value < 1e-50", true),
        ("value >= 0.0 && false", true),
    ] {
        let mut program = crate::tests::front_end::typed_program(&format!(
            "data Reading where {predicate} {{ value: f32; }}"
        ));
        assert!(program.data_definitions()[0].zero_gated);
        assert_eq!(
            validation::data_predicates_admit_zero(&program, &program.data_definitions()[0]),
            !expected_gated,
            "early establishment and checked refinement agree: {predicate}",
        );
        validation::land_float_literal_destinations(&mut program);
        refine_default_domain_zero(&mut program);
        assert_eq!(
            program.data_definitions()[0].zero_gated,
            expected_gated,
            "{predicate}"
        );
    }
}

#[test]
fn zero_refinement_cannot_clear_unknown_or_case_obligations() {
    for source in [
        "data Reading where value >= 0.0 && flag { value: f32; flag: bool; }",
        "data Reading where value >= 0.0 { value: f32; case Empty; case Full; }",
        "domain u64::Positive requires self > 0;
         data Reading where value >= 0.0, count in u64::Positive, { value: f32; count: u64; }",
        "boundary machine >= Other::greater(left: f32, right: f32) -> bool;
         data Reading where value >= 0.0 { value: f32; }",
    ] {
        let mut program = crate::tests::front_end::typed_program(source);
        validation::land_float_literal_destinations(&mut program);
        refine_default_domain_zero(&mut program);
        assert!(program.data_definitions()[0].zero_gated, "{source}");
    }
}
