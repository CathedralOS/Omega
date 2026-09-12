use super::arrival_program;

#[test]
fn declared_receiver_return_ranges_compose_through_locals_and_fields() {
    let program = arrival_program(
        "data Alignment [copy] { case One; case Eight; }
         data Holder { alignment: Alignment; width: u64 [0..=8]; }
         machine Alignment::size(&self) -> u64 [1..=8] { 1 }
         machine consume(alignment: &Alignment) -> u64 [1..=8] {
             let width: u64 [1..=8] = alignment.size();
             width
         }
         machine Holder::update(&mut self) -> u64 [1..=8] {
             let before: u64 [1..=8] = self.alignment.size();
             self.width = self.alignment.size();
             let after: u64 [1..=8] = self.alignment.size();
             after
         }",
    );
    let result = crate::validate_program(&program);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn receiver_return_ranges_follow_selected_owner_not_method_spelling() {
    for (receiver, accepted) in [("small", true), ("large", false)] {
        let program = arrival_program(&format!(
            "data Small [copy] {{}}
             data Large [copy] {{}}
             machine Small::size(&self) -> u64 [1..=8] {{ 1 }}
             machine Large::size(&self) -> u64 [16..=32] {{ 16 }}
             machine consume(small: &Small, large: &Large) -> u64 {{
                 let width: u64 [1..=8] = {receiver}.size();
                 width
             }}"
        ));
        let result = crate::validate_program(&program);
        assert_eq!(result.is_ok(), accepted, "{receiver}: {result:?}");
        if !accepted {
            assert!(
                format!("{result:?}").contains(
                    "local `width` stores a value not provably within its declared range"
                )
            );
        }
    }
}

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
        let program = arrival_program(&format!(
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

#[test]
fn selected_receiver_result_preserves_scalar_kind() {
    let program = arrival_program(
        "data Value [copy] {}
         machine Value::read(&self) -> u64 { 1 }
         machine consume(value: &Value) -> bool { value.read() }",
    );
    let diagnostics = crate::validate_program(&program).expect_err("integer is not Boolean");
    let report = format!("{diagnostics:?}");
    assert!(report.contains("bool"), "{report}");
}
