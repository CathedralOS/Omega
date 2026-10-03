use super::demands::{fields_equal, schema_accepts};
use super::migration::search_route;
use super::{WireCompatibilityDemand, WireField, WireShape, verification_diagnostics};
use language_core::BindingRelevance;
use std::collections::BTreeMap;

fn field(number: u64, relevance: BindingRelevance, type_display: &str) -> WireField {
    WireField {
        number,
        relevance,
        type_display: type_display.to_owned(),
    }
}

#[test]
fn directional_wire_shape_ignores_semantic_only_erased_fields() {
    let relevant = field(0, BindingRelevance::Relevant, "u32");
    let erased = field(1, BindingRelevance::Erased, "Evidence");

    assert!(fields_equal(
        &[relevant.clone(), erased],
        std::slice::from_ref(&relevant)
    ));
    assert!(!fields_equal(
        std::slice::from_ref(&relevant),
        &[field(0, BindingRelevance::Erased, "u32")]
    ));
}

#[test]
fn distinct_field_types_are_not_wire_compatible() {
    let reader = WireShape {
        name: "Reader".to_owned(),
        fields: vec![field(1, BindingRelevance::Relevant, "u32")],
        ..WireShape::default()
    };
    let writer = WireShape {
        name: "Writer".to_owned(),
        fields: vec![field(1, BindingRelevance::Relevant, "Text")],
        ..WireShape::default()
    };

    assert!(
        !schema_accepts(&reader, &writer),
        "different types on the same field number are incompatible"
    );
}

#[test]
fn divergent_generated_codec_is_refused() {
    let verifications = BTreeMap::from([(
        "Packet".to_owned(),
        Err::<checked_interpreter::WireCodecVerification, _>(
            "Packet: generated encode emission diverges".to_owned(),
        ),
    )]);
    let diagnostics = verification_diagnostics(&verifications);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].is_error());
    assert!(
        diagnostics[0]
            .message
            .contains("failed independent verification")
    );
    assert!(diagnostics[0].message.contains("Packet"));
}

#[test]
fn checking_accepts_a_verified_generated_codec() {
    // Exercise the stage entrance, not a separately callable report pass.
    let typed = typed_fixture(
        "data Packet { #0 tag: u32; #1 depth: i32; } \
         machine main() -> i32 { transition true { true -> 7 false -> 0 } }",
    );
    crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
        .expect("a verified codec admits");
}

#[test]
fn checking_rejects_a_refused_demand_after_an_exact_input_memo_hit() {
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
    let _retention = crate::retain_repeated_checks();
    crate::lower_typed_trees(typed.clone(), &crate::CheckingRequest::settled())
        .expect("the same source admits without a preservation demand");
    crate::lower_typed_trees(typed.clone(), &crate::CheckingRequest::settled())
        .expect("the exact repeated source admits");
    let demands = [demand];
    let diagnostics = crate::lower_typed_trees(
        typed,
        &crate::CheckingRequest::settled().with_wire_compatibility_demands(&demands),
    )
    .expect_err("a refused preservation demand rejects even after memo reuse");
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
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
    )
    .expect("resolve wire fixture");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type wire fixture")
}
