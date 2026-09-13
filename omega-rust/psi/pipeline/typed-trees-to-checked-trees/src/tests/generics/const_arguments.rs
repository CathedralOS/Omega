use super::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use checked_trees::CheckedTrees;

fn check(source: &str) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("const argument tokens");
    let syntax = parse_syntax_trees(&tokens).expect("const argument syntax");
    let syntax = syntax_trees_to_symbol_resolved_trees::normalize_generic_data(syntax)?;
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)?;
    let typed = lower_symbol_resolved_trees(&resolved).map_err(|error| vec![error])?;
    lower_typed_trees(typed)
}

fn accepts(source: &str) -> CheckedTrees {
    check(source).unwrap_or_else(|errors| panic!("{errors:#?}\n{source}"))
}

#[test]
fn exclusive_integer_normalization_does_not_claim_strict_float_support() {
    accepts("machine valid(value: f64[0.0..=1.5]) { }");
    let errors = check("machine unsupported(value: f64[0.0..1.5]) { }")
        .expect_err("strict floating predicates require their own evidence");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("exclusive floating range"))
    );
}

#[test]
fn declared_range_endpoints_select_const_arguments_before_compatibility() {
    let checked = accepts(
        "machine upper_bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
        machine inferred(value: u64[5..=256]) -> u64 { upper_bound(value) }
        machine exclusive(value: u64[0..257]) -> u64 { upper_bound(value) }
        machine hexadecimal(value: u64[0..=0x100]) -> u64 { upper_bound(value) }
        machine computed(value: u64[0..=128 + 128]) -> u64 { upper_bound(value) }
        machine computed_exclusive(value: u64[0..128 * 2 + 1]) -> u64 { upper_bound(value) }
        machine explicit(value: u64[0..=256]) -> u64 { upper_bound<512>(value) }",
    );
    assert_eq!(checked.machine_specializations.len(), 2);
    let mut arguments: Vec<_> = checked
        .machine_specializations
        .iter()
        .map(|specialization| specialization.const_arguments.clone())
        .collect();
    arguments.sort();
    assert_eq!(arguments, [vec!["256"], vec!["512"]]);
}

#[test]
fn literal_exclusive_and_inclusive_ranges_keep_the_same_type_identity() {
    let checked = accepts(
        "machine compare(first: u64[0..8], second: u64[0..=7], different: u64[0..9]) -> u64 { first }",
    );
    let program = &checked.typed;
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let parameters = program.state_parameters(state);
    assert_eq!(
        program.normalized_type_identity(parameters[0].type_reference),
        program.normalized_type_identity(parameters[1].type_reference)
    );
    assert_ne!(
        program.normalized_type_identity(parameters[0].type_reference),
        program.normalized_type_identity(parameters[2].type_reference)
    );
}

#[test]
fn range_inference_converts_authored_upper_endpoint_kinds_exactly() {
    for (formal, actual, expected) in [
        ("0..=N", "0..18446744073709551616", "18446744073709551615"),
        ("0..N", "0..=256", "257"),
        ("0..=N", "0..257", "256"),
        ("0..N", "0..=7u64 / 2 * 2", "7"),
        ("0..=N", "0..7u64 / 2 * 2", "5"),
    ] {
        let checked = accepts(&format!(
            "machine bound<const N: u64>(value: u64[{formal}]) -> u64 {{ N }}
             machine inferred(value: u64[{actual}]) -> u64 {{ bound(value) }}"
        ));
        assert_eq!(checked.machine_specializations.len(), 1);
        assert_eq!(
            checked.machine_specializations[0].const_arguments,
            [expected],
            "{formal} from {actual}"
        );
    }
}

#[test]
fn typed_and_computed_range_endpoints_keep_normalized_identity() {
    let checked = accepts(
        "machine compare(first: u64[0..7u64 / 2 * 2], same: u64[0..=5],
             different: u64[0..7 / 2 * 2], full: u64[0..18446744073709551616],
             full_inclusive: u64[0..=18446744073709551615u64]) -> u64 { first }",
    );
    let program = &checked.typed;
    let state = &program.machine_states(&program.machines()[0])[0];
    let parameters = program.state_parameters(state);
    let identity =
        |position: usize| program.normalized_type_identity(parameters[position].type_reference);
    assert_eq!(identity(0), identity(1));
    assert_ne!(identity(0), identity(2));
    assert_eq!(identity(3), identity(4));
}

