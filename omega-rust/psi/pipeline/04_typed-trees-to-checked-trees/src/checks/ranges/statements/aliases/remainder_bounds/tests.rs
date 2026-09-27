use crate::tests::front_end::checked_program_result;

fn rejects_index(source: &str) {
    let diagnostics = checked_program_result(source).expect_err("index remains unproved");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove index")),
        "{source}: {diagnostics:#?}",
    );
}

#[test]
fn signed_or_selected_remainders_do_not_supply_unsigned_bounds() {
    rejects_index(
        r#"
        machine write(output: &mut [u8; 64], input: i64) {
            let index: i64 = input % 64;
            output[index] = 7;
        }
        "#,
    );
    rejects_index(
        r#"
        boundary operator % Meaning::remainder(left: u64, right: u64) -> u64;
        machine write(output: &mut [u8; 64], input: u64) {
            let index: u64 = input % 64;
            output[index] = 7;
        }
        "#,
    );
}
