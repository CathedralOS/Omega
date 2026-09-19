//! Custody mutation matrix for retained opaque-representation selections.
//!
//! [`rederive_opaque_representation_selections`] is the independent replay of
//! the build-time `select_representation` custody: it reruns the harvest over
//! the final typed graph and compares the derived rows against the retained
//! build-evaluation custody before downstream property-receipt and
//! boundary-plan consumers may trust them. `OpaqueRepresentationSelection` is
//! an in-memory custody record, not a wire codec: its authenticated identity
//! is the domain-separated `selected_application_commitment`, recomputed by
//! `rederived_selected_application_commitment` from exactly the committed
//! fields (the closed application's commitment, lifecycle, copy disposition,
//! and origin). Every remaining field -- `opaque`, `carrier`,
//! `selecting_machine`, `source_span`, and the un-hashed application members
//! -- is joined by exact structural equality: `derived != retained` rejects.
//!
//! The tests below mutate every representable field independently, recompute
//! the containing commitment honestly through `from_validated_application`,
//! and prove the substituted row still cannot join the retained roster:
//! replay rejects before any checked-lowering write can consume it. Fields
//! whose substitution leaves the commitment unchanged are provenance fields
//! the digest deliberately excludes; replay still rejects them through the
//! exact row join. `lifecycle` and `origin` have exactly one representable
//! variant each and are pinned by exhaustive matches rather than mutations. A
//! substituted stored `selected_application_commitment` is only forgeable
//! inside the representation crate; `representation_selections`' own tests
//! pin that stored-vs-rederived divergence, while this matrix exercises the
//! stale-custody arm through `selecting_machine`.

