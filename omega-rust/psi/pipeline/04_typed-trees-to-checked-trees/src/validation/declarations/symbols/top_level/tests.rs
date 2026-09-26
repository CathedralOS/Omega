use super::TopLevelSymbols;
use crate::validation::front_end;

#[test]
fn distinct_boundary_token_operand_overloads_are_not_duplicate_machines() {
    let program = crate::front_end::typed_program_from_texts(&[
        "boundary machine + Float::add(left: f32, right: f32) -> f32;
         boundary machine + Float::add(left: f64, right: f64) -> f64;",
    ]);
    let mut diagnostics = Vec::new();
    TopLevelSymbols::build(&program, &mut diagnostics);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn ordinary_operand_overloads_still_reject_as_duplicate_machines() {
    let program = crate::front_end::typed_program_from_texts(&[
        "machine Float::add(left: f32, right: f32) -> f32 { left }
         machine Float::add(left: f64, right: f64) -> f64 { left }",
    ]);
    let mut diagnostics = Vec::new();
    TopLevelSymbols::build(&program, &mut diagnostics);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .to_string()
                .contains("duplicate machine `Float::add`")
        }),
        "{diagnostics:?}"
    );
}
