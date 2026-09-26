use crate::tests::front_end::{checked_program, checked_program_result};
use language_core::operator_spelling::OperatorSpelling;

#[test]
fn checked_program_retains_trait_owned_operator_token() {
    let checked = checked_program(
        r#"
        trait Ranked<T> {
            operator < compare(left: T, right: T) -> bool;
        }
        "#,
    );
    let trait_definition = checked
        .typed
        .traits
        .iter()
        .next()
        .map(|(_, definition)| definition)
        .expect("Ranked trait");
    let [requirement] = checked.typed.trait_machine_signatures(trait_definition) else {
        panic!("one trait operator requirement expected");
    };

    assert_eq!(requirement.spelling, Some(OperatorSpelling::Less));
}

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
fn trait_operator_return_retains_exact_structural_scalar_call_plan() {
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
    let [plan] = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .trait_operator_machines
        .as_slice()
    else {
        panic!("one specialized trait-token return should be retained")
    };
    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| specialization.instance == plan.machine)
        .expect("plan owner specialization");
    let [application] = specialization.conformance_applications.as_slice() else {
        panic!("one exact conformance application")
    };
    let row = application
        .rows
        .iter()
        .find(|row| row.requirement == plan.requirement)
        .expect("selected requirement row");

    assert_eq!(plan.attachment_type_identity, None);
    assert_eq!(plan.structural_parameters.len(), 2);
    assert_eq!(plan.argument_source_positions, [0, 1]);
    assert_eq!(plan.result_type, typed_trees::types::PrimitiveType::Bool);
    assert_eq!(plan.conformance, application.declaration);
    assert_eq!(
        plan.conformance_application_report_fingerprint,
        application.report_fingerprint
    );
    assert_eq!(plan.realization_machine, row.realization_machine);
    assert_eq!(plan.realization_state, row.realization_state);
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
fn visible_conformance_does_not_supply_an_unbound_trait_operator() {
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

        machine compare(left: Card, right: Card) -> bool {
            left < right
        }
    "#;
    let diagnostics =
        checked_program_result(source).expect_err("visible conformance is not authority");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("no such operator is declared")
            && diagnostic.message.contains("Card")
    }));
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

#[test]
fn aggregate_parameter_field_spelling_retains_float_operator_fact() {
    let source = r#"
        boundary operator + Float::add(left: f64, right: f64) -> f64;

        pub data Pair {
            first: f64;
            second: f64;
        }

        pub data Main { observed: f64; }

        boundary machine Main::main(&mut self, pair: Pair) -> i32 {
            self.observed = pair.first + pair.second;
            transition { _ -> (0) }
        }
    "#;

    let checked = checked_program(source);
    let add = checked
        .facts
        .operators
        .resolved_uses()
        .find(|operator_use| {
            operator_use.spelling == OperatorSpelling::Add
                && checked
                    .expression_table
                    .display_name(operator_use.expression)
                    == "pair.first + pair.second"
        })
        .expect("aggregate parameter field arithmetic must retain checked operator evidence");

    assert_eq!(
        add.policy_adapter,
        checked_trees::CheckedArithmeticPolicyAdapter::None
    );
}
