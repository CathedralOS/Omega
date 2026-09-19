//! One-field mutation coverage for the semantic module's reborrow root
//! handoff roster.
//!
//! `reborrow_root_handoffs` is the module-level custody record of each
//! closed direct-root handoff: the owning machine, the source machine and
//! state identities, the direct root's owner identity, owner path, place,
//! exclusive access, activation and weakening boundaries, the lifetime
//! identity that must reiterate the root place's root, and a nonempty
//! lineage of exclusive child steps. Each step carries its own owner
//! identity, owner path, child place that must extend its parent's place by
//! exactly the declared projection remainder, an exclusive child access, and
//! the activation, formation, and weakening boundaries with formation
//! equal to activation. Its wire fields — the roster count, each row's
//! machine, the length-framed identity strings, the counted owner paths and
//! place segments, both boundary envelopes, the lineage count, and every
//! step field — are each substituted independently. A substitution either
//! fails canonical decoding or the verifier's module-bound validation (a
//! non-canonical borrow identity, a non-exclusive root access, a lifetime
//! that strays from the root place, a child place that no longer extends
//! its parent, a formation boundary split from activation, a duplicate
//! coordinate, or a reordered roster), or it decodes to a different module
//! whose honestly recomputed semantic and artifact identities diverge and
//! whose replay against the retained manifest and sealed proof subject
//! rejects.
//!
//! The fixture keeps a second scalar-return machine beside the shared one so
//! the `machine` field has a representable substitution in both directions,
//! and two rows so the roster itself is reorderable. The second row's child
//! weakening is a `Call` boundary so both boundary variants appear on the
//! wire.

use std::ops::Range;

use super::{
    block_id, canonical_artifact, contract_id, edge_id, kernel_bundle, machine_id, obligation_id,
    operation_id, semantic_module, value_id,
};
use proof_admission::{AdmissionProfile, EvidenceRoute, PrimitiveJudgment};
use terminal_codec::{
    ArtifactManifestError, CodecError, ProofCodecError, build_artifact_manifest,
    build_identity_optimization_execution_record, decode_module, decode_proof_section_for,
    encode_module, terminal_psi_identity, validate_artifact_manifest,
};
use terminal_psi::{
    OperationResult, StructuralAccess, TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment,
    TerminalBorrowPlace, TerminalBorrowPlaceSegment, TerminalMachineResult,
    TerminalReborrowRootHandoff, TerminalReborrowRootHandoffStep, Terminator,
};
use terminal_verifier::{ModuleError, ObligationEvidence, verify_module};

/// Byte offsets of every wire field inside the reborrow root-handoff
/// roster. Every `string` field records its u32 length prefix and its
/// content bytes separately so a substitution can lie about either side,
/// and every counted roster records its own count span.
struct ModuleSpans {
    handoff_count: Range<usize>,
    handoffs: Vec<HandoffSpan>,
}

/// A `Statement` boundary is tag + statement index; a `Call` boundary adds
/// a call ordinal and a length-framed target identity. The fixture carries
/// both, so the optional spans are present on the second row's weakening.
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

struct StepSpan {
    row: Range<usize>,
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
}

struct HandoffSpan {
    row: Range<usize>,
    machine: Range<usize>,
    source_machine_len: Range<usize>,
    source_machine: Range<usize>,
    source_state_len: Range<usize>,
    source_state: Range<usize>,
    owner_len: Range<usize>,
    owner: Range<usize>,
    owner_path_count: Range<usize>,
    owner_path: Vec<OwnerSegmentSpan>,
    place: PlaceSpan,
    access: Range<usize>,
    activation: BoundarySpan,
    weakening: BoundarySpan,
    lifetime_len: Range<usize>,
    lifetime: Range<usize>,
    lineage_count: Range<usize>,
    steps: Vec<StepSpan>,
}

