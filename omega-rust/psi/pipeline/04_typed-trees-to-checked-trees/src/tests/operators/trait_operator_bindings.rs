use crate::tests::front_end::{checked_program, checked_program_result};

#[test]
fn trait_operator_use_consumes_only_the_selected_conformance_application() {
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

        machine caller(left: Card, right: Card) -> bool {
            choose<Card, Ascending>(left, right)
        }
        "#,
    );
    let selected = checked
        .typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "Ascending")
        })
        .expect("selected conformance")
        .symbol;
    let operator_use = checked
        .facts
        .operators
        .resolved_uses()
        .find(|operator_use| {
            checked
                .expression_table
                .display_name(operator_use.expression)
                == "left < right"
                && checked
                    .facts
                    .operators
                    .selected_candidate(operator_use)
                    .is_some_and(|candidate| candidate.is_trait_backed())
        })
        .expect("specialized trait token use");
    let candidate = checked
        .facts
        .operators
        .selected_candidate(operator_use)
        .expect("one selected trait candidate");

    assert_eq!(operator_use.candidate_count, 1);
    assert_eq!(candidate.conformance_symbol, selected);
    assert!(candidate.trait_requirement_symbol.is_valid());
    assert!(candidate.realization_machine_symbol.is_valid());
    assert!(candidate.realization_state_symbol.is_valid());
    assert_ne!(candidate.conformance_application_report_fingerprint, 0);
    let conformance_selections = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind()
                == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance
                && matches!(
                    selection.target(),
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                        if target.selected_symbol() == selected
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        conformance_selections.len(),
        2,
        "the explicit `Ascending` argument and inferred trait-token use retain distinct authored selections: {:#?}",
        checked.authored_declaration_selections(),
    );
    assert_ne!(
        conformance_selections[0].source_span(),
        conformance_selections[1].source_span()
    );
}

#[test]
fn trait_operator_use_rejects_multiple_selected_conformance_binders() {
    let source = r#"
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

        machine choose<
            Element,
            First: Element satisfies Ranked,
            Second: Element satisfies Ranked
        >(left: Element, right: Element) -> bool {
            left < right
        }

        machine caller(left: Card, right: Card) -> bool {
            choose<Card, Ascending, Descending>(left, right)
        }
    "#;
    let diagnostics =
        checked_program_result(source).expect_err("two selected binders are ambiguous");
    let message = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .find(|message| message.contains("ambiguous operator spelling `<`"))
        .expect("trait token ambiguity diagnostic");

    assert!(message.contains("2 viable candidates"));
    assert!(message.contains("proof-static conformance `Ascending`"));
    assert!(message.contains("proof-static conformance `Descending`"));
}

#[test]
fn trait_operator_bindings_are_unique_per_normalized_operand_telescope() {
    let duplicate = r#"
        trait Ranked<T> {
            operator < compare(left: T, right: T) -> bool;
            operator < before(first: T, second: T) -> bool;
        }
    "#;
    let diagnostics =
        checked_program_result(duplicate).expect_err("duplicate trait token must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("binds operator token `<` more than once")
    }));

    checked_program(
        r#"
        trait Ranked {
            operator < compare_i32(left: i32, right: i32) -> bool;
            operator < compare_u64(left: u64, right: u64) -> bool;
        }
        "#,
    );
}