#[test]
fn exclusive_predecessor_cannot_repair_an_invalid_typed_endpoint() {
    for endpoint in ["256u8", "255u8 + 1"] {
        let source = format!(
            "machine bound<const N: u64>(value: u64[0..=N]) -> u64 {{ N }}
             machine invalid(value: u64[0..{endpoint}]) -> u64 {{ bound(value) }}"
        );
        assert!(
            check(&source).is_err(),
            "invalid authored endpoint accepted: {source}"
        );
    }
    accepts("machine valid(value: u64[0..256]) -> u64 { value }");
}

#[test]
fn declared_range_checks_use_the_same_exact_arithmetic_as_inference() {
    let checked = accepts(
        "machine upper_bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
        machine fractional(value: u64[0..=1 / 2 * 512]) -> u64 { upper_bound(value) }
        machine signed(value: i64[0 - 1 / 2 * 4..=2]) -> i64 { value }
        machine main() -> u64 { _ = signed(-2); fractional(256) }",
    );
    assert_eq!(checked.machine_specializations[0].const_arguments, ["256"]);
    for (bound, endpoint) in [
        ("7 / 2 * 2", 7),
        ("7u64 / 2 * 2", 6),
        ("1u64 * (7 / 2 * 2)", 7),
        ("7u64 % 2", 1),
        ("18446744073709551615u64 % 512u64", 511),
        ("9223372036854775808u64 + 256 - 9223372036854775808u64", 256),
    ] {
        let source = format!(
            "machine accept(value: u64[0..={bound}]) -> u64 {{ value }}
            machine main() -> u64 {{ accept({endpoint}) }}"
        );
        accepts(&source);
        assert!(
            check(&source.replace(
                &format!("accept({endpoint})"),
                &format!("accept({})", endpoint + 1)
            ))
            .is_err()
        );
    }
    for source in [
        "machine accept(value: u64[(5 / 2) * 2..=10]) -> u64 { value }
        machine main() -> u64 { accept(4) }",
        "machine accept(value: u64[0..=(5 / 2) * 2]) -> u64 { value }
        machine main() -> u64 { accept(6) }",
        "machine accept(value: u64[0..=5 / 2]) -> u64 { value }",
        "machine accept(value: u64[0..=5 / 0]) -> u64 { value }",
        "machine accept(value: u64[0..=255u8 + 1]) -> u64 { value }",
        "machine accept(value: u64[0..=7u64 + 1 / 2]) -> u64 { value }",
    ] {
        assert!(check(source).is_err(), "unexpected acceptance: {source}");
    }
}

