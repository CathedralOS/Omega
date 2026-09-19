//! One-field mutation coverage for the semantic module's reborrow restored
//! call-use roster.
//!
//! `reborrow_restored_call_uses` is the module-level custody record of each
//! closed restored-parent call publication: the owning machine and the exact
//! mutating `CallUnit` operation it authorizes, the restoration class, the
//! source call boundary, the call target machine, the source machine and
//! state identities, the direct root's owner identity, its necessarily empty
//! owner path, the root place, the activation and weakening boundaries of an
//! exact one-hop lifecycle, the lifetime identity that must reiterate the
//! root place's root, the restored child's owner identity, owner path, place
//! that must extend the root place by exactly the declared projection
//! remainder, child access, and the activation, formation, and weakening
//! boundaries with formation equal to activation and the call boundary's
//! statement equal to the child weakening. A shared-freeze row additionally
//! carries a one-to-three-member cohort whose primary member reiterates the
//! row's child strand while every member keeps a shared access and the row's
//! own weakening. Its wire fields — the roster count, each row's machine and
//! operation identities, the class tag, the call boundary envelope, the
//! length-framed identity strings, the counted owner paths and place
//! segments, and every cohort member field — are each substituted
//! independently. A substitution either fails canonical decoding or the
//! verifier's module-bound validation (a machine that does not own the
//! operation, an operation that is not the exact mutating call, a class tag
//! that contradicts the child access or cohort roster, a call boundary split
//! from the child weakening, a lifetime that strays from the root place, a
//! child place that no longer extends its parent, a formation boundary split
//! from activation, a cohort member that drifts from the primary strand, or
//! a reordered roster), or it decodes to a different module whose honestly
//! recomputed semantic and artifact identities diverge and whose replay
//! against the retained manifest and sealed proof subject rejects.
//!
//! The fixture keeps two mutating callers beside the shared Unit callee so
//! the roster carries an exclusive-reactivation row and a shared-freeze row
//! whose single-member cohort reiterates the row's child strand. Every
//! lifecycle boundary is a `Statement` while each call boundary is a `Call`
//! with ordinal zero, so both boundary variants appear on the wire.

use std::ops::Range;

