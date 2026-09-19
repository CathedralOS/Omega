//! One-field mutation coverage for the semantic module's operation crash
//! contract roster.
//!
//! `operation_crash_contracts` carries one row per crash-contracted ordinary
//! operation: the owning machine and operation identities, the operator's
//! published crash routes in the row's own formal telescope (the scalar
//! operand at zero-based ordinal `k` is formal `k + 1`), and the surviving
//! continuations in the machine's actual-value namespace. Each route roster
//! is a counted list of cause buckets; each bucket is a `Trap`/`Abort` cause
//! tag plus a counted guard roster of `Truth` markers and predicate
//! propositions.
//!
//! The fixture machine `compare(left: i32, right: i32) -> bool` holds four
//! operations: the rowed equality over `(left, right)`, a second rowed
//! equality over the aliased pair `(left, left)`, an unrowed alias with the
//! same telescope so one row can rebind and still verify, and a constant with
//! no operand roster at all. The machine's published contract covers the
//! substituted continuations exactly: `Trap` on `right < 0` and `Abort` on
//! `left < 0`. Every roster count, row identity, cause and guard tag,
//! predicate tag, term tag, formal identity, scalar-type field, and literal
//! payload is substituted independently.
//!
//! A substitution either fails canonical decoding or module-bound
//! representation validation (a zero or unknown join identity, an
//! out-of-order or duplicated coordinate, a noncanonical route roster, a
//! formal outside the telescope, a mistyped or truth-valued predicate,
//! continuations that drift from the exact operand substitution), or it
//! decodes to a different module — a dropped row, the alias-preserving formal
//! rebinding, or the row retargeted to the unrowed spare operation — whose
//! honestly recomputed semantic and artifact identities diverge and whose
//! replay against the retained manifest and sealed proof subject rejects.
//! Producer-side states that cannot serialize canonically are asserted at the
//! encoder. No leg lands at verify-only rejection: representation validation
//! is the complete semantic gate for this roster, and the contract rows
//! produce no proof obligations, so every semantically invalid substitution
//! is already rejected at canonical decode.

use std::ops::Range;

