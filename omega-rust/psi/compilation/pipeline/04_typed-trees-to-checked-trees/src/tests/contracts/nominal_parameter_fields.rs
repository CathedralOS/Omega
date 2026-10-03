use crate::CheckingRequest;
use crate::borrow::build_borrow_facts;
use crate::flow::build_domain_facts;
use crate::flow::build_flow_facts;
use crate::lower_typed_trees;
use crate::proof::build_proof_facts;
use crate::semantic::facts::build_semantic_facts;
use crate::tests::contracts::parse_typed_trees;

const DEFINITIONS: &str = r#"
    domain [u8; 4]::Utf8 requires valid_utf8(self);
    data Text { bytes: [u8; 4] in Utf8; }
    data Packet { payload: Text; tag: u64; }
"#;

#[test]
fn boundary_write_preserves_disjoint_declared_field_domains() {
    let source = r#"
        domain [u8; 8]::Utf8(bytes) requires valid_utf8(bytes);
        pub data Line { case Invalid; case Complete(count: u64); case End(count: u64); }
        pub boundary trait Console { machine read(input: &mut [u8]) -> Line blocks; crashes Trap; }
        data Room { label: [u8; 8] in Utf8; }
        data Game { console: Binding<Console>; room: Room; input: [u8; 32]; }
        machine Game::main(game: &mut Game) reaches Console {
            transition block game.console.read(&mut game.input) {
                Line::Complete { count } -> received(game, count)
                Line::End { count } -> received(game, count)
                _ -> done(game)
            }
            state received(game: &mut Game, count: u64) { transition { _ -> done(game) } }
            state done(game: &mut Game) {}
        }
    "#;
    for (field, accepted) in [("input", true), ("room.label", false)] {
        let source = source.replace("&mut game.input", &format!("&mut game.{field}"));
        let mut typed = crate::tests::front_end::typed_program_with_core_service(&source);
        crate::tests::bind_fixture_fused_service_erasures(&mut typed);
        match lower_typed_trees(typed, &CheckingRequest::settled()) {
            Ok(_) => assert!(
                accepted,
                "writing the qualified field must retire its domain"
            ),
            Err(diagnostics) => {
                assert!(
                    !accepted,
                    "a sibling write must preserve its facts: {diagnostics:#?}"
                );
                assert!(
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("Utf8")),
                    "{diagnostics:#?}"
                );
            }
        }
    }
}

#[test]
fn receiver_and_named_inputs_share_nonzero_nested_field_obligations() {
    for (signature, subject, next_signature, next_call) in [
        ("Packet::touch(&mut self)", "self", "&mut self", "next()"),
        (
            "touch(packet: &mut Packet)",
            "packet",
            "packet: &mut Packet",
            "next(packet)",
        ),
    ] {
        let source = format!(
            r#"
            domain [u8; 8]::NonEmpty(bytes) requires non_empty(bytes);
            data Label {{ text: [u8; 8] in NonEmpty; }}
            data Packet {{ label: Label; }}
            machine observe(text: &[u8; 8] in NonEmpty) {{}}
            machine {signature} {{
                observe(&{subject}.label.text);
                {subject}.label.text = "changed";
                transition {{ _ -> {next_call} }}
                state next({next_signature}) {{ observe(&{subject}.label.text); }}
            }}
        "#
        );
        lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
            .unwrap_or_else(|diagnostics| panic!("{signature}: {diagnostics:#?}"));
    }
}

