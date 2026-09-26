//! Custody-substitution machinery for the retained opaque-representation
//! selection matrix.
//!
//! `OpaqueRepresentationSelection` is an in-memory custody record, not a wire
//! codec: its authenticated identity is the domain-separated
//! `selected_application_commitment`, recomputed by
//! `rederived_selected_application_commitment` from exactly the committed
//! fields (the closed application's commitment, lifecycle, copy disposition,
//! and origin). Every remaining field -- `opaque`, `carrier`,
//! `selecting_machine`, `source_span`, and the un-hashed application members
//! -- is joined by exact structural equality: `derived != retained` rejects.
//!
//! The declared `OpaqueRepresentationSelectionFieldForTest` inventory covers
//! every representable substitution lane: the closed application's hashed and
//! un-hashed members separately, plus the row-level handles. `lifecycle` and
//! `origin` each have exactly one representable variant, so no substitution
//! axis exists for them; the `honest_fixture_replays_retained_custody` test
//! pins that contract by exhaustive match. Each leg assembles the substituted
//! row through `from_validated_application`, which recomputes the stored
//! commitment honestly -- whether the containing digest moves is itself part
//! of the leg's expected outcome (`Digested` vs `Provenance` below).

use super::{harvest_opaque_representation_selections, rederive_opaque_representation_selections};
use crate::{
    OpaqueRepresentationApplicationOrigin, OpaqueRepresentationCopyDisposition,
    OpaqueRepresentationLifecycleDisposition, OpaqueRepresentationSelection,
};
use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_semantics::{DataSupplyMode, Multiplicity};
use optimization_core::MutationOutcome;
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

optimization_core::custody_field_inventory! {
    /// One substitutable field of a retained
    /// [`OpaqueRepresentationSelection`]. The custody matrix substitutes
    /// exactly one field per leg so a rejection attributes to that claim
    /// alone. The record has no wire form, so no field is
    /// canonical-encoding-closed; `lifecycle` and `origin` are unrepresentable
    /// (single-variant vocabularies) and stay outside the inventory. The
    /// `ApplicationSubjectIdentity` option admits two substitutions -- dropping
    /// to `None` and adopting a foreign identity -- so it occupies two
    /// variants. The independent checker is
    /// `rederive_opaque_representation_selections`, and joined
    /// `run_one_field_substitution_matrix` legs re-assert the substituted
    /// row's internal commitment honesty.
    pub enum OpaqueRepresentationSelectionFieldForTest {
        Opaque,
        Carrier,
        CopyDisposition,
        SelectingMachine,
        SourceSpan,
        ApplicationDeclaration,
        ApplicationArguments,
        ApplicationLifetimeArguments,
        ApplicationTypeArguments,
        ApplicationConstArguments,
        ApplicationMachineArguments,
        ApplicationSubjectIdentityDropped,
        ApplicationSubjectIdentityForeign,
        ApplicationTraitDefinition,
        ApplicationTraitLifetimeArguments,
        ApplicationTraitArguments,
        ApplicationRows,
        ApplicationReportFingerprint,
        ApplicationCommitment,
    }
}

/// The checker outcome a substituted retained row must produce.
///
/// `rederive_opaque_representation_selections` reports `Vec<Diagnostic>`; the
/// matrix needs an exact per-field expectation, so the check classifies the
/// rejection into the custody arm that fired:
///
/// - `StaleBuildMachineCustody` -- the `selecting_machine` provenance lane
///   mismatches the settlement's authoritative machine before row equality
///   runs.
/// - `MovedCommitmentDisagreement` -- the field is digested, so the honestly
///   recomputed commitment moves and the row join rejects.
/// - `ProvenanceDisagreement` -- the field is deliberately absent from the
///   commitment; the exact row join still binds it.
/// - `Unclassified` -- a rejection the matrix does not name; declaring it in
///   `outcome` is never correct, so producing it always fails the leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainedSelectionRejection {
    StaleBuildMachineCustody,
    MovedCommitmentDisagreement,
    ProvenanceDisagreement,
    Unclassified,
}