/// A cursor that walks the canonical module encoding exactly as the decoder
/// does, recording the byte span of every reborrow handoff field the matrix
/// substitutes. Every section ahead of the roster is empty in the fixture,
/// so the walk asserts each leading count is zero rather than silently
/// spanning unknown row bytes.
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

    fn walk_step(&mut self) -> StepSpan {
        let row_start = self.offset;
        let (child_owner_len, child_owner) = self.take_string();
        let (child_owner_path_count, child_owner_path) = self.walk_owner_path();
        let child_place = self.walk_place();
        let (remainder_count, remainder) = self.walk_place_segments();
        let child_access = self.take(1);
        let child_activation = self.walk_boundary();
        let formation = self.walk_boundary();
        let child_weakening = self.walk_boundary();
        StepSpan {
            row: row_start..self.offset,
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
        }
    }

    fn walk_handoff(&mut self) -> HandoffSpan {
        let row_start = self.offset;
        let machine = self.take(8);
        let (source_machine_len, source_machine) = self.take_string();
        let (source_state_len, source_state) = self.take_string();
        let (owner_len, owner) = self.take_string();
        let (owner_path_count, owner_path) = self.walk_owner_path();
        let place = self.walk_place();
        let access = self.take(1);
        let activation = self.walk_boundary();
        let weakening = self.walk_boundary();
        let (lifetime_len, lifetime) = self.take_string();
        let (lineage_count, steps) = self.take_count();
        let mut step_spans = Vec::with_capacity(usize::try_from(steps).expect("steps fit"));
        for _ in 0..steps {
            step_spans.push(self.walk_step());
        }
        HandoffSpan {
            row: row_start..self.offset,
            machine,
            source_machine_len,
            source_machine,
            source_state_len,
            source_state,
            owner_len,
            owner,
            owner_path_count,
            owner_path,
            place,
            access,
            activation,
            weakening,
            lifetime_len,
            lifetime,
            lineage_count,
            steps: step_spans,
        }
    }
}

/// Locate the reborrow root-handoff roster inside canonical module bytes by
/// mirroring `encode_raw`'s ordered section list. The fixture module keeps
/// every earlier roster empty, so each leading section is a zero count.
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
    for label in [
        "structural types",
        "structural domains",
        "services",
        "concrete root service reach",
        "installation reach dependencies",
        "placed-view inputs",
    ] {
        walker.expect_empty_count(label);
    }
    let (handoff_count, handoffs) = walker.take_count();
    let mut spans = Vec::with_capacity(usize::try_from(handoffs).expect("handoffs fit"));
    for _ in 0..handoffs {
        spans.push(walker.walk_handoff());
    }
    ModuleSpans {
        handoff_count,
        handoffs: spans,
    }
}

/// A canonical borrow identity: `terminal-borrow:` followed by a 64-digit
/// lowercase hex digest, the form `validate_reborrow_root_handoffs`
/// requires of every machine, state, owner, place-root, and lifetime
/// identity.
fn borrow(digest: u64) -> String {
    format!("terminal-borrow:{digest:064x}")
}