use super::{harvest_opaque_representation_selections, rederive_opaque_representation_selections};
use crate::{
    OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION, OpaqueRepresentationApplicationOrigin,
    OpaqueRepresentationCopyDisposition, OpaqueRepresentationLifecycleDisposition,
    OpaqueRepresentationSelection,
};
use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_semantics::{DataSupplyMode, Multiplicity};
use source::{SourceMap, SourceOrigin, SourceSpan, Span};
use std::path::PathBuf;
use std::sync::Arc;
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataProperties, TypeParameter, TypeParameterKind};
use typed_trees::expression::StaticMachineArgument;
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableCall};
use typed_trees::trait_definition::{
    Conformance, ConformanceImplementation, ConformanceSubject, TraitDefinition,
};
use typed_trees::typed_trees::{
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
    ClosedConformanceConstArgument, ClosedConformanceRowIdentity,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// One authentic program: the compiler-owned `OpaqueRepresentation<Opaque>`
/// trait declared in toolchain source, two boundary-opaque declarations with
/// checked-shape carriers and named closed conformances, and one authoritative
/// build machine whose state selects both representations.
struct Fixture {
    typed: TypedTrees,
    build_machine: SymbolHandle,
    foreign_machine: SymbolHandle,
    foreign_data: SymbolHandle,
    foreign_trait: SymbolHandle,
    foreign_state: SymbolHandle,
    foreign_type_reference: TypeReferenceHandle,
    other_source_span: SourceSpan,
    /// Harvested baseline in statement order: `[Token, Marker]`.
    selections: Vec<OpaqueRepresentationSelection>,
}

fn named_argument(path: &'static str, symbol: SymbolHandle) -> StaticMachineArgument {
    StaticMachineArgument {
        path: vec![Identifier::generated_static(path)].into_boxed_slice(),
        application: None,
        type_reference: Default::default(),
        const_literal: None,
        evidence_projection: None,
        symbol,
    }
}

fn fixture() -> Fixture {
    let mut sources = SourceMap::default();
    let toolchain_source = "trait OpaqueRepresentation<Opaque>;\n";
    let toolchain_id = sources
        .add_with_metadata(
            PathBuf::from("toolchain/core/representation.omg"),
            String::from(toolchain_source),
            PathBuf::from("toolchain"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let user_id = sources
        .add_with_metadata(
            PathBuf::from("project/build.omg"),
            String::from("machine build { state run { select_representation(..) } }\n"),
            PathBuf::from("project"),
            None,
            SourceOrigin::User,
        )
        .source_id;
    // `OpaqueRepresentation` starts after `trait ` inside the toolchain file.
    let trait_span = SourceSpan::new(toolchain_id, Span::new(6, 6 + "OpaqueRepresentation".len()));
    let call_span = SourceSpan::new(user_id, Span::new(0, 13));
    let second_call_span = SourceSpan::new(user_id, Span::new(14, 27));
    let other_source_span = SourceSpan::new(user_id, Span::new(27, 33));

    let mut symbols = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let trait_symbol = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Source(trait_span));
    let trait_parameter =
        symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Borrowed("Opaque"));
    let token = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Borrowed("Token"));
    let token_bytes = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Borrowed("TokenBytes"));
    let token_representation = symbols.insert_root(
        SymbolKind::Conformance,
        SymbolNameRef::Borrowed("TokenRepresentation"),
    );
    let marker = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Borrowed("Marker"));
    let marker_bytes =
        symbols.insert_root(SymbolKind::Data, SymbolNameRef::Borrowed("MarkerBytes"));
    let marker_representation = symbols.insert_root(
        SymbolKind::Conformance,
        SymbolNameRef::Borrowed("MarkerRepresentation"),
    );
    let build_machine = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Borrowed("build"));
    let run_state = symbols.insert_root(SymbolKind::State, SymbolNameRef::Borrowed("run"));
    let foreign_machine = symbols.insert_root(
        SymbolKind::Machine,
        SymbolNameRef::Borrowed("foreign-build"),
    );
    let foreign_data = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Borrowed("Foreign"));
    let foreign_trait =
        symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Borrowed("ForeignTrait"));
    let foreign_state =
        symbols.insert_root(SymbolKind::State, SymbolNameRef::Borrowed("foreign-state"));

    let mut typed = TypedTrees::default();
    typed.symbols = symbols.finish();

    let mut trait_definition = TraitDefinition {
        symbol: trait_symbol,
        name: Identifier::generated("OpaqueRepresentation"),
        ..Default::default()
    };
    typed.push_trait_type_parameter(
        &mut trait_definition,
        TypeParameter {
            symbol: trait_parameter,
            name: Identifier::generated("Opaque"),
            kind: TypeParameterKind::Type,
            bounds: DataProperties::default(),
        },
    );
    typed.push_trait_definition(trait_definition);

    // `Token` is affine, so its selected application is `PlacementOnly`.
    typed.push_data_definition(DataDefinition {
        symbol: token,
        name: Identifier::generated("Token"),
        supply_mode: DataSupplyMode::BoundaryOpaque,
        ..Default::default()
    });
    typed.push_data_definition(DataDefinition {
        symbol: token_bytes,
        name: Identifier::generated("TokenBytes"),
        supply_mode: DataSupplyMode::CheckedShape,
        ..Default::default()
    });
    // `Marker` is `[copy]` data, so its selected application is
    // `CheckedSemanticCopy`; the complete carrier must be unrestricted too.
    typed.push_data_definition(DataDefinition {
        symbol: marker,
        name: Identifier::generated("Marker"),
        supply_mode: DataSupplyMode::BoundaryOpaque,
        properties: DataProperties {
            multiplicity: Multiplicity::Unrestricted,
            ..Default::default()
        },
        ..Default::default()
    });
    typed.push_data_definition(DataDefinition {
        symbol: marker_bytes,
        name: Identifier::generated("MarkerBytes"),
        supply_mode: DataSupplyMode::CheckedShape,
        properties: DataProperties {
            multiplicity: Multiplicity::Unrestricted,
            ..Default::default()
        },
        ..Default::default()
    });
    // Foreign declarations stay out of every conformance and selection; they
    // only serve as substitution material.
    typed.push_data_definition(DataDefinition {
        symbol: foreign_data,
        name: Identifier::generated("Foreign"),
        supply_mode: DataSupplyMode::CheckedShape,
        ..Default::default()
    });

    let token_reference = typed.type_reference_table.insert(TypeReferenceNode::Named {
        symbol: token,
        name: Identifier::generated("Token"),
    });
    let marker_reference = typed.type_reference_table.insert(TypeReferenceNode::Named {
        symbol: marker,
        name: Identifier::generated("Marker"),
    });
    let foreign_type_reference = typed.type_reference_table.insert(TypeReferenceNode::Named {
        symbol: foreign_data,
        name: Identifier::generated("Foreign"),
    });

    for (conformance_symbol, carrier, carrier_name, opaque_reference, alias) in [
        (
            token_representation,
            token_bytes,
            "TokenBytes",
            token_reference,
            "TokenRepresentation",
        ),
        (
            marker_representation,
            marker_bytes,
            "MarkerBytes",
            marker_reference,
            "MarkerRepresentation",
        ),
    ] {
        let arguments = typed
            .type_reference_table
            .insert_type_reference_handles([opaque_reference]);
        typed.push_conformance(Conformance {
            symbol: conformance_symbol,
            subject: ConformanceSubject::Carrier(Identifier::generated(carrier_name)),
            carrier_symbol: carrier,
            trait_name: Identifier::generated("OpaqueRepresentation"),
            trait_symbol,
            arguments,
            alias: Some(Identifier::generated(alias)),
            implementation: ConformanceImplementation::Closed { rows: Vec::new() },
            ..Default::default()
        });
    }

    let mut statements = HandleSpan::empty();
    for (opaque_path, conformance_path, opaque, conformance, source_span) in [
        (
            "Token",
            "TokenRepresentation",
            token,
            token_representation,
            call_span,
        ),
        (
            "Marker",
            "MarkerRepresentation",
            marker,
            marker_representation,
            second_call_span,
        ),
    ] {
        typed.statement_table.push_statement(
            &mut statements,
            StatementNode::Call(TableCall {
                target: Identifier::generated("select_representation"),
                machine_arguments: vec![
                    named_argument(opaque_path, opaque),
                    named_argument(conformance_path, conformance),
                ]
                .into_boxed_slice(),
                source_span,
                ..Default::default()
            }),
        );
    }

    let mut machine = Machine {
        symbol: build_machine,
        name: Identifier::generated("build"),
        ..Default::default()
    };
    typed.push_machine_state(
        &mut machine,
        State {
            symbol: run_state,
            name: Identifier::generated("run"),
            statement_nodes: statements,
            ..Default::default()
        },
    );
    typed.push_machine(machine);

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == build_machine)
        .expect("fixture must declare the authoritative build machine");
    let selections = harvest_opaque_representation_selections(&typed, machine)
        .expect("baseline program must harvest cleanly");
    assert_eq!(selections.len(), 2, "fixture must produce both selections");

    Fixture {
        typed,
        build_machine,
        foreign_machine,
        foreign_data,
        foreign_trait,
        foreign_state,
        foreign_type_reference,
        other_source_span,
        selections,
    }
}

