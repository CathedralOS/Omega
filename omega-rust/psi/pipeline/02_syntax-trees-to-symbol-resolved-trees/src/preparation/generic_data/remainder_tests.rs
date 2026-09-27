use crate::pre_resolution::GenericDataRequest;
use crate::pre_resolution::normalize_generic_data;
use diagnostics::Diagnostic;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::Item;
use tokens_to_syntax_trees::parse_syntax_trees;

fn normalize(source: &str) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    normalize_generic_data(GenericDataRequest::new(
        parse_syntax_trees(&tokens).expect("parse"),
    ))
}

#[test]
fn const_argument_anonymous_division_retains_its_exact_value() {
    let syntax = normalize(
        "data Buffer<const N: u64> { values: [u8; N]; }
        data Main { value: Buffer<7 / 2 * 2>; }",
    )
    .expect("integral exact result");
    assert!(
        syntax.root_items().any(|item| matches!(item,
            Item::Data(definition) if definition.name.as_str() == "Buffer<7>"
        )),
        "anonymous quotient was truncated before multiplication"
    );
}

#[test]
fn typed_const_domain_remainders_preserve_truth_and_divisor_checks() {
    for (expression, expected_error) in [
        ("8u64 % 2 == 0", None),
        ("7 % 2u64 == 1", None),
        ("-3i32 % 2 == -1", None),
        ("self % 2 == 0", None),
        ("Sizes::COUNT % 2 == 1", None),
        ("8 / 2 == 4", None),
        ("8u64 % 2 == 1", Some("is false")),
        ("-3i32 % 2 == 1", Some("is false")),
        ("8u64 % 0 == 0", Some("remainder by zero")),
        ("self % 0 == 0", Some("remainder by zero")),
    ] {
        let source = format!(
            "const Sizes::COUNT: u64 = 7;
            domain u64::Checked requires {expression};
            domain u64::Transitive requires self in Checked;
            data Buffer<const N: u64> where N in Transitive, {{ values: [u8; N]; }}
            data Main {{ value: Buffer<2>; }}"
        );
        match (normalize(&source), expected_error) {
            (Ok(syntax), None) => {
                let instance = syntax
                    .root_items()
                    .find_map(|item| match item {
                        Item::Data(definition) if definition.name.as_str() == "Buffer<2>" => {
                            Some(definition)
                        }
                        _ => None,
                    })
                    .expect("closed generic instance");
                assert!(
                    instance.where_facts.is_empty(),
                    "true domain fact was not discharged"
                );
            }
            (Err(errors), Some(expected)) => assert!(
                errors.iter().any(|error| error.message.contains(expected)),
                "{source}: {errors:?}"
            ),
            (result, expected) => panic!("{source}: expected {expected:?}, got {result:?}"),
        }
    }
}

#[test]
fn authored_const_operator_arguments_retain_selection_before_folding() {
    let syntax = normalize(
        "operator % u64::remainder(left: u64, right: u64) -> u64;
         data Buffer<const N: u64> { values: [u8; N]; }
         data Main { value: Buffer<7u64 % 2>; }",
    )
    .expect("unselected argument remains available for typing");
    assert!(
        !syntax.root_items().any(|item| matches!(item,
            Item::Data(definition) if definition.name.as_str() == "Buffer<1>"
        )),
        "authored remainder was replaced with builtin arithmetic"
    );
}

#[test]
fn unrelated_operator_spelling_does_not_block_builtin_const_arguments() {
    let syntax = normalize(
        "operator + u64::add(left: u64, right: u64) -> u64;
         data Buffer<const N: u64> { values: [u8; N]; }
         data Main { value: Buffer<7u64 % 2>; }",
    )
    .expect("builtin remainder still folds");
    assert!(syntax.root_items().any(|item| matches!(item,
        Item::Data(definition) if definition.name.as_str() == "Buffer<1>"
    )));
}
