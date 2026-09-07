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
        "data Config { count: u8; enabled: bool; }
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
        "data Config { enabled: bool; }
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
        "data First { count: u8; } data Second { count: u8; }
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
