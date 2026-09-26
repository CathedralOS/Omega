use crate::tests::front_end::checked_program_result;
fn checked(
    source: &str,
) -> Result<crate::checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    checked_program_result(source)
}

#[test]
fn aggregate_field_write_cannot_replay_a_nonmutable_bindings_initializer() {
    let diagnostics = checked(
        "data Value { value: u64; }
         machine need(value: u64) -> u64 requires value == 256 { value }
         machine enter() -> u64 {
             let record: Value = Value { value: 256 };
             record.value = 5;
             need(record.value)
         }",
    )
    .map(|_| ())
    .expect_err("old aggregate initializer is not a current field value");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove requires")),
        "{diagnostics:#?}"
    );
}

#[test]
fn unchanged_aggregate_local_still_supplies_its_field() {
    checked(
        "data Value { value: u64; }
         machine need(value: u64) -> u64 requires value == 256 { value }
         machine enter() -> u64 {
             let record: Value = Value { value: 256 };
             need(record.value)
         }",
    )
    .unwrap_or_else(|diagnostics| panic!("unchanged aggregate field: {diagnostics:#?}"));
}