use super::{
    block_id, canonical_artifact, contract_id, edge_id, i32_type, machine_id, operation_id,
    value_id,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionError, ScalarTerm, ScalarType,
};
use terminal_codec::{
    ArtifactManifestError, CodecError, ProofCodecError, build_artifact_manifest,
    build_identity_optimization_execution_record, decode_module, decode_proof_section_for,
    encode_module, terminal_psi_identity, validate_artifact_manifest,
};
use terminal_psi::{
    Block, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, MachineContract,
    Operation, OperationKind, OperationResult, TerminalMachine, TerminalMachineResult,
    TerminalModule, TerminalOperationCrashContract, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{ModuleError, ProofBundle, verify_module};

/// Byte offsets of every wire field inside the crash contract roster. The
/// fixture's predicates are all `LessThan(Value, Integer)` in a fixed signed
/// 32-bit integer type, so the walker records that exact shape rather than
/// spanning arbitrary proposition or term structure.
struct ModuleSpans {
    contract_count: Range<usize>,
    contracts: Vec<ContractSpan>,
}

struct ContractSpan {
    row: Range<usize>,
    machine: Range<usize>,
    operation: Range<usize>,
    published: RoutesSpan,
    continuations: RoutesSpan,
}

struct RoutesSpan {
    count: Range<usize>,
    buckets: Vec<BucketSpan>,
}

struct BucketSpan {
    row: Range<usize>,
    cause: Range<usize>,
    alternative_count: Range<usize>,
    alternatives: Vec<GuardSpan>,
}

struct GuardSpan {
    row: Range<usize>,
    tag: Range<usize>,
    predicate: Option<PredicateSpan>,
}

struct PredicateSpan {
    tag: Range<usize>,
    left: ValueTermSpan,
    right: IntegerTermSpan,
}

/// A `ScalarTerm::Value` leaf: tag, identity, and the fixed-integer scalar
/// type (integer tag, carrier/sign, bit width).
struct ValueTermSpan {
    tag: Range<usize>,
    id: Range<usize>,
    scalar_tag: Range<usize>,
    integer_sign: Range<usize>,
    integer_bits: Range<usize>,
}

/// A `ScalarTerm::Integer` leaf: tag, the integer type (carrier/sign and bit
/// width), and the signed/unsigned-tagged i128 payload.
struct IntegerTermSpan {
    tag: Range<usize>,
    integer_sign: Range<usize>,
    integer_bits: Range<usize>,
    value_tag: Range<usize>,
    value: Range<usize>,
}

/// A cursor that walks the canonical module encoding exactly as the decoder
/// does, recording the byte span of every crash-contract field the matrix
/// substitutes. Every section ahead of the roster is empty in the fixture, so
/// the walk asserts each leading count rather than silently spanning unknown
/// row bytes.
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

    fn tag(&mut self) -> (Range<usize>, u8) {
        let span = self.take(1);
        (span.clone(), self.bytes[span.start])
    }

    /// A `Value` leaf: tag 1, the u64 identity, then the integer scalar type
    /// (tag 2, carrier/sign, u16 bits).
    fn walk_value_term(&mut self) -> ValueTermSpan {
        let (tag, value) = self.tag();
        assert_eq!(value, 1, "fixture operand is a Value scalar term");
        let id = self.take(8);
        let (scalar_tag, scalar) = self.tag();
        assert_eq!(scalar, 2, "fixture operand is an integer scalar type");
        let (integer_sign, sign) = self.tag();
        assert_eq!(sign, 1, "fixture operand is a fixed signed integer");
        let integer_bits = self.take(2);
        ValueTermSpan {
            tag,
            id,
            scalar_tag,
            integer_sign,
            integer_bits,
        }
    }

    /// An `Integer` leaf: tag 3, the integer type (carrier/sign, u16 bits),
    /// then the value tag and 16-byte payload.
    fn walk_integer_term(&mut self) -> IntegerTermSpan {
        let (tag, value) = self.tag();
        assert_eq!(value, 3, "fixture bound is an Integer scalar term");
        let (integer_sign, sign) = self.tag();
        assert_eq!(sign, 1, "fixture bound is a fixed signed integer");
        let integer_bits = self.take(2);
        let (value_tag, value) = self.tag();
        assert_eq!(value, 1, "fixture bound is a signed integer value");
        let value = self.take(16);
        IntegerTermSpan {
            tag,
            integer_sign,
            integer_bits,
            value_tag,
            value,
        }
    }

    /// The fixture's only predicate shape: `LessThan` (tag 5) over a value
    /// operand and an integer literal bound.
    fn walk_predicate(&mut self) -> PredicateSpan {
        let (tag, value) = self.tag();
        assert_eq!(value, 5, "fixture predicates are LessThan propositions");
        PredicateSpan {
            tag,
            left: self.walk_value_term(),
            right: self.walk_integer_term(),
        }
    }

    fn walk_guard(&mut self) -> GuardSpan {
        let row_start = self.offset;
        let (tag, value) = self.tag();
        let predicate = match value {
            0 => None,
            1 => Some(self.walk_predicate()),
            other => panic!("fixture guards are Truth or Predicate, not {other}"),
        };
        GuardSpan {
            row: row_start..self.offset,
            tag,
            predicate,
        }
    }

    fn walk_bucket(&mut self) -> BucketSpan {
        let row_start = self.offset;
        let (cause, value) = self.tag();
        assert!(
            matches!(value, 1 | 2),
            "fixture causes are Trap or Abort, not {value}"
        );
        let (alternative_count, alternatives) = self.take_count();
        let mut alternative_spans = Vec::with_capacity(alternatives as usize);
        for _ in 0..alternatives {
            alternative_spans.push(self.walk_guard());
        }
        BucketSpan {
            row: row_start..self.offset,
            cause,
            alternative_count,
            alternatives: alternative_spans,
        }
    }

    fn walk_routes(&mut self) -> RoutesSpan {
        let (count, buckets) = self.take_count();
        let mut bucket_spans = Vec::with_capacity(buckets as usize);
        for _ in 0..buckets {
            bucket_spans.push(self.walk_bucket());
        }
        RoutesSpan {
            count,
            buckets: bucket_spans,
        }
    }

    fn walk_contract(&mut self) -> ContractSpan {
        let row_start = self.offset;
        let machine = self.take(8);
        let operation = self.take(8);
        let published = self.walk_routes();
        let continuations = self.walk_routes();
        ContractSpan {
            row: row_start..self.offset,
            machine,
            operation,
            published,
            continuations,
        }
    }
}

/// Locate the crash contract roster inside canonical module bytes by
/// mirroring `encode_raw`'s ordered section list. The fixture module keeps
/// every earlier roster empty, so each leading section is an asserted zero
/// count.
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
        "reborrow root handoffs",
        "reborrow restored call uses",
        "boundary machines",
        "provider candidates",
        "float-meaning projections",
        "float-meaning equalities",
        "proposition declarations",
        "proposition applications",
        "evidence terms",
        "evidence contract lanes",
        "proof-output invocations",
        "proof recursive components",
        "closed conformance applications",
    ] {
        walker.expect_empty_count(label);
    }
    // The dynamic-dispatch catalog encodes nine counted rosters even when
    // empty.
    for label in [
        "dynamic descriptor parameters",
        "dynamic descriptor arguments",
        "dynamic conformance selections",
        "rebound dynamic descriptors",
        "stored dynamic descriptors",
        "direct dynamic dispatches",
        "indirect dynamic dispatches",
        "stored dynamic dispatches",
        "parameter dynamic dispatches",
    ] {
        walker.expect_empty_count(label);
    }
    walker.expect_empty_count("suspension call plan count");
    for label in [
        "suspension call sites",
        "suspension call plans",
        "quotient correspondences",
        "scalar block invariants",
    ] {
        walker.expect_empty_count(label);
    }
    let (contract_count, contracts) = walker.take_count();
    let mut contract_spans = Vec::with_capacity(contracts as usize);
    for _ in 0..contracts {
        contract_spans.push(walker.walk_contract());
    }
    ModuleSpans {
        contract_count,
        contracts: contract_spans,
    }
}