use super::{
    block_id, canonical_artifact, contract_id, edge_id, machine_id, operation_id, place_id,
    structural_type_id,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_codec::{
    ArtifactManifestError, CodecError, ProofCodecError, build_artifact_manifest,
    build_identity_optimization_execution_record, decode_module, decode_proof_section_for,
    encode_module, terminal_psi_identity, validate_artifact_manifest,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment, TerminalBorrowPlace,
    TerminalBorrowPlaceSegment, TerminalMachine, TerminalMachineResult, TerminalModule,
    TerminalReborrowRestorationClass, TerminalReborrowRestoredCallUse,
    TerminalReborrowSharedCohortMember, Terminator,
};
use terminal_verifier::{ModuleError, ProofBundle, verify_module};

/// Byte offsets of every wire field inside the reborrow restored-call-use
/// roster. Every `string` field records its u32 length prefix and its
/// content bytes separately so a substitution can lie about either side,
/// and every counted roster records its own count span.
struct ModuleSpans {
    use_count: Range<usize>,
    uses: Vec<UseSpan>,
}

/// A `Statement` boundary is tag + statement index; a `Call` boundary adds
/// a call ordinal and a length-framed target identity. The fixture carries
/// both, so the optional spans are present on each row's call boundary.
struct BoundarySpan {
    tag: Range<usize>,
    statement_index: Range<usize>,
    call_ordinal: Option<Range<usize>>,
    target_len: Option<Range<usize>>,
    target: Option<Range<usize>>,
}

struct OwnerSegmentSpan {
    row: Range<usize>,
    tag: Range<usize>,
    text_len: Option<Range<usize>>,
    text: Option<Range<usize>>,
    index: Option<Range<usize>>,
}

struct PlaceSegmentSpan {
    row: Range<usize>,
    tag: Range<usize>,
    text_len: Option<Range<usize>>,
    text: Option<Range<usize>>,
    index: Option<Range<usize>>,
    range_start: Option<Range<usize>>,
    range_end: Option<Range<usize>>,
}

struct PlaceSpan {
    root_len: Range<usize>,
    root: Range<usize>,
    segment_count: Range<usize>,
    segments: Vec<PlaceSegmentSpan>,
}

struct MemberSpan {
    row: Range<usize>,
    child_owner_len: Range<usize>,
    child_owner: Range<usize>,
    child_owner_path_count: Range<usize>,
    child_owner_path: Vec<OwnerSegmentSpan>,
    child_place: PlaceSpan,
    child_access: Range<usize>,
    child_activation: BoundarySpan,
    child_weakening: BoundarySpan,
}

struct UseSpan {
    row: Range<usize>,
    machine: Range<usize>,
    operation: Range<usize>,
    class: Range<usize>,
    call_boundary: BoundarySpan,
    call_target: Range<usize>,
    source_machine_len: Range<usize>,
    source_machine: Range<usize>,
    source_state_len: Range<usize>,
    source_state: Range<usize>,
    owner_len: Range<usize>,
    owner: Range<usize>,
    owner_path_count: Range<usize>,
    owner_path: Vec<OwnerSegmentSpan>,
    place: PlaceSpan,
    activation: BoundarySpan,
    weakening: BoundarySpan,
    lifetime_len: Range<usize>,
    lifetime: Range<usize>,
    child_owner_len: Range<usize>,
    child_owner: Range<usize>,
    child_owner_path_count: Range<usize>,
    child_owner_path: Vec<OwnerSegmentSpan>,
    child_place: PlaceSpan,
    remainder_count: Range<usize>,
    remainder: Vec<PlaceSegmentSpan>,
    child_access: Range<usize>,
    child_activation: BoundarySpan,
    formation: BoundarySpan,
    child_weakening: BoundarySpan,
    cohort_count: Range<usize>,
    cohort: Vec<MemberSpan>,
}

/// A cursor that walks the canonical module encoding exactly as the decoder
/// does, recording the byte span of every restored-call-use field the matrix
/// substitutes. Every section ahead of the roster is empty in the fixture
/// except the single-field structural type declaration, so the walk asserts
/// each leading count rather than silently spanning unknown row bytes.
struct SpanWalker<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> SpanWalker<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Range<usize> {
        let start = self.offset;
        self.offset += len;
        start..self.offset
    }

    fn take_count(&mut self) -> (Range<usize>, u32) {
        let span = self.take(4);
        let count = u32::from_le_bytes(
            self.bytes[span.clone()]
                .try_into()
                .expect("u32 count inside the module"),
        );
        (span, count)
    }

    fn expect_empty_count(&mut self, label: &'static str) {
        let (_, count) = self.take_count();
        assert_eq!(count, 0, "the fixture leaves the {label} roster empty");
    }

    fn take_string(&mut self) -> (Range<usize>, Range<usize>) {
        let (len_span, len) = self.take_count();
        let value = self.take(usize::try_from(len).expect("string length fits usize"));
        (len_span, value)
    }

    fn tag(&mut self) -> (Range<usize>, u8) {
        let span = self.take(1);
        (span.clone(), self.bytes[span.start])
    }

    fn walk_boundary(&mut self) -> BoundarySpan {
        let (tag, value) = self.tag();
        let statement_index = self.take(8);
        let (call_ordinal, target_len, target) = match value {
            1 => (None, None, None),
            2 => {
                let ordinal = self.take(8);
                let (target_len, target) = self.take_string();
                (Some(ordinal), Some(target_len), Some(target))
            }
            other => panic!("fixture boundaries are Statement or Call, not tag {other}"),
        };
        BoundarySpan {
            tag,
            statement_index,
            call_ordinal,
            target_len,
            target,
        }
    }

    fn walk_owner_segment(&mut self) -> OwnerSegmentSpan {
        let row_start = self.offset;
        let (tag, value) = self.tag();
        let (text_len, text, index) = match value {
            1 | 2 => {
                let (len, text) = self.take_string();
                (Some(len), Some(text), None)
            }
            3 => (None, None, Some(self.take(8))),
            4 => (None, None, None),
            other => panic!("fixture owner segments are known tags, not {other}"),
        };
        OwnerSegmentSpan {
            row: row_start..self.offset,
            tag,
            text_len,
            text,
            index,
        }
    }

    fn walk_owner_path(&mut self) -> (Range<usize>, Vec<OwnerSegmentSpan>) {
        let (count, segments) = self.take_count();
        let mut spans = Vec::with_capacity(usize::try_from(segments).expect("segments fit"));
        for _ in 0..segments {
            spans.push(self.walk_owner_segment());
        }
        (count, spans)
    }

    fn walk_place_segment(&mut self) -> PlaceSegmentSpan {
        let row_start = self.offset;
        let (tag, value) = self.tag();
        let (text_len, text, index, range_start, range_end) = match value {
            1 | 2 => {
                let (len, text) = self.take_string();
                (Some(len), Some(text), None, None, None)
            }
            3 => (None, None, Some(self.take(8)), None, None),
            4 => {
                let start = self.take(8);
                let end = self.take(8);
                (None, None, None, Some(start), Some(end))
            }
            other => panic!("fixture place segments are known tags, not {other}"),
        };
        PlaceSegmentSpan {
            row: row_start..self.offset,
            tag,
            text_len,
            text,
            index,
            range_start,
            range_end,
        }
    }

    fn walk_place_segments(&mut self) -> (Range<usize>, Vec<PlaceSegmentSpan>) {
        let (count, segments) = self.take_count();
        let mut spans = Vec::with_capacity(usize::try_from(segments).expect("segments fit"));
        for _ in 0..segments {
            spans.push(self.walk_place_segment());
        }
        (count, spans)
    }

    fn walk_place(&mut self) -> PlaceSpan {
        let (root_len, root) = self.take_string();
        let (segment_count, segments) = self.walk_place_segments();
        PlaceSpan {
            root_len,
            root,
            segment_count,
            segments,
        }
    }

    fn walk_member(&mut self) -> MemberSpan {
        let row_start = self.offset;
        let (child_owner_len, child_owner) = self.take_string();
        let (child_owner_path_count, child_owner_path) = self.walk_owner_path();
        let child_place = self.walk_place();
        let child_access = self.take(1);
        let child_activation = self.walk_boundary();
        let child_weakening = self.walk_boundary();
        MemberSpan {
            row: row_start..self.offset,
            child_owner_len,
            child_owner,
            child_owner_path_count,
            child_owner_path,
            child_place,
            child_access,
            child_activation,
            child_weakening,
        }
    }

    fn walk_use(&mut self) -> UseSpan {
        let row_start = self.offset;
        let machine = self.take(8);
        let operation = self.take(8);
        let class = self.take(1);
        let call_boundary = self.walk_boundary();
        let call_target = self.take(8);
        let (source_machine_len, source_machine) = self.take_string();
        let (source_state_len, source_state) = self.take_string();
        let (owner_len, owner) = self.take_string();
        let (owner_path_count, owner_path) = self.walk_owner_path();
        let place = self.walk_place();
        let activation = self.walk_boundary();
        let weakening = self.walk_boundary();
        let (lifetime_len, lifetime) = self.take_string();
        let (child_owner_len, child_owner) = self.take_string();
        let (child_owner_path_count, child_owner_path) = self.walk_owner_path();
        let child_place = self.walk_place();
        let (remainder_count, remainder) = self.walk_place_segments();
        let child_access = self.take(1);
        let child_activation = self.walk_boundary();
        let formation = self.walk_boundary();
        let child_weakening = self.walk_boundary();
        let (cohort_count, members) = self.take_count();
        let mut cohort = Vec::with_capacity(usize::try_from(members).expect("members fit"));
        for _ in 0..members {
            cohort.push(self.walk_member());
        }
        UseSpan {
            row: row_start..self.offset,
            machine,
            operation,
            class,
            call_boundary,
            call_target,
            source_machine_len,
            source_machine,
            source_state_len,
            source_state,
            owner_len,
            owner,
            owner_path_count,
            owner_path,
            place,
            activation,
            weakening,
            lifetime_len,
            lifetime,
            child_owner_len,
            child_owner,
            child_owner_path_count,
            child_owner_path,
            child_place,
            remainder_count,
            remainder,
            child_access,
            child_activation,
            formation,
            child_weakening,
            cohort_count,
            cohort,
        }
    }
}

/// Locate the reborrow restored-call-use roster inside canonical module
/// bytes by mirroring `encode_raw`'s ordered section list. The fixture
/// module keeps every earlier roster empty except the single structural
/// type declaration the callers' parameters reference.
fn module_spans(encoded: &[u8]) -> ModuleSpans {
    let mut walker = SpanWalker::new(encoded);
    walker.take(8); // module magic
    walker.take(2); // format marker
    walker.take(2); // vocabulary marker
    walker.take(8); // entry machine identity
    // The scalar-qualification catalog encodes four counted rosters even when
    // empty: domains, qualification sets, coercions, and float entry ranges.
    for label in [
        "scalar domains",
        "scalar qualification sets",
        "scalar qualification coercions",
        "scalar float entry ranges",
    ] {
        walker.expect_empty_count(label);
    }
    // The lone structural type is a fieldless record: identity, shape tag 1,
    // and an empty field roster.
    let (_, types) = walker.take_count();
    assert_eq!(types, 1, "the fixture declares one structural type");
    walker.take(8); // structural type identity
    walker.take_string(); // structural type identity string
    let (_, tag) = walker.tag();
    assert_eq!(tag, 1, "the fixture's structural type is a record");
    walker.expect_empty_count("structural fields");
    for label in [
        "structural domains",
        "services",
        "concrete root service reach",
        "installation reach dependencies",
        "placed-view inputs",
        "reborrow root handoffs",
    ] {
        walker.expect_empty_count(label);
    }
    let (use_count, uses) = walker.take_count();
    let mut spans = Vec::with_capacity(usize::try_from(uses).expect("uses fit"));
    for _ in 0..uses {
        spans.push(walker.walk_use());
    }
    ModuleSpans {
        use_count,
        uses: spans,
    }
}