/// One handoff row whose every identity derives from `seed`, so each row in
/// the fixture carries distinct canonical strings. The direct root takes a
/// mutable borrow, its owner path exercises all four owner-segment tags, its
/// place carries a field and a fixed range, and the single lineage step
/// extends that place by a two-segment projection remainder under a
/// write-only child. The step's weakening is a `Call` boundary when
/// `call_weakening` is set so both boundary variants reach the wire.
fn handoff_row(machine: u64, seed: u64, call_weakening: bool) -> TerminalReborrowRootHandoff {
    let root = borrow(seed * 0x100 + 1);
    let parent_segments = vec![
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
    let mut child_segments = parent_segments.clone();
    child_segments.extend(remainder.iter().cloned());
    TerminalReborrowRootHandoff {
        machine: machine_id(machine),
        source_machine_identity: borrow(seed * 0x100 + 2),
        source_state_identity: borrow(seed * 0x100 + 3),
        direct_root_owner_identity: borrow(seed * 0x100 + 4),
        direct_root_owner_path: vec![
            TerminalBorrowOwnerSegment::Field(borrow(seed * 0x100 + 5)),
            TerminalBorrowOwnerSegment::Case(borrow(seed * 0x100 + 6)),
            TerminalBorrowOwnerSegment::FixedIndex(seed + 7),
            TerminalBorrowOwnerSegment::DynamicIndex,
        ],
        direct_root_place: TerminalBorrowPlace {
            root_identity: root.clone(),
            segments: parent_segments,
        },
        direct_root_access: StructuralAccess::MutableBorrow,
        direct_root_activation: TerminalBorrowBoundarySource::Statement {
            statement_index: seed,
        },
        direct_root_weakening: TerminalBorrowBoundarySource::Statement {
            statement_index: seed + 90,
        },
        direct_root_lifetime_identity: root.clone(),
        lineage: vec![TerminalReborrowRootHandoffStep {
            child_owner_identity: borrow(seed * 0x100 + 9),
            child_owner_path: vec![
                TerminalBorrowOwnerSegment::Case(borrow(seed * 0x100 + 10)),
                TerminalBorrowOwnerSegment::FixedIndex(seed + 11),
            ],
            child_place: TerminalBorrowPlace {
                root_identity: root,
                segments: child_segments,
            },
            projection_remainder: remainder,
            child_access: StructuralAccess::WriteOnlyBorrow,
            child_activation: TerminalBorrowBoundarySource::Statement {
                statement_index: seed + 20,
            },
            formation_boundary: TerminalBorrowBoundarySource::Statement {
                statement_index: seed + 20,
            },
            child_weakening: if call_weakening {
                TerminalBorrowBoundarySource::Call {
                    statement_index: seed + 31,
                    call_ordinal: 2,
                    target_identity: borrow(seed * 0x100 + 14),
                }
            } else {
                TerminalBorrowBoundarySource::Statement {
                    statement_index: seed + 30,
                }
            },
        }],
    }
}

/// A second isolated scalar-return machine, cloned from the shared fixture's
/// machine with fresh identities so a `machine` substitution has a real
/// target in both directions.
fn second_machine() -> terminal_psi::TerminalMachine {
    let mut machine = semantic_module().machines[0].clone();
    machine.id = machine_id(2);
    machine.contract.id = contract_id(2);
    machine.contract.ensures[0].obligation = obligation_id(2);
    machine.entry = block_id(2);
    machine.blocks[0].id = block_id(2);
    machine.blocks[0].operations[0].id = operation_id(2);
    if let OperationResult::Scalar(result) = &mut machine.blocks[0].operations[0].result {
        result.id = value_id(3);
    }
    machine.blocks[0].terminator = Terminator::Return {
        cleanup_actions: Vec::new(),
        edge: edge_id(2),
        value: value_id(3),
    };
    if let TerminalMachineResult::Scalar(result) = &mut machine.result {
        result.id = value_id(4);
    }
    machine
}

fn reborrow_module() -> terminal_psi::TerminalModule {
    let mut module = semantic_module();
    module.machines.push(second_machine());
    module.reborrow_root_handoffs = vec![handoff_row(1, 0x10, false), handoff_row(2, 0x20, true)];
    module
}

/// The kernel bundle extended with evidence for the second machine's
/// contract obligation, so the two-machine fixture verifies end to end.
fn reborrow_bundle() -> terminal_verifier::ProofBundle {
    let mut bundle = kernel_bundle();
    bundle.evidence.push(ObligationEvidence {
        obligation: obligation_id(2),
        route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
    });
    bundle
}

#[test]
fn terminal_reborrow_root_handoffs_reject_every_one_field_substitution() {
    let module = reborrow_module();
    let bundle = reborrow_bundle();
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
        spans.handoffs.len(),
        2,
        "fixture roster: a machine-1 row and a machine-2 row"
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

    let first = &spans.handoffs[0];
    let second = &spans.handoffs[1];
    let step = &first.steps[0];
    let second_step = &second.steps[0];
    let invalid_first = || {
        CodecError::InvalidModule(ModuleError::InvalidReborrowRootHandoff {
            machine: machine_id(1),
        })
    };
    let invalid_second = || {
        CodecError::InvalidModule(ModuleError::InvalidReborrowRootHandoff {
            machine: machine_id(2),
        })
    };

    // --- roster axes -----------------------------------------------------

    // A roster count lying about its rows reads the following empty
    // sections as a zero machine identity or starves the row.
    rejected(
        "a reborrow handoff roster count one over",
        &put_u32(spans.handoff_count.clone(), 3),
        CodecError::ZeroIdentity("MachineId"),
    );
    rejected(
        "a maximal reborrow handoff roster count",
        &put_u32(spans.handoff_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );
    // Clearing the roster or dropping either row stays representable: the
    // recomputed identity diverges and the retained custody replays reject.
    divergent(
        "a cleared reborrow handoff roster",
        &excise(
            spans.handoff_count.clone(),
            first.row.start..second.row.end,
            0,
        ),
    );
    divergent(
        "a dropped first reborrow handoff row",
        &drop_row(spans.handoff_count.clone(), first.row.clone()),
    );
    divergent(
        "a dropped second reborrow handoff row",
        &drop_row(spans.handoff_count.clone(), second.row.clone()),
    );
    // A duplicated row collides on the (machine, source state, leaf child
    // owner, leaf activation) coordinate; a swapped roster violates the
    // strict row order.
    rejected(
        "a duplicated reborrow handoff row",
        &duplicate_row(spans.handoff_count.clone(), first.row.clone()),
        CodecError::InvalidModule(ModuleError::DuplicateReborrowRootHandoff),
    );
    let mut swapped = encoded[..first.row.start].to_vec();
    swapped.extend_from_slice(&encoded[second.row.clone()]);
    swapped.extend_from_slice(&encoded[first.row.clone()]);
    swapped.extend_from_slice(&encoded[second.row.end..]);
    rejected(
        "a reordered reborrow handoff roster",
        &swapped,
        CodecError::InvalidModule(ModuleError::NonCanonicalReborrowRootHandoffOrder),
    );

    // --- machine ----------------------------------------------------------

    // The machine must name a module machine. Either direction between the
    // fixture's two machines stays ordered and diverges the identity.
    rejected(
        "a zero reborrow handoff machine",
        &put_u64(first.machine.clone(), 0),
        CodecError::ZeroIdentity("MachineId"),
    );
    rejected(
        "a reborrow handoff machine outside the module",
        &put_u64(first.machine.clone(), 9),
        CodecError::InvalidModule(ModuleError::InvalidReborrowRootHandoff {
            machine: machine_id(9),
        }),
    );
    divergent(
        "a reborrow handoff row rebound to the second machine",
        &put_u64(first.machine.clone(), 2),
    );
    divergent(
        "a reborrow handoff row rebound to the first machine",
        &put_u64(second.machine.clone(), 1),
    );

    // --- source identities -------------------------------------------------

    // Each source identity must stay a canonical borrow identity; any other
    // canonical spelling reidentifies the source and diverges.
    divergent(
        "a renamed reborrow source machine identity",
        &restring(
            first.source_machine_len.clone(),
            first.source_machine.clone(),
            &borrow(0x91),
        ),
    );
    rejected(
        "an emptied reborrow source machine identity",
        &restring(
            first.source_machine_len.clone(),
            first.source_machine.clone(),
            "",
        ),
        invalid_first(),
    );
    rejected(
        "a non-canonical reborrow source machine identity",
        &restring(
            first.source_machine_len.clone(),
            first.source_machine.clone(),
            "host-machine",
        ),
        invalid_first(),
    );
    rejected(
        "a non-UTF-8 reborrow source machine identity",
        &put_u8(first.source_machine.clone(), 0xFF),
        CodecError::InvalidUtf8("reborrow machine identity"),
    );

    divergent(
        "a renamed reborrow source state identity",
        &restring(
            first.source_state_len.clone(),
            first.source_state.clone(),
            &borrow(0x92),
        ),
    );
    rejected(
        "an emptied reborrow source state identity",
        &restring(
            first.source_state_len.clone(),
            first.source_state.clone(),
            "",
        ),
        invalid_first(),
    );
    rejected(
        "a reborrow source state identity length lie",
        &put_u32(first.source_state_len.clone(), u32::MAX),
        CodecError::StringTooLong("reborrow state identity"),
    );

    divergent(
        "a renamed reborrow direct owner identity",
        &restring(first.owner_len.clone(), first.owner.clone(), &borrow(0x93)),
    );
    rejected(
        "a non-canonical reborrow direct owner identity",
        &restring(first.owner_len.clone(), first.owner.clone(), "owner"),
        invalid_first(),
    );

    // --- direct root owner path --------------------------------------------

    // The owner path is a free counted roster: a count one over reads the
    // place root's 80-byte length prefix as an unknown segment tag, and an
    // emptied path still validates while diverging the identity.
    rejected(
        "a reborrow owner path count one over",
        &put_u32(first.owner_path_count.clone(), 5),
        CodecError::InvalidTag("TerminalBorrowOwnerSegment", 80),
    );
    rejected(
        "a maximal reborrow owner path count",
        &put_u32(first.owner_path_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );
    divergent(
        "an emptied reborrow direct owner path",
        &excise(
            first.owner_path_count.clone(),
            first.owner_path[0].row.start..first.owner_path[3].row.end,
            0,
        ),
    );
    // Field and Case carry the same string payload, so a tag swap still
    // decodes and diverges; a non-canonical field identity rejects.
    divergent(
        "a reborrow owner field recast as a case",
        &put_u8(first.owner_path[0].tag.clone(), 2),
    );
    divergent(
        "a reborrow owner case recast as a field",
        &put_u8(first.owner_path[1].tag.clone(), 1),
    );
    rejected(
        "a non-canonical reborrow owner field identity",
        &restring(
            first.owner_path[0].text_len.clone().unwrap(),
            first.owner_path[0].text.clone().unwrap(),
            "field",
        ),
        invalid_first(),
    );
    divergent(
        "a moved reborrow owner fixed index",
        &put_u64(first.owner_path[2].index.clone().unwrap(), 0x55),
    );
    rejected(
        "an unknown reborrow owner segment tag",
        &put_u8(first.owner_path[3].tag.clone(), 0),
        CodecError::InvalidTag("TerminalBorrowOwnerSegment", 0),
    );

    // --- direct root place ---------------------------------------------------

    // The lifetime identity reiterates the root place's root: substituting
    // either strand alone strands the join.
    rejected(
        "a reborrow direct root place rebound elsewhere",
        &restring(
            first.place.root_len.clone(),
            first.place.root.clone(),
            &borrow(0x94),
        ),
        invalid_first(),
    );
    rejected(
        "an emptied reborrow direct root place",
        &restring(first.place.root_len.clone(), first.place.root.clone(), ""),
        invalid_first(),
    );
    rejected(
        "a non-UTF-8 reborrow direct root place",
        &put_u8(first.place.root.clone(), 0xFF),
        CodecError::InvalidUtf8("reborrow place root"),
    );
    rejected(
        "a reborrow lifetime identity split from the root",
        &restring(
            first.lifetime_len.clone(),
            first.lifetime.clone(),
            &borrow(0x95),
        ),
        invalid_first(),
    );
    rejected(
        "a non-canonical reborrow lifetime identity",
        &restring(
            first.lifetime_len.clone(),
            first.lifetime.clone(),
            "lifetime",
        ),
        invalid_first(),
    );

    // The child place must extend the parent's segments by exactly the
    // projection remainder, so every parent-segment substitution strands
    // the join: a recast field, an inverted fixed range, a dropped trailing
    // segment, or a maximal count.
    rejected(
        "a reborrow direct root field recast as a case",
        &put_u8(first.place.segments[0].tag.clone(), 2),
        invalid_first(),
    );
    rejected(
        "a non-canonical reborrow direct root field identity",
        &restring(
            first.place.segments[0].text_len.clone().unwrap(),
            first.place.segments[0].text.clone().unwrap(),
            "field",
        ),
        invalid_first(),
    );
    rejected(
        "an inverted reborrow direct root fixed range",
        &put_u64(first.place.segments[1].range_start.clone().unwrap(), 0x99),
        invalid_first(),
    );
    rejected(
        "a reborrow direct root fixed range end below its start",
        &put_u64(first.place.segments[1].range_end.clone().unwrap(), 0x05),
        invalid_first(),
    );
    rejected(
        "a maximal reborrow direct root segment count",
        &put_u32(first.place.segment_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );
    rejected(
        "a trimmed reborrow direct root segment roster",
        &excise(
            first.place.segment_count.clone(),
            first.place.segments[1].row.clone(),
            1,
        ),
        invalid_first(),
    );

    // --- direct root access and boundaries -----------------------------------

    // The direct root must be an exclusive borrow: mutable to write-only
    // stays permitted while a shared or owned access rejects.
    divergent(
        "a write-only reborrow direct root access",
        &put_u8(first.access.clone(), 4),
    );
    rejected(
        "a shared-borrow reborrow direct root access",
        &put_u8(first.access.clone(), 2),
        invalid_first(),
    );
    rejected(
        "an owned reborrow direct root access",
        &put_u8(first.access.clone(), 1),
        invalid_first(),
    );
    for tag in [0, 5, u8::MAX] {
        rejected(
            "an unknown reborrow direct root access tag",
            &put_u8(first.access.clone(), tag),
            CodecError::InvalidTag("StructuralAccess", tag),
        );
    }

    // Statement boundaries are free coordinates: a moved index diverges,
    // while an unknown tag rejects at the envelope.
    divergent(
        "a moved reborrow direct root activation",
        &put_u64(first.activation.statement_index.clone(), 0x61),
    );
    rejected(
        "an unknown reborrow activation boundary tag",
        &put_u8(first.activation.tag.clone(), 9),
        CodecError::InvalidTag("TerminalBorrowBoundarySource", 9),
    );
    divergent(
        "a moved reborrow direct root weakening",
        &put_u64(first.weakening.statement_index.clone(), 0x62),
    );
    rejected(
        "an unknown reborrow weakening boundary tag",
        &put_u8(first.weakening.tag.clone(), 0),
        CodecError::InvalidTag("TerminalBorrowBoundarySource", 0),
    );

    // --- lineage ------------------------------------------------------------

    // The lineage is a nonempty roster: clearing it rejects at semantic
    // validation, while count lies starve or overrun the step. A count one
    // over decodes a phantom step from the second row's leading bytes: a
    // two-byte child owner out of the machine identity, then an owner-path
    // count of 0x00500000 that exceeds the remaining bytes.
    rejected(
        "an emptied reborrow handoff lineage",
        &excise(first.lineage_count.clone(), step.row.clone(), 0),
        invalid_first(),
    );
    rejected(
        "a reborrow handoff lineage count one over",
        &put_u32(first.lineage_count.clone(), 2),
        CodecError::UnexpectedEnd,
    );
    rejected(
        "a maximal reborrow handoff lineage count",
        &put_u32(first.lineage_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );

    // --- step owner -----------------------------------------------------------

    divergent(
        "a renamed reborrow child owner identity",
        &restring(
            step.child_owner_len.clone(),
            step.child_owner.clone(),
            &borrow(0x96),
        ),
    );
    rejected(
        "a non-canonical reborrow child owner identity",
        &restring(
            step.child_owner_len.clone(),
            step.child_owner.clone(),
            "child-owner",
        ),
        invalid_first(),
    );
    divergent(
        "a reborrow child owner case recast as a field",
        &put_u8(step.child_owner_path[0].tag.clone(), 1),
    );
    rejected(
        "a non-canonical reborrow child owner case identity",
        &restring(
            step.child_owner_path[0].text_len.clone().unwrap(),
            step.child_owner_path[0].text.clone().unwrap(),
            "case",
        ),
        invalid_first(),
    );
    divergent(
        "a moved reborrow child owner fixed index",
        &put_u64(step.child_owner_path[1].index.clone().unwrap(), 0x57),
    );
    divergent(
        "an emptied reborrow child owner path",
        &excise(
            step.child_owner_path_count.clone(),
            step.child_owner_path[0].row.start..step.child_owner_path[1].row.end,
            0,
        ),
    );

    // --- step place and projection remainder -----------------------------------

    // The child place reiterates the root and extends the parent's segments
    // by exactly the projection remainder: every join-constrained strand
    // rejects alone.
    rejected(
        "a reborrow child place rebound to another root",
        &restring(
            step.child_place.root_len.clone(),
            step.child_place.root.clone(),
            &borrow(0x97),
        ),
        invalid_first(),
    );
    rejected(
        "a reborrow child prefix segment recast as a case",
        &put_u8(step.child_place.segments[0].tag.clone(), 2),
        invalid_first(),
    );
    rejected(
        "a reborrow child remainder copy retargeted",
        &put_u64(step.child_place.segments[2].index.clone().unwrap(), 0x58),
        invalid_first(),
    );
    // A child-segment count one over reads the remainder count's tag byte
    // as a case and the following bytes as an impossible string length.
    rejected(
        "a reborrow child segment roster count one over",
        &put_u32(step.child_place.segment_count.clone(), 5),
        CodecError::StringTooLong("reborrow place case"),
    );
    rejected(
        "a reborrow projection remainder segment retargeted",
        &put_u64(step.remainder[0].index.clone().unwrap(), 0x59),
        invalid_first(),
    );
    rejected(
        "a reborrow projection remainder case recast as a field",
        &put_u8(step.remainder[1].tag.clone(), 1),
        invalid_first(),
    );
    rejected(
        "a trimmed reborrow projection remainder",
        &excise(
            step.remainder_count.clone(),
            step.remainder[1].row.clone(),
            1,
        ),
        invalid_first(),
    );

    // --- step access and boundaries ----------------------------------------------

    // The child stays exclusive under the mutable root: write-only to
    // mutable stays permitted while shared or owned rejects.
    divergent(
        "a mutable reborrow child access",
        &put_u8(step.child_access.clone(), 3),
    );
    rejected(
        "a shared-borrow reborrow child access",
        &put_u8(step.child_access.clone(), 2),
        invalid_first(),
    );
    rejected(
        "an owned reborrow child access",
        &put_u8(step.child_access.clone(), 1),
        invalid_first(),
    );

    // Formation reiterates activation: moving either strand alone strands
    // the join, while the weakening is a free coordinate.
    rejected(
        "a moved reborrow child activation",
        &put_u64(step.child_activation.statement_index.clone(), 0x63),
        invalid_first(),
    );
    rejected(
        "an unknown reborrow child activation tag",
        &put_u8(step.child_activation.tag.clone(), 7),
        CodecError::InvalidTag("TerminalBorrowBoundarySource", 7),
    );
    rejected(
        "a moved reborrow formation boundary",
        &put_u64(step.formation.statement_index.clone(), 0x64),
        invalid_first(),
    );
    divergent(
        "a moved reborrow child weakening",
        &put_u64(step.child_weakening.statement_index.clone(), 0x65),
    );

    // --- second-row call boundary -------------------------------------------------

    // The second row's child weakening is a `Call` boundary: its statement,
    // ordinal, and canonical target identity are each free coordinates,
    // while a non-canonical target rejects.
    divergent(
        "a moved reborrow call boundary statement",
        &put_u64(second_step.child_weakening.statement_index.clone(), 0x66),
    );
    divergent(
        "a moved reborrow call boundary ordinal",
        &put_u64(
            second_step.child_weakening.call_ordinal.clone().unwrap(),
            0x67,
        ),
    );
    divergent(
        "a renamed reborrow call boundary target",
        &restring(
            second_step.child_weakening.target_len.clone().unwrap(),
            second_step.child_weakening.target.clone().unwrap(),
            &borrow(0x98),
        ),
    );
    rejected(
        "a non-canonical reborrow call boundary target",
        &restring(
            second_step.child_weakening.target_len.clone().unwrap(),
            second_step.child_weakening.target.clone().unwrap(),
            "target",
        ),
        invalid_second(),
    );

    // --- producer-side rejections -------------------------------------------

    // Every validation failure above also fails closed on the way out: the
    // canonical encoder runs the same module validation before emitting
    // bytes.
    let mut changed = module.clone();
    changed.reborrow_root_handoffs.swap(0, 1);
    encode_rejected(
        "a producer-side reordered reborrow handoff roster",
        &changed,
        CodecError::InvalidModule(ModuleError::NonCanonicalReborrowRootHandoffOrder),
    );
    let mut changed = module.clone();
    changed
        .reborrow_root_handoffs
        .push(changed.reborrow_root_handoffs[0].clone());
    encode_rejected(
        "a producer-side duplicated reborrow handoff row",
        &changed,
        CodecError::InvalidModule(ModuleError::DuplicateReborrowRootHandoff),
    );
    let mut changed = module.clone();
    changed.reborrow_root_handoffs[0].lineage.clear();
    encode_rejected(
        "a producer-side emptied reborrow lineage",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_root_handoffs[0].source_machine_identity = "producer-machine".into();
    encode_rejected(
        "a producer-side non-canonical reborrow source identity",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_root_handoffs[0].direct_root_access = StructuralAccess::SharedBorrow;
    encode_rejected(
        "a producer-side shared reborrow direct root access",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_root_handoffs[0].direct_root_lifetime_identity = borrow(0x99);
    encode_rejected(
        "a producer-side reborrow lifetime split from the root",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_root_handoffs[0].direct_root_weakening = TerminalBorrowBoundarySource::Call {
        statement_index: 5,
        call_ordinal: 0,
        target_identity: "producer-target".into(),
    };
    encode_rejected(
        "a producer-side non-canonical reborrow call boundary target",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_root_handoffs[0].lineage[0]
        .child_place
        .root_identity = borrow(0x9A);
    encode_rejected(
        "a producer-side reborrow child place rebound to another root",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_root_handoffs[0].lineage[0].formation_boundary =
        TerminalBorrowBoundarySource::Statement {
            statement_index: 0x6B,
        };
    encode_rejected(
        "a producer-side reborrow formation split from activation",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_root_handoffs[0].lineage[0]
        .projection_remainder
        .pop();
    encode_rejected(
        "a producer-side trimmed reborrow projection remainder",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.reborrow_root_handoffs[0].lineage[0].child_access = StructuralAccess::SharedBorrow;
    encode_rejected(
        "a producer-side shared reborrow child access",
        &changed,
        invalid_first(),
    );

    // --- module envelope boundaries ---------------------------------------

    // Truncation inside the roster and a trailing byte reject at the
    // envelope, before semantic replay ever runs.
    for cut in [
        first.row.end - 1,
        step.row.end - 1,
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
