use psi_checked_interpreter::interpret_entry;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

fn checked_program(source: &str) -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("checked lowering")
}

#[test]
fn interpreter_dispatches_fixed_token_through_selected_conformance_row() {
    let checked = checked_program(
        r#"
        trait Ranked {
            operator < before(left: Self, right: Self) -> bool;
        }

        data Card { rank: i32; }

        Ascending: Card satisfies Ranked {
            machine before(left: Card, right: Card) -> bool {
                left.rank < right.rank
            }
        }

        Descending: Card satisfies Ranked {
            machine before(left: Card, right: Card) -> bool {
                left.rank > right.rank
            }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: Element,
            right: Element
        ) -> bool {
            left < right
        }

        machine Main::main() -> i32 {
            let low: Card = Card { rank: 3 };
            let high: Card = Card { rank: 8 };
            transition choose<Card, Ascending>(low, high) {
                true -> (70)
                false -> (1)
            }
        }
        "#,
    );
    let outcome = interpret_entry(&checked, "Main::main", &[]);

    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}