const LEFT: u64 = 10;
const RIGHT: u64 = 20;
const COMPARISON: u64 = 30;
const ALIASED: u64 = 31;
const SPARE: u64 = 32;
const CONSTANT: u64 = 33;
const RESULT: u64 = 40;

/// `operand < 0` — the only predicate shape the fixture carries. In a
/// published route the operand names a formal of the row's own telescope; in
/// a continuation or the machine contract it names an actual value.
fn negative(operand: u64) -> Proposition {
    Proposition::LessThan(
        ScalarTerm::value(value_id(operand), ScalarType::Integer(i32_type())),
        ScalarTerm::integer(i32_type(), IntegerValue::Signed(0)).unwrap(),
    )
}

fn guarded(cause: CrashCause, proposition: Proposition) -> CrashRouteBucket {
    CrashRouteBucket {
        cause,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            proposition,
        ))],
    }
}

fn declaration(raw: u64, scalar_type: ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(raw),
        scalar_type,
    }
}

fn operation(raw: u64, result: u64, kind: OperationKind) -> Operation {
    Operation {
        static_reach_binding: None,
        id: operation_id(raw),
        result: OperationResult::Scalar(declaration(result, ScalarType::Boolean)),
        kind,
    }
}

/// `compare(left: i32, right: i32) -> bool`. Operation 1 is the rowed
/// equality over both parameters; operation 2 is the rowed equality over the
/// aliased pair `(left, left)` whose formals substitute to the same actual;
/// operation 3 is the unrowed alias that shares operation 2's telescope; and
/// operation 4 is a constant with no scalar operands at all.
fn crash_module() -> TerminalModule {
    let integer_type = ScalarType::Integer(i32_type());
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: vec![
            TerminalOperationCrashContract {
                machine: machine_id(1),
                operation: operation_id(1),
                published_routes: vec![guarded(CrashCause::Trap, negative(2))],
                crash_continuations: vec![guarded(CrashCause::Trap, negative(RIGHT))],
            },
            TerminalOperationCrashContract {
                machine: machine_id(1),
                operation: operation_id(2),
                published_routes: vec![guarded(CrashCause::Abort, negative(2))],
                crash_continuations: vec![guarded(CrashCause::Abort, negative(LEFT))],
            },
        ],
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
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
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine_id(1),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![
                declaration(LEFT, integer_type),
                declaration(RIGHT, integer_type),
            ],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(RESULT, ScalarType::Boolean)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![Block {
                structural_parameters: Vec::new(),
                id: block_id(1),
                parameters: Vec::new(),
                operations: vec![
                    operation(
                        1,
                        COMPARISON,
                        OperationKind::IntegerEqual {
                            left: value_id(LEFT),
                            right: value_id(RIGHT),
                        },
                    ),
                    operation(
                        2,
                        ALIASED,
                        OperationKind::IntegerEqual {
                            left: value_id(LEFT),
                            right: value_id(LEFT),
                        },
                    ),
                    operation(
                        3,
                        SPARE,
                        OperationKind::IntegerEqual {
                            left: value_id(LEFT),
                            right: value_id(LEFT),
                        },
                    ),
                    Operation {
                        static_reach_binding: None,
                        id: operation_id(4),
                        result: OperationResult::Scalar(declaration(CONSTANT, integer_type)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(0),
                        },
                    },
                ],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: edge_id(1),
                    value: value_id(COMPARISON),
                },
            }],
            contract: MachineContract {
                id: contract_id(1),
                crash_routes: vec![
                    guarded(CrashCause::Trap, negative(RIGHT)),
                    guarded(CrashCause::Abort, negative(LEFT)),
                ],
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

/// The fixture carries no contract obligations, so the retained proof bundle
/// is the empty bundle.
fn crash_bundle() -> ProofBundle {
    ProofBundle::default()
}

#[test]
fn terminal_operation_crash_contracts_reject_every_one_field_substitution() {
    let module = crash_module();
    let bundle = crash_bundle();
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
        spans.contracts.len(),
        2,
        "fixture roster: a Trap row on the two-operand equality and an Abort row on the aliased equality"
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
    let encode_rejected = |name: &'static str, changed: &TerminalModule, expected: CodecError| {
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
    let put_u16 = |range: Range<usize>, value: u16| -> Vec<u8> {
        let mut mutated = encoded.clone();
        mutated[range].copy_from_slice(&value.to_le_bytes());
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
    // Overwrite the 16-byte integer literal payload.
    let put_i128 = |range: Range<usize>, value: i128| -> Vec<u8> {
        let mut mutated = encoded.clone();
        mutated[range].copy_from_slice(&value.to_le_bytes());
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
    // Insert `bytes` at `at`, bumping the counted roster honestly.
    let insert_bytes = |count_span: Range<usize>, at: usize, bytes: &[u8]| -> Vec<u8> {
        let grown = u32::from_le_bytes(
            encoded[count_span.clone()]
                .try_into()
                .expect("roster count"),
        ) + 1;
        let mut mutated = encoded[..count_span.start].to_vec();
        mutated.extend_from_slice(&grown.to_le_bytes());
        mutated.extend_from_slice(&encoded[count_span.end..at]);
        mutated.extend_from_slice(bytes);
        mutated.extend_from_slice(&encoded[at..]);
        mutated
    };
    // Swap two adjacent rows of the same roster, keeping the count honest.
    let swap_rows = |first: Range<usize>, second: Range<usize>| -> Vec<u8> {
        assert_eq!(first.end, second.start, "fixture rows are adjacent");
        let mut mutated = encoded[..first.start].to_vec();
        mutated.extend_from_slice(&encoded[second.clone()]);
        mutated.extend_from_slice(&encoded[first.clone()]);
        mutated.extend_from_slice(&encoded[second.end..]);
        mutated
    };

    let first = &spans.contracts[0];
    let second = &spans.contracts[1];
    let first_published_bucket = &first.published.buckets[0];
    let first_published_guard = &first_published_bucket.alternatives[0];
    let first_published_predicate = first_published_guard
        .predicate
        .as_ref()
        .expect("the first row publishes a predicate guard");
    let first_continuations_bucket = &first.continuations.buckets[0];
    let first_continuations_guard = &first_continuations_bucket.alternatives[0];
    let first_continuations_predicate = first_continuations_guard
        .predicate
        .as_ref()
        .expect("the first row continues with a predicate guard");
    let second_published_bucket = &second.published.buckets[0];
    let second_published_guard = &second_published_bucket.alternatives[0];
    let second_published_predicate = second_published_guard
        .predicate
        .as_ref()
        .expect("the second row publishes a predicate guard");
    let second_continuations_bucket = &second.continuations.buckets[0];
    let second_continuations_predicate = second_continuations_bucket.alternatives[0]
        .predicate
        .as_ref()
        .expect("the second row continues with a predicate guard");

    // --- roster axes -------------------------------------------------------

    // Clearing the roster or dropping either row stays representable: the
    // recomputed identity diverges and the retained custody replays reject.
    divergent(
        "a cleared operation crash contract roster",
        &excise(
            spans.contract_count.clone(),
            first.row.start..second.row.end,
            0,
        ),
    );
    divergent(
        "a dropped first crash contract row",
        &drop_row(spans.contract_count.clone(), first.row.clone()),
    );
    divergent(
        "a dropped second crash contract row",
        &drop_row(spans.contract_count.clone(), second.row.clone()),
    );
    // A count lying under its rows strands the second row's bytes at the
    // machines section; a count lying over reads the machines section as a
    // third row; a maximal count exhausts the decoder.
    rejected(
        "a crash contract roster count one under",
        &put_u32(spans.contract_count.clone(), 1),
        CodecError::InvalidTag("ScalarType", 32),
    );
    rejected(
        "a crash contract roster count one over",
        &put_u32(spans.contract_count.clone(), 3),
        CodecError::InvalidTag("CrashCause", 0),
    );
    rejected(
        "a maximal crash contract roster count",
        &put_u32(spans.contract_count.clone(), u32::MAX),
        CodecError::UnexpectedEnd,
    );
    // A duplicated row collides on the (machine, operation) coordinate; a
    // swapped roster violates the strict row order.
    rejected(
        "a duplicated crash contract row",
        &duplicate_row(spans.contract_count.clone(), first.row.clone()),
        CodecError::NonCanonicalOrder("operation crash contracts by machine and operation"),
    );
    rejected(
        "a reordered crash contract roster",
        &swap_rows(first.row.clone(), second.row.clone()),
        CodecError::NonCanonicalOrder("operation crash contracts by machine and operation"),
    );

    // --- machine and operation joins ----------------------------------------

    // The row coordinate is join-constrained twice over: the machine must
    // exist, and it must own exactly the named operation. Raising the first
    // row's machine or operation identity past the second row's coordinate
    // desynchronizes the strict order before the join is ever consulted.
    rejected(
        "a zero crash contract machine",
        &put_u64(first.machine.clone(), 0),
        CodecError::ZeroIdentity("MachineId"),
    );
    rejected(
        "a first-row machine lifted past its peer",
        &put_u64(first.machine.clone(), 9),
        CodecError::NonCanonicalOrder("operation crash contracts by machine and operation"),
    );
    rejected(
        "a second-row machine outside the module",
        &put_u64(second.machine.clone(), 9),
        CodecError::InvalidModule(ModuleError::InvalidOperationCrashContract {
            machine: machine_id(9),
            operation: operation_id(2),
        }),
    );
    rejected(
        "a zero crash contract operation",
        &put_u64(first.operation.clone(), 0),
        CodecError::ZeroIdentity("OperationId"),
    );
    rejected(
        "a first-row operation colliding with its peer",
        &put_u64(first.operation.clone(), 2),
        CodecError::NonCanonicalOrder("operation crash contracts by machine and operation"),
    );
    rejected(
        "a first-row operation lifted past its peer",
        &put_u64(first.operation.clone(), 3),
        CodecError::NonCanonicalOrder("operation crash contracts by machine and operation"),
    );
    // The second row keeps the roster ordered under every retarget: an
    // unknown operation strands the join, the operand-free constant takes no
    // row at all, and the unrowed aliased spare shares the telescope, the
    // substitution, and the caller coverage exactly — so it still verifies.
    rejected(
        "a second-row operation outside the machine",
        &put_u64(second.operation.clone(), 9),
        CodecError::InvalidModule(ModuleError::InvalidOperationCrashContract {
            machine: machine_id(1),
            operation: operation_id(9),
        }),
    );
    rejected(
        "a second-row operation named on the operand-free constant",
        &put_u64(second.operation.clone(), 4),
        CodecError::InvalidModule(ModuleError::UnsupportedOperationCrashContractOperation {
            machine: machine_id(1),
            operation: operation_id(4),
        }),
    );
    rejected(
        "a second-row operation colliding with its peer",
        &put_u64(second.operation.clone(), 1),
        CodecError::NonCanonicalOrder("operation crash contracts by machine and operation"),
    );
    divergent(
        "a crash contract row rebound to the unrowed alias",
        &put_u64(second.operation.clone(), 3),
    );

    // --- published routes: bucket and guard structure ------------------------

    // A published bucket count lying over reads the continuations count and
    // bucket cause as a second bucket's alternative count — 0x01000000
    // alternatives starve the decoder. An honestly emptied published roster
    // is uncanonical: the operator must publish at least one route.
    rejected(
        "a published bucket count one over",
        &put_u32(first.published.count.clone(), 2),
        CodecError::InvalidTag("Proposition", 0),
    );
    rejected(
        "an emptied published route roster",
        &excise(
            first.published.count.clone(),
            first_published_bucket.row.clone(),
            0,
        ),
        CodecError::NonCanonicalOrder("operation crash contract published route buckets"),
    );
    // A duplicated bucket repeats the cause; the second row's Abort bucket
    // spliced ahead of the Trap bucket inverts the cause order.
    rejected(
        "a duplicated published route bucket",
        &insert_bytes(
            first.published.count.clone(),
            first_published_bucket.row.end,
            &encoded[first_published_bucket.row.clone()],
        ),
        CodecError::NonCanonicalOrder("operation crash contract published route buckets"),
    );
    rejected(
        "a published route bucket spliced out of cause order",
        &insert_bytes(
            first.published.count.clone(),
            first_published_bucket.row.start,
            &encoded[second_published_bucket.row.clone()],
        ),
        CodecError::NonCanonicalOrder("operation crash contract published route buckets"),
    );
    // An unknown cause tag dies in the decoder; recasting Trap as Abort
    // leaves a published Abort route whose substitution no longer matches
    // the stored Trap continuation.
    for tag in [0, 3, u8::MAX] {
        rejected(
            "an unknown published cause tag",
            &put_u8(first_published_bucket.cause.clone(), tag),
            CodecError::InvalidTag("CrashCause", tag),
        );
    }
    rejected(
        "a published Trap route recast as Abort",
        &put_u8(first_published_bucket.cause.clone(), 2),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    // An alternative count lying over reads the continuations count's first
    // byte as a second guard tag; an honestly emptied alternative roster is
    // uncanonical; a Truth marker spliced ahead of the predicate violates the
    // Truth-alone rule.
    rejected(
        "a published alternative count one over",
        &put_u32(first_published_bucket.alternative_count.clone(), 2),
        CodecError::InvalidTag("Proposition", 0),
    );
    rejected(
        "an emptied published alternative roster",
        &excise(
            first_published_bucket.alternative_count.clone(),
            first_published_guard.row.clone(),
            0,
        ),
        CodecError::NonCanonicalOrder("operation crash contract published route buckets"),
    );
    rejected(
        "a Truth marker spliced into the published alternatives",
        &insert_bytes(
            first_published_bucket.alternative_count.clone(),
            first_published_guard.row.start,
            &[0],
        ),
        CodecError::NonCanonicalOrder("operation crash contract published route buckets"),
    );
    // An unknown guard tag dies in the decoder; recasting the predicate as
    // Truth publishes an unconditional route whose substitution no longer
    // matches the stored guarded continuation.
    for tag in [2, u8::MAX] {
        rejected(
            "an unknown published guard tag",
            &put_u8(first_published_guard.tag.clone(), tag),
            CodecError::InvalidTag("CrashRouteGuard", tag),
        );
    }
    // Recasting the predicate as Truth truncates a variable-length row: the
    // stranded proposition bytes are read as the continuations roster.
    rejected(
        "a published predicate recast as unconditional Truth",
        &put_u8(first_published_guard.tag.clone(), 0),
        CodecError::InvalidTag("CrashCause", 0),
    );

    // --- published predicate -------------------------------------------------

    // Recasting LessThan as Equal or LessOrEqual keeps the operand pair but
    // changes the published claim; the Truth and Falsehood tags are banned
    // from route predicates outright.
    rejected(
        "a published predicate recast as an equality",
        &put_u8(first_published_predicate.tag.clone(), 4),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    rejected(
        "a published predicate recast as a non-strict bound",
        &put_u8(first_published_predicate.tag.clone(), 6),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    rejected(
        "a published predicate recast as Truth",
        &put_u8(first_published_predicate.tag.clone(), 1),
        CodecError::InvalidTag("CrashCause", 0),
    );
    for tag in [0, 99, u8::MAX] {
        rejected(
            "an unknown published proposition tag",
            &put_u8(first_published_predicate.tag.clone(), tag),
            CodecError::InvalidTag("Proposition", tag),
        );
    }
    // The operand term must stay a scalar Value of the operand's type:
    // recasting it as a literal or a Boolean dies in the decoder or the
    // formal telescope's type check, and rebinding it to a formal outside the
    // two-operand roster names nothing.
    rejected(
        "a published operand recast as a Boolean term",
        &put_u8(first_published_predicate.left.tag.clone(), 2),
        CodecError::InvalidBoolean(2),
    );
    rejected(
        "a published operand recast as an integer literal",
        &put_u8(first_published_predicate.left.tag.clone(), 3),
        CodecError::MalformedProposition(PropositionError::InvalidIntegerWidth(0)),
    );
    for tag in [0, 99, u8::MAX] {
        rejected(
            "an unknown published operand term tag",
            &put_u8(first_published_predicate.left.tag.clone(), tag),
            CodecError::InvalidTag("ScalarTerm", tag),
        );
    }
    rejected(
        "a zero published formal identity",
        &put_u64(first_published_predicate.left.id.clone(), 0),
        CodecError::ZeroIdentity("ValueId"),
    );
    rejected(
        "a published formal outside the operand telescope",
        &put_u64(first_published_predicate.left.id.clone(), 3),
        CodecError::InvalidModule(ModuleError::MalformedProposition(
            PropositionError::UnknownValue(value_id(3)),
        )),
    );
    // Rebinding the predicate to formal 1 substitutes `left < 0`, which the
    // stored `right < 0` continuation no longer matches. On the aliased row
    // the same rebind substitutes identically and still verifies.
    rejected(
        "a published formal rebound across distinct operands",
        &put_u64(first_published_predicate.left.id.clone(), 1),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    divergent(
        "a published formal rebound across aliased operands",
        &put_u64(second_published_predicate.left.id.clone(), 1),
    );
    // The operand's scalar type must equal the operand's own: a Boolean tag,
    // an unsigned carrier, or a different width each mistypes the predicate.
    rejected(
        "a published operand recast as Boolean",
        &put_u8(first_published_predicate.left.scalar_tag.clone(), 1),
        CodecError::InvalidTag("ScalarType", 0),
    );
    rejected(
        "a published operand recast as an unsigned integer",
        &put_u8(first_published_predicate.left.integer_sign.clone(), 2),
        CodecError::InvalidModule(ModuleError::NonCanonicalOperationCrashContractRoutes {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    rejected(
        "a published operand retyped to a narrower integer",
        &put_u16(first_published_predicate.left.integer_bits.clone(), 8),
        CodecError::InvalidModule(ModuleError::NonCanonicalOperationCrashContractRoutes {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    rejected(
        "a published operand retyped to a zero-width integer",
        &put_u16(first_published_predicate.left.integer_bits.clone(), 0),
        CodecError::MalformedProposition(PropositionError::InvalidIntegerWidth(0)),
    );
    // The bound term must stay a signed integer literal of the operand's
    // type: recasting it as a Value wanders into the continuations bytes, an
    // unsigned carrier or value tag mistypes the literal, and a moved bound
    // substitutes a different continuation.
    rejected(
        "a published bound recast as a value term",
        &put_u8(first_published_predicate.right.tag.clone(), 1),
        CodecError::InvalidTag("ScalarType", 0),
    );
    rejected(
        "a published bound recast as unsigned",
        &put_u8(first_published_predicate.right.integer_sign.clone(), 2),
        CodecError::MalformedProposition(PropositionError::IntegerLiteralOutsideType {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            value: IntegerValue::Signed(0),
        }),
    );
    rejected(
        "a published bound retyped to a wider integer",
        &put_u16(first_published_predicate.right.integer_bits.clone(), 64),
        CodecError::InvalidModule(ModuleError::NonCanonicalOperationCrashContractRoutes {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    rejected(
        "a published bound carrying an unsigned value tag",
        &put_u8(first_published_predicate.right.value_tag.clone(), 2),
        CodecError::MalformedProposition(PropositionError::IntegerLiteralOutsideType {
            scalar_type: i32_type(),
            value: IntegerValue::Unsigned(0),
        }),
    );
    rejected(
        "a published bound shifted below zero",
        &put_i128(first_published_predicate.right.value.clone(), -1),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    rejected(
        "a published bound shifted above zero",
        &put_i128(first_published_predicate.right.value.clone(), 1),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );

    // --- continuations ---------------------------------------------------------

    // Every representable change to the stored continuations divorces them
    // from the exact operand substitution the verifier recomputes — the
    // continuations are validated against nothing but that substitution.
    rejected(
        "a continuation bucket count one over",
        &put_u32(first.continuations.count.clone(), 2),
        CodecError::InvalidTag("CrashRouteGuard", 2),
    );
    rejected(
        "an emptied continuation roster",
        &excise(
            first.continuations.count.clone(),
            first_continuations_bucket.row.clone(),
            0,
        ),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    rejected(
        "a duplicated continuation bucket",
        &insert_bytes(
            first.continuations.count.clone(),
            first_continuations_bucket.row.end,
            &encoded[first_continuations_bucket.row.clone()],
        ),
        CodecError::NonCanonicalOrder("operation crash contract continuation buckets"),
    );
    for tag in [0, 3, u8::MAX] {
        rejected(
            "an unknown continuation cause tag",
            &put_u8(first_continuations_bucket.cause.clone(), tag),
            CodecError::InvalidTag("CrashCause", tag),
        );
    }
    rejected(
        "a continuation Trap route recast as Abort",
        &put_u8(first_continuations_bucket.cause.clone(), 2),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    rejected(
        "an emptied continuation alternative roster",
        &excise(
            first_continuations_bucket.alternative_count.clone(),
            first_continuations_guard.row.clone(),
            0,
        ),
        CodecError::NonCanonicalOrder("operation crash contract continuation buckets"),
    );
    rejected(
        "a Truth marker spliced into the continuations",
        &insert_bytes(
            first_continuations_bucket.alternative_count.clone(),
            first_continuations_guard.row.start,
            &[0],
        ),
        CodecError::NonCanonicalOrder("operation crash contract continuation buckets"),
    );
    for tag in [2, u8::MAX] {
        rejected(
            "an unknown continuation guard tag",
            &put_u8(first_continuations_guard.tag.clone(), tag),
            CodecError::InvalidTag("CrashRouteGuard", tag),
        );
    }
    rejected(
        "a continuation predicate recast as unconditional Truth",
        &put_u8(first_continuations_guard.tag.clone(), 0),
        CodecError::InvalidTag("CrashCause", 0),
    );
    rejected(
        "a continuation predicate recast as an equality",
        &put_u8(first_continuations_predicate.tag.clone(), 4),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    rejected(
        "a zero continuation operand identity",
        &put_u64(first_continuations_predicate.left.id.clone(), 0),
        CodecError::ZeroIdentity("ValueId"),
    );
    // The actual namespace is not the formal telescope: rebinding the
    // continuation operand to the machine's other parameter or to a value
    // that exists only as an operation result each drifts from the
    // substituted publication.
    rejected(
        "a continuation operand rebound to the other parameter",
        &put_u64(first_continuations_predicate.left.id.clone(), LEFT),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    rejected(
        "a continuation operand rebound to an operation result",
        &put_u64(first_continuations_predicate.left.id.clone(), COMPARISON),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    rejected(
        "a continuation operand rebound to a formal identity",
        &put_u64(first_continuations_predicate.left.id.clone(), 2),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    rejected(
        "a continuation bound shifted above zero",
        &put_i128(first_continuations_predicate.right.value.clone(), 7),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );

    // --- second row axes ---------------------------------------------------------

    // The Abort row repeats the same field inventory under the aliased
    // telescope: bucket count lies wander into the machines section, cause
    // and guard recasts divorce the substitution, and the bound literal is
    // the same honest field.
    rejected(
        "a second-row published bucket count one over",
        &put_u32(second.published.count.clone(), 2),
        CodecError::InvalidTag("Proposition", 0),
    );
    rejected(
        "a second-row published cause recast as Trap",
        &put_u8(second_published_bucket.cause.clone(), 1),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(2),
        }),
    );
    rejected(
        "a second-row continuation cause recast as Trap",
        &put_u8(second_continuations_bucket.cause.clone(), 1),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(2),
        }),
    );
    rejected(
        "a second-row continuation operand rebound to the other operand's actual",
        &put_u64(second_continuations_predicate.left.id.clone(), RIGHT),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(2),
        }),
    );
    rejected(
        "a second-row continuation operand rebound to a formal identity",
        &put_u64(second_continuations_predicate.left.id.clone(), 2),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(2),
        }),
    );
    rejected(
        "a second-row continuation bound shifted below zero",
        &put_i128(second_continuations_predicate.right.value.clone(), -1),
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(2),
        }),
    );

    // --- producer-side canonical rejections --------------------------------------

    // Roster and route shapes the producer can still express reject inside
    // the encoder's canonical validation: unordered or duplicated
    // coordinates, empty or unordered bucket rosters, mixed Truth
    // alternatives, and noncanonical propositions.
    let mut reordered = module.clone();
    reordered.operation_crash_contracts.swap(0, 1);
    encode_rejected(
        "producer rows out of coordinate order",
        &reordered,
        CodecError::NonCanonicalOrder("operation crash contracts by machine and operation"),
    );
    let mut duplicated = module.clone();
    duplicated
        .operation_crash_contracts
        .push(duplicated.operation_crash_contracts[0].clone());
    encode_rejected(
        "producer rows with a duplicated coordinate",
        &duplicated,
        CodecError::NonCanonicalOrder("operation crash contracts by machine and operation"),
    );
    let mut unpublished = module.clone();
    unpublished.operation_crash_contracts[0]
        .published_routes
        .clear();
    encode_rejected(
        "producer rows with no published route",
        &unpublished,
        CodecError::NonCanonicalOrder("operation crash contract published route buckets"),
    );
    let mut guardless = module.clone();
    guardless.operation_crash_contracts[0].published_routes[0]
        .alternatives
        .clear();
    encode_rejected(
        "producer buckets with no alternative",
        &guardless,
        CodecError::NonCanonicalOrder("operation crash contract published route buckets"),
    );
    let mut misordered_buckets = module.clone();
    misordered_buckets.operation_crash_contracts[0].published_routes = vec![
        guarded(CrashCause::Abort, negative(2)),
        guarded(CrashCause::Trap, negative(2)),
    ];
    encode_rejected(
        "producer buckets out of cause order",
        &misordered_buckets,
        CodecError::NonCanonicalOrder("operation crash contract published route buckets"),
    );
    let mut mixed_truth = module.clone();
    mixed_truth.operation_crash_contracts[0].published_routes[0]
        .alternatives
        .insert(0, CrashRouteGuard::Truth);
    encode_rejected(
        "producer alternatives mixing Truth with a predicate",
        &mixed_truth,
        CodecError::NonCanonicalOrder("operation crash contract published route buckets"),
    );
    let mut misordered_continuations = module.clone();
    misordered_continuations.operation_crash_contracts[0].crash_continuations = vec![
        guarded(CrashCause::Abort, negative(RIGHT)),
        guarded(CrashCause::Trap, negative(RIGHT)),
    ];
    encode_rejected(
        "producer continuation buckets out of cause order",
        &misordered_continuations,
        CodecError::NonCanonicalOrder("operation crash contract continuation buckets"),
    );
    let mut equal_swapped = module.clone();
    equal_swapped.operation_crash_contracts[0].crash_continuations = vec![guarded(
        CrashCause::Trap,
        Proposition::Equal(
            ScalarTerm::integer(i32_type(), IntegerValue::Signed(0)).unwrap(),
            ScalarTerm::value(value_id(RIGHT), ScalarType::Integer(i32_type())),
        ),
    )];
    encode_rejected(
        "producer continuations with a noncanonical equality",
        &equal_swapped,
        CodecError::NonCanonicalOrder("equality operands"),
    );
    // Producer-side semantic drift rejects at the encoder's representation
    // validation: unknown joins, an operand-free operation, a truth-valued or
    // mistyped published predicate, continuations out of the formal
    // namespace, and coverage the caller never published.
    let mut unknown_machine = module.clone();
    unknown_machine.operation_crash_contracts[1].machine = machine_id(9);
    encode_rejected(
        "producer rows naming an unknown machine",
        &unknown_machine,
        CodecError::InvalidModule(ModuleError::InvalidOperationCrashContract {
            machine: machine_id(9),
            operation: operation_id(2),
        }),
    );
    let mut unknown_operation = module.clone();
    unknown_operation.operation_crash_contracts[1].operation = operation_id(9);
    encode_rejected(
        "producer rows naming an unknown operation",
        &unknown_operation,
        CodecError::InvalidModule(ModuleError::InvalidOperationCrashContract {
            machine: machine_id(1),
            operation: operation_id(9),
        }),
    );
    let mut constant_operation = module.clone();
    constant_operation.operation_crash_contracts[1].operation = operation_id(4);
    encode_rejected(
        "producer rows naming the operand-free constant",
        &constant_operation,
        CodecError::InvalidModule(ModuleError::UnsupportedOperationCrashContractOperation {
            machine: machine_id(1),
            operation: operation_id(4),
        }),
    );
    let mut truth_published = module.clone();
    truth_published.operation_crash_contracts[0].published_routes =
        vec![guarded(CrashCause::Trap, Proposition::Truth)];
    encode_rejected(
        "producer predicates carrying a truth proposition",
        &truth_published,
        CodecError::InvalidModule(ModuleError::NonCanonicalOperationCrashContractRoutes {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    let mut out_of_telescope = module.clone();
    out_of_telescope.operation_crash_contracts[0].published_routes =
        vec![guarded(CrashCause::Trap, negative(3))];
    encode_rejected(
        "producer predicates naming a formal outside the telescope",
        &out_of_telescope,
        CodecError::InvalidModule(ModuleError::MalformedProposition(
            PropositionError::UnknownValue(value_id(3)),
        )),
    );
    let mut mistyped = module.clone();
    mistyped.operation_crash_contracts[0].published_routes = vec![guarded(
        CrashCause::Trap,
        Proposition::Equal(
            ScalarTerm::value(value_id(2), ScalarType::Boolean),
            ScalarTerm::boolean(true),
        ),
    )];
    encode_rejected(
        "producer predicates mistyping the formal telescope",
        &mistyped,
        CodecError::InvalidModule(ModuleError::MalformedProposition(
            PropositionError::ValueTypeMismatch {
                id: value_id(2),
                expected: ScalarType::Integer(i32_type()),
                actual: ScalarType::Boolean,
            },
        )),
    );
    let mut unsubstituted = module.clone();
    unsubstituted.operation_crash_contracts[0].crash_continuations =
        vec![guarded(CrashCause::Trap, negative(2))];
    encode_rejected(
        "producer continuations left in the formal namespace",
        &unsubstituted,
        CodecError::InvalidModule(ModuleError::OperationCrashContinuationsMismatch {
            machine: machine_id(1),
            operation: operation_id(1),
        }),
    );
    let mut uncovered = module.clone();
    uncovered.machines[0].contract.crash_routes.clear();
    encode_rejected(
        "producer continuations the caller never published",
        &uncovered,
        CodecError::InvalidModule(ModuleError::CallCrashContinuationUncovered {
            operation: operation_id(1),
            cause: CrashCause::Trap,
        }),
    );
}
