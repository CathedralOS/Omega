use super::demands::{fields_equal, schema_accepts};
use super::migration::search_route;
use super::schemas::{
    ScopeTable, build_wire_protocol_report, codec_requirement_report_identity,
    compatibility_verdicts, encode_requirement_report_identity,
    normalized_wire_plan_report_identity, qualified_schema_path,
};
use super::{WireCompatibilityDemand, validate_wire_protocol, verification_diagnostics};
use super::{WireFieldRelevance, WireFieldReportEntry, WireSchemaReportEntry, WireTrustClass};
use std::collections::BTreeMap;
use typed_trees::wire::WirePlacement;

fn field(number: u64, relevance: WireFieldRelevance, type_display: &str) -> WireFieldReportEntry {
    WireFieldReportEntry {
        number,
        name: format!("field_{number}"),
        relevance,
        type_display: type_display.to_owned(),
    }
}

#[test]
fn directional_wire_shape_ignores_semantic_only_erased_fields() {
    let relevant = field(0, WireFieldRelevance::Relevant, "u32");
    let erased = field(1, WireFieldRelevance::Erased, "Evidence");

    assert!(fields_equal(
        &[relevant.clone(), erased],
        std::slice::from_ref(&relevant)
    ));
    assert!(!fields_equal(
        std::slice::from_ref(&relevant),
        &[field(0, WireFieldRelevance::Erased, "u32")]
    ));
}

#[test]
fn relevance_change_requires_cross_era_migration() {
    let predecessor = ScopeTable {
        fields: vec![field(3, WireFieldRelevance::Relevant, "u32")],
        reserved: Vec::new(),
    };
    let successor = ScopeTable {
        fields: vec![field(3, WireFieldRelevance::Erased, "u32")],
        reserved: Vec::new(),
    };

    let verdicts = compatibility_verdicts(&predecessor, &successor);
    assert_eq!(verdicts.requires_migration.len(), 1);
    assert!(verdicts.requires_migration[0].contains("changes relevance relevant -> erased"));
    assert!(verdicts.compatible.is_empty());
}

#[test]
fn codec_requirement_report_identity_binds_the_normalized_schema_report_coordinate() {
    assert_ne!(
        codec_requirement_report_identity(11),
        codec_requirement_report_identity(12)
    );
    assert_eq!(
        codec_requirement_report_identity(11),
        codec_requirement_report_identity(11)
    );
    assert_ne!(
        encode_requirement_report_identity(11),
        encode_requirement_report_identity(12)
    );
    assert_ne!(
        encode_requirement_report_identity(11),
        codec_requirement_report_identity(11),
        "encode and strict-decode are distinct requirement report coordinates"
    );
}

#[test]
fn normalized_wire_plan_report_identity_binds_kind_tag_and_schema_report_coordinate() {
    let scalar = [WirePlacement::Varint { tag: 1 }];
    let length = [WirePlacement::LengthPrefixed { tag: 1 }];
    let retagged = [WirePlacement::Varint { tag: 2 }];

    let report_identity = normalized_wire_plan_report_identity(7, &scalar, &[]);
    assert_eq!(
        report_identity,
        normalized_wire_plan_report_identity(7, &scalar, &[])
    );
    assert_ne!(
        report_identity,
        normalized_wire_plan_report_identity(8, &scalar, &[])
    );
    assert_ne!(
        report_identity,
        normalized_wire_plan_report_identity(7, &length, &[])
    );
    assert_ne!(
        report_identity,
        normalized_wire_plan_report_identity(7, &retagged, &[])
    );

    let obligation = typed_trees::wire::WireEncodeObligation {
        field_number: 1,
        element: typed_trees::wire::WireScalarEncoding {
            byte_size: 4,
            zigzag: false,
        },
        length: typed_trees::wire::WireEncodeLengthObligation::RuntimeElementCount,
        work: typed_trees::wire::WireEncodeWorkObligation::TwoPassesPerElement,
        output_capacity: typed_trees::wire::WireEncodeOutputCapacityObligation::ExactPackedPayload,
    };
    assert_ne!(
        report_identity,
        normalized_wire_plan_report_identity(7, &scalar, &[obligation])
    );
}