/// A canonical borrow identity: `terminal-borrow:` followed by a 64-digit
/// lowercase hex digest, the form `validate_reborrow_restored_call_uses`
/// requires of every machine, state, owner, place-root, and lifetime
/// identity.
fn borrow(digest: u64) -> String {
    format!("terminal-borrow:{digest:064x}")
}

fn statement(statement_index: u64) -> TerminalBorrowBoundarySource {
    TerminalBorrowBoundarySource::Statement { statement_index }
}

/// One restored-call-use row whose every identity derives from `seed`, so
/// each row in the fixture carries distinct canonical strings. The direct
/// root takes a field and a fixed range, the child owner path exercises a
/// case and a fixed index, and the child place extends the root place by a
/// two-segment projection remainder. `restoration_class` selects the
/// exclusive or the shared-freeze variant; the shared variant's single
/// cohort member reiterates the row's child strand exactly.
fn use_row(
    machine: u64,
    operation: u64,
    seed: u64,
    restoration_class: TerminalReborrowRestorationClass,
) -> TerminalReborrowRestoredCallUse {
    let root = borrow(seed * 0x100 + 1);
    let root_segments = vec![
        TerminalBorrowPlaceSegment::Field(borrow(seed * 0x100 + 8)),
        TerminalBorrowPlaceSegment::FixedRange {
            start: seed + 1,
            end: seed + 4,
        },
    ];
    let remainder = vec![
        TerminalBorrowPlaceSegment::FixedIndex(seed + 12),
        TerminalBorrowPlaceSegment::Case(borrow(seed * 0x100 + 13)),
    ];
    let mut child_segments = root_segments.clone();
    child_segments.extend(remainder.iter().cloned());
    let child_place = TerminalBorrowPlace {
        root_identity: root.clone(),
        segments: child_segments,
    };
    let child_owner_path = vec![
        TerminalBorrowOwnerSegment::Case(borrow(seed * 0x100 + 10)),
        TerminalBorrowOwnerSegment::FixedIndex(seed + 11),
    ];
    let (child_access, shared_cohort) = match restoration_class {
        TerminalReborrowRestorationClass::ExclusiveReactivation => {
            (StructuralAccess::WriteOnlyBorrow, Vec::new())
        }
        TerminalReborrowRestorationClass::SharedFreezeRestoration => (
            StructuralAccess::SharedBorrow,
            vec![TerminalReborrowSharedCohortMember {
                child_owner_identity: borrow(seed * 0x100 + 9),
                child_owner_path: child_owner_path.clone(),
                child_place: child_place.clone(),
                child_access: StructuralAccess::SharedBorrow,
                child_activation: statement(seed * 10 + 1),
                child_weakening: statement(seed * 10 + 2),
            }],
        ),
    };
    TerminalReborrowRestoredCallUse {
        machine: machine_id(machine),
        operation: operation_id(operation),
        restoration_class,
        call_boundary: TerminalBorrowBoundarySource::Call {
            statement_index: seed * 10 + 2,
            call_ordinal: 0,
            target_identity: borrow(seed * 0x100 + 14),
        },
        call_target_machine: machine_id(2),
        source_machine_identity: borrow(seed * 0x100 + 2),
        source_state_identity: borrow(seed * 0x100 + 3),
        direct_root_owner_identity: borrow(seed * 0x100 + 4),
        direct_root_owner_path: Vec::new(),
        direct_root_place: TerminalBorrowPlace {
            root_identity: root.clone(),
            segments: root_segments,
        },
        direct_root_activation: statement(seed * 10),
        direct_root_weakening: statement(seed * 10 + 10),
        direct_root_lifetime_identity: root,
        child_owner_identity: borrow(seed * 0x100 + 9),
        child_owner_path,
        child_place,
        projection_remainder: remainder,
        child_access,
        child_activation: statement(seed * 10 + 1),
        formation_boundary: statement(seed * 10 + 1),
        child_weakening: statement(seed * 10 + 2),
        shared_cohort,
    }
}

/// The empty structural record every mutating parameter and call argument
/// references.
fn restored_cell() -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "RestoredCell".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    }
}