/// The eight constructor lanes of `OpaqueRepresentationSelection`, extracted
/// through its public surface so each mutation re-enters
/// `from_validated_application` and the stored commitment is recomputed
/// honestly over the substituted fields.
#[derive(Clone)]
struct SelectionParts {
    opaque: SymbolHandle,
    carrier: SymbolHandle,
    application: ClosedConformanceApplication,
    lifecycle: OpaqueRepresentationLifecycleDisposition,
    copy_disposition: OpaqueRepresentationCopyDisposition,
    origin: OpaqueRepresentationApplicationOrigin,
    selecting_machine: SymbolHandle,
    source_span: SourceSpan,
}

impl SelectionParts {
    fn of(selection: &OpaqueRepresentationSelection) -> Self {
        Self {
            opaque: selection.opaque(),
            carrier: selection.carrier(),
            application: selection.application().clone(),
            lifecycle: selection.lifecycle(),
            copy_disposition: selection.copy_disposition(),
            origin: selection.origin(),
            selecting_machine: selection.selecting_machine(),
            source_span: selection.source_span(),
        }
    }

    fn assemble(&self) -> OpaqueRepresentationSelection {
        OpaqueRepresentationSelection::from_validated_application(
            self.opaque,
            self.carrier,
            self.application.clone(),
            self.lifecycle,
            self.copy_disposition,
            self.origin,
            self.selecting_machine,
            self.source_span,
        )
    }
}

fn replay(
    fixture: &Fixture,
    retained: &[OpaqueRepresentationSelection],
) -> Result<Vec<OpaqueRepresentationSelection>, Vec<Diagnostic>> {
    rederive_opaque_representation_selections(&fixture.typed, Some(fixture.build_machine), retained)
}

