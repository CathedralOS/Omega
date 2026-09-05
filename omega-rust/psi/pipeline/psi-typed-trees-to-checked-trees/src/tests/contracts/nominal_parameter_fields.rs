use super::*;

const DEFINITIONS: &str = r#"
    domain [u8; 4]::Utf8 requires valid_utf8(self);
    data Text { bytes: [u8; 4] in Utf8; }
    data Packet { payload: Text; tag: u64; }
"#;

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
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
        if !matches!(fact.payload, psi_facts::FactPayload::AssignedValue { .. }) {
            continue;
        }
        let psi_facts::FactPlace::Place(place) = fact.place else {
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
                        psi_facts::PlaceSegment::Index { .. }
                            | psi_facts::PlaceSegment::FixedRange { .. }
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