#[test]
fn compact_equal_wire_schema_reports_do_not_override_exact_shape_compatibility() {
    let reader = WireSchemaReportEntry {
        name: "Reader".to_owned(),
        normalized_schema_report_identity: 0xfeed,
        fields: vec![field(1, WireFieldRelevance::Relevant, "u32")],
        ..WireSchemaReportEntry::default()
    };
    let writer = WireSchemaReportEntry {
        name: "Writer".to_owned(),
        normalized_schema_report_identity: 0xfeed,
        fields: vec![field(1, WireFieldRelevance::Relevant, "Text")],
        ..WireSchemaReportEntry::default()
    };

    assert!(
        !schema_accepts(&reader, &writer),
        "compact-equal schema reports cannot authorize an incompatible exact wire shape"
    );
}

#[test]
fn probe_checked_generated_codec_stays_compiler_admitted() {
    // The whole probe suite passes with no coverage gap — yet
    // differential probes over selected members are finite regression
    // checks, not the general agreement law, so the codec's row stays
    // `Admitted`, naming the compiler. The passed checks remain on the
    // artifact report as the evidence they are.
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use tokens_to_syntax_trees::parse_syntax_trees;

    let tokens = Lexer::new(
        "data Packet { #0 tag: u32; #1 depth: i32; } \
         machine main() -> i32 { transition true { true -> 7 false -> 0 } }",
    )
    .tokenize()
    .expect("wire tokens");
    let syntax = parse_syntax_trees(&tokens).expect("wire syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("wire symbols");
    let typed = lower_symbol_resolved_trees(&resolved).expect("wire types");

    let verifications: BTreeMap<_, _> = typed
        .wire_schemas()
        .iter()
        .map(|schema| {
            (
                qualified_schema_path(&typed, schema.symbol, schema.name.as_str()),
                checked_interpreter::verify_wire_schema_codec(&typed, schema),
            )
        })
        .collect();
    let report = build_wire_protocol_report(&typed, &[], &verifications);
    let row = report
        .schemas
        .iter()
        .find(|row| row.name == "Packet")
        .expect("packet row");
    assert_eq!(
        row.trust_class,
        Some(WireTrustClass::Admitted {
            authority: "Omega compiler".to_owned(),
        }),
        "finite probes never earn Derived"
    );
    assert!(
        row.realization_evidence
            .iter()
            .any(|entry| entry.contains("independent verification passed")),
        "{:#?}",
        row.realization_evidence
    );
    assert!(
        row.realization_evidence
            .iter()
            .any(|entry| entry.contains("not the general agreement law")),
        "the row names why finite evidence stays admitted: {:#?}",
        row.realization_evidence
    );
}

#[test]
fn divergent_generated_codec_is_admitted_and_refused() {
    // A PROVEN divergence is refused trust: the report row names it and
    // keeps the codec compiler-admitted, and validation emits the
    // diagnostic that rejects the build.
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use tokens_to_syntax_trees::parse_syntax_trees;

    let tokens = Lexer::new(
        "data Packet { #0 tag: u32; #1 depth: i32; } \
         machine main() -> i32 { transition true { true -> 7 false -> 0 } }",
    )
    .tokenize()
    .expect("wire tokens");
    let syntax = parse_syntax_trees(&tokens).expect("wire syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("wire symbols");
    let typed = lower_symbol_resolved_trees(&resolved).expect("wire types");

    let packet_path = qualified_schema_path(
        &typed,
        typed.wire_schemas()[0].symbol,
        typed.wire_schemas()[0].name.as_str(),
    );
    let verifications: BTreeMap<_, _> = [(
        packet_path.clone(),
        Err::<checked_interpreter::WireCodecVerification, _>(
            "Packet: generated encode emission diverges".to_owned(),
        ),
    )]
    .into_iter()
    .collect();
    let report = build_wire_protocol_report(&typed, &[], &verifications);
    let row = report
        .schemas
        .iter()
        .find(|row| row.name == "Packet")
        .expect("packet row");
    assert_eq!(
        row.trust_class,
        Some(WireTrustClass::Admitted {
            authority: "Omega compiler".to_owned(),
        }),
        "a proven divergence never reports Derived"
    );
    assert!(
        row.realization_evidence
            .iter()
            .any(|entry| entry.contains("codec divergence")),
        "{:#?}",
        row.realization_evidence
    );

    let diagnostics = verification_diagnostics(&verifications);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].is_error());
    assert!(
        diagnostics[0]
            .message
            .contains("failed independent verification"),
        "{}",
        diagnostics[0].message
    );
    assert!(diagnostics[0].message.contains("Packet"));
}