fn expect_rejection(
    result: Result<Vec<OpaqueRepresentationSelection>, Vec<Diagnostic>>,
    expected: &str,
) {
    let Err(diagnostics) = result else {
        panic!("substituted custody replayed cleanly; expected `{expected}`");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "expected rejection containing `{expected}`, got {diagnostics:?}",
    );
}

/// One substituted row must differ from the retained original, must carry its
/// own honestly recomputed commitment, must move the commitment exactly when
/// the mutated field is digested, and must be rejected by independent replay
/// when it replaces the first retained row.
fn expect_row_rejection(
    fixture: &Fixture,
    baseline: &OpaqueRepresentationSelection,
    substituted: OpaqueRepresentationSelection,
    commitment_changes: bool,
) {
    assert_ne!(
        &substituted, baseline,
        "substitution must change the retained row"
    );
    assert_eq!(
        substituted.selected_application_commitment(),
        substituted.rederived_selected_application_commitment(),
        "the substituted row must carry its honest commitment recomputation",
    );
    assert_eq!(
        substituted.selected_application_commitment() != baseline.selected_application_commitment(),
        commitment_changes,
        "commitment movement must match the field's digested/provenance role",
    );
    let mut retained = fixture.selections.clone();
    retained[0] = substituted;
    expect_rejection(
        replay(fixture, &retained),
        "rederivation disagrees with retained build custody",
    );
}

#[test]
fn honest_fixture_replays_retained_custody() {
    let fixture = fixture();
    let [token, marker] = fixture.selections.as_slice() else {
        unreachable!("fixture guarantees two selections");
    };
    assert_eq!(
        token.copy_disposition(),
        OpaqueRepresentationCopyDisposition::PlacementOnly
    );
    assert_eq!(
        marker.copy_disposition(),
        OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
    );
    for selection in &fixture.selections {
        assert_eq!(
            selection.selected_application_commitment(),
            selection.rederived_selected_application_commitment(),
            "stored commitment must equal the honest recomputation",
        );
        assert_eq!(
            selection.schema_version(),
            OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION
        );
        // `lifecycle` and `origin` each have exactly one representable value,
        // so no substitution axis exists. These exhaustive matches pin that
        // contract: a new variant fails compilation until its mutation
        // coverage lands.
        match selection.lifecycle() {
            OpaqueRepresentationLifecycleDisposition::Inert => {}
        }
        match selection.origin() {
            OpaqueRepresentationApplicationOrigin::NamedConformance => {}
        }
    }
    // Forward direction: independent replay rederives the identical roster.
    let derived = replay(&fixture, &fixture.selections).expect("honest custody must replay");
    assert_eq!(derived, fixture.selections);
}