/// One caller machine holding the exact mutating `CallUnit` operation a
/// restored-call-use row authorizes: a Unit result, one unrestricted
/// mutable-borrow structural parameter, and a single structural argument
/// loaned mutably to the shared callee.
fn caller_machine(raw: u64, place: u64, operation: u64) -> TerminalMachine {
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(raw),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: place_id(place),
            position: 0,
            is_self: false,
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: place_id(place),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(raw),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: block_id(raw),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: operation_id(operation),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    callee: machine_id(2),
                    arguments: Vec::new(),
                    structural_arguments: vec![StructuralArgument {
                        place: place_id(place),
                        path: Vec::new(),
                        access: StructuralAccess::MutableBorrow,
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: edge_id(raw),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(raw),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

/// The shared callee: a Unit machine taking one mutable-borrow structural
/// parameter, as `exact_restored_mutating_call` requires.
fn callee_machine() -> TerminalMachine {
    let mut machine = caller_machine(2, 2, 2);
    machine.blocks[0].operations.clear();
    machine
}

fn restored_use_module() -> TerminalModule {
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![restored_cell()],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: vec![
            use_row(
                1,
                1,
                1,
                TerminalReborrowRestorationClass::ExclusiveReactivation,
            ),
            use_row(
                3,
                3,
                3,
                TerminalReborrowRestorationClass::SharedFreezeRestoration,
            ),
        ],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![
            caller_machine(1, 1, 1),
            callee_machine(),
            caller_machine(3, 3, 3),
        ],
    }
}

/// The fixture carries no contract obligations, so the retained proof bundle
/// is the empty bundle.
fn restored_use_bundle() -> ProofBundle {
    ProofBundle::default()
}

#[test]
fn terminal_reborrow_restored_call_uses_reject_every_one_field_substitution() {
    let module = restored_use_module();
    let bundle = restored_use_bundle();
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("the fixture module verifies under its proof bundle");
    let encoded = encode_module(&module).expect("canonical module bytes");
    assert_eq!(
        decode_module(&encoded),
        Ok(module.clone()),
        "the canonical module round-trips"
    );
    let spans = module_spans(&encoded);
    assert_eq!(
        spans.uses.len(),
        2,
        "fixture roster: an exclusive row on machine 1 and a shared-freeze row on machine 3"
    );
    assert!(
        spans.uses.iter().all(|row| row.owner_path.is_empty()),
        "the restored-use direct root owner path is closed empty"
    );

    // The retained artifact binds the semantic identity into its manifest
    // and the sealed proof section to this exact module; replaying either
    // against a substituted module is the independent-replay leg for every
    // representable field.
    let artifact = canonical_artifact(&module, &bundle, None);
    let retained = artifact.manifest();
    let semantic_identity = terminal_psi_identity(&module).expect("semantic identity");
    assert_eq!(retained.semantic(), semantic_identity);

    // A substitution that still forms a canonical module honestly
    // recomputes a divergent semantic and artifact identity: the
    // substituted module still verifies under the retained bundle (the
    // roster carries no proof obligations), while the retained custody
    // replays — the manifest join and the sealed proof subject join —
    // reject it.
    let divergent = |name: &'static str, mutated: &[u8]| {
        let substituted = decode_module(mutated)
            .unwrap_or_else(|error| panic!("{name} must still decode: {error:?}"));
        assert_ne!(substituted, module, "{name} must change the module");
        assert_eq!(
            encode_module(&substituted).expect("re-encode the substitution"),
            mutated,
            "{name} must re-encode canonically"
        );
        assert_ne!(
            terminal_psi_identity(&substituted).expect("substituted semantic identity"),
            semantic_identity,
            "{name} must diverge the honestly recomputed semantic identity"
        );
        verify_module(&substituted, &bundle, &AdmissionProfile::default()).unwrap_or_else(
            |error| panic!("{name} must keep the substituted module verifiable: {error:?}"),
        );
        let recomputed_optimization =
            build_identity_optimization_execution_record(&substituted, &bundle)
                .expect("identity optimization over the substituted module");
        let recomputed =
            build_artifact_manifest(&substituted, &bundle, &recomputed_optimization, None, None)
                .expect("honest manifest over the substituted module");
        assert_ne!(
            recomputed.identity(),
            retained.identity(),
            "{name} must diverge the recomputed artifact identity"
        );
        assert_eq!(
            validate_artifact_manifest(
                &substituted,
                &bundle,
                &recomputed_optimization,
                None,
                None,
                retained,
            ),
            Err(ArtifactManifestError::ManifestMismatch),
            "{name} must reject at the retained-manifest replay"
        );
        assert!(
            matches!(
                decode_proof_section_for(&substituted, artifact.proof_bytes()),
                Err(ProofCodecError::ProofSubjectMismatch { .. })
            ),
            "{name} must reject at the sealed proof subject join"
        );
        substituted
    };
    // A substitution that cannot form a canonical module rejects inside the
    // canonical decoder with an exact error.
    let rejected = |name: &'static str, mutated: &[u8], expected: CodecError| {
        assert_eq!(
            decode_module(mutated),
            Err(expected),
            "{name} must reject at canonical decoding"
        );
    };
    // A module-level mutation the producer can express rejects inside the
    // canonical encoder's semantic validation.
    let encode_rejected =
        |name: &'static str, changed: &terminal_psi::TerminalModule, expected: CodecError| {
            assert_eq!(
                encode_module(changed),
                Err(expected.clone()),
                "{name} must reject at canonical encoding"
            );
        };
    let put_u8 = |range: Range<usize>, value: u8| -> Vec<u8> {
        let mut mutated = encoded.clone();
        mutated[range.start] = value;
        mutated
    };
    let put_u32 = |range: Range<usize>, value: u32| -> Vec<u8> {
        let mut mutated = encoded.clone();
        mutated[range].copy_from_slice(&value.to_le_bytes());
        mutated
    };
    let put_u64 = |range: Range<usize>, value: u64| -> Vec<u8> {
        let mut mutated = encoded.clone();
        mutated[range].copy_from_slice(&value.to_le_bytes());
        mutated
    };
    // Replace one encoded string, keeping its length prefix honest.
    let restring = |len_span: Range<usize>, value_span: Range<usize>, value: &str| -> Vec<u8> {
        let mut mutated = encoded[..len_span.start].to_vec();
        mutated.extend_from_slice(
            &u32::try_from(value.len())
                .expect("string length fits u32")
                .to_le_bytes(),
        );
        mutated.extend_from_slice(value.as_bytes());
        mutated.extend_from_slice(&encoded[value_span.end..]);
        mutated
    };
    // Set a counted roster to `remaining` rows and remove the row bytes in
    // `rows`, keeping the count honest.
    let excise = |count_span: Range<usize>, rows: Range<usize>, remaining: u32| -> Vec<u8> {
        let mut mutated = encoded[..count_span.start].to_vec();
        mutated.extend_from_slice(&remaining.to_le_bytes());
        mutated.extend_from_slice(&encoded[count_span.end..rows.start]);
        mutated.extend_from_slice(&encoded[rows.end..]);
        mutated
    };
    // Remove one roster row and decrement its roster count honestly.
    let drop_row = |count_span: Range<usize>, row: Range<usize>| -> Vec<u8> {
        let remaining = u32::from_le_bytes(
            encoded[count_span.clone()]
                .try_into()
                .expect("roster count"),
        ) - 1;
        excise(count_span, row, remaining)
    };
    // Duplicate one roster row directly behind itself with an honest count.
    let duplicate_row = |count_span: Range<usize>, row: Range<usize>| -> Vec<u8> {
        let grown = u32::from_le_bytes(
            encoded[count_span.clone()]
                .try_into()
                .expect("roster count"),
        ) + 1;
        let mut mutated = encoded[..count_span.start].to_vec();
        mutated.extend_from_slice(&grown.to_le_bytes());
        mutated.extend_from_slice(&encoded[count_span.end..row.end]);
        mutated.extend_from_slice(&encoded[row.clone()]);
        mutated.extend_from_slice(&encoded[row.end..]);
        mutated
    };
    // Write `segment` as a lone owner-path segment behind an honest count of
    // one.
    let insert_owner_segment = |count_span: Range<usize>, segment: &[u8]| -> Vec<u8> {
        let mut mutated = encoded[..count_span.start].to_vec();
        mutated.extend_from_slice(&1_u32.to_le_bytes());
        mutated.extend_from_slice(segment);
        mutated.extend_from_slice(&encoded[count_span.end..]);
        mutated
    };

    let first = &spans.uses[0];
    let second = &spans.uses[1];
    let member = &second.cohort[0];
    let invalid_first = || {
        CodecError::InvalidModule(ModuleError::InvalidReborrowRestoredCallUse {
            machine: machine_id(1),
            operation: operation_id(1),
        })
    };
    let invalid_second = || {
        CodecError::InvalidModule(ModuleError::InvalidReborrowRestoredCallUse {
            machine: machine_id(3),
            operation: operation_id(3),
        })
    };

    // --- roster axes -----------------------------------------------------

    // A roster count lying about its rows reads the following empty
    // sections as a zero machine identity or starves the row.
    rejected(
        "a restored-use roster count one over",
        &put_u32(spans.use_count.clone(), 3),
        CodecError::ZeroIdentity("MachineId"),
    );
    rejected(
        "a maximal restored-use roster count",
        &put_u32(spans.use_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );
    // Clearing the roster or dropping either row stays representable: the
    // recomputed identity diverges and the retained custody replays reject.
    divergent(
        "a cleared restored-use roster",
        &excise(spans.use_count.clone(), first.row.start..second.row.end, 0),
    );
    divergent(
        "a dropped first restored-use row",
        &drop_row(spans.use_count.clone(), first.row.clone()),
    );
    divergent(
        "a dropped second restored-use row",
        &drop_row(spans.use_count.clone(), second.row.clone()),
    );
    // A duplicated row collides on the (machine, operation) coordinate; a
    // swapped roster violates the strict row order.
    rejected(
        "a duplicated restored-use row",
        &duplicate_row(spans.use_count.clone(), first.row.clone()),
        CodecError::InvalidModule(ModuleError::DuplicateReborrowRestoredCallUse),
    );
    let mut swapped = encoded[..first.row.start].to_vec();
    swapped.extend_from_slice(&encoded[second.row.clone()]);
    swapped.extend_from_slice(&encoded[first.row.clone()]);
    swapped.extend_from_slice(&encoded[second.row.end..]);
    rejected(
        "a reordered restored-use roster",
        &swapped,
        CodecError::InvalidModule(ModuleError::NonCanonicalReborrowRestoredCallUseOrder),
    );

    // --- machine and operation --------------------------------------------

    // The row is join-constrained twice over: the machine must own exactly
    // the named operation, and that operation must be the exact mutating
    // call to the named target. Every representable machine or operation
    // substitution strands one side of the join.
    rejected(
        "a zero restored-use machine",
        &put_u64(first.machine.clone(), 0),
        CodecError::ZeroIdentity("MachineId"),
    );
    rejected(
        "a restored-use machine outside the module",
        &put_u64(first.machine.clone(), 9),
        CodecError::InvalidModule(ModuleError::InvalidReborrowRestoredCallUse {
            machine: machine_id(9),
            operation: operation_id(1),
        }),
    );
    rejected(
        "a restored-use row rebound to the callee",
        &put_u64(first.machine.clone(), 2),
        CodecError::InvalidModule(ModuleError::InvalidReborrowRestoredCallUse {
            machine: machine_id(2),
            operation: operation_id(1),
        }),
    );
    rejected(
        "a restored-use row rebound to the other caller",
        &put_u64(first.machine.clone(), 3),
        CodecError::InvalidModule(ModuleError::InvalidReborrowRestoredCallUse {
            machine: machine_id(3),
            operation: operation_id(1),
        }),
    );
    rejected(
        "a zero restored-use operation",
        &put_u64(first.operation.clone(), 0),
        CodecError::ZeroIdentity("OperationId"),
    );
    rejected(
        "a restored-use operation outside the caller",
        &put_u64(first.operation.clone(), 9),
        CodecError::InvalidModule(ModuleError::InvalidReborrowRestoredCallUse {
            machine: machine_id(1),
            operation: operation_id(9),
        }),
    );
    rejected(
        "a restored-use operation named on another machine",
        &put_u64(first.operation.clone(), 3),
        CodecError::InvalidModule(ModuleError::InvalidReborrowRestoredCallUse {
            machine: machine_id(1),
            operation: operation_id(3),
        }),
    );

    // --- restoration class --------------------------------------------------

    // The class tag must agree with the child access and the cohort roster:
    // exclusive with a shared-freeze cohort shape and shared with an empty
    // cohort both reject.
    rejected(
        "an exclusive row recast as a shared freeze",
        &put_u8(first.class.clone(), 2),
        invalid_first(),
    );
    rejected(
        "a shared-freeze row recast as exclusive",
        &put_u8(second.class.clone(), 1),
        invalid_second(),
    );
    for tag in [0, 3, u8::MAX] {
        rejected(
            "an unknown restored-use class tag",
            &put_u8(first.class.clone(), tag),
            CodecError::InvalidTag("TerminalReborrowRestorationClass", tag),
        );
    }

    // --- call boundary -------------------------------------------------------

    // The call boundary is a `Call` whose statement reiterates the child
    // weakening and whose ordinal is exactly zero; only the canonical target
    // identity is a free coordinate.
    rejected(
        "a restored-use call boundary recast as a statement",
        &put_u8(first.call_boundary.tag.clone(), 1),
        CodecError::ZeroIdentity("MachineId"),
    );
    rejected(
        "an unknown restored-use call boundary tag",
        &put_u8(first.call_boundary.tag.clone(), 9),
        CodecError::InvalidTag("TerminalBorrowBoundarySource", 9),
    );
    rejected(
        "a moved restored-use call boundary statement",
        &put_u64(first.call_boundary.statement_index.clone(), 13),
        invalid_first(),
    );
    rejected(
        "a nonzero restored-use call boundary ordinal",
        &put_u64(first.call_boundary.call_ordinal.clone().unwrap(), 1),
        invalid_first(),
    );
    divergent(
        "a renamed restored-use call boundary target",
        &restring(
            first.call_boundary.target_len.clone().unwrap(),
            first.call_boundary.target.clone().unwrap(),
            &borrow(0x91),
        ),
    );
    rejected(
        "a non-canonical restored-use call boundary target",
        &restring(
            first.call_boundary.target_len.clone().unwrap(),
            first.call_boundary.target.clone().unwrap(),
            "target",
        ),
        invalid_first(),
    );
    rejected(
        "a non-UTF-8 restored-use call boundary target",
        &put_u8(first.call_boundary.target.clone().unwrap(), 0xFF),
        CodecError::InvalidUtf8("reborrow call target identity"),
    );

    // --- call target machine ---------------------------------------------------

    // The call target must equal the operation's own callee: every
    // representable substitution strands the join.
    rejected(
        "a zero restored-use call target",
        &put_u64(first.call_target.clone(), 0),
        CodecError::ZeroIdentity("MachineId"),
    );
    rejected(
        "a restored-use call target outside the module",
        &put_u64(first.call_target.clone(), 9),
        invalid_first(),
    );
    rejected(
        "a restored-use call target rebound to the caller",
        &put_u64(first.call_target.clone(), 1),
        invalid_first(),
    );
    rejected(
        "a restored-use call target rebound to the other caller",
        &put_u64(first.call_target.clone(), 3),
        invalid_first(),
    );

    // --- source identities -------------------------------------------------

    // Each source identity must stay a canonical borrow identity; any other
    // canonical spelling reidentifies the source and diverges.
    divergent(
        "a renamed restored-use source machine identity",
        &restring(
            first.source_machine_len.clone(),
            first.source_machine.clone(),
            &borrow(0x92),
        ),
    );
    rejected(
        "an emptied restored-use source machine identity",
        &restring(
            first.source_machine_len.clone(),
            first.source_machine.clone(),
            "",
        ),
        invalid_first(),
    );
    rejected(
        "a non-canonical restored-use source machine identity",
        &restring(
            first.source_machine_len.clone(),
            first.source_machine.clone(),
            "host-machine",
        ),
        invalid_first(),
    );
    rejected(
        "a non-UTF-8 restored-use source machine identity",
        &put_u8(first.source_machine.clone(), 0xFF),
        CodecError::InvalidUtf8("restored-use machine identity"),
    );

    divergent(
        "a renamed restored-use source state identity",
        &restring(
            first.source_state_len.clone(),
            first.source_state.clone(),
            &borrow(0x93),
        ),
    );
    rejected(
        "an emptied restored-use source state identity",
        &restring(
            first.source_state_len.clone(),
            first.source_state.clone(),
            "",
        ),
        invalid_first(),
    );
    rejected(
        "a restored-use source state identity length lie",
        &put_u32(first.source_state_len.clone(), u32::MAX),
        CodecError::StringTooLong("restored-use state identity"),
    );

    divergent(
        "a renamed restored-use direct owner identity",
        &restring(first.owner_len.clone(), first.owner.clone(), &borrow(0x94)),
    );
    rejected(
        "a non-canonical restored-use direct owner identity",
        &restring(first.owner_len.clone(), first.owner.clone(), "owner"),
        invalid_first(),
    );

    // The source identities stay free on the shared-freeze row too: the
    // cohort joins only the child strand.
    divergent(
        "a renamed shared-freeze source machine identity",
        &restring(
            second.source_machine_len.clone(),
            second.source_machine.clone(),
            &borrow(0x95),
        ),
    );
    divergent(
        "a renamed shared-freeze source state identity",
        &restring(
            second.source_state_len.clone(),
            second.source_state.clone(),
            &borrow(0x96),
        ),
    );
    divergent(
        "a renamed shared-freeze direct owner identity",
        &restring(
            second.owner_len.clone(),
            second.owner.clone(),
            &borrow(0x97),
        ),
    );

    // --- direct root owner path --------------------------------------------

    // The direct root owner path is closed empty: a count one over reads the
    // place root's 80-byte length prefix as an unknown segment tag, and an
    // inserted segment decodes to a nonempty path the validator refuses.
    rejected(
        "a restored-use owner path count one over",
        &put_u32(first.owner_path_count.clone(), 1),
        CodecError::InvalidTag("TerminalBorrowOwnerSegment", 80),
    );
    rejected(
        "a maximal restored-use owner path count",
        &put_u32(first.owner_path_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );
    rejected(
        "an inserted restored-use owner path segment",
        &insert_owner_segment(first.owner_path_count.clone(), &[4]),
        invalid_first(),
    );

    // --- direct root place ---------------------------------------------------

    // The lifetime identity reiterates the root place's root and every child
    // strand reiterates it too: substituting either strand alone strands the
    // join.
    rejected(
        "a restored-use direct root place rebound elsewhere",
        &restring(
            first.place.root_len.clone(),
            first.place.root.clone(),
            &borrow(0x98),
        ),
        invalid_first(),
    );
    rejected(
        "an emptied restored-use direct root place",
        &restring(first.place.root_len.clone(), first.place.root.clone(), ""),
        invalid_first(),
    );
    rejected(
        "a non-UTF-8 restored-use direct root place",
        &put_u8(first.place.root.clone(), 0xFF),
        CodecError::InvalidUtf8("reborrow place root"),
    );
    rejected(
        "a restored-use lifetime identity split from the root",
        &restring(
            first.lifetime_len.clone(),
            first.lifetime.clone(),
            &borrow(0x99),
        ),
        invalid_first(),
    );
    rejected(
        "a non-canonical restored-use lifetime identity",
        &restring(
            first.lifetime_len.clone(),
            first.lifetime.clone(),
            "lifetime",
        ),
        invalid_first(),
    );

    // The child place must extend the root's segments by exactly the
    // projection remainder, so every root-segment substitution strands the
    // join: a recast field, a non-canonical field identity, an inverted
    // fixed range, a dropped trailing segment, or a maximal count.
    rejected(
        "a restored-use direct root field recast as a case",
        &put_u8(first.place.segments[0].tag.clone(), 2),
        invalid_first(),
    );
    rejected(
        "a non-canonical restored-use direct root field identity",
        &restring(
            first.place.segments[0].text_len.clone().unwrap(),
            first.place.segments[0].text.clone().unwrap(),
            "field",
        ),
        invalid_first(),
    );
    rejected(
        "an inverted restored-use direct root fixed range",
        &put_u64(first.place.segments[1].range_start.clone().unwrap(), 0x99),
        invalid_first(),
    );
    rejected(
        "a restored-use direct root fixed range end below its start",
        &put_u64(first.place.segments[1].range_end.clone().unwrap(), 0x01),
        invalid_first(),
    );
    rejected(
        "a maximal restored-use direct root segment count",
        &put_u32(first.place.segment_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );
    rejected(
        "a trimmed restored-use direct root segment roster",
        &excise(
            first.place.segment_count.clone(),
            first.place.segments[1].row.clone(),
            1,
        ),
        invalid_first(),
    );

    // --- direct root lifecycle boundaries -----------------------------------

    // The lifecycle is the exact nesting
    // activation < child activation <= child weakening < weakening: an
    // earlier activation or a later weakening stays representable while the
    // collapsing substitutions reject.
    divergent(
        "an earlier restored-use direct root activation",
        &put_u64(first.activation.statement_index.clone(), 5),
    );
    rejected(
        "a restored-use direct root activation at the child start",
        &put_u64(first.activation.statement_index.clone(), 11),
        invalid_first(),
    );
    rejected(
        "an unknown restored-use activation boundary tag",
        &put_u8(first.activation.tag.clone(), 9),
        CodecError::InvalidTag("TerminalBorrowBoundarySource", 9),
    );
    divergent(
        "a later restored-use direct root weakening",
        &put_u64(first.weakening.statement_index.clone(), 50),
    );
    rejected(
        "a restored-use direct root weakening inside the child span",
        &put_u64(first.weakening.statement_index.clone(), 11),
        invalid_first(),
    );
    rejected(
        "an unknown restored-use weakening boundary tag",
        &put_u8(first.weakening.tag.clone(), 0),
        CodecError::InvalidTag("TerminalBorrowBoundarySource", 0),
    );

    // --- child owner -----------------------------------------------------------

    divergent(
        "a renamed restored-use child owner identity",
        &restring(
            first.child_owner_len.clone(),
            first.child_owner.clone(),
            &borrow(0x9A),
        ),
    );
    rejected(
        "a non-canonical restored-use child owner identity",
        &restring(
            first.child_owner_len.clone(),
            first.child_owner.clone(),
            "child-owner",
        ),
        invalid_first(),
    );
    // On the shared-freeze row the child owner identity reiterates through
    // the cohort's primary member: substituting it alone strands the join.
    rejected(
        "a renamed shared-freeze child owner identity",
        &restring(
            second.child_owner_len.clone(),
            second.child_owner.clone(),
            &borrow(0x9B),
        ),
        invalid_second(),
    );
    divergent(
        "a restored-use child owner case recast as a field",
        &put_u8(first.child_owner_path[0].tag.clone(), 1),
    );
    rejected(
        "a non-canonical restored-use child owner case identity",
        &restring(
            first.child_owner_path[0].text_len.clone().unwrap(),
            first.child_owner_path[0].text.clone().unwrap(),
            "case",
        ),
        invalid_first(),
    );
    divergent(
        "a moved restored-use child owner fixed index",
        &put_u64(first.child_owner_path[1].index.clone().unwrap(), 0x57),
    );
    divergent(
        "an emptied restored-use child owner path",
        &excise(
            first.child_owner_path_count.clone(),
            first.child_owner_path[0].row.start..first.child_owner_path[1].row.end,
            0,
        ),
    );
    rejected(
        "a restored-use child owner path count one over",
        &put_u32(first.child_owner_path_count.clone(), 3),
        CodecError::InvalidTag("TerminalBorrowOwnerSegment", 80),
    );

    // --- child place and projection remainder -----------------------------------

    // The child place reiterates the root and extends the parent's segments
    // by exactly the projection remainder: every join-constrained strand
    // rejects alone.
    rejected(
        "a restored-use child place rebound to another root",
        &restring(
            first.child_place.root_len.clone(),
            first.child_place.root.clone(),
            &borrow(0x9C),
        ),
        invalid_first(),
    );
    rejected(
        "a restored-use child prefix segment recast as a case",
        &put_u8(first.child_place.segments[0].tag.clone(), 2),
        invalid_first(),
    );
    rejected(
        "a restored-use child remainder copy retargeted",
        &put_u64(first.child_place.segments[2].index.clone().unwrap(), 0x58),
        invalid_first(),
    );
    rejected(
        "a maximal restored-use child segment count",
        &put_u32(first.child_place.segment_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );
    rejected(
        "a trimmed restored-use child segment roster",
        &excise(
            first.child_place.segment_count.clone(),
            first.child_place.segments[3].row.clone(),
            3,
        ),
        invalid_first(),
    );
    rejected(
        "a restored-use projection remainder segment retargeted",
        &put_u64(first.remainder[0].index.clone().unwrap(), 0x59),
        invalid_first(),
    );
    rejected(
        "a restored-use projection remainder case recast as a field",
        &put_u8(first.remainder[1].tag.clone(), 1),
        invalid_first(),
    );
    rejected(
        "a trimmed restored-use projection remainder",
        &excise(
            first.remainder_count.clone(),
            first.remainder[1].row.clone(),
            1,
        ),
        invalid_first(),
    );
    rejected(
        "a maximal restored-use projection remainder count",
        &put_u32(first.remainder_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );

    // --- child access and boundaries ----------------------------------------------

    // The exclusive child stays mutable or write-only: write-only to
    // mutable stays permitted while shared or owned rejects. The
    // shared-freeze child is pinned to shared by its class.
    divergent(
        "a mutable restored-use child access",
        &put_u8(first.child_access.clone(), 3),
    );
    rejected(
        "a shared-borrow restored-use child access",
        &put_u8(first.child_access.clone(), 2),
        invalid_first(),
    );
    rejected(
        "an owned restored-use child access",
        &put_u8(first.child_access.clone(), 1),
        invalid_first(),
    );
    for tag in [0, 5, u8::MAX] {
        rejected(
            "an unknown restored-use child access tag",
            &put_u8(first.child_access.clone(), tag),
            CodecError::InvalidTag("StructuralAccess", tag),
        );
    }
    rejected(
        "a mutable shared-freeze child access",
        &put_u8(second.child_access.clone(), 3),
        invalid_second(),
    );

    // Formation reiterates activation and the call boundary reiterates the
    // child weakening: moving any strand alone strands a join, and only the
    // still-ordered substitutions stay representable.
    rejected(
        "a moved restored-use child activation",
        &put_u64(first.child_activation.statement_index.clone(), 13),
        invalid_first(),
    );
    rejected(
        "an unknown restored-use child activation tag",
        &put_u8(first.child_activation.tag.clone(), 7),
        CodecError::InvalidTag("TerminalBorrowBoundarySource", 7),
    );
    rejected(
        "a moved restored-use formation boundary",
        &put_u64(first.formation.statement_index.clone(), 14),
        invalid_first(),
    );
    rejected(
        "an unknown restored-use formation boundary tag",
        &put_u8(first.formation.tag.clone(), 0),
        CodecError::InvalidTag("TerminalBorrowBoundarySource", 0),
    );
    rejected(
        "a moved restored-use child weakening",
        &put_u64(first.child_weakening.statement_index.clone(), 15),
        invalid_first(),
    );
    rejected(
        "an unknown restored-use child weakening tag",
        &put_u8(first.child_weakening.tag.clone(), 9),
        CodecError::InvalidTag("TerminalBorrowBoundarySource", 9),
    );

    // --- shared cohort ---------------------------------------------------------

    // The shared-freeze cohort is a closed nonempty roster: emptying it or
    // duplicating the member rejects, and a count one over decodes a phantom
    // member out of the following empty sections' zero counts — empty
    // strings, an empty path and place, then a zero access tag.
    rejected(
        "an emptied shared-freeze cohort",
        &excise(second.cohort_count.clone(), member.row.clone(), 0),
        invalid_second(),
    );
    rejected(
        "a shared-freeze cohort count one over",
        &put_u32(second.cohort_count.clone(), 2),
        CodecError::InvalidTag("StructuralAccess", 0),
    );
    rejected(
        "a maximal shared-freeze cohort count",
        &put_u32(second.cohort_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );
    rejected(
        "a duplicated shared-freeze cohort member",
        &duplicate_row(second.cohort_count.clone(), member.row.clone()),
        invalid_second(),
    );

    // The primary member reiterates the row's child strand exactly: every
    // member field is join-constrained.
    rejected(
        "a renamed cohort member child owner identity",
        &restring(
            member.child_owner_len.clone(),
            member.child_owner.clone(),
            &borrow(0x9D),
        ),
        invalid_second(),
    );
    rejected(
        "a non-canonical cohort member child owner identity",
        &restring(
            member.child_owner_len.clone(),
            member.child_owner.clone(),
            "member-owner",
        ),
        invalid_second(),
    );
    rejected(
        "a cohort member owner case recast as a field",
        &put_u8(member.child_owner_path[0].tag.clone(), 1),
        invalid_second(),
    );
    rejected(
        "a cohort member owner path count one over",
        &put_u32(member.child_owner_path_count.clone(), 3),
        CodecError::InvalidTag("TerminalBorrowOwnerSegment", 80),
    );
    rejected(
        "a cohort member place rebound to another root",
        &restring(
            member.child_place.root_len.clone(),
            member.child_place.root.clone(),
            &borrow(0x9E),
        ),
        invalid_second(),
    );
    rejected(
        "a cohort member prefix segment recast as a case",
        &put_u8(member.child_place.segments[0].tag.clone(), 2),
        invalid_second(),
    );
    rejected(
        "a cohort member remainder copy retargeted",
        &put_u64(member.child_place.segments[2].index.clone().unwrap(), 0x5A),
        invalid_second(),
    );
    rejected(
        "a mutable cohort member access",
        &put_u8(member.child_access.clone(), 3),
        invalid_second(),
    );
    rejected(
        "an unknown cohort member access tag",
        &put_u8(member.child_access.clone(), 9),
        CodecError::InvalidTag("StructuralAccess", 9),
    );
    rejected(
        "a moved cohort member activation",
        &put_u64(member.child_activation.statement_index.clone(), 35),
        invalid_second(),
    );
    rejected(
        "an unknown cohort member activation tag",
        &put_u8(member.child_activation.tag.clone(), 7),
        CodecError::InvalidTag("TerminalBorrowBoundarySource", 7),
    );
    rejected(
        "a moved cohort member weakening",
        &put_u64(member.child_weakening.statement_index.clone(), 36),
        invalid_second(),
    );
    rejected(
        "an unknown cohort member weakening tag",
        &put_u8(member.child_weakening.tag.clone(), 0),
        CodecError::InvalidTag("TerminalBorrowBoundarySource", 0),
    );

    // --- producer-side rejections -------------------------------------------

    // Every validation failure above also fails closed on the way out: the
    // canonical encoder runs the same module validation before emitting
    // bytes.
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses.swap(0, 1);
    encode_rejected(
        "a producer-side reordered restored-use roster",
        &changed,
        CodecError::InvalidModule(ModuleError::NonCanonicalReborrowRestoredCallUseOrder),
    );
    let mut changed = module.clone();
    changed
        .reborrow_restored_call_uses
        .push(changed.reborrow_restored_call_uses[0].clone());
    encode_rejected(
        "a producer-side duplicated restored-use row",
        &changed,
        CodecError::InvalidModule(ModuleError::DuplicateReborrowRestoredCallUse),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[1].machine = machine_id(1);
    encode_rejected(
        "a producer-side colliding restored-use lifecycle machine",
        &changed,
        CodecError::InvalidModule(ModuleError::InvalidReborrowRestoredCallUse {
            machine: machine_id(1),
            operation: operation_id(3),
        }),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[0].restoration_class =
        TerminalReborrowRestorationClass::SharedFreezeRestoration;
    encode_rejected(
        "a producer-side shared-freeze class without a cohort",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[0].call_boundary = TerminalBorrowBoundarySource::Call {
        statement_index: 13,
        call_ordinal: 0,
        target_identity: borrow(0x9F),
    };
    encode_rejected(
        "a producer-side call boundary split from the child weakening",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[0].call_boundary = TerminalBorrowBoundarySource::Call {
        statement_index: 12,
        call_ordinal: 1,
        target_identity: borrow(0xA0),
    };
    encode_rejected(
        "a producer-side nonzero call boundary ordinal",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[0].call_target_machine = machine_id(3);
    encode_rejected(
        "a producer-side restored-use call target rebound",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[0].source_machine_identity = "producer-machine".into();
    encode_rejected(
        "a producer-side non-canonical restored-use source identity",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[0]
        .direct_root_owner_path
        .push(TerminalBorrowOwnerSegment::DynamicIndex);
    encode_rejected(
        "a producer-side nonempty restored-use owner path",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[0].direct_root_lifetime_identity = borrow(0xA1);
    encode_rejected(
        "a producer-side restored-use lifetime split from the root",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[0]
        .child_place
        .root_identity = borrow(0xA2);
    encode_rejected(
        "a producer-side restored-use child place rebound to another root",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[0].formation_boundary = statement(19);
    encode_rejected(
        "a producer-side restored-use formation split from activation",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[0].child_access = StructuralAccess::SharedBorrow;
    encode_rejected(
        "a producer-side shared exclusive child access",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[0]
        .projection_remainder
        .pop();
    encode_rejected(
        "a producer-side trimmed restored-use projection remainder",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[1].shared_cohort.clear();
    encode_rejected(
        "a producer-side emptied shared-freeze cohort",
        &changed,
        invalid_second(),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[1].shared_cohort[0].child_access =
        StructuralAccess::MutableBorrow;
    encode_rejected(
        "a producer-side mutable cohort member access",
        &changed,
        invalid_second(),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[1].shared_cohort[0].child_weakening = statement(33);
    encode_rejected(
        "a producer-side cohort member weakening split from the row",
        &changed,
        invalid_second(),
    );
    let mut changed = module.clone();
    changed.reborrow_restored_call_uses[1].shared_cohort[0].child_owner_identity = borrow(0xA3);
    encode_rejected(
        "a producer-side cohort member owner drifted from the primary strand",
        &changed,
        invalid_second(),
    );

    // --- module envelope boundaries ---------------------------------------

    // Truncation inside the roster and a trailing byte reject at the
    // envelope, before semantic replay ever runs.
    for cut in [
        first.row.end - 1,
        member.row.end - 1,
        second.row.end - 1,
        encoded.len() - 1,
    ] {
        assert!(
            decode_module(&encoded[..cut]).is_err(),
            "truncation at byte {cut} must reject"
        );
    }
    let mut trailing = encoded.clone();
    trailing.push(0);
    rejected(
        "a trailing byte after the module",
        &trailing,
        CodecError::TrailingBytes(1),
    );
}