#[test]
fn partial_probe_coverage_stays_admitted_and_names_the_gap() {
    // A suite that exercised some requirements but left named coverage
    // gaps is still finite-scope evidence: the artifact row stays
    // compiler-admitted and carries the uncovered obligation verbatim.
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use tokens_to_syntax_trees::parse_syntax_trees;

    let tokens = Lexer::new(
        "data Packet { #0 tag: u32; #1 depth: i32; } \
         machine main() -> i32 { transition true { true -> 7 false -> 0 } }",
    )
    .tokenize()
    .expect("wire tokens");
    let syntax = parse_syntax_trees(&tokens).expect("wire syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("wire symbols");
    let typed = lower_symbol_resolved_trees(&resolved).expect("wire types");

    let packet_path = qualified_schema_path(
        &typed,
        typed.wire_schemas()[0].symbol,
        typed.wire_schemas()[0].name.as_str(),
    );
    let verifications: BTreeMap<_, _> = [(
        packet_path,
        Ok(checked_interpreter::WireCodecVerification {
            checks: vec![
                "canonical emission byte-compared against an independent compact_binary \
                 reference for 2 members"
                    .to_owned(),
            ],
            gaps: vec![
                "strict decode not exercised: field(s) `depth` have no decode-side wire kind"
                    .to_owned(),
            ],
        }),
    )]
    .into_iter()
    .collect();
    let report = build_wire_protocol_report(&typed, &[], &verifications);
    let row = report
        .schemas
        .iter()
        .find(|row| row.name == "Packet")
        .expect("packet row");
    assert_eq!(
        row.trust_class,
        Some(WireTrustClass::Admitted {
            authority: "Omega compiler".to_owned(),
        }),
        "partial probe coverage never licenses Derived"
    );
    assert!(
        row.realization_evidence
            .iter()
            .any(|entry| entry.contains("independent verification passed")),
        "{:#?}",
        row.realization_evidence
    );
    assert!(
        row.realization_evidence
            .iter()
            .any(|entry| entry.contains("coverage gap: strict decode not exercised")),
        "the uncovered obligation is named on the artifact row: {:#?}",
        row.realization_evidence
    );
}

#[test]
fn verification_that_exercised_no_check_stays_admitted() {
    // When the generator cannot realize a codec for the schema at all,
    // the probe suite runs zero checks. The row must not claim
    // "verification passed" over an empty list: it says nothing was
    // exercised and stays compiler-admitted.
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use tokens_to_syntax_trees::parse_syntax_trees;

    let tokens = Lexer::new(
        "data Packet { #0 tag: u32; #1 depth: i32; } \
         machine main() -> i32 { transition true { true -> 7 false -> 0 } }",
    )
    .tokenize()
    .expect("wire tokens");
    let syntax = parse_syntax_trees(&tokens).expect("wire syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("wire symbols");
    let typed = lower_symbol_resolved_trees(&resolved).expect("wire types");

    let packet_path = qualified_schema_path(
        &typed,
        typed.wire_schemas()[0].symbol,
        typed.wire_schemas()[0].name.as_str(),
    );
    let verifications: BTreeMap<_, _> = [(
        packet_path,
        Ok(checked_interpreter::WireCodecVerification {
            checks: Vec::new(),
            gaps: vec![
                "no codec realization to verify: encode field collection: halted".to_owned(),
            ],
        }),
    )]
    .into_iter()
    .collect();
    let report = build_wire_protocol_report(&typed, &[], &verifications);
    let row = report
        .schemas
        .iter()
        .find(|row| row.name == "Packet")
        .expect("packet row");
    assert_eq!(
        row.trust_class,
        Some(WireTrustClass::Admitted {
            authority: "Omega compiler".to_owned(),
        })
    );
    assert!(
        row.realization_evidence
            .iter()
            .any(|entry| entry.contains("exercised no check")),
        "an empty check list is reported as no evidence, not a pass: {:#?}",
        row.realization_evidence
    );
    assert!(
        !row.realization_evidence
            .iter()
            .any(|entry| entry.contains("independent verification passed")),
        "{:#?}",
        row.realization_evidence
    );
}

#[test]
fn probe_pass_with_policy_agreement_stays_compiler_admitted() {
    // The strongest finite evidence available today -- the full probe
    // suite passing AND the authored `CompactBinary::plan` grammar policy
    // agreeing with the codec walk -- still does not reach the general
    // agreement law. Both are recorded as the evidence they are, and the
    // realization stays compiler-admitted.
    let mut typed = typed_fixture(
        r#"
data Packet { #1 seed: u64; #2 label: &[u8]; }

data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { size: u64 [0..=4096]; align: u64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64 [0..=32]; }
data FieldPlan [copy] { case Varint(tag: u64); case LengthPrefixed(tag: u64); }
data Plan { fields: [FieldPlan; 32]; entry_count: u64; size_fixed: u64; size_is_dynamic: bool; align: u64; }

data CompactBinary { fields: [FieldPlan; 32]; }
machine CompactBinary::plan(&mut self, schema: Schema) -> Plan {
self.fields[0] = FieldPlan::Varint { tag: 1 };
self.fields[1] = FieldPlan::LengthPrefixed { tag: 2 };
Plan {
    fields: self.fields,
    entry_count: schema.field_count,
    size_fixed: 0,
    size_is_dynamic: true,
    align: 1,
}
}

data Main { }
machine Main::main(&mut self) { }
"#,
    );
    crate::compute_wire_plans(&mut typed, None, 0).expect("wire plan pass accepts the fixture");
    let verifications: BTreeMap<_, _> = typed
        .wire_schemas()
        .iter()
        .map(|schema| {
            (
                qualified_schema_path(&typed, schema.symbol, schema.name.as_str()),
                checked_interpreter::verify_wire_schema_codec(&typed, schema),
            )
        })
        .collect();
    let report = build_wire_protocol_report(&typed, &[], &verifications);
    let packet = report
        .schemas
        .iter()
        .find(|schema| schema.name == "Packet")
        .expect("Packet schema row");
    assert_eq!(
        packet.trust_class,
        Some(WireTrustClass::Admitted {
            authority: "Omega compiler".to_owned(),
        }),
        "clean probes plus plan agreement still do not reach Derived"
    );
    for fragment in [
        "independent verification passed",
        "not the general agreement law",
        "independently checked against the authored",
    ] {
        assert!(
            packet
                .realization_evidence
                .iter()
                .any(|line| line.contains(fragment)),
            "missing evidence line `{fragment}`: {:#?}",
            packet.realization_evidence
        );
    }
}

#[test]
fn validate_wire_protocol_accepts_a_verified_generated_codec() {
    // The public entry exercises the whole producer route -- every wire
    // schema's codec is independently verified, the artifact report is
    // built and validated, and a clean suite admits the build.
    let typed = typed_fixture(
        "data Packet { #0 tag: u32; #1 depth: i32; } \
         machine main() -> i32 { transition true { true -> 7 false -> 0 } }",
    );
    validate_wire_protocol(&typed, &[]).expect("a verified codec admits");
}

#[test]
fn validate_wire_protocol_rejects_a_refused_demand() {
    // The consumer-facing refusal: an authored compatibility demand whose
    // requested fact the artifact cannot satisfy rejects the build,
    // naming the unmet fact. compact_binary publishes strict
    // unknown-member behavior, so a preservation demand is refused.
    let typed = typed_fixture(
        "data Packet { #0 tag: u32; #1 depth: i32; } \
         machine main() -> i32 { transition true { true -> 7 false -> 0 } }",
    );
    let demand = WireCompatibilityDemand {
        edge: "Store".to_owned(),
        lineage: "PacketLineage".to_owned(),
        local_schema: "Packet".to_owned(),
        peer_schema: "Packet".to_owned(),
        require_readable: false,
        require_writable: false,
        require_unknown_preservation: true,
        require_canonical: false,
        require_complete_migration: false,
    };
    let diagnostics = validate_wire_protocol(&typed, &[demand])
        .expect_err("a refused preservation demand rejects the build");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0].message.contains("is unsatisfied"),
        "{}",
        diagnostics[0].message
    );
    assert!(
        diagnostics[0].message.contains("unknown preservation"),
        "{}",
        diagnostics[0].message
    );
}

fn edge(
    old: u32,
    new: u32,
    machine: &str,
) -> (symbols::SymbolHandle, symbols::SymbolHandle, String) {
    (
        symbols::SymbolHandle::from_arena_index(old),
        symbols::SymbolHandle::from_arena_index(new),
        machine.to_owned(),
    )
}

#[test]
fn checked_conversion_route_composes_eras_oldest_to_current() {
    // A two-era chain converts across an intermediate shape: the bound
    // machines run peer-to-local and the route records every era it
    // traverses, oldest first.
    let edges = [
        edge(1, 2, "V1::to_v2"),
        edge(2, 3, "V2::to_v3"),
        edge(9, 10, "Unrelated::edge"),
    ];

    let route = search_route(
        &edges,
        symbols::SymbolHandle::from_arena_index(1),
        symbols::SymbolHandle::from_arena_index(3),
    )
    .expect("a bound edge chain reaches the local era");

    assert_eq!(route.machines, ["V1::to_v2", "V2::to_v3"]);
    assert_eq!(
        route.eras,
        [
            symbols::SymbolHandle::from_arena_index(1),
            symbols::SymbolHandle::from_arena_index(2),
            symbols::SymbolHandle::from_arena_index(3),
        ]
    );
}

#[test]
fn checked_conversion_route_uses_a_direct_edge() {
    let edges = [edge(1, 2, "V1::to_v2")];

    let route = search_route(
        &edges,
        symbols::SymbolHandle::from_arena_index(1),
        symbols::SymbolHandle::from_arena_index(2),
    )
    .expect("the single bound edge is the route");

    assert_eq!(route.machines, ["V1::to_v2"]);
    assert_eq!(route.eras.len(), 2);
}

#[test]
fn checked_conversion_route_rejects_eras_with_no_bound_chain() {
    // A demand between eras that no FormatMigration edge connects must not
    // certify a conversion; partial chains that stop short do not satisfy
    // it either.
    let edges = [edge(1, 2, "V1::to_v2")];

    assert!(
        search_route(
            &edges,
            symbols::SymbolHandle::from_arena_index(1),
            symbols::SymbolHandle::from_arena_index(3),
        )
        .is_none()
    );
    assert!(
        search_route(
            &edges,
            symbols::SymbolHandle::from_arena_index(2),
            symbols::SymbolHandle::from_arena_index(1),
        )
        .is_none(),
        "a downgrade edge is a separate binding, not the reverse of the upgrade"
    );
}

#[test]
fn checked_conversion_route_terminates_through_a_cycle_edge() {
    // Lineages may bind a downgrade edge alongside the upgrade; the
    // search must not follow the cycle back into a visited era.
    let cyclic = [edge(1, 2, "V1::to_v2"), edge(2, 1, "V2::to_v1")];
    assert!(
        search_route(
            &cyclic,
            symbols::SymbolHandle::from_arena_index(1),
            symbols::SymbolHandle::from_arena_index(3),
        )
        .is_none()
    );

    let cyclic_with_exit = [
        edge(1, 2, "V1::to_v2"),
        edge(2, 1, "V2::to_v1"),
        edge(2, 3, "V2::to_v3"),
    ];
    let route = search_route(
        &cyclic_with_exit,
        symbols::SymbolHandle::from_arena_index(1),
        symbols::SymbolHandle::from_arena_index(3),
    )
    .expect("the route continues past the cycle to the local era");
    assert_eq!(route.machines, ["V1::to_v2", "V2::to_v3"]);
}

fn typed_fixture(source_text: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source_text)
        .tokenize()
        .expect("tokenize wire fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse wire fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve wire fixture");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type wire fixture")
}