#[test]
fn receiver_and_named_inputs_reject_unrestored_mutation() {
    for (signature, subject) in [
        ("Packet::touch(&mut self)", "self"),
        ("touch(packet: &mut Packet)", "packet"),
    ] {
        for continuation in ["", "transition { _ -> self }"] {
            let source = format!(
                r#"
                domain [u8; 8]::NonEmpty(bytes) requires non_empty(bytes);
                data Label {{ text: [u8; 8] in NonEmpty; }}
                data Packet {{ label: Label; }}
                machine clear(text: &mut [u8; 8]) {{ text = ""; }}
                machine {signature} {{ clear(&mut {subject}.label.text); {continuation} }}
            "#
            );
            let diagnostics =
                lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
                    .expect_err("an unqualified mutable alias cannot restore the declared field");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("NonEmpty")),
                "{signature}: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn receiver_and_named_input_requires_are_not_implicit_postconditions() {
    for (signature, subject) in [
        ("Packet::clear(&mut self)", "self"),
        ("clear(packet: &mut Packet)", "packet"),
    ] {
        let source = format!(
            r#"
            domain [u8; 8]::NonEmpty(bytes) requires non_empty(bytes);
            data Packet {{ text: [u8; 8]; }}
            machine {signature} requires {subject}.text in NonEmpty {{ {subject}.text = ""; }}
            "#
        );
        lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
            .expect("extra input requirements are not declared field invariants");
    }
}

fn check_nominal_self_edge(source: &str, accepted: bool) {
    let typed = parse_typed_trees(source);
    check_typed_nominal_self_edge(typed, source, accepted);
}

fn check_typed_nominal_self_edge(typed: typed_trees::TypedTrees, source: &str, accepted: bool) {
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
    assert!(
        typed.machines().iter().any(|machine| {
            typed.machine_states(machine).iter().any(|state| {
                typed
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .any(|statement| {
                        matches!(statement, StatementNode::Transition(transition)
                    if [transition.target, transition.continuation].into_iter().any(|target|
                        target.is_valid() && matches!(typed.statement_table.transition_target(target),
                            TransitionTargetNode::SelfTarget)))
                    })
            })
        }),
        "the regression must exercise an actual SelfTarget, not a named call"
    );
    match lower_typed_trees(typed, &CheckingRequest::settled()) {
        Ok(_) => assert!(
            accepted,
            "a corrupted default field crossed SelfTarget: {source}"
        ),
        Err(diagnostics) => {
            assert!(
                !accepted,
                "a restored field should satisfy its back-edge: {diagnostics:#?}\n{source}"
            );
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("cannot prove state arrival contract on self-transition")),
                "the self-edge obligation must reject the corrupted field: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn nominal_self_edge_uses_only_its_own_guard_polarity() {
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
    for (guard, self_arm, accepted) in [
        ("self.flag", "true", true),
        ("self.flag", "false", false),
        ("!self.flag", "false", true),
        ("!self.flag", "true", false),
    ] {
        let (true_target, false_target) = if self_arm == "true" {
            ("self", "done()")
        } else {
            ("done()", "self")
        };
        let source = format!(
            r#"
            data Main {{ flag: bool; }}
            machine Main::run(&mut self) {{
                transition self.flag {{ true -> waiting() false -> done() }}
                state waiting(&mut self)
                requires self.flag;
                {{
                    self.flag = false;
                    transition {guard} {{ true -> {true_target} false -> {false_target} }}
                }}
                state done(&mut self) {{}}
            }}
        "#
        );
        let original = parse_typed_trees(&source);
        for combined in [false, true] {
            let mut typed = original.clone();
            if combined {
                let machine = typed
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == "Main::run")
                    .expect("run")
                    .clone();
                let state_index = typed
                    .machine_states(&machine)
                    .iter()
                    .position(|state| state.name.as_str() == "waiting")
                    .expect("waiting");
                let nodes = typed.machine_states(&machine)[state_index].statement_nodes;
                let transitions = typed
                    .statement_table
                    .statements(nodes)
                    .iter()
                    .enumerate()
                    .filter_map(|(index, statement)| {
                        matches!(statement, StatementNode::Transition(_)).then_some(index)
                    })
                    .collect::<Vec<_>>();
                let first_index = transitions[0];
                if let [first, second] = transitions.as_slice() {
                    assert_eq!(*first + 1, *second);
                    assert_eq!(*second + 1, nodes.count() as usize);
                    let StatementNode::Transition(second_transition) =
                        &typed.statement_table.statements(nodes)[*second]
                    else {
                        unreachable!();
                    };
                    let continuation = second_transition.target;
                    let StatementNode::Transition(first_transition) =
                        &mut typed.statement_table.statements_mut(nodes)[*first]
                    else {
                        unreachable!();
                    };
                    assert!(!first_transition.continuation.is_valid());
                    first_transition.continuation = continuation;
                    typed.machine_states_mut(&machine)[state_index].statement_nodes =
                        arena::HandleSpan::from_parts(nodes.start(), nodes.count() - 1);
                } else {
                    assert_eq!(transitions.len(), 1);
                }
                let StatementNode::Transition(transition) =
                    &typed.statement_table.statements(nodes)[first_index]
                else {
                    unreachable!();
                };
                assert!(transition.continuation.is_valid());
                if self_arm == "false" {
                    assert!(
                        matches!(
                            typed
                                .statement_table
                                .transition_target(transition.continuation),
                            TransitionTargetNode::SelfTarget
                        ),
                        "the false arm must be represented by an actual continuation SelfTarget"
                    );
                }
            }
            check_typed_nominal_self_edge(
                typed,
                &format!("combined={combined}\n{source}"),
                accepted,
            );
        }
    }
}

#[test]
fn nominal_self_edge_checks_fields_after_guard_call_effects() {
    for (effect, contract, accepted) in [
        ("", "", true),
        ("bytes = \"okay\";", "ensures bytes in Utf8", true),
        ("bytes[0] = 255;", "", false),
    ] {
        let source = format!(
            r#"{DEFINITIONS}
            data Main {{ packet: Packet; }}
            machine decide(bytes: &mut [u8; 4]) -> bool {contract} {{ {effect} true }}
            machine Main::run(&mut self) {{
                transition decide(&mut self.packet.payload.bytes) {{
                    true -> self
                    false -> done()
                }}
                state done(&mut self) {{}}
            }}
        "#
        );
        check_nominal_self_edge(&source, accepted);
    }
}

#[test]
fn constructed_values_do_not_publish_snapshots_at_mutable_indices() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        data Row { bytes: [u8; 4] in Utf8; }
        machine consume(row: Row) ensures row.bytes in Utf8 { }
        machine invalid(rows: &mut [Row; 2], mut index: u64) {
            rows[index] = Row {bytes: "okay"};
            index = 1;
            consume(rows[index]);
        }
    "#;
    let typed = parse_typed_trees(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let _ = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    for (_, fact) in semantic.facts.iter() {
        if !matches!(fact.payload, facts::FactPayload::AssignedValue { .. }) {
            continue;
        }
        let facts::FactPlace::Place(place) = fact.place else {
            continue;
        };
        assert!(
            semantic
                .place_segments
                .span_or_empty(semantic.places.get(place).segments)
                .iter()
                .all(|segment| {
                    !matches!(
                        segment,
                        facts::PlaceSegment::Index { .. } | facts::PlaceSegment::FixedRange { .. }
                    )
                }),
            "a mutable selector cannot identify a retained constructor snapshot"
        );
    }
    assert!(
        lower_typed_trees(typed, &CheckingRequest::settled()).is_err(),
        "changing the index must not prove the newly selected row valid"
    );
}
