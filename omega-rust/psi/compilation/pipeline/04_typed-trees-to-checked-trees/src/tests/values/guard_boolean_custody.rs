use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(typed_program(source), &CheckingRequest::settled()) {
        Ok(_) => assert!(
            accepted,
            "authored Boolean equality supplied a bound: {source}"
        ),
        Err(diagnostics) => {
            assert!(!accepted, "{source}\n{diagnostics:#?}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("may overflow")),
                "{source}\n{diagnostics:#?}"
            );
        }
    }
}

#[test]
fn builtin_boolean_wrappers_keep_nested_polarity_and_snapshot_bounds() {
    for condition in [
        "(n > 0) == true",
        "true == (n > 0)",
        "(n > 0) != false",
        "false != (n > 0)",
        "(!(n > 0)) == false",
        "true != (!(n > 0))",
        "!((n > 0) == false)",
    ] {
        for (parameters, prefix) in [
            ("n: u64", ""),
            ("mut n: u64", ""),
            ("input: u64", "let n: u64 = input;"),
        ] {
            check(
                &format!(
                    "machine decrement({parameters}) -> u64 {{ {prefix}
                transition {condition} {{ true -> (n - 1) false -> n }}
            }}"
                ),
                true,
            );
        }
    }
}