#[test]
fn retained_row_rejects_every_one_field_substitution() {
    let fixture = fixture();
    let baseline = &fixture.selections[0];
    const DIGESTED: bool = true;
    const PROVENANCE: bool = false;

    // `opaque`: the boundary-opaque declaration handle. Deliberately absent
    // from the commitment; the exact row join still rejects the substitution.
    let mut parts = SelectionParts::of(baseline);
    parts.opaque = fixture.foreign_data;
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);

    // `carrier`: the concrete checked-shape carrier handle. Provenance under
    // the commitment, authoritative under the row join.
    let mut parts = SelectionParts::of(baseline);
    parts.carrier = fixture.foreign_data;
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);

    // `copy_disposition`: digested, so the recomputed commitment moves and the
    // substituted row is still rejected.
    let mut parts = SelectionParts::of(baseline);
    parts.copy_disposition = OpaqueRepresentationCopyDisposition::CheckedSemanticCopy;
    expect_row_rejection(&fixture, baseline, parts.assemble(), DIGESTED);

    // `selecting_machine`: the stale-custody arm compares this provenance lane
    // against the settlement's authoritative machine before row equality runs.
    let mut parts = SelectionParts::of(baseline);
    parts.selecting_machine = fixture.foreign_machine;
    let substituted = parts.assemble();
    assert_eq!(
        substituted.selected_application_commitment(),
        substituted.rederived_selected_application_commitment(),
    );
    let mut retained = fixture.selections.clone();
    retained[0] = substituted;
    expect_rejection(
        replay(&fixture, &retained),
        "stale build-machine or application custody",
    );

    // `source_span`: authored provenance, deliberately excluded from the
    // commitment; the row join still binds the exact occurrence.
    let mut parts = SelectionParts::of(baseline);
    parts.source_span = fixture.other_source_span;
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);

    // `application.declaration`: the exact closed conformance symbol.
    let mut parts = SelectionParts::of(baseline);
    parts.application.declaration = fixture.foreign_data;
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);

    // `application.arguments`: the retained resolved argument tree extends
    // with a foreign element.
    let mut parts = SelectionParts::of(baseline);
    parts.application.arguments =
        vec![named_argument("Foreign", fixture.foreign_data)].into_boxed_slice();
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);

    // `application.lifetime_arguments`.
    let mut parts = SelectionParts::of(baseline);
    parts.application.lifetime_arguments = vec![String::from("'foreign")];
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);

    // `application.type_arguments`.
    let mut parts = SelectionParts::of(baseline);
    parts.application.type_arguments = vec![String::from("named(name(Foreign))")];
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);

    // `application.const_arguments`: a foreign caller binder.
    let mut parts = SelectionParts::of(baseline);
    parts.application.const_arguments = vec![ClosedConformanceConstArgument::CallerBinder {
        parameter_carrier: fixture.foreign_type_reference,
        binder: fixture.foreign_data,
        binder_carrier: fixture.foreign_type_reference,
    }];
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);

    // `application.machine_arguments`.
    let mut parts = SelectionParts::of(baseline);
    parts.application.machine_arguments = vec![fixture.foreign_machine];
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);

    // `application.subject_identity`: both representable substitutions.
    let mut parts = SelectionParts::of(baseline);
    parts.application.subject_identity = None;
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);
    let mut parts = SelectionParts::of(baseline);
    parts.application.subject_identity = Some(String::from("ForeignBytes"));
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);

    // `application.trait_definition`: the resolved trait symbol.
    let mut parts = SelectionParts::of(baseline);
    parts.application.trait_definition = fixture.foreign_trait;
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);

    // `application.trait_lifetime_arguments`.
    let mut parts = SelectionParts::of(baseline);
    parts.application.trait_lifetime_arguments = vec![String::from("'foreign")];
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);

    // `application.trait_arguments`: substitute the single retained lane.
    let mut parts = SelectionParts::of(baseline);
    parts.application.trait_arguments = vec![String::from("named(name(Foreign))")];
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);

    // `application.rows`: a foreign requirement-realization row.
    let mut parts = SelectionParts::of(baseline);
    parts.application.rows = vec![ClosedConformanceRowIdentity {
        declaring_trait: fixture.foreign_trait,
        requirement: fixture.foreign_data,
        realization_machine: fixture.foreign_machine,
        realization_state: fixture.foreign_state,
    }];
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);

    // `application.report_fingerprint`: the historical compact coordinate is
    // deliberately not hashed, but the exact row join still binds it.
    let mut parts = SelectionParts::of(baseline);
    parts.application.report_fingerprint =
        baseline.application().report_fingerprint.wrapping_add(1);
    expect_row_rejection(&fixture, baseline, parts.assemble(), PROVENANCE);

    // `application.commitment`: a foreign conformance commitment moves the
    // digested selected-application commitment yet still fails the row join.
    let mut parts = SelectionParts::of(baseline);
    parts.application.commitment = ClosedConformanceApplicationCommitment::from_digest(
        baseline
            .application()
            .commitment
            .as_bytes()
            .map(|byte| byte ^ 0xff),
    );
    expect_row_rejection(&fixture, baseline, parts.assemble(), DIGESTED);
}

