use super::{
    has_selected_domain_add, indexed_selection_fixture, named_type, operator_with_spelling,
};
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::operators::build_operator_facts;
use crate::tests::front_end::{checked_program, typed_program};
use crate::tests::{
    Expression, Identifier, Machine, NamePath, State, StateParameter, SymbolHandle,
    TypeReferenceNode,
};
use language_core::operator_spelling::OperatorSpelling;
use typed_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};
use typed_trees::operator::{operator_operand_signature, resolve_spelling_for_operands};

#[test]
fn indexed_shared_collection_views_retain_exact_operator_and_closed_element() {
    for collection in [
        "[i32; 3]",
        "&[i32; 3]",
        "&mut [i32; 3]",
        "&[i32]",
        "&mut [i32]",
    ] {
        let source = format!(
            r#"
            boundary operator [] Collection::read<Element>(items: &[Element], index: u64) -> Element
            requires index < items.len;
            machine inspect(items: {collection}, index: u64) {{ let value: i32 = items[index]; }}
        "#
        );
        let (program, facts) = indexed_selection_fixture(&source);
        let operator_use = facts
            .uses
            .iter()
            .find_map(|(_, value)| (value.spelling == OperatorSpelling::Index).then_some(value))
            .expect("index use");
        assert_eq!(
            operator_use.status,
            checked_trees::CheckedOperatorResolutionStatus::Resolved,
            "{collection}"
        );
        let operator = program
            .operators()
            .iter()
            .find(|operator| operator.symbol == operator_use.selected_operator_symbol)
            .expect("exact selected declaration");
        let candidate = facts
            .selected_candidate(operator_use)
            .expect("selected candidate");
        assert_eq!(candidate.contracts, operator.contracts);
        assert_eq!(candidate.contract_count, 1);
        let ExpressionNode::Indexed(indexed) =
            program.expression_table.expression(operator_use.expression)
        else {
            panic!("indexed use");
        };
        let operands =
            crate::operators::indexed_operand_types(&program, indexed, operator_use.origin);
        let application = typed_trees::operator::closed_indexed_operator_application_for_operands(
            &program, operator, &operands,
        )
        .expect("closed indexed binding");
        let [
            typed_trees::operator::ClosedOperatorApplicationArgument::Type {
                binder_symbol,
                type_reference,
            },
        ] = application.as_slice()
        else {
            panic!("one element type binding");
        };
        assert_eq!(
            *binder_symbol,
            program.operator_type_parameters(operator)[0].symbol
        );
        assert_eq!(program.display_type_reference(*type_reference), "i32");
        if collection == "[i32; 3]" {
            assert!(
                resolve_spelling_for_operands(&program, OperatorSpelling::Index, &operands)
                    .is_empty(),
                "ordinary matching must not gain collection coercion"
            );
            assert!(
                typed_trees::operator::closed_operator_application_for_operands(
                    &program, operator, &operands
                )
                .is_none()
            );
        }
    }
}

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
fn indexed_element_binding_is_shared_with_the_remaining_tuple() {
    for (index_type, expected_status) in [
        (
            "i32",
            checked_trees::CheckedOperatorResolutionStatus::Resolved,
        ),
        (
            "u64",
            checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback,
        ),
    ] {
        let source = format!(
            r#"
            boundary operator [] Collection::read<Element>(items: &[Element], index: Element) -> Element;
            machine inspect(items: [i32; 3], index: {index_type}) {{ let value: i32 = items[index]; }}
        "#
        );
        let (_, facts) = indexed_selection_fixture(&source);
        let operator_use = facts
            .uses
            .iter()
            .find_map(|(_, value)| (value.spelling == OperatorSpelling::Index).then_some(value))
            .expect("index use");
        assert_eq!(operator_use.status, expected_status, "{source}");
        if expected_status == checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback {
            assert!(!operator_use.selected_operator_symbol.is_valid());
            assert_eq!(operator_use.candidate_count, 0);
        }
    }
}