#[test]
fn full_width_static_bounds_do_not_waive_operand_or_intermediate_landing() {
    for bound in [
        "18446744073709551615u64 + 1 - 1",
        "9223372036854775808u64 * 2 / 2",
        "0u64 + 18446744073709551616",
    ] {
        let source = format!(
            "machine bound<const N: u64>(value: u64[0..=N]) -> u64 {{ N }}
            machine invalid(value: u64[0..={bound}]) -> u64 {{ bound(value) }}"
        );
        assert!(
            check(&source).is_err(),
            "invalid typed endpoint accepted: {source}"
        );
    }
    for expression in [
        "input < 18446744073709551616",
        "18446744073709551616 > input",
    ] {
        let source = format!("machine compare(input: u64) -> bool {{ {expression} }}");
        let diagnostics = check(&source)
            .expect_err("a comparison bound cannot authorize oversized operand landing");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot land exactly in `u64`")),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn declared_range_inference_uses_lower_and_upper_endpoint_positions() {
    let checked = accepts(
        "const Values::LOW: i32 = -5;
        machine bound<const Low: i32, const High: i32>(value: i32[Low..=High]) -> i32 { Low }
        machine inferred(value: i32[0 - 5..=5 * 5]) -> i32 { bound(value) }
        machine explicit(value: i32[-5..=25]) -> i32 { bound<Values::LOW, 25>(value) }",
    );
    assert_eq!(checked.machine_specializations.len(), 1);
    assert_eq!(
        checked.machine_specializations[0].const_arguments,
        ["-5", "25"]
    );
}

#[test]
fn declared_range_inference_keeps_partial_and_forwarded_slots_in_order() {
    let checked = accepts(
        "machine bounds<const Low: u64[0..=5], const High: u64>(value: u64[Low..=High]) -> u64 { High }
        machine forward<const K: u64[0..=5]>(value: u64[5..=256]) -> u64 { bounds<K, 512>(value) }
        machine partial(value: u64[5..=256]) -> u64 { bounds<0>(value) }
        machine main(value: u64[5..=256]) -> u64 { forward<0>(value) }",
    );
    let mut arguments: Vec<_> = checked
        .machine_specializations
        .iter()
        .map(|specialization| specialization.const_arguments.clone())
        .collect();
    arguments.sort();
    assert_eq!(arguments, [vec!["0"], vec!["0", "256"], vec!["0", "512"]]);
}

#[test]
fn declared_range_inference_composes_with_borrowed_parameters_and_expected_results() {
    let checked = accepts(
        "machine bound<const N: u64>(value: &u64[0..=N]) -> u64 { N }
        machine produce<const N: u64>() -> u64[0..=N] { 0 }
        machine main(value: u64[5..=256]) -> u64 {
            let produced: u64[0..=128] = produce();
            bound(&value)
        }",
    );
    assert_eq!(checked.machine_specializations.len(), 2);
}

#[test]
fn declared_range_inference_waits_for_explicit_forwarded_const_arguments() {
    let checked = accepts(
        "machine upper_bound<const N: u64[256..=512]>(value: u64[0..=N]) -> u64 { N }
        machine forward<const K: u64[256..=512]>(value: u64[0..=256]) -> u64 { upper_bound<K>(value) }
        machine main(value: u64[0..=256]) -> u64 { forward<512>(value) }",
    );
    assert_eq!(checked.machine_specializations.len(), 2);
    for specialization in &checked.machine_specializations {
        assert_eq!(specialization.const_arguments, ["512"]);
    }
}

#[test]
fn declared_range_inference_preserves_repeated_endpoint_agreement() {
    let checked = accepts(
        "machine bound<const N: u64>(first: u64[0..=N], second: u64[0..=N]) -> u64 { N }
        machine inferred(first: u64[0..=128 + 128], second: u64[5..=256]) -> u64 { bound(first, second) }",
    );
    assert_eq!(checked.machine_specializations.len(), 1);
    assert!(check(
        "machine bound<const N: u64>(first: u64[0..=N], second: u64[0..=N]) -> u64 { N }
        machine inferred(first: u64[0..=256], second: u64[0..=128]) -> u64 { bound(first, second) }",
    ).is_err());
}

#[test]
fn declared_range_inference_retains_open_and_unusable_repeated_occurrences() {
    for (selected, accepted) in [(128, false), (256, true)] {
        let source = format!(
            "machine bound<const N: u64>(first: u64[0..=N], second: u64[0..=N]) -> u64 {{ N }}
            machine forward<const K: u64[256..=256]>(first: u64[0..=K], second: u64[0..=256]) -> u64 {{ bound(first, second) }}
            machine main(first: u64[0..={selected}], second: u64[0..=256]) -> u64 {{ forward<{selected}>(first, second) }}"
        );
        assert_eq!(check(&source).is_ok(), accepted, "{source}");
    }
    assert!(check(
        "machine bound<const N: u64>(first: u64[0..=N], second: u64[0..=N]) -> u64 { N }
        machine main(first: u64[0..=64 + 64], second: u64[0..=256]) -> u64 { bound(first, second) }",
    ).is_err());
}

#[test]
fn observed_range_inference_cannot_authorize_an_unrestricted_template() {
    for source in [
        "machine bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
        machine forward<const K: u64>(value: u64[0..=256]) -> u64 { bound<K>(value) }
        machine main(value: u64[0..=256]) -> u64 { forward<512>(value) }",
        "machine bound<const N: u64>(first: u64[0..=N], second: u64[0..=N]) -> u64 { N }
        machine forward<const K: u64>(first: u64[0..=K], second: u64[0..=256]) -> u64 { bound(first, second) }
        machine main(first: u64[0..=256], second: u64[0..=256]) -> u64 { forward<256>(first, second) }",
    ] {
        assert!(check(source).is_err(), "observed selection cannot validate: {source}");
    }
}

#[test]
fn explicit_forwarded_const_arguments_cannot_be_overwritten_by_array_inference() {
    assert!(
        check(
            "machine bound<const N: u64>(value: &[u8; N]) -> u64 { N }
        machine forward<const K: u64>(value: &[u8; 4]) -> u64 { bound<K>(value) }
        machine main(value: &[u8; 4]) -> u64 { forward<8>(value) }",
        )
        .is_err()
    );
}

#[test]
fn declared_range_inference_does_not_replace_compatibility_or_const_validation() {
    for source in [
        "machine bound<const N: u64>(value: u64[10..=N]) -> u64 { N }
        machine invalid(value: u64[5..=256]) -> u64 { bound(value) }",
        "machine bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
        machine invalid(value: u64[0..=256]) -> u64 { bound<128>(value) }",
        "machine bound<const N: u8>(value: u64[0..=N]) -> u8 { N }
        machine invalid(value: u64[0..=256]) -> u8 { bound(value) }",
        "machine bound<const N: u64>(value: i32[N..=25]) -> u64 { N }
        machine invalid(value: i32[-5..=25]) -> u64 { bound(value) }",
        "machine bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
        machine invalid(value: u32[0..=256]) -> u64 { bound(value) }",
        "machine bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
        machine invalid(value: u64[0..=256] in Wrapping) -> u64 { bound(value) }",
    ] {
        assert!(check(source).is_err(), "unexpected acceptance: {source}");
    }
}

#[test]
fn declared_range_inference_does_not_invent_missing_or_unusable_endpoints() {
    for source in [
        "machine bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
        machine invalid(value: u64) -> u64 { bound(value) }",
        "machine bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
        machine invalid() -> u64 { let value: u64 = 5; bound(value) }",
        "machine bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
        machine invalid(value: u64[0..=1 / 2]) -> u64 { bound(value) }",
        "machine bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
        machine invalid(value: u64[0..=256 / 0]) -> u64 { bound(value) }",
        "machine bound<const N: u64>(value: u64[0..=N * 2]) -> u64 { N }
        machine invalid(value: u64[0..=256]) -> u64 { bound(value) }",
        "machine bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
        machine invalid(value: u64[256..=128]) -> u64 { bound(value) }",
    ] {
        assert!(check(source).is_err(), "unexpected acceptance: {source}");
    }
}

#[test]
fn explicit_boolean_and_named_const_arguments_select_closed_values() {
    accepts(
        "machine value<const N: bool>() -> bool { N }
        machine main() -> bool { value<true>() }",
    );
    accepts(
        "const Values::N: i32 = -2;
        machine value<const N: i32>() -> i32 { N }
        machine main() -> i32 { value<Values::N>() }",
    );
}

#[test]
fn explicit_and_inferred_boolean_values_share_one_instance() {
    let checked = accepts(
        "const Values::ENABLED: bool = true;
        data Witness<const N: bool> { case Only; }
        data Scenario { witness: Witness<Values::ENABLED>; }
        machine value<const N: bool>(witness: &Witness<N>) -> bool { N }
        machine Scenario::run(&self) -> bool {
            let inferred: bool = value(&self.witness);
            let literal: bool = value<true>(&self.witness);
            value<Values::ENABLED>(&self.witness)
        }",
    );
    assert_eq!(checked.machine_specializations.len(), 1);
}

#[test]
fn explicit_integer_spellings_and_inferred_values_share_one_instance() {
    let checked = accepts(
        "const Values::TWO: u64 = 2;
        machine value<const N: u64>(witness: &[u8; N]) -> u64 { N }
        machine main() -> u64 {
            let pair: [u8; 2] = [0, 0];
            let inferred: u64 = value(&pair);
            let decimal: u64 = value<2>(&pair);
            let hexadecimal: u64 = value<0x2>(&pair);
            value<Values::TWO>(&pair)
        }",
    );
    assert_eq!(checked.machine_specializations.len(), 1);
}

#[test]
fn equivalent_named_structured_and_inferred_values_share_one_instance() {
    let checked = accepts(
        "data Config [copy] { count: u8; enabled: bool; }
        const Values::FIRST: Config = Config { count: 2, enabled: true };
        const Values::SAME: Config = Config { enabled: true, count: 2 };
        data Witness<const N: Config> { case Only; }
        data Scenario { witness: Witness<Values::FIRST>; }
        machine value<const N: Config>(witness: &Witness<N>) -> Config { N }
        machine Scenario::run(&self) -> Config {
            let inferred: Config = value(&self.witness);
            let first: Config = value<Values::FIRST>(&self.witness);
            value<Values::SAME>(&self.witness)
        }",
    );
    assert_eq!(checked.machine_specializations.len(), 1);
}

#[test]
fn forwarded_boolean_values_select_independent_inner_instances() {
    let checked = accepts(
        "machine inner<const N: bool>() -> bool { N }
        machine outer<const N: bool>() -> bool { inner<N>() }
        machine main() -> bool {
            let first: bool = outer<true>();
            outer<false>()
        }",
    );
    assert_eq!(checked.machine_specializations.len(), 4);
}

#[test]
fn statement_calls_admit_boolean_values_without_declaration_symbols() {
    let checked = accepts(
        "machine consume<const N: bool>() {}
        machine main() -> u64 { consume<true>(); consume<false>(); 7 }",
    );
    assert_eq!(checked.machine_specializations.len(), 2);
}

#[test]
fn structured_values_forward_without_becoming_integer_literals() {
    accepts(
        "data Config [copy] { enabled: bool; }
        const Values::CONFIG: Config = Config { enabled: true };
        const Values::ARRAY: [u8; 2] = [2, 3];
        machine record_inner<const N: Config>() -> Config { N }
        machine record_outer<const N: Config>() -> Config { record_inner<N>() }
        machine array_inner<const N: [u8; 2]>() -> [u8; 2] { N }
        machine array_outer<const N: [u8; 2]>() -> [u8; 2] { array_inner<N>() }
        machine main() -> u64 {
            let config: Config = record_outer<Values::CONFIG>();
            let items: [u8; 2] = array_outer<Values::ARRAY>();
            7
        }",
    );
}

#[test]
fn invalid_explicit_values_reject_even_when_the_binder_is_unused() {
    for source in [
        "machine value<const N: u8>() -> u64 { 7 } machine main() -> u64 { value<256>() }",
        "machine value<const N: bool>() -> u64 { 7 } machine main() -> u64 { value<2>() }",
        "machine value<const N: u8>() -> u64 { 7 } machine main() -> u64 { value<true>() }",
        "const Values::N: u16 = 2; machine value<const N: u8>() -> u64 { 7 }
         machine main() -> u64 { value<Values::N>() }",
        "data First [copy] { count: u8; } data Second [copy] { count: u8; }
         const Values::N: First = First { count: 2 };
         machine value<const N: Second>() -> u64 { 7 }
         machine main() -> u64 { value<Values::N>() }",
        "const Values::N: f64 = 2.0; machine value<const N: f64>() -> u64 { 7 }
         machine main() -> u64 { value<Values::N>() }",
        "machine value<const N: u8>() -> u64 { 7 } machine main() -> u64 { value<2, 3>() }",
        "machine value<const N: u8>() -> u64 { 7 } machine main() -> u64 { value<u8>() }",
        "machine value<const N: u8>() -> u64 { 7 } machine main() -> u64 { value() }",
    ] {
        assert!(
            check(source).is_err(),
            "invalid const selection checked: {source}"
        );
    }
}

#[test]
fn forwarded_named_integer_carriers_are_checked_before_erasure() {
    assert!(
        check(
            "machine inner<const N: u8>() -> u64 { 7 }
        machine outer<const N: u64>() -> u64 { inner<N>() }
        machine main() -> u64 { outer<2>() }"
        )
        .is_err()
    );
}

#[test]
fn inferred_values_must_fit_unused_const_binders_in_every_instance() {
    assert!(
        check(
            "machine value<const N: u8>(witness: &[u8; N]) -> u64 { 7 }
        machine main(pair: &[u8; 2], oversized: &[u8; 256]) -> u64 {
            let first: u64 = value(pair);
            value(oversized)
        }"
        )
        .is_err()
    );
    accepts(
        "machine value<const N: u8>() -> u64 { 7 }
        machine main() -> u64 { value<255>() }",
    );
}

#[test]
fn conflicting_explicit_and_inferred_values_reject() {
    assert!(
        check(
            "const Values::ENABLED: bool = true;
        data Witness<const N: bool> { case Only; }
        data Scenario { witness: Witness<Values::ENABLED>; }
        machine value<const N: bool>(witness: &Witness<N>) -> bool { N }
        machine Scenario::run(&self) -> bool { value<false>(&self.witness) }"
        )
        .is_err()
    );
}
