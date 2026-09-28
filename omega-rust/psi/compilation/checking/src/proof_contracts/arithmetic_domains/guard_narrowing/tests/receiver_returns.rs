#[test]
fn receiver_results_require_declared_and_valid_bounds() {
    for (return_type, body, diagnostic) in [
        (
            "u64",
            "1",
            "local `width` stores a value not provably within its declared range",
        ),
        (
            "u64 [1..=8]",
            "9",
            "returns a value not provably within its declared range",
        ),
    ] {
        let program = crate::front_end::typed_program(&format!(
            "data Alignment [copy] {{}}
             machine Alignment::size(&self) -> {return_type} {{ {body} }}
             machine consume(alignment: &Alignment) -> u64 {{
                 let width: u64 [1..=8] = alignment.size();
                 width
             }}"
        ));
        let diagnostics = crate::validate_program(&program).expect_err("unproved range");
        assert!(
            format!("{diagnostics:?}").contains(diagnostic),
            "{diagnostics:?}"
        );
    }
}