#[test]
fn indexed_collection_views_do_not_rank_competing_candidates() {
    let (program, facts) = indexed_selection_fixture(
        r#"
        boundary operator [] Collection::generic<Element>(items: &[Element], index: u64) -> Element;
        boundary operator [] Collection::concrete(items: &[i32], index: u64) -> i32;
        machine inspect(items: [i32; 3], index: u64) { let value: i32 = items[index]; }
    "#,
    );
    let operator_use = facts
        .uses
        .iter()
        .find_map(|(_, value)| (value.spelling == OperatorSpelling::Index).then_some(value))
        .expect("index use");
    assert_eq!(
        operator_use.status,
        checked_trees::CheckedOperatorResolutionStatus::Ambiguous
    );
    assert_eq!(operator_use.candidate_count, 2);
    assert!(!operator_use.selected_operator_symbol.is_valid());
    assert_eq!(
        facts.candidate_symbols(operator_use).collect::<Vec<_>>(),
        program
            .operators()
            .iter()
            .map(|operator| operator.symbol)
            .collect::<Vec<_>>()
    );
}

#[test]
fn ranged_collection_views_check_both_endpoint_types() {
    for (end_type, expected_status) in [
        (
            "u64",
            checked_trees::CheckedOperatorResolutionStatus::Resolved,
        ),
        (
            "i32",
            checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback,
        ),
    ] {
        let source = format!(
            r#"
            boundary operator [..] Collection::window<Element>(items: &[Element], start: u64, end: u64) -> &[Element];
            machine inspect(items: [i32; 3], start: u64, end: {end_type}) {{ let value: &[i32] = items[start..end]; }}
        "#
        );
        let (_, facts) = indexed_selection_fixture(&source);
        let operator_use = facts
            .uses
            .iter()
            .find_map(|(_, value)| (value.spelling == OperatorSpelling::Range).then_some(value))
            .expect("range use");
        assert_eq!(operator_use.status, expected_status, "{source}");
        if expected_status == checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback {
            assert!(!operator_use.selected_operator_symbol.is_valid());
            assert_eq!(operator_use.candidate_count, 0);
        }
    }
}

#[test]
fn checked_software_may_satisfy_a_contracted_ordinary_operator() {
    let source = r#"
        data Amount { value: i32; }

        operator - Amount::subtract(left: Amount, right: Amount) -> Amount
        requires
            right == left;

        machine subtract(left: Amount, right: Amount) -> Amount
        satisfies Amount::subtract
        {
            left
        }
    "#;

    checked_program(source);
}

#[test]
fn signature_requires_selects_domain_operator_without_flow_lookup() {
    let source = r#"
        data Quantity { value: i32; }

        domain Quantity::Additive
        requires
            self.value >= 0;

        operator + Quantity::Additive::add(left: Quantity, right: Quantity) -> Quantity;

        data Main {}

        machine Main::combine(&self, left: Quantity, right: Quantity)
        requires
            left in Quantity::Additive
        {
            let sum: Quantity = left + right;
        }

        machine Main::main(&mut self) {}
    "#;

    let typed = typed_program(source);
    let combine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::combine")
        .expect("combine machine");
    let combine_state = typed
        .machine_states(combine)
        .first()
        .expect("combine state");
    assert!(
        typed
            .machine_contracts(combine)
            .iter()
            .chain(typed.state_contracts(combine_state))
            .any(|contract| {
                typed
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .any(|fact| match fact {
                        typed_trees::domain::ProofFact::Membership(membership) => {
                            typed.expression_table.display_name(membership.value) == "left"
                        }
                        _ => false,
                    })
            })
    );
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("signature selection should resolve +");

    assert!(checked.facts.operators.resolved_uses().any(|operator_use| {
        operator_use.spelling == OperatorSpelling::Add
            && checked
                .facts
                .operators
                .selected_candidate(operator_use)
                .is_some_and(|candidate| candidate.is_domain_owned())
    }));
}

