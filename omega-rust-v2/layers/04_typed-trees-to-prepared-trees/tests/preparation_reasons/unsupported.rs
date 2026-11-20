use super::*;

fn assert_integer_constant(
    source: &str,
    value: u8,
) {
    with_package(source, |package| {
        let prepared = prepare_source_package_diagnosed(package).unwrap();
        let [result] = prepared.typed().constants.as_slice() else {
            panic!("expected one constant")
        };
        assert_eq!(prepared.constant(&result.declaration), Some(&PreparedValue::Integer(value.into())));
    });
}

#[test]
fn storage_addresses_are_not_constant_inputs() {
    let source = r#"machine observe()->u8 {_ = "a".address; 7u8} const RESULT:u8=observe();"#;
    assert_integer_constant(&source.replace(r#""a".address"#, "4u8"), 7);
    admitted(
        source,
        PreparationReason::UnsupportedBody,
        "the selected body is outside the supported static evaluation fragment",
        "observe()",
        "",
    );
}

#[test]
fn an_argument_depending_on_a_runtime_address_is_not_closed() {
    let source = r#"machine keep(n:u64)->u64 {n} const RESULT:u64=keep(match "a".address {_->1u64,});"#;
    assert_integer_constant(&source.replace(r#""a".address"#, "true"), 1);
    admitted(
        source,
        PreparationReason::UnsupportedArgument,
        "the static call has an argument without a supported closed value",
        r#"keep(match "a".address {_->1u64,})"#,
        "",
    );
}

#[test]
fn a_selected_block_in_an_operand_is_still_an_operand() {
    let source = r#"machine keep(n:u64)->u64 {n} const RESULT:u64=keep(match true {true->{_ = "a".address; 1u64}, false->2u64});"#;
    assert_integer_constant(&source.replace("match true", "match false"), 2);
    assert_integer_constant(&source.replace(r#""a".address"#, "4u8"), 1);
    admitted(
        source,
        PreparationReason::UnsupportedArgument,
        "the static call has an argument without a supported closed value",
        r#"keep(match true {true->{_ = "a".address; 1u64}, false->2u64})"#,
        "",
    );
}

#[test]
fn a_boolean_operand_with_an_address_read_is_not_closed() {
    let source = r#"machine keep(n:bool)->u8 {7u8} const RESULT:u8=keep(match true {true->{_ = "a".address; true}, false->false});"#;
    assert_integer_constant(&source.replace(r#""a".address"#, "4u8"), 7);
    admitted(
        source,
        PreparationReason::UnsupportedArgument,
        "the static call has an argument without a supported closed value",
        r#"keep(match true {true->{_ = "a".address; true}, false->false})"#,
        "",
    );
}

#[test]
fn an_invocation_in_an_operand_still_checks_its_own_body() {
    let source = r#"machine keep(n:u8)->u8 {n} machine observe()->u8 {_ = "a".address; 7u8} const RESULT:u8=keep(observe());"#;
    assert_integer_constant(&source.replace(r#""a".address"#, "4u8"), 7);
    admitted(
        source,
        PreparationReason::UnsupportedBody,
        "the selected body is outside the supported static evaluation fragment",
        "observe()",
        "\ncall chain: keep -> observe",
    );
}

#[test]
fn domain_membership_precondition_needs_proof() {
    let source = r#"machine marked(n:u8)->bool reaches none crashes none terminates {_ = "a".address; n == 9}
        domain u8::Marked requires marked(self);
        machine keep(n:u8)->u8 requires n in Marked {n} const RESULT:u8=keep(9);"#;
    assert_integer_constant(&source.replace("n in Marked", "n == 9"), 9);
    admitted(
        source,
        PreparationReason::UnsupportedContract,
        "the selected contract needs proof outside the supported static evaluation fragment",
        "keep(9)",
        "",
    );
}

#[test]
fn integer_result_of_token_machine_uses_selected_body() {
    let source =
        "data Box [copy]{value:u8;} machine + choose(a:Box,b:u8)->u8 requires b != 0 {a.value} machine run()->u8 {Box{value:42u8}+3u8} const RESULT:u8=run();";
    assert_integer_constant(source, 42);
    admitted(
        &source.replace("+3u8", "+0u8"),
        PreparationReason::IntegerPreconditionFalse,
        "the selected integer precondition is false for the supplied arguments",
        "Box{value:42u8}+0u8",
        "\ncall chain: run -> choose",
    );
}
