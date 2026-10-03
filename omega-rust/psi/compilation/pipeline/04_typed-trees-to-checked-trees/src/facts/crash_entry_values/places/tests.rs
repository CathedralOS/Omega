use super::entry_place;
use crate::tests::front_end::typed_program;
use checked_trees::CrashPredicateExpression;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

fn observed_entry(source: &str, state_name: Option<&str>) -> Option<CrashPredicateExpression> {
    let program = typed_program(source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let states = program.machine_states(machine);
    let state = state_name.map_or(&states[0], |name| {
        states
            .iter()
            .find(|state| state.name.as_str() == name)
            .unwrap()
    });
    let (ordinal, argument) = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .find_map(|(ordinal, statement)| {
            let arguments = match statement {
                StatementNode::Call(call) => {
                    program.statement_table.expression_handles(call.arguments)
                }
                StatementNode::Expression(expression) => {
                    match program.expression_table.expression(*expression) {
                        ExpressionNode::Call(call) => {
                            program.expression_table.expression_handles(call.arguments)
                        }
                        _ => return None,
                    }
                }
                _ => return None,
            };
            arguments
                .first()
                .copied()
                .map(|argument| (ordinal, argument))
        })
        .expect("observed sink call");
    let place = crate::flow::canonical_place_from_expression_in_state(
        &program,
        state.symbol,
        ordinal,
        argument,
    )
    .expect("observed canonical place");
    entry_place(&program, machine.symbol, state.symbol, ordinal, &place)
}

fn ready() -> CrashPredicateExpression {
    CrashPredicateExpression::Member {
        receiver: Box::new(CrashPredicateExpression::Parameter(0)),
        member: "ready".to_owned(),
    }
}

#[test]
fn owned_field_keeps_entry_identity_only_while_its_storage_is_pristine() {
    for (written, expected) in [("other", Some(ready())), ("ready", None)] {
        let source = format!(
            "data Token {{ ready: bool; other: bool; }}
             machine sink(value: bool) {{}}
             machine value(mut input: Token) {{ input.{written} = false; sink(input.ready); }}"
        );
        assert_eq!(
            observed_entry(&source, None),
            expected,
            "write to {written}"
        );
    }
}

#[test]
fn owned_alias_and_named_state_preserve_the_original_parameter() {
    let source = "data Token { ready: bool; other: bool; }
        machine sink(value: bool) {}
        machine value(input: Token) -> bool {
            let held: Token = input;
            transition true { true -> next(held) false -> false }
            state next(mut received: Token) -> bool {
                received.other = false;
                sink(received.ready);
                true
            }
        }";
    assert_eq!(observed_entry(source, Some("next")), Some(ready()));
    assert_eq!(
        observed_entry(
            &source.replace("received.other =", "received.ready ="),
            Some("next")
        ),
        None
    );
}

#[test]
fn fixed_element_provenance_distinguishes_sibling_writes() {
    for (written, present) in [(1, true), (0, false)] {
        let source = format!(
            "machine sink(value: bool) {{}}
             machine value(mut input: [bool; 2]) {{ input[{written}] = false; sink(input[0]); }}"
        );
        let expected = present.then(|| CrashPredicateExpression::Indexed {
            collection: Box::new(CrashPredicateExpression::Parameter(0)),
            index: Box::new(CrashPredicateExpression::Integer("0".to_owned())),
        });
        assert_eq!(observed_entry(&source, None), expected);
    }
}

#[test]
fn nominal_owner_permits_stable_field_observation_without_whole_value_provenance() {
    for parameter in ["mut input: Token", "input: &mut Token"] {
        for (written, expected) in [("other", Some(ready())), ("ready", None)] {
            let source = format!(
                "data Token {{ ready: bool; other: bool; }}
                 machine Token::drop(owner: &mut Token) {{}}
                 machine sink(value: bool) {{}}
                 machine value({parameter}) {{ input.{written} = false; sink(input.ready); }}"
            );
            assert_eq!(
                observed_entry(&source, None),
                expected,
                "{parameter}, write to {written}"
            );
        }
    }
    let source = "data Token { ready: bool; }
        machine Token::drop(owner: &mut Token) {}
        machine sink(value: Token) {}
        machine value(input: Token) { sink(input); }";
    assert_eq!(observed_entry(source, None), None);
}

#[test]
fn projection_cannot_hide_interior_authority_behind_a_shared_descriptor() {
    for (sibling_type, present) in [("bool", true), ("AtomicU64", false)] {
        let source = format!(
            "data Shared {{ ready: bool; sibling: {sibling_type}; }}
             data Token {{ shared: &Shared; }}
             machine Token::drop(owner: &mut Token) {{}}
             machine sink(value: bool) {{}}
             machine value(input: Token) {{ sink(input.shared.ready); }}"
        );
        let expected = present.then(|| CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Member {
                receiver: Box::new(CrashPredicateExpression::Parameter(0)),
                member: "shared".to_owned(),
            }),
            member: "ready".to_owned(),
        });
        assert_eq!(observed_entry(&source, None), expected, "{sibling_type}");
    }
}

#[test]
fn field_identity_must_belong_to_the_observed_parameter_type() {
    let program = typed_program(
        "data Token { ready: bool; }
         data Other { ready: bool; }
         machine value(input: Token) {}",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let parameter = &program.state_parameters(state)[0];
    for name in ["Token", "Other"] {
        let data = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == name)
            .unwrap();
        let typed_trees::data::DataMember::Field(field) = &program.data_members(data)[0] else {
            panic!("field");
        };
        let place = crate::flow::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(parameter.symbol),
            segments: vec![facts::PlaceSegment::Field {
                symbol: field.symbol,
            }],
        };
        assert_eq!(
            entry_place(&program, machine.symbol, state.symbol, 0, &place),
            (name == "Token").then(ready),
            "{name}"
        );
    }
}

#[test]
fn nominal_payload_projection_preserves_its_case_qualification() {
    let program = typed_program(
        "data Choice { case Present(ready: bool); case Missing; }
         machine Choice::drop(owner: &mut Choice) {}
         machine value(input: Choice) {}",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let parameter = &program.state_parameters(state)[0];
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Choice")
        .unwrap();
    let typed_trees::data::DataMember::Variant(variant) = &program.data_members(data)[0] else {
        panic!("variant");
    };
    let field = &program.data_payload_fields(variant)[0];
    let place = crate::flow::CanonicalPlace {
        root: facts::PlaceRoot::Symbol(parameter.symbol),
        segments: vec![
            facts::PlaceSegment::Case {
                variant: variant.symbol,
            },
            facts::PlaceSegment::Field {
                symbol: field.symbol,
            },
        ],
    };
    assert_eq!(
        entry_place(&program, machine.symbol, state.symbol, 0, &place),
        Some(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(0)),
            member: "Present::ready".to_owned(),
        })
    );
}