fn wire_report(source_text: &str) -> super::WireProtocolReport {
    let mut typed = typed_fixture(source_text);
    crate::compute_wire_plans(&mut typed, None, 0).expect("wire plan pass accepts the fixture");
    build_wire_protocol_report(&typed, &[], &BTreeMap::new())
}

#[test]
fn synthesized_codec_stays_admitted_without_an_authored_grammar_policy() {
    let report = wire_report(
        "data Packet { #1 seed: u64; #2 label: &[u8]; }\ndata Main { }\nmachine Main::main(&mut self) { }\n",
    );

    let packet = report
        .schemas
        .iter()
        .find(|schema| schema.name == "Packet")
        .expect("Packet schema row");
    assert!(packet.synthesized_codec);
    assert_eq!(
        packet.trust_class,
        Some(super::WireTrustClass::Admitted {
            authority: "Omega compiler".to_owned()
        }),
        "a generated codec with no independent check remains compiler-admitted"
    );
    assert!(
        packet
            .realization_evidence
            .iter()
            .any(|line| line.contains("not yet independently checked"))
    );
}

#[test]
fn policy_agreement_alone_stays_compiler_admitted() {
    // The authored `CompactBinary::plan` grammar policy agreeing with the
    // codec walk is real evidence and is recorded, but it compares
    // PLACEMENTS, not the body's `decode(encode(value)) == value`. The
    // codec spec admits Derived only for a body "independently checked
    // against the public requirement", and says "an unverified generated
    // realization remains compiler-admitted".
    let report = wire_report(
        r#"
data Packet { #1 seed: u64; #2 label: &[u8]; }

data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { size: u64 [0..=4096]; align: u64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64 [0..=32]; }
data FieldPlan [copy] { case Varint(tag: u64); case LengthPrefixed(tag: u64); }
data Plan { fields: [FieldPlan; 32]; entry_count: u64; size_fixed: u64; size_is_dynamic: bool; align: u64; }

data CompactBinary { fields: [FieldPlan; 32]; }
machine CompactBinary::plan(&mut self, schema: Schema) -> Plan {
self.fields[0] = FieldPlan::Varint { tag: 1 };
self.fields[1] = FieldPlan::LengthPrefixed { tag: 2 };
Plan {
    fields: self.fields,
    entry_count: schema.field_count,
    size_fixed: 0,
    size_is_dynamic: true,
    align: 1,
}
}

data Main { }
machine Main::main(&mut self) { }
"#,
    );

    let packet = report
        .schemas
        .iter()
        .find(|schema| schema.name == "Packet")
        .expect("Packet schema row");
    assert_eq!(
        packet.trust_class,
        Some(super::WireTrustClass::Admitted {
            authority: "Omega compiler".to_owned(),
        }),
        "plan agreement is not a body check, so the realization stays admitted"
    );
    // The policy agreement is still recorded: downgrading the trust class
    // must not lose the evidence that was actually established.
    assert!(
        packet
            .realization_evidence
            .iter()
            .any(|line| line.contains("independently checked against the authored")),
        "{:#?}",
        packet.realization_evidence
    );
}
