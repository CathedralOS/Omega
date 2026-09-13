use numerics::bignum::BigInt;
use source_files_to_tokens::Lexer;
use syntax_trees::{
    SyntaxTrees,
    types::{IntegerRangeNormalization, TypeReferenceHandle, TypeReferenceNode},
};

fn parse(arguments: &[&str]) -> SyntaxTrees {
    let parameters = arguments
        .iter()
        .enumerate()
        .map(|(ordinal, argument)| format!("value_{ordinal}: RangeValue<{argument}>"))
        .collect::<Vec<_>>()
        .join(", ");
    let text = format!(
        "data RangeValue<T> [copy] {{ value: T; }} machine read({parameters}) -> u64 {{ 0 }}"
    );
    let tokens = Lexer::new(&text).tokenize().expect("range argument tokens");
    tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("range argument syntax")
}

fn range_arguments(syntax: &SyntaxTrees) -> Vec<TypeReferenceHandle> {
    syntax
        .type_references
        .generic_nodes()
        .into_iter()
        .flat_map(|reference| {
            let TypeReferenceNode::Generic { arguments, .. } =
                syntax.type_references.type_reference(reference)
            else {
                return Vec::new();
            };
            syntax
                .type_references
                .type_reference_handles(*arguments)
                .iter()
                .copied()
                .filter(|argument| {
                    matches!(
                        syntax.type_references.type_reference(*argument),
                        TypeReferenceNode::Constrained { .. }
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn range_argument_observations_normalize_equivalent_closed_intervals() {
    let syntax = parse(&["u64[0..=256]", "u64[0..257]", "u64[0..=1 / 2 * 512]"]);
    let syntax = super::evaluate(syntax, None, &[], None).expect("observe closed arguments");
    let ranges = range_arguments(&syntax);
    assert_eq!(ranges.len(), 3);
    let expected = IntegerRangeNormalization {
        minimum: BigInt::from_u64(0),
        maximum: BigInt::from_u64(256),
    };
    for range in ranges {
        assert_eq!(
            syntax.type_references.integer_range_normalization(range, 0),
            Some(&expected)
        );
    }
}

#[test]
fn range_argument_observations_preserve_wide_and_empty_proof_integer_endpoints() {
    for (argument, maximum) in [
        ("u64[0..18446744073709551616]", BigInt::from_u64(u64::MAX)),
        ("u64[0..0]", BigInt::from_i64(-1)),
    ] {
        let syntax = super::evaluate(parse(&[argument]), None, &[], None)
            .expect("observe proof integer endpoint");
        let ranges = range_arguments(&syntax);
        assert_eq!(ranges.len(), 1);
        assert_eq!(
            syntax
                .type_references
                .integer_range_normalization(ranges[0], 0),
            Some(&IntegerRangeNormalization {
                minimum: BigInt::from_u64(0),
                maximum
            })
        );
    }
}

#[test]
fn malformed_closed_endpoints_never_receive_range_observations() {
    for argument in [
        "u64[0..=256u8]",
        "u64[0..=255u8 + 1u8 - 1u8]",
        "u64[0..=1 / 0]",
    ] {
        if let Ok(syntax) = super::evaluate(parse(&[argument]), None, &[], None) {
            for range in range_arguments(&syntax) {
                assert!(
                    syntax
                        .type_references
                        .integer_range_normalization(range, 0)
                        .is_none(),
                    "invalid endpoint admitted: {argument}"
                );
            }
        }
    }
}

#[test]
fn completed_typed_replay_rejects_forged_equal_range_observations() {
    let mut syntax = parse(&["u64[0..=256]", "u64[0..=255]"]);
    let ranges = range_arguments(&syntax);
    assert_eq!(ranges.len(), 2);
    for range in ranges {
        syntax.type_references.retain_integer_range_normalization(
            range,
            0,
            IntegerRangeNormalization {
                minimum: BigInt::from_u64(0),
                maximum: BigInt::from_u64(256),
            },
        );
    }
    let syntax = syntax_trees_to_symbol_resolved_trees::normalize_generic_data(syntax)
        .expect("synthesis consumes forged observation");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve forged shared application");
    let error = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect_err("completed typing must reconstruct both original intervals");
    assert!(format!("{error:?}").contains(
        "generated generic application does not match its exact retained instance origin"
    ));
}

#[test]
fn range_argument_probe_cannot_capture_a_shadowed_builtin_carrier() {
    for text in [
        "data RangeValue<T> [copy] { value: T; } machine read<u64>(value: RangeValue<u64[0..=256]>) -> u32 { 0 }",
        "data RangeValue<T> [copy] { value: T; } data Outer<u64> { value: RangeValue<u64[0..=256]>; }",
    ] {
        let tokens = Lexer::new(text)
            .tokenize()
            .expect("shadowed carrier tokens");
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("shadowed carrier syntax");
        let ranges = range_arguments(&syntax);
        assert!(!ranges.is_empty());
        if let Ok(syntax) = super::evaluate(syntax, None, &[], None) {
            for range in ranges {
                assert!(
                    syntax
                        .type_references
                        .integer_range_normalization(range, 0)
                        .is_none(),
                    "probe captured a builtin instead of the authored type binder: {text}"
                );
            }
        }
    }
}
