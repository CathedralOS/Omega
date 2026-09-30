use crate::preparation::generic_data::normalize_generic_data_with_warnings;
use source::SourceId;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::Item;
use tokens_to_syntax_trees::parse_syntax_trees_with_id;

fn parse(source: &str) -> SyntaxTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize const fact");
    parse_syntax_trees_with_id(SourceId(17), &tokens).expect("parse const fact")
}

fn generic_fact(expression: &str) -> String {
    format!(
        "data Checked<const N: u64> where {expression}, {{ field: i64; }}
         data Main {{ value: Checked<7>; }}"
    )
}

fn assert_case(source: &str, expected_error: Option<&str>, retained: bool) {
    let (syntax, warnings) = match normalize_generic_data_with_warnings(parse(source)) {
        Ok(result) => {
            assert!(
                expected_error.is_none(),
                "expected {expected_error:?}: {source}"
            );
            result
        }
        Err(errors) => {
            let Some(expected) = expected_error else {
                panic!("{source}: {errors:?}");
            };
            assert!(
                errors.iter().any(|error| error.message.contains(expected)),
                "{source}: {errors:?}"
            );
            return;
        }
    };
    let instance = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == "Checked<7>" => Some(definition),
            _ => None,
        })
        .expect("closed generic instance");
    assert_eq!(!instance.where_facts.is_empty(), retained, "{source}");
    assert!(warnings.is_empty(), "{source}: {warnings:?}");
}

/// Authored `<=`/`-` spellings reach this evaluator through the core `Nat`
/// machines, whose `Nat` parameters can never bind a `u64` or anonymous
/// operand. Those facts keep the builtin meaning rather than declining.
#[test]
fn builtin_integer_operands_ignore_machine_spellings_on_foreign_carriers() {
    for (true_fact, false_fact) in [
        ("N <= 64", "N <= 6"),
        ("7 <= N", "8 <= N"),
        ("N - 1 <= 6", "N - 1 <= 5"),
        ("N - 0 == 7", "N - 0 == 6"),
        ("N - 2 >= 5", "N - 2 >= 6"),
        ("1 <= N - 4", "1 <= N - 7"),
    ] {
        assert_case(&generic_fact(true_fact), None, false);
        assert_case(&generic_fact(false_fact), Some("is false"), false);
    }
}

/// A spelled machine whose parameter cannot be read as one concrete carrier —
/// here a bound borrow of `Wrapped` — may still bind any operand, so the fact
/// keeps declining rather than borrowing the builtin meaning.
#[test]
fn uninspectable_authored_parameter_carriers_keep_the_conservative_decline() {
    let source = "data Wrapped { field: u64; }
         machine - Wrapped::halve(left: &Wrapped, right: u64) -> u64 { 0 }
         data Checked<const N: u64> where N - 1 <= 64, { field: i64; }
         data Main { value: Checked<7>; }";
    assert_case(&source, None, true);
}

/// Operator declarations, domain-attached operators and trait member
/// signatures author spellings through shapes this pass does not narrow; an
/// authored `<=` in any of those lanes still declines the fact.
#[test]
fn trait_lane_authored_spellings_keep_the_conservative_decline() {
    let source = "trait Ordered {
             machine <= lte(left: u64, right: u64) -> bool;
         }
         data Checked<const N: u64> where N <= 64, { field: i64; }
         data Main { value: Checked<7>; }";
    assert_case(&source, None, true);
}