#[test]
fn denotation_role_on_bodyless_declared_type_selects_domain_operator() {
    let source = r#"
        domain i32::Degrees;

        operator + i32::Degrees::add(left: i32, right: i32) -> i32;

        data Main {}

        machine Main::rotate(
            &self,
            value: i32 in Degrees & Wrapping,
            delta: i32 in Wrapping
        ) {
            let sum: i32 in Wrapping = value + delta;
        }

        machine Main::main(&mut self) {}
    "#;

    let checked = checked_program(source);
    assert!(has_selected_domain_add(&checked));
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
fn explicit_mint_initializer_selects_domain_operator() {
    let source = r#"
        domain i32::Degrees
        requires
            self >= 0;

        operator + i32::Degrees::add(left: i32, right: i32) -> i32;

        data Main {}

        machine Main::rotate(&self) {
            let value: i32 in Degrees & Wrapping = 1 as i32 in Degrees;
            let sum: i32 in Degrees & Wrapping = value + 1;
        }

        machine Main::main(&mut self) {}
    "#;

    let checked = checked_program(source);
    assert!(has_selected_domain_add(&checked));
}

#[test]
fn flow_established_membership_does_not_select_domain_operator() {
    let source = r#"
        domain i32::Degrees
        requires
            self >= 0;

        operator + i32::Degrees::add(left: i32, right: i32) -> i32;

        data Main { value: i32 in Wrapping; }

        machine Main::mark(&mut self)
        ensures
            self.value in i32::Degrees
        {
            self.value = 0;
        }

        machine Main::main(&mut self) {
            self.mark();
            let sum: i32 in Wrapping = self.value + 1;
        }
    "#;

    let checked = checked_program(source);
    assert!(!has_selected_domain_add(&checked));
    assert!(
        checked
            .facts
            .operators
            .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback)
            .any(|operator_use| operator_use.spelling == OperatorSpelling::Add)
    );
}

#[test]
fn declared_binding_selects_domain_index_operator() {
    let source = r#"
        data Buffer { value: i32; }

        domain Buffer::Indexed;

        operator [] Buffer::Indexed::index(items: Buffer, index: u64) -> i32;

        data Main {}

        machine Main::read(
            &self,
            items: Buffer in Indexed,
            index: u64
        ) {
            let value: i32 = items[index];
        }

        machine Main::main(&mut self) {}
    "#;

    let checked = checked_program(source);
    assert!(checked.facts.operators.resolved_uses().any(|operator_use| {
        operator_use.spelling == OperatorSpelling::Index
            && checked
                .facts
                .operators
                .selected_candidate(operator_use)
                .is_some_and(|candidate| candidate.is_domain_owned())
    }));
}