#[test]
fn retained_roster_rejects_omission_duplication_reorder_and_extension() {
    let fixture = fixture();
    let [first, second] = fixture.selections.as_slice() else {
        unreachable!("fixture guarantees two selections");
    };
    let expected = "rederivation disagrees with retained build custody";

    // Omission: the whole roster and a single dropped row.
    expect_rejection(replay(&fixture, &[]), expected);
    expect_rejection(replay(&fixture, &[first.clone()]), expected);
    expect_rejection(replay(&fixture, &[second.clone()]), expected);

    // Duplication: one retained row appearing twice.
    expect_rejection(
        replay(&fixture, &[first.clone(), second.clone(), first.clone()]),
        expected,
    );

    // Reorder: retained custody binds statement order.
    expect_rejection(replay(&fixture, &[second.clone(), first.clone()]), expected);

    // Foreign row substitution and extension: a syntactically valid row that
    // never occurred in the build cannot join either position or the tail.
    let mut foreign_parts = SelectionParts::of(second);
    foreign_parts.opaque = fixture.foreign_data;
    foreign_parts.carrier = fixture.foreign_data;
    foreign_parts.application.declaration = fixture.foreign_trait;
    foreign_parts.application.commitment =
        ClosedConformanceApplicationCommitment::from_digest([0x5f; 32]);
    foreign_parts.source_span = fixture.other_source_span;
    let foreign = foreign_parts.assemble();
    assert_eq!(
        foreign.selected_application_commitment(),
        foreign.rederived_selected_application_commitment(),
    );
    expect_rejection(
        replay(&fixture, &[foreign.clone(), second.clone()]),
        expected,
    );
    expect_rejection(
        replay(&fixture, &[first.clone(), second.clone(), foreign.clone()]),
        expected,
    );
}

#[test]
fn replay_requires_exactly_one_authoritative_build_machine() {
    let fixture = fixture();

    // No authoritative build machine: retained custody cannot survive.
    expect_rejection(
        rederive_opaque_representation_selections(&fixture.typed, None, &fixture.selections),
        "selections exist without an authoritative build machine",
    );

    // The settlement selects a different machine than the retained rows claim:
    // the stale-custody arm fires before the machine lookup runs.
    expect_rejection(
        rederive_opaque_representation_selections(
            &fixture.typed,
            Some(fixture.foreign_machine),
            &fixture.selections,
        ),
        "stale build-machine or application custody",
    );

    // Rows honestly rebuilt under a machine absent from the typed graph map
    // to zero final declarations.
    let foreign_owned = fixture
        .selections
        .iter()
        .map(|selection| {
            let mut parts = SelectionParts::of(selection);
            parts.selecting_machine = fixture.foreign_machine;
            parts.assemble()
        })
        .collect::<Vec<_>>();
    expect_rejection(
        rederive_opaque_representation_selections(
            &fixture.typed,
            Some(fixture.foreign_machine),
            &foreign_owned,
        ),
        "map to 0 final build-machine declarations; expected one",
    );

    // A duplicated authoritative machine maps to two final declarations.
    let mut typed = fixture.typed.clone();
    typed.push_machine(Machine {
        symbol: fixture.build_machine,
        name: Identifier::generated("build-duplicate"),
        ..Default::default()
    });
    expect_rejection(
        rederive_opaque_representation_selections(
            &typed,
            Some(fixture.build_machine),
            &fixture.selections,
        ),
        "map to 2 final build-machine declarations; expected one",
    );
}

#[test]
fn replay_recomputes_from_current_program_evidence() {
    let fixture = fixture();
    let expected = "rederivation disagrees with retained build custody";

    // Erased evidence: strip the selection statements from the authoritative
    // machine. The retained roster has no surviving program support, so the
    // mutated-to-original direction rejects too.
    let mut typed = fixture.typed.clone();
    let states = typed.machines()[0].states;
    typed.machine_states.span_mut_or_empty(states)[0].statement_nodes = HandleSpan::empty();
    expect_rejection(
        rederive_opaque_representation_selections(
            &typed,
            Some(fixture.build_machine),
            &fixture.selections,
        ),
        expected,
    );

    // Reordered evidence: the derived roster follows statement order, so a
    // swapped program cannot satisfy the retained ordering either.
    let mut typed = fixture.typed.clone();
    let states = typed.machines()[0].states;
    let statement_nodes = typed.machine_states.span_or_empty(states)[0].statement_nodes;
    let reordered = typed
        .statement_table
        .statements(statement_nodes)
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>();
    let mut reversed = HandleSpan::empty();
    for statement in reordered {
        typed
            .statement_table
            .push_statement(&mut reversed, statement);
    }
    typed.machine_states.span_mut_or_empty(states)[0].statement_nodes = reversed;
    expect_rejection(
        rederive_opaque_representation_selections(
            &typed,
            Some(fixture.build_machine),
            &fixture.selections,
        ),
        expected,
    );
}