/// One authentic program: the compiler-owned `OpaqueRepresentation<Opaque>`
/// trait declared in toolchain source, two boundary-opaque declarations with
/// checked-shape carriers and named closed conformances, and one authoritative
/// build machine whose state selects both representations.
pub struct Fixture {
    pub typed: TypedTrees,
    pub build_machine: SymbolHandle,
    pub foreign_machine: SymbolHandle,
    pub foreign_data: SymbolHandle,
    pub foreign_trait: SymbolHandle,
    pub foreign_state: SymbolHandle,
    pub foreign_type_reference: TypeReferenceHandle,
    pub other_source_span: SourceSpan,
    /// Harvested baseline in statement order: `[Token, Marker]`.
    pub selections: Vec<OpaqueRepresentationSelection>,
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

pub fn fixture() -> Fixture {
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

    let mut typed = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };

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
pub struct SelectionParts {
    pub opaque: SymbolHandle,
    pub carrier: SymbolHandle,
    pub application: ClosedConformanceApplication,
    pub lifecycle: OpaqueRepresentationLifecycleDisposition,
    pub copy_disposition: OpaqueRepresentationCopyDisposition,
    pub origin: OpaqueRepresentationApplicationOrigin,
    pub selecting_machine: SymbolHandle,
    pub source_span: SourceSpan,
}

impl SelectionParts {
    pub fn of(selection: &OpaqueRepresentationSelection) -> Self {
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

    pub fn assemble(&self) -> OpaqueRepresentationSelection {
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

pub fn replay(
    fixture: &Fixture,
    retained: &[OpaqueRepresentationSelection],
) -> Result<Vec<OpaqueRepresentationSelection>, Vec<Diagnostic>> {
    rederive_opaque_representation_selections(&fixture.typed, Some(fixture.build_machine), retained)
}

pub fn expect_rejection(
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

/// The record one substitution leg mutates: the program fixture that produced
/// the retained roster plus the substituted first row. The checker replays
/// the roster with this row at position zero.
pub struct RetainedSelectionCustodyCase {
    pub fixture: Fixture,
    pub row: OpaqueRepresentationSelection,
}

/// The honestly produced case: the baseline fixture with its authentic first
/// retained row.
pub fn honest_retained_selection_custody_case() -> RetainedSelectionCustodyCase {
    let fixture = fixture();
    RetainedSelectionCustodyCase {
        row: fixture.selections[0].clone(),
        fixture,
    }
}

/// An authentic foreign row of the same family: every substitutable lane
/// carries foreign evidence assembled through the honest constructor, so
/// donor-drawn substitutions move real values rather than fabricated noise.
/// `subject_identity` takes the foreign identity; the `Dropped` leg still
/// substitutes the fixed `None` alternate.
pub fn foreign_retained_selection_donor() -> RetainedSelectionCustodyCase {
    let fixture = fixture();
    let mut parts = SelectionParts::of(&fixture.selections[0]);
    parts.opaque = fixture.foreign_data;
    parts.carrier = fixture.foreign_data;
    parts.copy_disposition = match parts.copy_disposition {
        OpaqueRepresentationCopyDisposition::PlacementOnly => {
            OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
        }
        _ => OpaqueRepresentationCopyDisposition::PlacementOnly,
    };
    parts.selecting_machine = fixture.foreign_machine;
    parts.source_span = fixture.other_source_span;
    parts.application.declaration = fixture.foreign_trait;
    parts.application.arguments =
        vec![named_argument("Foreign", fixture.foreign_data)].into_boxed_slice();
    parts.application.lifetime_arguments = vec![String::from("'foreign")];
    parts.application.type_arguments = vec![String::from("named(name(Foreign))")];
    parts.application.const_arguments = vec![ClosedConformanceConstArgument::CallerBinder {
        parameter_carrier: fixture.foreign_type_reference,
        binder: fixture.foreign_data,
        binder_carrier: fixture.foreign_type_reference,
    }];
    parts.application.machine_arguments = vec![fixture.foreign_machine];
    parts.application.subject_identity = Some(String::from("ForeignBytes"));
    parts.application.trait_definition = fixture.foreign_trait;
    parts.application.trait_lifetime_arguments = vec![String::from("'foreign")];
    parts.application.trait_arguments = vec![String::from("named(name(Foreign))")];
    parts.application.rows = vec![ClosedConformanceRowIdentity {
        declaring_trait: fixture.foreign_trait,
        requirement: fixture.foreign_data,
        realization_machine: fixture.foreign_machine,
        realization_state: fixture.foreign_state,
    }];
    parts.application.report_fingerprint = parts.application.report_fingerprint.wrapping_add(1);
    parts.application.commitment = ClosedConformanceApplicationCommitment::from_digest(
        parts
            .application
            .commitment
            .as_bytes()
            .map(|byte| byte ^ 0xff),
    );
    RetainedSelectionCustodyCase {
        row: parts.assemble(),
        fixture,
    }
}

/// Mutate exactly one declared lane of the retained row. Each arm re-enters
/// `from_validated_application`, so the stored commitment always reflects the
/// substituted fields honestly; whether it diverges from the baseline is the
/// leg's digested/provenance expectation, not a choice the mutation makes.
pub fn corrupt_retained_selection_custody_for_test(
    case: &mut RetainedSelectionCustodyCase,
    field: OpaqueRepresentationSelectionFieldForTest,
    donor: &RetainedSelectionCustodyCase,
) {
    use OpaqueRepresentationSelectionFieldForTest as Field;
    let mut parts = SelectionParts::of(&case.row);
    let donor_parts = SelectionParts::of(&donor.row);
    match field {
        Field::Opaque => parts.opaque = donor_parts.opaque,
        Field::Carrier => parts.carrier = donor_parts.carrier,
        Field::CopyDisposition => parts.copy_disposition = donor_parts.copy_disposition,
        Field::SelectingMachine => parts.selecting_machine = donor_parts.selecting_machine,
        Field::SourceSpan => parts.source_span = donor_parts.source_span,
        Field::ApplicationDeclaration => {
            parts.application.declaration = donor_parts.application.declaration;
        }
        Field::ApplicationArguments => {
            parts.application.arguments = donor_parts.application.arguments.clone();
        }
        Field::ApplicationLifetimeArguments => {
            parts.application.lifetime_arguments =
                donor_parts.application.lifetime_arguments.clone();
        }
        Field::ApplicationTypeArguments => {
            parts.application.type_arguments = donor_parts.application.type_arguments.clone();
        }
        Field::ApplicationConstArguments => {
            parts.application.const_arguments = donor_parts.application.const_arguments.clone();
        }
        Field::ApplicationMachineArguments => {
            parts.application.machine_arguments = donor_parts.application.machine_arguments.clone();
        }
        Field::ApplicationSubjectIdentityDropped => {
            parts.application.subject_identity = None;
        }
        Field::ApplicationSubjectIdentityForeign => {
            parts.application.subject_identity = donor_parts.application.subject_identity.clone();
        }
        Field::ApplicationTraitDefinition => {
            parts.application.trait_definition = donor_parts.application.trait_definition;
        }
        Field::ApplicationTraitLifetimeArguments => {
            parts.application.trait_lifetime_arguments =
                donor_parts.application.trait_lifetime_arguments.clone();
        }
        Field::ApplicationTraitArguments => {
            parts.application.trait_arguments = donor_parts.application.trait_arguments.clone();
        }
        Field::ApplicationRows => {
            parts.application.rows = donor_parts.application.rows.clone();
        }
        Field::ApplicationReportFingerprint => {
            parts.application.report_fingerprint = donor_parts.application.report_fingerprint;
        }
        Field::ApplicationCommitment => {
            parts.application.commitment = donor_parts.application.commitment;
        }
    }
    case.row = parts.assemble();
}

/// Classify the replay rejection into the custody arm that fired. A
/// `selecting_machine` substitution trips the stale-custody arm before row
/// equality runs; every other representable lane reaches the row join, where
/// a digested field has already moved the recomputed commitment and a
/// provenance field still binds through exact equality.
fn classify_rejection(
    fixture: &Fixture,
    substituted: &OpaqueRepresentationSelection,
    diagnostics: &[Diagnostic],
) -> RetainedSelectionRejection {
    if diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stale build-machine or application custody")
    }) {
        return RetainedSelectionRejection::StaleBuildMachineCustody;
    }
    if diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rederivation disagrees with retained build custody")
    }) {
        return if substituted.selected_application_commitment()
            != fixture.selections[0].selected_application_commitment()
        {
            RetainedSelectionRejection::MovedCommitmentDisagreement
        } else {
            RetainedSelectionRejection::ProvenanceDisagreement
        };
    }
    RetainedSelectionRejection::Unclassified
}

/// The family's independent checker: replay the retained roster with the
/// substituted row at position zero.
pub fn check_retained_selection_custody(
    case: &RetainedSelectionCustodyCase,
) -> Result<OpaqueRepresentationSelection, RetainedSelectionRejection> {
    let mut retained = case.fixture.selections.clone();
    retained[0] = case.row.clone();
    match replay(&case.fixture, &retained) {
        Ok(_) => Ok(case.row.clone()),
        Err(diagnostics) => Err(classify_rejection(&case.fixture, &case.row, &diagnostics)),
    }
}

/// The expected checker arm per declared field: `selecting_machine` trips the
/// stale-custody arm, digested fields move the recomputed commitment, and
/// every provenance lane still fails the exact row join.
pub fn retained_selection_custody_outcome(
    field: OpaqueRepresentationSelectionFieldForTest,
) -> MutationOutcome<RetainedSelectionRejection> {
    use OpaqueRepresentationSelectionFieldForTest as Field;
    let rejection = match field {
        Field::SelectingMachine => RetainedSelectionRejection::StaleBuildMachineCustody,
        Field::CopyDisposition | Field::ApplicationCommitment => {
            RetainedSelectionRejection::MovedCommitmentDisagreement
        }
        _ => RetainedSelectionRejection::ProvenanceDisagreement,
    };
    MutationOutcome::ExactError(rejection)
}

/// Per-leg joined assertion: `from_validated_application` recomputed the
/// stored commitment over the substituted fields, so the row must carry its
/// own honest recomputation even though the roster join still rejects it.
pub fn retained_selection_joined_replay(
    case: &RetainedSelectionCustodyCase,
    field: OpaqueRepresentationSelectionFieldForTest,
) {
    assert_eq!(
        case.row.selected_application_commitment(),
        case.row.rederived_selected_application_commitment(),
        "substituted {field:?} must carry its honest commitment recomputation",
    );
}
