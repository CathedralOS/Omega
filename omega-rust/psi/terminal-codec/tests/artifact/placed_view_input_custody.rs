//! One-field mutation coverage for the semantic module's placed-view input
//! roster.
//!
//! `placed_view_inputs` is the module-level custody record of each
//! synthesized `Placed<P, T>` view: the owning machine and parameter
//! position, the source machine/state/parameter identities, the borrow
//! access and binding flags, the canonical view identity derived from the
//! policy/schema pair, the policy-plan machine identity, and the placement
//! report fingerprint and commitment. Its wire fields — the roster count,
//! each row's machine, position, six length-framed strings, the access tag,
//! both binding booleans, the u64 fingerprint, and the 32-byte placement
//! commitment — are each substituted independently, every leg declared once
//! in `placed_view_input_custody_fields.rs` and driven through the shared
//! `run_one_field_substitution_matrix` driver. A substitution either
//! fails canonical decoding or the verifier's module-bound validation
//! (`Owned` access, a non-hermetic identity, a stale derived view identity,
//! a zero fingerprint or commitment, a duplicate coordinate, or a reordered
//! roster), or it decodes to a different module whose honestly recomputed
//! semantic and artifact identities diverge and whose replay against the
//! retained manifest and sealed proof subject rejects.
//!
//! The fixture keeps a second scalar-return machine beside the shared one so
//! the `machine` field has a representable substitution in both directions,
//! and two rows so the roster itself is reorderable.

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
    OperationResult, StructuralAccess, TerminalMachineResult, TerminalPlacedViewInput, Terminator,
    canonical_placed_view_identity,
};
use terminal_verifier::{ModuleError, ObligationEvidence, verify_module};

use optimization_core::{
    MutationOutcome, OneFieldSubstitutionMatrix, run_one_field_substitution_matrix,
};

#[path = "placed_view_input_custody_fields.rs"]
mod placed_view_input_custody_fields;

use placed_view_input_custody_fields::PlacedViewInputCustodyFieldForTest;

/// Byte offsets of every wire field inside the placed-view input roster.
/// Every `string` field records its u32 length prefix and its content bytes
/// separately so a substitution can lie about either side.
struct ModuleSpans {
    input_count: Range<usize>,
    inputs: Vec<InputSpan>,
}

struct InputSpan {
    row: Range<usize>,
    machine: Range<usize>,
    position: Range<usize>,
    source_machine_len: Range<usize>,
    source_machine: Range<usize>,
    source_state_len: Range<usize>,
    source_state: Range<usize>,
    source_parameter_len: Range<usize>,
    source_parameter: Range<usize>,
    access: Range<usize>,
    binding_const: Range<usize>,
    binding_mutable: Range<usize>,
    view_len: Range<usize>,
    view: Range<usize>,
    policy_len: Range<usize>,
    policy: Range<usize>,
    plan_len: Range<usize>,
    plan: Range<usize>,
    schema_len: Range<usize>,
    schema: Range<usize>,
    fingerprint: Range<usize>,
    commitment: Range<usize>,
}

/// A cursor that walks the canonical module encoding exactly as the decoder
/// does, recording the byte span of every placed-view field the matrix
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

    fn walk_input(&mut self) -> InputSpan {
        let row_start = self.offset;
        let machine = self.take(8);
        let position = self.take(4);
        let (source_machine_len, source_machine) = self.take_string();
        let (source_state_len, source_state) = self.take_string();
        let (source_parameter_len, source_parameter) = self.take_string();
        let access = self.take(1);
        let binding_const = self.take(1);
        let binding_mutable = self.take(1);
        let (view_len, view) = self.take_string();
        let (policy_len, policy) = self.take_string();
        let (plan_len, plan) = self.take_string();
        let (schema_len, schema) = self.take_string();
        let fingerprint = self.take(8);
        let commitment = self.take(32);
        InputSpan {
            row: row_start..self.offset,
            machine,
            position,
            source_machine_len,
            source_machine,
            source_state_len,
            source_state,
            source_parameter_len,
            source_parameter,
            access,
            binding_const,
            binding_mutable,
            view_len,
            view,
            policy_len,
            policy,
            plan_len,
            plan,
            schema_len,
            schema,
            fingerprint,
            commitment,
        }
    }
}