#[test]
fn binary_resolution_matches_the_complete_operand_tuple() {
    let i32_i32_symbol = SymbolHandle::from_arena_index(140);
    let i32_u64_symbol = SymbolHandle::from_arena_index(141);
    let machine_symbol = SymbolHandle::from_arena_index(142);
    let state_symbol = SymbolHandle::from_arena_index(143);
    let left_symbol = SymbolHandle::from_arena_index(144);
    let right_symbol = SymbolHandle::from_arena_index(145);

    let mut program = typed_trees::TypedTrees::default();
    let i32_type = named_type(&mut program, "i32");
    let u64_type = named_type(&mut program, "u64");
    for (operator_symbol, right_type) in [(i32_i32_symbol, i32_type), (i32_u64_symbol, u64_type)] {
        let mut operator = operator_with_spelling(operator_symbol, OperatorSpelling::Add);
        for (symbol, name, type_reference) in [
            (left_symbol, "left", i32_type),
            (right_symbol, "right", right_type),
        ] {
            program.push_operator_parameter(
                &mut operator,
                StateParameter {
                    symbol,
                    name: Identifier::generated(name),
                    type_reference,
                    is_const: false,
                    is_mutable: false,
                    is_self: false,
                    relevance: language_core::BindingRelevance::Relevant,
                },
            );
        }
        program.push_operator(operator);
    }

    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Main"),
        ..Default::default()
    };
    let mut state = State {
        symbol: state_symbol,
        name: Identifier::generated("entry"),
        ..Default::default()
    };
    for (symbol, name, type_reference) in [
        (left_symbol, "left", i32_type),
        (right_symbol, "right", u64_type),
    ] {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol,
                name: Identifier::generated(name),
                type_reference,
                is_const: false,
                is_mutable: false,
                is_self: false,
                relevance: language_core::BindingRelevance::Relevant,
            },
        );
    }
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);

    let left = program
        .expression_table
        .insert_tree(&Expression::Name(NamePath::resolved(
            vec![Identifier::generated("left")],
            left_symbol,
            left_symbol,
        )));
    let right = program
        .expression_table
        .insert_tree(&Expression::Name(NamePath::resolved(
            vec![Identifier::generated("right")],
            right_symbol,
            right_symbol,
        )));
    let binary = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator: BinaryOperator::Add,
            right,
        }));
    let origin = checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        statement_index: 0,
        role: checked_trees::CheckedValueStatementRole::Expression,
    };
    let mut value_roots = arena::Arena::default();
    value_roots.append(checked_trees::CheckedValueFact {
        expression: binary,
        origin,
        ..Default::default()
    });

    let facts = build_operator_facts(
        &program,
        &checked_trees::CheckedValueFacts::with_roots(value_roots),
    );
    let operator_use = facts
        .expression_use_in_origin(binary, origin)
        .expect("binary operator use");
    assert_eq!(operator_use.candidate_count, 1);
    assert_eq!(operator_use.selected_operator_symbol, i32_u64_symbol);
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
        )
        .len(),
        1
    );
    assert!(
        resolve_spelling_for_operands(
            &program,
            OperatorSpelling::Add,
            &[Some(right_type), Some(receiver_type)],
        )
        .is_empty()
    );
}

#[test]
fn complete_operand_matching_shares_generic_bindings_across_positions() {
    let generic_operator_symbol = SymbolHandle::from_arena_index(146);
    let heterogeneous_operator_symbol = SymbolHandle::from_arena_index(147);
    let type_parameter_symbol = SymbolHandle::from_arena_index(148);
    let left_symbol = SymbolHandle::from_arena_index(149);
    let right_symbol = SymbolHandle::from_arena_index(150);

    let mut program = typed_trees::TypedTrees::default();
    let i32_type = named_type(&mut program, "i32");
    let u64_type = named_type(&mut program, "u64");
    let type_parameter = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: type_parameter_symbol,
            name: Identifier::generated("T"),
        });

    let mut generic_operator =
        operator_with_spelling(generic_operator_symbol, OperatorSpelling::Add);
    program.push_operator_type_parameter(
        &mut generic_operator,
        typed_trees::data::TypeParameter {
            symbol: type_parameter_symbol,
            name: Identifier::generated("T"),
            kind: typed_trees::data::TypeParameterKind::Type,
            bounds: typed_trees::data::DataProperties::default(),
        },
    );
    for (symbol, name) in [(left_symbol, "left"), (right_symbol, "right")] {
        program.push_operator_parameter(
            &mut generic_operator,
            StateParameter {
                symbol,
                name: Identifier::generated(name),
                type_reference: type_parameter,
                is_const: false,
                is_mutable: false,
                is_self: false,
                relevance: language_core::BindingRelevance::Relevant,
            },
        );
    }
    program.push_operator(generic_operator);

    let mut heterogeneous_operator =
        operator_with_spelling(heterogeneous_operator_symbol, OperatorSpelling::Add);
    for (symbol, name, type_reference) in [
        (left_symbol, "left", i32_type),
        (right_symbol, "right", u64_type),
    ] {
        program.push_operator_parameter(
            &mut heterogeneous_operator,
            StateParameter {
                symbol,
                name: Identifier::generated(name),
                type_reference,
                is_const: false,
                is_mutable: false,
                is_self: false,
                relevance: language_core::BindingRelevance::Relevant,
            },
        );
    }
    program.push_operator(heterogeneous_operator);

    let candidates = resolve_spelling_for_operands(
        &program,
        OperatorSpelling::Add,
        &[Some(i32_type), Some(u64_type)],
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].operator.symbol, heterogeneous_operator_symbol);
}
