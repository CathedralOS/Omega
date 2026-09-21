//! Authored integer entry ranges keep their exact inclusive endpoints through
//! source lowering: an exclusive maximum becomes its predecessor, the retained
//! rows reach the published catalog beside the matching `requires` bounds, a
//! conforming delivery executes, and an exact out-of-range constant rejects.

use checked_trees::CheckedTrees;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_codec::{decode_module, encode_module, encode_proof_section};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn check(source: &str) -> CheckedTrees {
    lower_typed_trees(typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn lower(source: &str, entry: &str) -> lowered_psi::LoweredPsi {
    checked_trees_to_lowered_psi::lower_machine(&check(source), entry)
        .unwrap_or_else(|error| panic!("{source}: {error:?}"))
}

fn u64_integer() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

/// The owner contract publishes both inclusive bounds of `row` as
/// `LTE(minimum, parameter)` and `LTE(parameter, maximum)` conjuncts.
fn publishes_bounds(lowered: &lowered_psi::LoweredPsi, row: &terminal_psi::ScalarIntegerRange) {
    let owner = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == row.machine)
        .expect("range owner");
    let parameter = owner
        .parameters
        .iter()
        .find(|parameter| parameter.id == row.parameter)
        .expect("ranged parameter");
    assert_eq!(parameter.scalar_type, ScalarType::Integer(row.integer_type));
    let carrier = ScalarType::Integer(row.integer_type);
    let subject = ScalarTerm::value(parameter.id, carrier);
    let minimum = ScalarTerm::integer(row.integer_type, row.minimum).unwrap();
    let maximum = ScalarTerm::integer(row.integer_type, row.maximum).unwrap();
    let published = |expected: &Proposition| {
        let mut pending: Vec<&Proposition> = owner.contract.requires.iter().collect();
        while let Some(proposition) = pending.pop() {
            match proposition {
                Proposition::Conjunction(terms) => pending.extend(terms.iter()),
                proposition if proposition == expected => return true,
                _ => {}
            }
        }
        false
    };
    assert!(
        published(&Proposition::LessOrEqual(minimum, subject.clone())),
        "owner must publish the inclusive minimum bound"
    );
    assert!(
        published(&Proposition::LessOrEqual(subject, maximum)),
        "owner must publish the inclusive maximum bound"
    );
}

#[test]
fn inclusive_range_rows_reach_the_terminal_catalog_and_execute() {
    let source = r#"
        machine accept(value: u64[0..=3]) -> u64 { value }
        machine value() -> u64 { accept(2) }
    "#;
    let lowered = lower(source, "value");
    let ranges = &lowered
        .semantic_module
        .scalar_qualifications
        .integer_entry_ranges;
    let [range] = ranges.as_slice() else {
        panic!("one retained integer range: {source}")
    };
    assert_eq!(range.integer_type, u64_integer());
    assert_eq!(range.minimum, IntegerValue::Unsigned(0));
    assert_eq!(range.maximum, IntegerValue::Unsigned(3));
    publishes_bounds(&lowered, range);
    let module_bytes = encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    let decoded = decode_module(&module_bytes).expect("canonical module decodes");
    assert_eq!(decoded, lowered.semantic_module);
    assert_eq!(
        interpret_terminal_artifact(
            &module_bytes,
            &proof_bytes,
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .unwrap_or_else(|error| panic!("{source}: {error:?}")),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: u64_integer(),
            value: IntegerValue::Unsigned(2),
        }),
        "{source}",
    );
}

#[test]
fn exclusive_maximum_retains_its_inclusive_predecessor() {
    // `u64[0..4]` excludes 4, so the retained inclusive maximum is 3.
    let source = r#"
        machine accept(value: u64[0..4]) -> u64 { value }
        machine value() -> u64 { accept(3) }
    "#;
    let lowered = lower(source, "value");
    let [range] = lowered
        .semantic_module
        .scalar_qualifications
        .integer_entry_ranges
        .as_slice()
    else {
        panic!("one retained integer range: {source}")
    };
    assert_eq!(range.minimum, IntegerValue::Unsigned(0));
    assert_eq!(range.maximum, IntegerValue::Unsigned(3));
    publishes_bounds(&lowered, range);
}

#[test]
fn out_of_range_constant_delivery_is_rejected() {
    // Integer bounds are decidable at source check: a constant the authored
    // range excludes is refused before lowering, as a check diagnostic. The
    // retained roster row only reaches a delivery check for artifacts that
    // bypass source checking — the terminal-verifier suite exercises that
    // `ScalarIntegerRangeDelivery` backstop directly.
    for argument in ["4", "7"] {
        let source = format!(
            r#"
            machine accept(value: u64[0..=3]) -> u64 {{ value }}
            machine value() -> u64 {{ accept({argument}) }}
            "#,
        );
        assert!(
            lower_typed_trees(typed(&source)).is_err(),
            "{argument} is outside u64[0..=3] and must reject at check: {source}"
        );
    }
}

#[test]
fn signed_carrier_retains_signed_endpoints() {
    let source = r#"
        machine accept(value: i64[-4..=4]) -> i64 { value }
        machine value() -> i64 { accept(-1) }
    "#;
    let lowered = lower(source, "value");
    let [range] = lowered
        .semantic_module
        .scalar_qualifications
        .integer_entry_ranges
        .as_slice()
    else {
        panic!("one retained integer range: {source}")
    };
    let signed = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    assert_eq!(range.integer_type, signed);
    assert_eq!(range.minimum, IntegerValue::Signed(-4));
    assert_eq!(range.maximum, IntegerValue::Signed(4));
    publishes_bounds(&lowered, range);
    let module_bytes = encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    assert_eq!(
        interpret_terminal_artifact(
            &module_bytes,
            &proof_bytes,
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .unwrap_or_else(|error| panic!("{source}: {error:?}")),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: signed,
            value: IntegerValue::Signed(-1),
        }),
        "{source}",
    );
}
