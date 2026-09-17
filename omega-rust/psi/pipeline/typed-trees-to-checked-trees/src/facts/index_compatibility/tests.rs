//! A restating write over a value-position call compares the instance the
//! callee's `ensures result in Family<...>` establishes, not only the family
//! symbol. The declared return type may name no instance at all (`-> i64`
//! here, `-> Extent` in the bump-allocator canary); the ensures fact is the
//! only carrier of the identity, and a restatement under another index is a
//! distinct normalized instance exactly as it is when the source is a
//! declared field.
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

fn check(source: &str, accepted: bool) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    match crate::lower_typed_trees(typed) {
        Ok(_) => assert!(accepted, "mismatched restatement accepted:\n{source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("actual `Coordinate<7>` and expected `Coordinate<9>`")
                        && diagnostic
                            .message
                            .contains("are distinct normalized instances")
                }),
                "{diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn restating_let_compares_the_instance_the_callee_ensures_on_its_result() {
    for declared_index in [7, 9] {
        check(
            &format!(
                r#"
            domain<const Axis: u64> i64::Coordinate<Axis>;
            boundary trait Marker {{
                machine mark(value: i64) -> i64
                ensures
                    result in Coordinate<7>;
                machine relay(value: i64 in Coordinate<{declared_index}>) -> i64 in Coordinate<{declared_index}>;
            }}
            machine run(host: &Marker, value: i64) -> i64 in Coordinate<{declared_index}>
            reaches Marker
            {{
                let placed: i64 in Coordinate<{declared_index}> = host.mark(value);
                host.relay(placed)
            }}
        "#
            ),
            declared_index == 7,
        );
    }
}

/// A reassigned domained parameter keeps no membership after the write (the
/// write discharge proves predicate domains only), so only the refusal is
/// pinned here: the store's declared instance is compared against the
/// callee's ensured instance through the same collector as the `let`.
#[test]
fn restating_store_names_the_instance_the_callee_ensures_on_its_result() {
    check(
        r#"
            domain<const Axis: u64> i64::Coordinate<Axis>;
            boundary trait Marker {
                machine mark(value: i64) -> i64
                ensures
                    result in Coordinate<7>;
                machine relay(value: i64 in Coordinate<9>) -> i64 in Coordinate<9>;
            }
            machine run(host: &Marker, value: i64, mut placed: i64 in Coordinate<9>) -> i64 in Coordinate<9>
            reaches Marker
            {
                placed = host.mark(value);
                host.relay(placed)
            }
        "#,
        false,
    );
}
