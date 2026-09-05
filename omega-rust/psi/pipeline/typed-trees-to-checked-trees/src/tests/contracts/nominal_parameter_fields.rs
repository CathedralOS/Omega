use super::*;

const DEFINITIONS: &str = r#"
    domain [u8; 4]::Utf8 requires valid_utf8(self);
    data Text { bytes: [u8; 4] in Utf8; }
    data Packet { payload: Text; tag: u64; }
"#;

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
    match lower_typed_trees(typed) {
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
fn nominal_self_edge_rechecks_attached_default_fields() {
    for field in ["self.bytes", "self.packet.payload.bytes"] {
        for (restore, accepted) in [(true, true), (false, false)] {
            let repair = if restore {
                format!("{field} = \"okay\";")
            } else {
                String::new()
            };
            let source = format!(
                r#"{DEFINITIONS}
                data Main {{ bytes: [u8; 4] in Utf8; packet: Packet; }}
                machine Main::run(&mut self) {{
                    {field}[0] = 255;
                    {repair}
                    transition {{ _ -> self }}
                }}
            "#
            );
            check_nominal_self_edge(&source, accepted);
        }
    }
}

#[test]
fn nominal_self_edge_rechecks_named_state_attached_fields() {
    for (repair, accepted) in [("self.packet.payload.bytes = \"okay\";", true), ("", false)] {
        let source = format!(
            r#"{DEFINITIONS}
            data Main {{ packet: Packet; }}
            machine Main::run(&mut self) {{
                transition {{ _ -> visit() }}
                state visit(&mut self) {{
                    self.packet.payload.bytes[0] = 255;
                    {repair}
                    transition {{ _ -> self }}
                }}
            }}
        "#
        );
        check_nominal_self_edge(&source, accepted);
    }
}

#[test]
fn nominal_self_edge_rechecks_raw_parameter_default_fields() {
    for (repair, accepted) in [("packet.payload.bytes = \"okay\";", true), ("", false)] {
        let source = format!(
            r#"{DEFINITIONS}
            machine run(packet: &mut Packet) {{
                packet.payload.bytes[0] = 255;
                {repair}
                transition {{ _ -> self }}
            }}
        "#
        );
        check_nominal_self_edge(&source, accepted);
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
fn readable_mutable_nominal_parameters_carry_declared_field_facts() {
    for parameter in ["packet: &mut Packet", "mut packet: Packet"] {
        for body in ["", "packet.tag = 3;", "packet.payload.bytes = \"okay\";"] {
            let source = format!(
                "{DEFINITIONS}\nmachine keep({parameter}) ensures packet.payload.bytes in Utf8 {{ {body} }}"
            );
            lower_typed_trees(parse_typed_trees(&source))
                .unwrap_or_else(|diagnostics| panic!("{parameter}; {body}: {diagnostics:#?}"));
        }
    }
}

#[test]
fn whole_nominal_replacement_reestablishes_declared_field_facts() {
    for body in [
        "packet = replacement;",
        "packet.payload = replacement.payload;",
    ] {
        let source = format!(
            "{DEFINITIONS}\nmachine replace(packet: &mut Packet, replacement: Packet) ensures packet.payload.bytes in Utf8 {{ {body} }}"
        );
        lower_typed_trees(parse_typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{body}: {diagnostics:#?}"));
    }
}

#[test]
fn mutated_nominal_parameter_fields_cannot_reuse_entry_facts() {
    for body in [
        "packet.payload.bytes[0] = 255;",
        "let alias: &mut [u8; 4] = &mut packet.payload.bytes; alias[0] = 255;",
        "corrupt(&mut packet.payload.bytes);",
    ] {
        let source = format!(
            r#"{DEFINITIONS}
            machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
            machine invalid(packet: &mut Packet)
            ensures packet.payload.bytes in Utf8 {{ {body} }}
        "#
        );
        let diagnostics = lower_typed_trees(parse_typed_trees(&source)).expect_err(body);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{body}: {diagnostics:#?}"
        );
    }
}

#[test]
fn write_only_nominal_parameters_do_not_grant_readable_entry_facts() {
    let source = format!(
        "{DEFINITIONS}\nmachine invalid(packet: &write Packet) ensures packet.payload.bytes in Utf8 {{ }}"
    );
    lower_typed_trees(parse_typed_trees(&source))
        .expect_err("write-only repair access does not establish the incoming content predicate");
}

#[test]
fn nominal_parameter_entry_facts_require_valid_caller_fields() {
    let source = format!(
        r#"{DEFINITIONS}
        machine keep(packet: &mut Packet) ensures packet.payload.bytes in Utf8 {{ }}
        machine invalid(packet: &mut Packet) {{
            packet.payload.bytes[0] = 255;
            keep(packet);
        }}
    "#
    );
    assert!(
        lower_typed_trees(parse_typed_trees(&source)).is_err(),
        "calling a nominal identity cannot repair a broken default field invariant"
    );
}

#[test]
fn nominal_call_inputs_preserve_checked_construction_evidence() {
    for body in [
        "consume(Packet {payload: Text {bytes: \"okay\"}, tag: 0});",
        "let packet: Packet = Packet {payload: Text {bytes: \"okay\"}, tag: 0}; consume(packet);",
    ] {
        let source = format!(
            "{DEFINITIONS}\nmachine consume(packet: Packet) ensures packet.payload.bytes in Utf8 {{ }}\nmachine start() {{ {body} }}"
        );
        lower_typed_trees(parse_typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{body}: {diagnostics:#?}"));
    }
}

#[test]
fn nominal_copy_cannot_repair_an_invalidated_source_field() {
    let source = format!(
        r#"{DEFINITIONS}
        machine invalid(output: &mut Packet, source: &mut Packet)
        ensures output.payload.bytes in Utf8 {{
            source.payload.bytes[0] = 255;
            output.payload = source.payload;
        }}"#
    );
    assert!(lower_typed_trees(parse_typed_trees(&source)).is_err());
}

#[test]
fn constructed_field_snapshots_are_invalidated_at_their_destination() {
    for write in [
        "packet.payload.bytes[0] = 255;",
        "let alias: &mut [u8; 4] = &mut packet.payload.bytes; alias[0] = 255;",
    ] {
        let source = format!(
            r#"{DEFINITIONS}
            machine consume(packet: Packet) ensures packet.payload.bytes in Utf8 {{ }}
            machine invalid() {{
                let mut packet: Packet = Packet {{payload: Text {{bytes: "okay"}}, tag: 0}};
                {write}
                consume(packet);
            }}"#
        );
        assert!(
            lower_typed_trees(parse_typed_trees(&source)).is_err(),
            "{write}"
        );
    }
}

#[test]
fn disjoint_machine_field_updates_preserve_arrival_obligations() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        data Main { left: [u8; 4] in Utf8; right: [u8; 4] in Utf8; }
        machine Main::run(&mut self) ensures self.right in Utf8 {
            self.left = "okay";
            transition { _ -> finish() }
            state finish(&mut self) { }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
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
    let operations = flow_effects::infer_operational_may(&typed);
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
        lower_typed_trees(typed).is_err(),
        "changing the index must not prove the newly selected row valid"
    );
}