/// Locate the placed-view input roster inside canonical module bytes by
/// mirroring `encode_raw`'s ordered section list. The fixture module keeps
/// every earlier roster empty, so each leading section is a zero count.
fn module_spans(encoded: &[u8]) -> ModuleSpans {
    let mut walker = SpanWalker::new(encoded);
    walker.take(8); // module magic
    walker.take(2); // format marker
    walker.take(2); // vocabulary marker
    walker.take(8); // entry machine identity
    // The scalar-qualification catalog encodes five counted rosters even when
    // empty: domains, qualification sets, coercions, and the float and integer
    // entry ranges.
    for label in [
        "scalar domains",
        "scalar qualification sets",
        "scalar qualification coercions",
        "scalar float entry ranges",
        "scalar integer entry ranges",
    ] {
        walker.expect_empty_count(label);
    }
    for label in [
        "structural types",
        "structural domains",
        "services",
        "concrete root service reach",
        "installation reach dependencies",
    ] {
        walker.expect_empty_count(label);
    }
    let (input_count, inputs) = walker.take_count();
    let mut spans = Vec::with_capacity(usize::try_from(inputs).expect("inputs fit"));
    for _ in 0..inputs {
        spans.push(walker.walk_input());
    }
    ModuleSpans {
        input_count,
        inputs: spans,
    }
}

/// A canonical hermetic identity: `package:` followed by a 64-digit lowercase
/// hex digest and a nonempty `::` path, the form `validate_placed_view_inputs`
/// requires of every source, policy, plan, and schema identity.
fn hermetic(digest: u64, path: &str) -> String {
    format!("package:{digest:064x}::{path}")
}

