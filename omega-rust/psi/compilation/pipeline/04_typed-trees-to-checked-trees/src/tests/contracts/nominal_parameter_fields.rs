use crate::CheckingRequest;
use crate::borrow::build_borrow_facts;
use crate::flow::build_domain_facts;
use crate::flow::build_flow_facts;
use crate::lower_typed_trees;
use crate::proof::build_proof_facts;
use crate::semantic::facts::build_semantic_facts;
use crate::tests::contracts::parse_typed_trees;

const DEFINITIONS: &str = r#"
    domain [u8; 4]::Utf8(bytes) requires valid_utf8(bytes);
    data Text { bytes: [u8; 4] in Utf8; }
    data Packet { payload: Text; tag: u64; }
"#;

const CASE_DEFINITIONS: &str = r#"
    domain i32::Ten(value) requires 0 <= value && value <= 10;
    data Lamp { case Dark; case Lit(amount: i32 in Ten); }
    data Holder { lamp: Lamp; }
    machine consume(holder: &mut Holder) {}
"#;

#[test]
fn mutable_call_restores_conditional_payload_invariants() {
    let source = format!(
        r#"{CASE_DEFINITIONS}
        machine install(holder: &mut Holder) {{
            let replacement: Lamp = Lamp::Lit {{ amount: 4 }};
            holder.lamp = replacement;
        }}
        machine inspect(holder: &mut Holder) {{
            install(holder);
            consume(holder);
            transition holder.lamp {{
                Lamp::Lit {{ amount }} -> active(holder, amount)
                Lamp::Dark -> inactive(holder)
            }}
            state active(holder: &mut Holder, amount: i32)
                requires amount >= 0 && amount <= 10 {{}}
            state inactive(holder: &mut Holder) {{}}
        }}
        "#
    );
    lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn inactive_payload_obligations_discharge_at_calls_and_returns() {
    let source = format!(
        r#"{CASE_DEFINITIONS}
        machine extinguish(holder: &mut Holder) {{
            holder.lamp = Lamp::Dark;
            consume(holder);
        }}
        machine replace(holder: &mut Holder) {{
            extinguish(holder);
            consume(holder);
        }}
        machine empty() -> Lamp {{ Lamp::Dark }}
        "#
    );
    lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn mutable_return_rejects_a_corrupted_active_payload() {
    let source = format!(
        r#"{CASE_DEFINITIONS}
        machine corrupt(amount: &mut i32) {{ amount = -1; }}
        machine invalidate(holder: &mut Holder) {{
            holder.lamp = Lamp::Lit {{ amount: 4 }};
            corrupt(&mut holder.lamp.amount);
        }}
        "#
    );
    let diagnostics = lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
        .expect_err("a call returning a whole value must restore the active payload");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("default-domain field requirement for return")
                && diagnostic.message.contains("Ten")
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn restored_payload_invariants_do_not_restore_an_old_case_tag() {
    let source = format!(
        r#"{CASE_DEFINITIONS}
        machine extinguish(holder: &mut Holder) {{ holder.lamp = Lamp::Dark; }}
        machine observe(amount: i32 in Ten) {{}}
        machine stale(holder: &mut Holder) {{
            holder.lamp = Lamp::Lit {{ amount: 4 }};
            extinguish(holder);
            observe(holder.lamp.amount);
        }}
        "#
    );
    let diagnostics = lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
        .expect_err("a conditional payload invariant grants no live case selection");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("observe") && diagnostic.message.contains("Ten")
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn mutable_call_restoration_keeps_exact_indexed_payload_instances() {
    let source = r#"
        domain<const Bound: u64> u64::AtMost<Bound>(value) requires value <= Bound;
        data Limits { case Pair(first: u64 in AtMost<10>, second: u64 in AtMost<20>); }
        machine inspect(limits: &mut Limits) {}
        machine forward(limits: &mut Limits) { inspect(limits); }
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
    let forward = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .unwrap();
    let state = &typed.machine_states(forward)[0];
    let root = typed.state_parameters(state)[0].symbol;
    let mut restored = Vec::new();
    for (_, fact) in semantic.facts.iter() {
        if !matches!(fact.point, facts::ProgramPoint::CallEnsures { machine_symbol, .. } if machine_symbol == forward.symbol)
        {
            continue;
        }
        let (
            facts::FactPlace::Place(place),
            facts::FactPayload::DomainMembership {
                domain_symbol,
                semantic_domain,
                ..
            },
        ) = (fact.place, fact.payload)
        else {
            continue;
        };
        let place = semantic.places.get(place);
        if place.root == facts::PlaceRoot::Symbol(root) {
            restored.push((
                semantic
                    .place_segments
                    .span_or_empty(place.segments)
                    .to_vec(),
                domain_symbol,
                semantic_domain,
            ));
        }
    }
    let limits = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Limits")
        .unwrap();
    let declared = crate::facts::field_domain::declared_data_field_domain_rows(&typed, limits);
    assert_eq!(
        restored.len(),
        2,
        "each payload instance must be restored once"
    );
    for row in declared {
        assert!(row.semantic_domain.is_valid());
        assert!(restored.contains(&(row.path, row.domain_symbol, row.semantic_domain)));
    }
    assert_ne!(
        restored[0].2, restored[1].2,
        "equal domain names cannot merge distinct indices"
    );
}

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
fn qualified_and_free_inputs_share_nonzero_nested_field_obligations() {
    for signature in [
        "Packet::touch(packet: &mut Packet)",
        "touch(packet: &mut Packet)",
    ] {
        let source = format!(
            r#"
            domain [u8; 8]::NonEmpty(bytes) requires non_empty(bytes);
            data Label {{ text: [u8; 8] in NonEmpty; }}
            data Packet {{ label: Label; }}
            machine observe(text: &[u8; 8] in NonEmpty) {{}}
            machine {signature} {{
                observe(&packet.label.text);
                packet.label.text = "changed";
                transition {{ _ -> next(packet) }}
                state next(packet: &mut Packet) {{ observe(&packet.label.text); }}
            }}
        "#
        );
        lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
            .unwrap_or_else(|diagnostics| panic!("{signature}: {diagnostics:#?}"));
    }
}

#[test]
fn qualified_and_free_inputs_reject_unrestored_mutation() {
    for signature in [
        "Packet::touch(packet: &mut Packet)",
        "touch(packet: &mut Packet)",
    ] {
        for body in [
            "clear(&mut packet.label.text);",
            "transition { _ -> repeat(packet) }
             state repeat(packet: &mut Packet) {
                 clear(&mut packet.label.text);
                 transition { _ -> repeat(packet) }
             }",
        ] {
            let source = format!(
                r#"
                domain [u8; 8]::NonEmpty(bytes) requires non_empty(bytes);
                data Label {{ text: [u8; 8] in NonEmpty; }}
                data Packet {{ label: Label; }}
                machine clear(text: &mut [u8; 8]) {{ text = ""; }}
                machine {signature} {{ {body} }}
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
fn qualified_and_free_input_requires_are_not_implicit_postconditions() {
    for signature in [
        "Packet::clear(packet: &mut Packet)",
        "clear(packet: &mut Packet)",
    ] {
        let source = format!(
            r#"
            domain [u8; 8]::NonEmpty(bytes) requires non_empty(bytes);
            data Packet {{ text: [u8; 8]; }}
            machine {signature} requires packet.text in NonEmpty {{ packet.text = ""; }}
        "#
        );
        lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
            .expect("extra input requirements are not declared field invariants");
    }
}

fn check_nominal_back_edge(source: &str, accepted: bool, rejection: &str) {
    match lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()) {
        Ok(_) => assert!(
            accepted,
            "an invalid field crossed the named back-edge: {source}"
        ),
        Err(diagnostics) => {
            assert!(
                !accepted,
                "a restored field should satisfy its back-edge: {diagnostics:#?}\n{source}"
            );
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(rejection)),
                "the named arrival obligation must reject the invalid field: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn nominal_named_back_edge_uses_only_its_own_guard_polarity() {
    for (guard, back_edge_arm, accepted) in [
        ("main.flag", "true", true),
        ("main.flag", "false", false),
        ("!main.flag", "false", true),
        ("!main.flag", "true", false),
    ] {
        let (true_target, false_target) = if back_edge_arm == "true" {
            ("waiting(main)", "done(main)")
        } else {
            ("done(main)", "waiting(main)")
        };
        let source = format!(
            r#"
            data Main {{ flag: bool; }}
            machine Main::run(main: &mut Main) {{
                transition main.flag {{ true -> waiting(main) false -> done(main) }}
                state waiting(main: &mut Main)
                requires main.flag;
                {{
                    main.flag = false;
                    transition {guard} {{ true -> {true_target} false -> {false_target} }}
                }}
                state done(main: &mut Main) {{}}
            }}
        "#
        );
        check_nominal_back_edge(
            &source,
            accepted,
            "cannot prove requires contract for call waiting",
        );
    }
}

#[test]
fn nominal_named_back_edge_checks_fields_after_guard_call_effects() {
    for (effect, contract, accepted) in [
        ("", "", true),
        ("bytes = \"okay\";", "ensures bytes in Utf8", true),
        ("bytes[0] = 255;", "", false),
    ] {
        let source = format!(
            r#"{DEFINITIONS}
            data Main {{ packet: Packet; }}
            machine decide(bytes: &mut [u8; 4]) -> bool {contract} {{ {effect} true }}
            machine Main::run(main: &mut Main) {{
                transition {{ _ -> repeat(main) }}
                state repeat(main: &mut Main) {{
                    transition decide(&mut main.packet.payload.bytes) {{
                        true -> repeat(main)
                        false -> done(main)
                    }}
                }}
                state done(main: &mut Main) {{}}
            }}
        "#
        );
        check_nominal_back_edge(
            &source,
            accepted,
            "cannot prove default-domain field requirement for call",
        );
    }
}

#[test]
fn constructed_values_do_not_publish_snapshots_at_mutable_indices() {
    let source = r#"
        domain [u8; 4]::Utf8(bytes) requires valid_utf8(bytes);
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
