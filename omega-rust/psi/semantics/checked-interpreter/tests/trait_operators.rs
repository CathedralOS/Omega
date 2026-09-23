use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use checked_interpreter::interpret_entry;

#[test]
fn interpreter_dispatches_fixed_token_through_selected_conformance_row() {
    let checked = crate::front_end::checked_program(
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
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("Main::main"),
        &[],
        InterpretOptions::default(),
    );

    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_resolves_crowned_token_to_concrete_declaration() {
    let checked = crate::front_end::checked_program(
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

        machine < Card::less_by_rank(left: Card, right: Card) -> bool {
            left.rank < right.rank
        }

        machine Main::main() -> i32 {
            let low: Card = Card { rank: 3 };
            let high: Card = Card { rank: 8 };
            transition low < high {
                true -> (70)
                false -> (1)
            }
        }
        "#,
    );
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("Main::main"),
        &[],
        InterpretOptions::default(),
    );

    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}