/// One placed-view row whose every identity derives from `seed`, so each row
/// in the fixture carries distinct canonical strings. The view identity is
/// the derived canonical form of the policy/schema pair, and the fingerprint
/// and commitment are nonzero.
fn placed_view_row(
    machine: u64,
    position: u32,
    seed: u64,
    access: StructuralAccess,
    binding_is_const: bool,
    binding_is_mutable: bool,
) -> TerminalPlacedViewInput {
    let policy_identity = hermetic(seed + 4, "policy::placed");
    let schema_identity = hermetic(seed + 5, "schema::row");
    let commitment_byte = u8::try_from(seed).expect("commitment byte");
    TerminalPlacedViewInput {
        machine: machine_id(machine),
        position,
        source_machine_identity: hermetic(seed + 1, "host"),
        source_state_identity: hermetic(seed + 2, "host::init"),
        source_parameter_identity: hermetic(seed + 3, "host::init::view"),
        access,
        binding_is_const,
        binding_is_mutable,
        view_identity: canonical_placed_view_identity(&policy_identity, &schema_identity),
        policy_identity,
        policy_plan_machine_identity: format!("toolchain::placed-plan-{seed}"),
        schema_identity,
        placement_report_fingerprint: 0xA500 + seed,
        placement_commitment: [commitment_byte; 32],
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

fn placed_view_module() -> terminal_psi::TerminalModule {
    let mut module = semantic_module();
    module.machines.push(second_machine());
    module.placed_view_inputs = vec![
        placed_view_row(1, 0, 0x10, StructuralAccess::SharedBorrow, false, true),
        placed_view_row(2, 3, 0x20, StructuralAccess::MutableBorrow, true, false),
    ];
    module
}

/// The kernel bundle extended with evidence for the second machine's contract
/// obligation, so the two-machine fixture verifies end to end.
fn placed_view_bundle() -> terminal_verifier::ProofBundle {
    let mut bundle = kernel_bundle();
    bundle.evidence.push(ObligationEvidence {
        obligation: obligation_id(2),
        route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
    });
    bundle
}

/// The family's combined independent checker verdict: canonical decoding
/// first, then the retained artifact-manifest replay for a substitution that
/// still decodes.
#[derive(Debug, Clone, PartialEq)]
enum PlacedViewCheck {
    Decode(CodecError),
    ManifestReplay(ArtifactManifestError),
}

/// A substitution that still forms a canonical module is rejected by the
/// retained manifest replay.
const REPLAYED: PlacedViewCheck =
    PlacedViewCheck::ManifestReplay(ArtifactManifestError::ManifestMismatch);

fn decoded(error: CodecError) -> PlacedViewCheck {
    PlacedViewCheck::Decode(error)
}

fn invalid_first() -> PlacedViewCheck {
    decoded(CodecError::InvalidModule(
        ModuleError::InvalidPlacedViewInput {
            machine: machine_id(1),
            position: 0,
        },
    ))
}

fn put_u8(encoded: &[u8], range: Range<usize>, value: u8) -> Vec<u8> {
    let mut mutated = encoded.to_vec();
    mutated[range.start] = value;
    mutated
}

fn put_bytes(encoded: &[u8], range: Range<usize>, value: &[u8]) -> Vec<u8> {
    let mut mutated = encoded.to_vec();
    mutated[range].copy_from_slice(value);
    mutated
}

/// Replace one encoded string, keeping its length prefix honest.
fn restring(
    encoded: &[u8],
    len_span: Range<usize>,
    value_span: Range<usize>,
    value: &str,
) -> Vec<u8> {
    let mut mutated = encoded[..len_span.start].to_vec();
    mutated.extend_from_slice(
        &u32::try_from(value.len())
            .expect("string length fits u32")
            .to_le_bytes(),
    );
    mutated.extend_from_slice(value.as_bytes());
    mutated.extend_from_slice(&encoded[value_span.end..]);
    mutated
}

fn roster_count(encoded: &[u8], count_span: Range<usize>) -> u32 {
    u32::from_le_bytes(encoded[count_span].try_into().expect("roster count"))
}

/// Remove one roster row and decrement its roster count honestly.
fn drop_row(encoded: &[u8], count_span: Range<usize>, row: Range<usize>) -> Vec<u8> {
    let remaining = roster_count(encoded, count_span.clone()) - 1;
    let mut mutated = encoded[..count_span.start].to_vec();
    mutated.extend_from_slice(&remaining.to_le_bytes());
    mutated.extend_from_slice(&encoded[count_span.end..row.start]);
    mutated.extend_from_slice(&encoded[row.end..]);
    mutated
}

/// Duplicate one roster row directly behind itself with an honest count.
fn duplicate_row(encoded: &[u8], count_span: Range<usize>, row: Range<usize>) -> Vec<u8> {
    let grown = roster_count(encoded, count_span.clone()) + 1;
    let mut mutated = encoded[..count_span.start].to_vec();
    mutated.extend_from_slice(&grown.to_le_bytes());
    mutated.extend_from_slice(&encoded[count_span.end..row.end]);
    mutated.extend_from_slice(&encoded[row.clone()]);
    mutated.extend_from_slice(&encoded[row.end..]);
    mutated
}

/// One declared leg over the canonical fixture encoding: the substituted wire
/// form and the exact verdict the family's independent checker must reach.
/// Keeping both in one arm keeps each substitution beside its rejection.
fn placed_view_leg(
    encoded: &[u8],
    field: PlacedViewInputCustodyFieldForTest,
) -> (Vec<u8>, PlacedViewCheck) {
    use PlacedViewInputCustodyFieldForTest as Leg;
    let spans = module_spans(encoded);
    let first = &spans.inputs[0];
    let second = &spans.inputs[1];
    let u32_at =
        |range: &Range<usize>, value: u32| put_bytes(encoded, range.clone(), &value.to_le_bytes());
    let u64_at =
        |range: &Range<usize>, value: u64| put_bytes(encoded, range.clone(), &value.to_le_bytes());
    let u8_at = |range: &Range<usize>, value: u8| put_u8(encoded, range.clone(), value);
    match field {
        // --- roster axes -------------------------------------------------
        //
        // A roster count lying about its rows reads the following empty
        // sections as a zero machine identity or starves the row.
        Leg::RosterCountOver => (
            u32_at(&spans.input_count, 3),
            decoded(CodecError::ZeroIdentity("MachineId")),
        ),
        Leg::RosterCountMax => (
            u32_at(&spans.input_count, u32::MAX),
            decoded(CodecError::UnexpectedEnd),
        ),
        // Clearing the roster or dropping either row stays representable:
        // the recomputed identity diverges and the retained custody replays
        // reject.
        Leg::RosterCleared => {
            let mut cleared = encoded[..spans.input_count.start].to_vec();
            cleared.extend_from_slice(&0_u32.to_le_bytes());
            cleared.extend_from_slice(&encoded[second.row.end..]);
            (cleared, REPLAYED)
        }
        Leg::FirstRowDropped => (
            drop_row(encoded, spans.input_count.clone(), first.row.clone()),
            REPLAYED,
        ),
        Leg::SecondRowDropped => (
            drop_row(encoded, spans.input_count.clone(), second.row.clone()),
            REPLAYED,
        ),
        // A duplicated row collides on the (machine, state, position)
        // coordinate; a swapped roster violates the strict row order.
        Leg::RowDuplicated => (
            duplicate_row(encoded, spans.input_count.clone(), first.row.clone()),
            decoded(CodecError::InvalidModule(
                ModuleError::DuplicatePlacedViewInput {
                    machine: machine_id(1),
                    source_state_identity: std::str::from_utf8(
                        &encoded[first.source_state.clone()],
                    )
                    .expect("the fixture source state identity is UTF-8")
                    .to_owned(),
                    position: 0,
                },
            )),
        ),
        Leg::RosterReordered => {
            let mut swapped = encoded[..first.row.start].to_vec();
            swapped.extend_from_slice(&encoded[second.row.clone()]);
            swapped.extend_from_slice(&encoded[first.row.clone()]);
            swapped.extend_from_slice(&encoded[second.row.end..]);
            (
                swapped,
                decoded(CodecError::InvalidModule(
                    ModuleError::NonCanonicalPlacedViewInputOrder,
                )),
            )
        }

        // --- machine and position ----------------------------------------
        //
        // The machine must name a module machine. Either direction between
        // the fixture's two machines stays ordered and diverges the identity.
        Leg::MachineZero => (
            u64_at(&first.machine, 0),
            decoded(CodecError::ZeroIdentity("MachineId")),
        ),
        Leg::MachineOutsideModule => (
            u64_at(&first.machine, 9),
            decoded(CodecError::InvalidModule(
                ModuleError::InvalidPlacedViewInput {
                    machine: machine_id(9),
                    position: 0,
                },
            )),
        ),
        Leg::FirstRowReboundToSecondMachine => (u64_at(&first.machine, 2), REPLAYED),
        Leg::SecondRowReboundToFirstMachine => (u64_at(&second.machine, 1), REPLAYED),
        // The position is a free coordinate while the strict row order holds.
        Leg::FirstPositionMoved => (u32_at(&first.position, 1), REPLAYED),
        Leg::SecondPositionMoved => (u32_at(&second.position, 1), REPLAYED),

        // --- source identities -------------------------------------------
        //
        // Each source identity must stay a canonical hermetic identity; any
        // other hermetic spelling reidentifies the source and diverges.
        Leg::SourceMachineRenamed => (
            restring(
                encoded,
                first.source_machine_len.clone(),
                first.source_machine.clone(),
                &hermetic(0x31, "guest"),
            ),
            REPLAYED,
        ),
        Leg::SourceMachineEmptied => (
            restring(
                encoded,
                first.source_machine_len.clone(),
                first.source_machine.clone(),
                "",
            ),
            invalid_first(),
        ),
        Leg::SourceMachineNonHermetic => (
            restring(
                encoded,
                first.source_machine_len.clone(),
                first.source_machine.clone(),
                "host-machine",
            ),
            invalid_first(),
        ),
        Leg::SourceMachineNonUtf8 => (
            u8_at(&first.source_machine, 0xFF),
            decoded(CodecError::InvalidUtf8(
                "placed-view source machine identity",
            )),
        ),
        Leg::SourceStateRenamed => (
            restring(
                encoded,
                first.source_state_len.clone(),
                first.source_state.clone(),
                &hermetic(0x32, "host::boot"),
            ),
            REPLAYED,
        ),
        Leg::SourceStateEmptied => (
            restring(
                encoded,
                first.source_state_len.clone(),
                first.source_state.clone(),
                "",
            ),
            invalid_first(),
        ),
        Leg::SourceStateLengthLie => (
            u32_at(&first.source_state_len, u32::MAX),
            decoded(CodecError::StringTooLong(
                "placed-view source state identity",
            )),
        ),
        Leg::SourceParameterRenamed => (
            restring(
                encoded,
                first.source_parameter_len.clone(),
                first.source_parameter.clone(),
                &hermetic(0x33, "host::init::input"),
            ),
            REPLAYED,
        ),
        Leg::SourceParameterNonHermetic => (
            restring(
                encoded,
                first.source_parameter_len.clone(),
                first.source_parameter.clone(),
                "bare-parameter",
            ),
            invalid_first(),
        ),

        // --- access and binding flags ------------------------------------
        //
        // Every borrow access is representable; the `Owned` tag is not a
        // placed view, and other bytes are not an access at all.
        Leg::AccessWriteOnly => (u8_at(&first.access, 4), REPLAYED),
        Leg::SecondAccessSharedBorrow => (u8_at(&second.access, 2), REPLAYED),
        Leg::AccessOwned => (u8_at(&first.access, 1), invalid_first()),
        Leg::AccessTagZero => (
            u8_at(&first.access, 0),
            decoded(CodecError::InvalidTag("StructuralAccess", 0)),
        ),
        Leg::AccessTagFive => (
            u8_at(&first.access, 5),
            decoded(CodecError::InvalidTag("StructuralAccess", 5)),
        ),
        Leg::AccessTagMax => (
            u8_at(&first.access, u8::MAX),
            decoded(CodecError::InvalidTag("StructuralAccess", u8::MAX)),
        ),
        Leg::BindingConstSet => (u8_at(&first.binding_const, 1), REPLAYED),
        Leg::BindingConstNonBoolean => (
            u8_at(&first.binding_const, 2),
            decoded(CodecError::InvalidBoolean(2)),
        ),
        Leg::BindingMutableCleared => (u8_at(&first.binding_mutable, 0), REPLAYED),
        Leg::BindingMutableNonBoolean => (
            u8_at(&first.binding_mutable, 9),
            decoded(CodecError::InvalidBoolean(9)),
        ),

        // --- view identity and its policy/schema join ---------------------
        //
        // The view identity is the derived canonical spelling of the policy
        // and schema pair: substituting any one of the three strands the join.
        Leg::ViewIdentityForged => (
            restring(
                encoded,
                first.view_len.clone(),
                first.view.clone(),
                "placed-view:0::0:",
            ),
            invalid_first(),
        ),
        Leg::ViewIdentityEmptied => (
            restring(encoded, first.view_len.clone(), first.view.clone(), ""),
            invalid_first(),
        ),
        Leg::PolicySubstituted => (
            restring(
                encoded,
                first.policy_len.clone(),
                first.policy.clone(),
                &hermetic(0x34, "policy::other"),
            ),
            invalid_first(),
        ),
        Leg::PolicyNonHermetic => (
            restring(
                encoded,
                first.policy_len.clone(),
                first.policy.clone(),
                "policy",
            ),
            invalid_first(),
        ),
        Leg::SchemaSubstituted => (
            restring(
                encoded,
                first.schema_len.clone(),
                first.schema.clone(),
                &hermetic(0x35, "schema::cell"),
            ),
            invalid_first(),
        ),
        // The policy-plan machine identity carries no derived join: another
        // canonical hermetic spelling stays representable.
        Leg::PlanRenamed => (
            restring(
                encoded,
                first.plan_len.clone(),
                first.plan.clone(),
                "toolchain::other-plan",
            ),
            REPLAYED,
        ),
        Leg::PlanNonHermetic => (
            restring(encoded, first.plan_len.clone(), first.plan.clone(), "plan"),
            invalid_first(),
        ),

        // --- placement evidence -------------------------------------------
        //
        // The report fingerprint and commitment must be nonzero; any other
        // nonzero value reidentifies the placement and diverges.
        Leg::FingerprintZero => (u64_at(&first.fingerprint, 0), invalid_first()),
        Leg::FingerprintSubstituted => (u64_at(&first.fingerprint, 0x5A5A), REPLAYED),
        Leg::CommitmentZero => (
            put_bytes(encoded, first.commitment.clone(), &[0; 32]),
            invalid_first(),
        ),
        Leg::CommitmentSubstituted => (u8_at(&first.commitment, 0x77), REPLAYED),
    }
}

#[test]
fn terminal_placed_view_inputs_reject_every_one_field_substitution() {
    let module = placed_view_module();
    let bundle = placed_view_bundle();
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
        spans.inputs.len(),
        2,
        "fixture roster: a machine-1 row and a machine-2 row"
    );

    // The retained artifact binds the semantic identity into its manifest and
    // the sealed proof section to this exact module; replaying either against
    // a substituted module is the independent-replay leg for every
    // representable field.
    let artifact = canonical_artifact(&module, &bundle, None);
    let retained = artifact.manifest();
    let semantic_identity = terminal_psi_identity(&module).expect("semantic identity");
    assert_eq!(retained.semantic(), semantic_identity);

    // Every declared wire leg substitutes independently. A substitution that
    // still forms a canonical module honestly recomputes a divergent semantic
    // and artifact identity and is rejected by the retained manifest replay;
    // every other leg rejects inside the canonical decoder with an exact
    // error. The donor is the producer's own canonical encoding of the
    // fixture with only its first row retained.
    let mut donor_module = module.clone();
    donor_module.placed_view_inputs.truncate(1);
    let donor = encode_module(&donor_module).expect("the single-row donor encodes canonically");
    let check = |bytes: &Vec<u8>| -> Result<Vec<u8>, PlacedViewCheck> {
        let substituted = decode_module(bytes).map_err(PlacedViewCheck::Decode)?;
        let recomputed_optimization =
            build_identity_optimization_execution_record(&substituted, &bundle)
                .expect("identity optimization over the substituted module");
        validate_artifact_manifest(
            &substituted,
            &bundle,
            &recomputed_optimization,
            None,
            None,
            retained,
        )
        .map_err(PlacedViewCheck::ManifestReplay)?;
        Ok(bytes.clone())
    };
    // A leg that still decodes must also re-encode canonically, keep the
    // substituted module verifiable under the retained bundle (the roster
    // carries no proof obligations), diverge the honestly recomputed
    // semantic and artifact identities, and reject at the sealed proof
    // subject join.
    let joined_replay = |bytes: &Vec<u8>, field: PlacedViewInputCustodyFieldForTest| {
        if placed_view_leg(&encoded, field).1 != REPLAYED {
            return;
        }
        let substituted = decode_module(bytes)
            .unwrap_or_else(|error| panic!("{field:?} must still decode: {error:?}"));
        assert_ne!(substituted, module, "{field:?} must change the module");
        assert_eq!(
            &encode_module(&substituted).expect("re-encode the substitution"),
            bytes,
            "{field:?} must re-encode canonically"
        );
        assert_ne!(
            terminal_psi_identity(&substituted).expect("substituted semantic identity"),
            semantic_identity,
            "{field:?} must diverge the honestly recomputed semantic identity"
        );
        verify_module(&substituted, &bundle, &AdmissionProfile::default()).unwrap_or_else(
            |error| panic!("{field:?} must keep the substituted module verifiable: {error:?}"),
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
            "{field:?} must diverge the recomputed artifact identity"
        );
        assert!(
            matches!(
                decode_proof_section_for(&substituted, artifact.proof_bytes()),
                Err(ProofCodecError::ProofSubjectMismatch { .. })
            ),
            "{field:?} must reject at the sealed proof subject join"
        );
    };
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "placed-view input roster",
        fields: PlacedViewInputCustodyFieldForTest::INVENTORY,
        honest: &|| encoded.clone(),
        donor,
        custody: &|bytes: &Vec<u8>| bytes.clone(),
        substitute: &|bytes, field, _donor| *bytes = placed_view_leg(bytes, field).0,
        check: &check,
        outcome: &|field| MutationOutcome::ExactError(placed_view_leg(&encoded, field).1),
        joined_replay: Some(&joined_replay),
    });
    let replayed_legs = PlacedViewInputCustodyFieldForTest::INVENTORY
        .iter()
        .filter(|&&field| placed_view_leg(&encoded, field).1 == REPLAYED)
        .count();
    assert_eq!(
        replayed_legs, 17,
        "every representable placed-view substitution is declared as a replayed leg"
    );

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
    let first = &spans.inputs[0];
    let second = &spans.inputs[1];
    let invalid_first = || {
        CodecError::InvalidModule(ModuleError::InvalidPlacedViewInput {
            machine: machine_id(1),
            position: 0,
        })
    };

    // --- producer-side rejections ------------------------------------------

    // Every validation failure above also fails closed on the way out: the
    // canonical encoder runs the same module validation before emitting
    // bytes.
    let mut changed = module.clone();
    changed.placed_view_inputs.swap(0, 1);
    encode_rejected(
        "a producer-side reordered placed-view roster",
        &changed,
        CodecError::InvalidModule(ModuleError::NonCanonicalPlacedViewInputOrder),
    );
    let mut changed = module.clone();
    changed.placed_view_inputs[0].access = StructuralAccess::Owned;
    encode_rejected("a producer-side owned access", &changed, invalid_first());
    let mut changed = module.clone();
    changed.placed_view_inputs[0].view_identity = "placed-view:forged".into();
    encode_rejected(
        "a producer-side forged view identity",
        &changed,
        invalid_first(),
    );
    let mut changed = module.clone();
    changed.placed_view_inputs[0].placement_commitment = [0; 32];
    encode_rejected("a producer-side zero commitment", &changed, invalid_first());
    let mut changed = module.clone();
    changed
        .placed_view_inputs
        .push(changed.placed_view_inputs[0].clone());
    encode_rejected(
        "a producer-side duplicated row",
        &changed,
        CodecError::InvalidModule(ModuleError::DuplicatePlacedViewInput {
            machine: machine_id(1),
            source_state_identity: module.placed_view_inputs[0].source_state_identity.clone(),
            position: 0,
        }),
    );

    // --- module envelope boundaries ----------------------------------------

    // Truncation inside the roster and a trailing byte reject at the
    // envelope, before semantic replay ever runs.
    for cut in [first.row.end - 1, second.row.end - 1, encoded.len() - 1] {
        assert!(
            decode_module(&encoded[..cut]).is_err(),
            "truncation at byte {cut} must reject"
        );
    }
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        decode_module(&trailing),
        Err(CodecError::TrailingBytes(1)),
        "a trailing byte after the module must reject at canonical decoding"
    );
}
