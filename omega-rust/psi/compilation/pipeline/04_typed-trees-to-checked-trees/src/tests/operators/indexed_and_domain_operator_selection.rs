use super::{indexed_selection_fixture, named_type, operator_with_spelling};
use crate::tests::front_end::checked_program;
use crate::tests::{Identifier, StateParameter, SymbolHandle};
use language_core::operator_spelling::OperatorSpelling;
use typed_trees::operator::{operator_operand_signature, resolve_spelling_for_operands};

#[test]
fn indexed_collection_adaptation_preserves_other_operands_and_access() {
    for (element, index_type, actual_collection, expected_collection) in [
        ("u8", "u64", "[i32; 3]", "&[u8]"),
        ("i32", "i32", "[i32; 3]", "&[i32]"),
        ("i32", "u64", "[i32; 3]", "&mut [i32]"),
        ("i32", "u64", "&[i32; 3]", "&mut [i32]"),
        ("i32", "u64", "&i32", "&[i32]"),
        ("[i32; 3]", "u64", "[[i32; 2]; 3]", "&[[i32; 3]]"),
    ] {
        let source = format!(
            r#"
            boundary operator [] Collection::read(items: {expected_collection}, index: {index_type}) -> {element};
            machine inspect(items: {actual_collection}, index: u64) {{ let value: i32 = items[index]; }}
        "#
        );
        let (_, facts) = indexed_selection_fixture(&source);
        let operator_use = facts
            .uses
            .iter()
            .find_map(|(_, value)| (value.spelling == OperatorSpelling::Index).then_some(value))
            .expect("index use");
        assert_eq!(
            operator_use.status,
            if actual_collection == "&i32" {
                checked_trees::CheckedOperatorResolutionStatus::Missing
            } else {
                checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
            },
            "{source}"
        );
        // Builtin meaning is not a matching authored declaration. Bounds and
        // access remain independent checks on the operation and its consumer.
        assert!(!operator_use.selected_operator_symbol.is_valid());
        assert_eq!(operator_use.candidate_count, 0);
    }
}

#[test]
fn closed_index_instances_select_distinct_same_carrier_operators() {
    let source = r#"
        domain<T, const U: i32> T::Quantity<U>;

        operator / Quantity::meters_per_second(
            distance: f64 in Quantity<1>,
            duration: f64 in Quantity<2>
        ) -> f64 in Quantity<3>;

        operator / Quantity::kilometers_per_hour(
            distance: f64 in Quantity<4>,
            duration: f64 in Quantity<5>
        ) -> f64 in Quantity<6>;

        machine rates(
            meters: f64 in Quantity<1>,
            seconds: f64 in Quantity<2>,
            kilometers: f64 in Quantity<4>,
            hours: f64 in Quantity<5>
        ) {
            let metric: f64 in Quantity<3> = meters / seconds;
            let road: f64 in Quantity<6> = kilometers / hours;
        }

        data Main {}
        machine Main::main(&mut self) {}
    "#;

    let checked = checked_program(source);
    let selected = checked
        .facts
        .operators
        .resolved_uses()
        .filter(|operator_use| operator_use.spelling == OperatorSpelling::Divide)
        .filter_map(|operator_use| checked.facts.operators.selected_candidate(operator_use))
        .map(|candidate| candidate.operator_symbol)
        .collect::<Vec<_>>();
    let expected = checked
        .operators()
        .iter()
        .filter(|operator| {
            let path = checked.operator_path_members(operator.name);
            path.last().is_some_and(|name| {
                matches!(name.as_str(), "meters_per_second" | "kilometers_per_hour")
            })
        })
        .map(|operator| operator.symbol)
        .collect::<Vec<_>>();

    assert_eq!(selected.len(), 2);
    assert!(expected.iter().all(|symbol| selected.contains(symbol)));
}

#[test]
fn attached_receiver_normalizes_to_operand_position_zero() {
    let mut program = typed_trees::TypedTrees::default();
    let receiver_type = named_type(&mut program, "Receiver");
    let right_type = named_type(&mut program, "Right");
    let mut operator =
        operator_with_spelling(SymbolHandle::from_arena_index(149), OperatorSpelling::Add);
    for (name, type_reference, is_self) in
        [("right", right_type, false), ("self", receiver_type, true)]
    {
        program.push_operator_parameter(
            &mut operator,
            StateParameter {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated(name),
                type_reference,
                is_const: false,
                is_mutable: false,
                is_self,
                relevance: language_core::BindingRelevance::Relevant,
            },
        );
    }
    program.push_operator(operator);

    assert_eq!(
        operator_operand_signature(&program, &program.operators()[0]),
        "named(name(Receiver)), named(name(Right))"
    );
    assert_eq!(
        resolve_spelling_for_operands(
            &program,
            OperatorSpelling::Add,
            &[Some(receiver_type), Some(right_type)],
            source::SourceSpan::default(),
        )
        .len(),
        1
    );
    assert!(
        resolve_spelling_for_operands(
            &program,
            OperatorSpelling::Add,
            &[Some(right_type), Some(receiver_type)],
            source::SourceSpan::default(),
        )
        .is_empty()
    );
}
