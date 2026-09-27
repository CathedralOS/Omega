//! Closed generic constant calls retain their authored application for replay.

use super::{evaluate, evaluate_fully, integer_encoding};

#[test]
fn generic_calls_compose_type_const_and_nested_applications() {
    let syntax = evaluate(
        "machine identity<T>(value: T) -> T { value }
         machine forward<T>(value: T) -> T { identity<T>(value) }
         machine amount<const N: u64>() -> u64 { N }
         const VALUE: u64 = forward<u64>(identity<u64>(amount<7>())) + amount<9>();",
    )
    .expect("complete ordinary specializations compose");
    integer_encoding(&syntax, "VALUE", 16);
    for (carrier, value, expected) in [
        ("bool", "true", "boolean4:true"),
        ("f32", "-0.0f32", "float:f32:80000000"),
        ("f64", "1.5f64", "float:f64:3ff8000000000000"),
    ] {
        let text = format!(
            "machine identity<T>(value: T) -> T {{ value }}
             const VALUE: {carrier} = identity<{carrier}>({value});"
        );
        let typed = evaluate_fully(&[("main.omg", &text)], &[]);
        assert_eq!(
            typed.const_declarations()[0]
                .canonical_value_encoding
                .as_deref(),
            Some(expected)
        );
    }
}

#[test]
fn generic_calls_reject_invalid_complete_tuples_and_false_premises() {
    let mut accepted = Vec::new();
    for source in [
        "machine identity<T>(value: T) -> T { value } const UNUSED: u64 = identity<bool>(7);",
        "machine identity<T>(value: T) -> T { value } const UNUSED: u64 = identity<u64, u8>(7);",
        "machine amount<const N: u8>() -> u64 { 7 } const UNUSED: u64 = amount<256>();",
        "machine guarded<const N: u64>(ready: bool) -> u64 requires ready; { N }
         const UNUSED: u64 = guarded<7>(false);",
        "machine amount<const N: u64>() -> u64 { N } const UNUSED: u64 = amount<7u8>();",
    ] {
        if evaluate(source).is_ok() {
            accepted.push(source);
        }
    }
    assert!(
        accepted.is_empty(),
        "invalid applications accepted: {accepted:?}"
    );
}
