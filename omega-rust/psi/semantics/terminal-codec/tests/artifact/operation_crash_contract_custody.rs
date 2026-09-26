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
//! Every wire leg is declared once in `operation_crash_contract_custody_fields.rs`
//! and driven through the shared `run_one_field_substitution_matrix` driver.
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
use terminal_verifier::{
    CrashObligationOwner, ModuleError, ProofBundle, VerificationError, verify_module,
};

use mutation_matrix::{
    MutationOutcome, OneFieldSubstitutionMatrix, run_one_field_substitution_matrix,
};

#[path = "operation_crash_contract_custody_fields.rs"]
mod operation_crash_contract_custody_fields;

use operation_crash_contract_custody_fields::OperationCrashContractCustodyFieldForTest;

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
        suspension_crossing: None,
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
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
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
                        suspension_crossing: None,
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
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
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

/// The family's combined independent checker verdict: canonical decoding
/// first, then the retained artifact-manifest replay for a substitution that
/// still decodes.
#[derive(Debug, Clone, PartialEq)]
enum CrashContractCheck {
    Decode(CodecError),
    ManifestReplay(ArtifactManifestError),
}

const ROW_ORDER: &str = "operation crash contracts by machine and operation";
const PUBLISHED_ORDER: &str = "operation crash contract published route buckets";
const CONTINUATION_ORDER: &str = "operation crash contract continuation buckets";

/// A substitution that still forms a canonical module is rejected by the
/// retained manifest replay.
const REPLAYED: CrashContractCheck =
    CrashContractCheck::ManifestReplay(ArtifactManifestError::ManifestMismatch);

fn decoded(error: CodecError) -> CrashContractCheck {
    CrashContractCheck::Decode(error)
}

fn invalid(error: ModuleError) -> CrashContractCheck {
    decoded(CodecError::InvalidModule(error))
}

fn continuations_mismatch(operation: u64) -> CrashContractCheck {
    invalid(ModuleError::OperationCrashContinuationsMismatch {
        machine: machine_id(1),
        operation: operation_id(operation),
    })
}

fn noncanonical_routes() -> CrashContractCheck {
    invalid(ModuleError::NonCanonicalOperationCrashContractRoutes {
        machine: machine_id(1),
        operation: operation_id(1),
    })
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

fn roster_count(encoded: &[u8], count_span: Range<usize>) -> u32 {
    u32::from_le_bytes(encoded[count_span].try_into().expect("roster count"))
}

/// Set a counted roster to `remaining` rows and remove the row bytes in
/// `rows`, keeping the count honest.
fn excise(encoded: &[u8], count_span: Range<usize>, rows: Range<usize>, remaining: u32) -> Vec<u8> {
    let mut mutated = encoded[..count_span.start].to_vec();
    mutated.extend_from_slice(&remaining.to_le_bytes());
    mutated.extend_from_slice(&encoded[count_span.end..rows.start]);
    mutated.extend_from_slice(&encoded[rows.end..]);
    mutated
}

/// Remove one roster row and decrement its roster count honestly.
fn drop_row(encoded: &[u8], count_span: Range<usize>, row: Range<usize>) -> Vec<u8> {
    let remaining = roster_count(encoded, count_span.clone()) - 1;
    excise(encoded, count_span, row, remaining)
}

/// Insert `bytes` at `at`, bumping the counted roster honestly.
fn insert_bytes(encoded: &[u8], count_span: Range<usize>, at: usize, bytes: &[u8]) -> Vec<u8> {
    let grown = roster_count(encoded, count_span.clone()) + 1;
    let mut mutated = encoded[..count_span.start].to_vec();
    mutated.extend_from_slice(&grown.to_le_bytes());
    mutated.extend_from_slice(&encoded[count_span.end..at]);
    mutated.extend_from_slice(bytes);
    mutated.extend_from_slice(&encoded[at..]);
    mutated
}

/// Duplicate one roster row directly behind itself with an honest count.
fn duplicate_row(encoded: &[u8], count_span: Range<usize>, row: Range<usize>) -> Vec<u8> {
    insert_bytes(encoded, count_span, row.end, &encoded[row])
}

/// Swap two adjacent rows of the same roster, keeping the count honest.
fn swap_rows(encoded: &[u8], first: Range<usize>, second: Range<usize>) -> Vec<u8> {
    assert_eq!(first.end, second.start, "fixture rows are adjacent");
    let mut mutated = encoded[..first.start].to_vec();
    mutated.extend_from_slice(&encoded[second.clone()]);
    mutated.extend_from_slice(&encoded[first]);
    mutated.extend_from_slice(&encoded[second.end..]);
    mutated
}

/// One declared leg over the canonical fixture encoding: the substituted wire
/// form and the exact verdict the family's independent checker must reach.
/// Keeping both in one arm keeps each substitution beside its rejection.
fn crash_contract_leg(
    encoded: &[u8],
    field: OperationCrashContractCustodyFieldForTest,
) -> (Vec<u8>, CrashContractCheck) {
    use OperationCrashContractCustodyFieldForTest as Leg;
    let spans = module_spans(encoded);
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
    let u32_at =
        |range: &Range<usize>, value: u32| put_bytes(encoded, range.clone(), &value.to_le_bytes());
    let u64_at =
        |range: &Range<usize>, value: u64| put_bytes(encoded, range.clone(), &value.to_le_bytes());
    let u16_at =
        |range: &Range<usize>, value: u16| put_bytes(encoded, range.clone(), &value.to_le_bytes());
    // Overwrite the 16-byte integer literal payload.
    let i128_at =
        |range: &Range<usize>, value: i128| put_bytes(encoded, range.clone(), &value.to_le_bytes());
    let u8_at = |range: &Range<usize>, value: u8| put_u8(encoded, range.clone(), value);
    match field {
        // --- roster axes ---------------------------------------------------
        //
        // Clearing the roster or dropping either row stays representable: the
        // recomputed identity diverges and the retained custody replays
        // reject.
        Leg::RosterCleared => (
            excise(
                encoded,
                spans.contract_count.clone(),
                first.row.start..second.row.end,
                0,
            ),
            REPLAYED,
        ),
        Leg::FirstRowDropped => (
            drop_row(encoded, spans.contract_count.clone(), first.row.clone()),
            REPLAYED,
        ),
        Leg::SecondRowDropped => (
            drop_row(encoded, spans.contract_count.clone(), second.row.clone()),
            REPLAYED,
        ),
        // A count lying under its rows strands the second row's bytes at the
        // machines section; a count lying over reads the machines section as
        // a third row; a maximal count exhausts the decoder.
        Leg::RosterCountUnder => (
            u32_at(&spans.contract_count, 1),
            decoded(CodecError::InvalidTag("ScalarType", 32)),
        ),
        Leg::RosterCountOver => (
            u32_at(&spans.contract_count, 3),
            decoded(CodecError::InvalidTag("CrashCause", 0)),
        ),
        Leg::RosterCountMax => (
            u32_at(&spans.contract_count, u32::MAX),
            decoded(CodecError::UnexpectedEnd),
        ),
        // A duplicated row collides on the (machine, operation) coordinate; a
        // swapped roster violates the strict row order.
        Leg::RowDuplicated => (
            duplicate_row(encoded, spans.contract_count.clone(), first.row.clone()),
            decoded(CodecError::NonCanonicalOrder(ROW_ORDER)),
        ),
        Leg::RosterReordered => (
            swap_rows(encoded, first.row.clone(), second.row.clone()),
            decoded(CodecError::NonCanonicalOrder(ROW_ORDER)),
        ),

        // --- machine and operation joins -----------------------------------
        //
        // The row coordinate is join-constrained twice over: the machine must
        // exist, and it must own exactly the named operation. Raising the
        // first row's machine or operation identity past the second row's
        // coordinate desynchronizes the strict order before the join is ever
        // consulted.
        Leg::FirstMachineZero => (
            u64_at(&first.machine, 0),
            decoded(CodecError::ZeroIdentity("MachineId")),
        ),
        Leg::FirstMachineLiftedPastPeer => (
            u64_at(&first.machine, 9),
            decoded(CodecError::NonCanonicalOrder(ROW_ORDER)),
        ),
        Leg::SecondMachineOutsideModule => (
            u64_at(&second.machine, 9),
            invalid(ModuleError::InvalidOperationCrashContract {
                machine: machine_id(9),
                operation: operation_id(2),
            }),
        ),
        Leg::FirstOperationZero => (
            u64_at(&first.operation, 0),
            decoded(CodecError::ZeroIdentity("OperationId")),
        ),
        Leg::FirstOperationCollidingWithPeer => (
            u64_at(&first.operation, 2),
            decoded(CodecError::NonCanonicalOrder(ROW_ORDER)),
        ),
        Leg::FirstOperationLiftedPastPeer => (
            u64_at(&first.operation, 3),
            decoded(CodecError::NonCanonicalOrder(ROW_ORDER)),
        ),
        // The second row keeps the roster ordered under every retarget: an
        // unknown operation strands the join, the operand-free constant takes
        // no row at all, and the unrowed aliased spare shares the telescope,
        // the substitution, and the caller coverage exactly — so it still
        // verifies.
        Leg::SecondOperationOutsideMachine => (
            u64_at(&second.operation, 9),
            invalid(ModuleError::InvalidOperationCrashContract {
                machine: machine_id(1),
                operation: operation_id(9),
            }),
        ),
        Leg::SecondOperationOnOperandFreeConstant => (
            u64_at(&second.operation, 4),
            invalid(ModuleError::UnsupportedOperationCrashContractOperation {
                machine: machine_id(1),
                operation: operation_id(4),
            }),
        ),
        Leg::SecondOperationCollidingWithPeer => (
            u64_at(&second.operation, 1),
            decoded(CodecError::NonCanonicalOrder(ROW_ORDER)),
        ),
        Leg::SecondOperationReboundToUnrowedAlias => (u64_at(&second.operation, 3), REPLAYED),

        // --- published routes: bucket and guard structure ------------------
        //
        // A published bucket count lying over reads the continuations count
        // and bucket cause as a second bucket's alternative count —
        // 0x01000000 alternatives starve the decoder. An honestly emptied
        // published roster is uncanonical: the operator must publish at least
        // one route.
        Leg::PublishedBucketCountOver => (
            u32_at(&first.published.count, 2),
            decoded(CodecError::InvalidTag("Proposition", 0)),
        ),
        Leg::PublishedRoutesEmptied => (
            excise(
                encoded,
                first.published.count.clone(),
                first_published_bucket.row.clone(),
                0,
            ),
            decoded(CodecError::NonCanonicalOrder(PUBLISHED_ORDER)),
        ),
        // A duplicated bucket repeats the cause; the second row's Abort
        // bucket spliced ahead of the Trap bucket inverts the cause order.
        Leg::PublishedBucketDuplicated => (
            insert_bytes(
                encoded,
                first.published.count.clone(),
                first_published_bucket.row.end,
                &encoded[first_published_bucket.row.clone()],
            ),
            decoded(CodecError::NonCanonicalOrder(PUBLISHED_ORDER)),
        ),
        Leg::PublishedBucketSplicedOutOfCauseOrder => (
            insert_bytes(
                encoded,
                first.published.count.clone(),
                first_published_bucket.row.start,
                &encoded[second_published_bucket.row.clone()],
            ),
            decoded(CodecError::NonCanonicalOrder(PUBLISHED_ORDER)),
        ),
        // An unknown cause tag dies in the decoder; recasting Trap as Abort
        // leaves a published Abort route whose substitution no longer matches
        // the stored Trap continuation.
        Leg::PublishedCauseTagZero => (
            u8_at(&first_published_bucket.cause, 0),
            decoded(CodecError::InvalidTag("CrashCause", 0)),
        ),
        Leg::PublishedCauseTagThree => (
            u8_at(&first_published_bucket.cause, 3),
            decoded(CodecError::InvalidTag("CrashCause", 3)),
        ),
        Leg::PublishedCauseTagMax => (
            u8_at(&first_published_bucket.cause, u8::MAX),
            decoded(CodecError::InvalidTag("CrashCause", u8::MAX)),
        ),
        Leg::PublishedTrapRecastAsAbort => (
            u8_at(&first_published_bucket.cause, 2),
            continuations_mismatch(1),
        ),
        // An alternative count lying over reads the continuations count's
        // first byte as a second guard tag; an honestly emptied alternative
        // roster is uncanonical; a Truth marker spliced ahead of the predicate
        // violates the Truth-alone rule.
        Leg::PublishedAlternativeCountOver => (
            u32_at(&first_published_bucket.alternative_count, 2),
            decoded(CodecError::InvalidTag("Proposition", 0)),
        ),
        Leg::PublishedAlternativesEmptied => (
            excise(
                encoded,
                first_published_bucket.alternative_count.clone(),
                first_published_guard.row.clone(),
                0,
            ),
            decoded(CodecError::NonCanonicalOrder(PUBLISHED_ORDER)),
        ),
        Leg::PublishedAlternativesTruthSpliced => (
            insert_bytes(
                encoded,
                first_published_bucket.alternative_count.clone(),
                first_published_guard.row.start,
                &[0],
            ),
            decoded(CodecError::NonCanonicalOrder(PUBLISHED_ORDER)),
        ),
        // An unknown guard tag dies in the decoder.
        Leg::PublishedGuardTagTwo => (
            u8_at(&first_published_guard.tag, 2),
            decoded(CodecError::InvalidTag("CrashRouteGuard", 2)),
        ),
        Leg::PublishedGuardTagMax => (
            u8_at(&first_published_guard.tag, u8::MAX),
            decoded(CodecError::InvalidTag("CrashRouteGuard", u8::MAX)),
        ),
        // Recasting the predicate as Truth truncates a variable-length row:
        // the stranded proposition bytes are read as the continuations roster.
        Leg::PublishedGuardRecastAsTruth => (
            u8_at(&first_published_guard.tag, 0),
            decoded(CodecError::InvalidTag("CrashCause", 0)),
        ),

        // --- published predicate -------------------------------------------
        //
        // Recasting LessThan as Equal or LessOrEqual keeps the operand pair
        // but changes the published claim; the Truth and Falsehood tags are
        // banned from route predicates outright.
        Leg::PublishedPredicateRecastAsEquality => (
            u8_at(&first_published_predicate.tag, 4),
            continuations_mismatch(1),
        ),
        Leg::PublishedPredicateRecastAsNonStrictBound => (
            u8_at(&first_published_predicate.tag, 6),
            continuations_mismatch(1),
        ),
        Leg::PublishedPredicateRecastAsTruth => (
            u8_at(&first_published_predicate.tag, 1),
            decoded(CodecError::InvalidTag("CrashCause", 0)),
        ),
        Leg::PublishedPropositionTagZero => (
            u8_at(&first_published_predicate.tag, 0),
            decoded(CodecError::InvalidTag("Proposition", 0)),
        ),
        Leg::PublishedPropositionTagNinetyNine => (
            u8_at(&first_published_predicate.tag, 99),
            decoded(CodecError::InvalidTag("Proposition", 99)),
        ),
        Leg::PublishedPropositionTagMax => (
            u8_at(&first_published_predicate.tag, u8::MAX),
            decoded(CodecError::InvalidTag("Proposition", u8::MAX)),
        ),
        // The operand term must stay a scalar Value of the operand's type:
        // recasting it as a literal or a Boolean dies in the decoder or the
        // formal telescope's type check, and rebinding it to a formal outside
        // the two-operand roster names nothing.
        Leg::PublishedOperandRecastAsBooleanTerm => (
            u8_at(&first_published_predicate.left.tag, 2),
            decoded(CodecError::InvalidBoolean(2)),
        ),
        Leg::PublishedOperandRecastAsIntegerLiteral => (
            u8_at(&first_published_predicate.left.tag, 3),
            decoded(CodecError::MalformedProposition(
                PropositionError::InvalidIntegerWidth(0),
            )),
        ),
        Leg::PublishedOperandTermTagZero => (
            u8_at(&first_published_predicate.left.tag, 0),
            decoded(CodecError::InvalidTag("ScalarTerm", 0)),
        ),
        Leg::PublishedOperandTermTagNinetyNine => (
            u8_at(&first_published_predicate.left.tag, 99),
            decoded(CodecError::InvalidTag("ScalarTerm", 99)),
        ),
        Leg::PublishedOperandTermTagMax => (
            u8_at(&first_published_predicate.left.tag, u8::MAX),
            decoded(CodecError::InvalidTag("ScalarTerm", u8::MAX)),
        ),
        Leg::PublishedFormalZero => (
            u64_at(&first_published_predicate.left.id, 0),
            decoded(CodecError::ZeroIdentity("ValueId")),
        ),
        Leg::PublishedFormalOutsideTelescope => (
            u64_at(&first_published_predicate.left.id, 3),
            invalid(ModuleError::MalformedProposition(
                PropositionError::UnknownValue(value_id(3)),
            )),
        ),
        // Rebinding the predicate to formal 1 substitutes `left < 0`, which
        // the stored `right < 0` continuation no longer matches. On the
        // aliased row the same rebind substitutes identically and still
        // verifies.
        Leg::PublishedFormalReboundAcrossDistinctOperands => (
            u64_at(&first_published_predicate.left.id, 1),
            continuations_mismatch(1),
        ),
        Leg::PublishedFormalReboundAcrossAliasedOperands => {
            (u64_at(&second_published_predicate.left.id, 1), REPLAYED)
        }
        // The operand's scalar type must equal the operand's own: a Boolean
        // tag, an unsigned carrier, or a different width each mistypes the
        // predicate.
        Leg::PublishedOperandScalarRecastAsBoolean => (
            u8_at(&first_published_predicate.left.scalar_tag, 1),
            decoded(CodecError::InvalidTag("ScalarType", 0)),
        ),
        Leg::PublishedOperandRecastAsUnsigned => (
            u8_at(&first_published_predicate.left.integer_sign, 2),
            noncanonical_routes(),
        ),
        Leg::PublishedOperandNarrowed => (
            u16_at(&first_published_predicate.left.integer_bits, 8),
            noncanonical_routes(),
        ),
        Leg::PublishedOperandZeroWidth => (
            u16_at(&first_published_predicate.left.integer_bits, 0),
            decoded(CodecError::MalformedProposition(
                PropositionError::InvalidIntegerWidth(0),
            )),
        ),
        // The bound term must stay a signed integer literal of the operand's
        // type: recasting it as a Value wanders into the continuations bytes,
        // an unsigned carrier or value tag mistypes the literal, and a moved
        // bound substitutes a different continuation.
        Leg::PublishedBoundRecastAsValueTerm => (
            u8_at(&first_published_predicate.right.tag, 1),
            decoded(CodecError::InvalidTag("ScalarType", 0)),
        ),
        Leg::PublishedBoundRecastAsUnsigned => (
            u8_at(&first_published_predicate.right.integer_sign, 2),
            decoded(CodecError::MalformedProposition(
                PropositionError::IntegerLiteralOutsideType {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                    value: IntegerValue::Signed(0),
                },
            )),
        ),
        Leg::PublishedBoundWidened => (
            u16_at(&first_published_predicate.right.integer_bits, 64),
            noncanonical_routes(),
        ),
        Leg::PublishedBoundUnsignedValueTag => (
            u8_at(&first_published_predicate.right.value_tag, 2),
            decoded(CodecError::MalformedProposition(
                PropositionError::IntegerLiteralOutsideType {
                    scalar_type: i32_type(),
                    value: IntegerValue::Unsigned(0),
                },
            )),
        ),
        Leg::PublishedBoundBelowZero => (
            i128_at(&first_published_predicate.right.value, -1),
            continuations_mismatch(1),
        ),
        Leg::PublishedBoundAboveZero => (
            i128_at(&first_published_predicate.right.value, 1),
            continuations_mismatch(1),
        ),

        // --- continuations -------------------------------------------------
        //
        // Every representable change to the stored continuations divorces
        // them from the exact operand substitution the verifier recomputes —
        // the continuations are validated against nothing but that
        // substitution.
        Leg::ContinuationBucketCountOver => (
            u32_at(&first.continuations.count, 2),
            decoded(CodecError::InvalidTag("CrashRouteGuard", 2)),
        ),
        Leg::ContinuationsEmptied => (
            excise(
                encoded,
                first.continuations.count.clone(),
                first_continuations_bucket.row.clone(),
                0,
            ),
            continuations_mismatch(1),
        ),
        Leg::ContinuationBucketDuplicated => (
            insert_bytes(
                encoded,
                first.continuations.count.clone(),
                first_continuations_bucket.row.end,
                &encoded[first_continuations_bucket.row.clone()],
            ),
            decoded(CodecError::NonCanonicalOrder(CONTINUATION_ORDER)),
        ),
        Leg::ContinuationCauseTagZero => (
            u8_at(&first_continuations_bucket.cause, 0),
            decoded(CodecError::InvalidTag("CrashCause", 0)),
        ),
        Leg::ContinuationCauseTagThree => (
            u8_at(&first_continuations_bucket.cause, 3),
            decoded(CodecError::InvalidTag("CrashCause", 3)),
        ),
        Leg::ContinuationCauseTagMax => (
            u8_at(&first_continuations_bucket.cause, u8::MAX),
            decoded(CodecError::InvalidTag("CrashCause", u8::MAX)),
        ),
        Leg::ContinuationTrapRecastAsAbort => (
            u8_at(&first_continuations_bucket.cause, 2),
            continuations_mismatch(1),
        ),
        Leg::ContinuationAlternativesEmptied => (
            excise(
                encoded,
                first_continuations_bucket.alternative_count.clone(),
                first_continuations_guard.row.clone(),
                0,
            ),
            decoded(CodecError::NonCanonicalOrder(CONTINUATION_ORDER)),
        ),
        Leg::ContinuationAlternativesTruthSpliced => (
            insert_bytes(
                encoded,
                first_continuations_bucket.alternative_count.clone(),
                first_continuations_guard.row.start,
                &[0],
            ),
            decoded(CodecError::NonCanonicalOrder(CONTINUATION_ORDER)),
        ),
        Leg::ContinuationGuardTagTwo => (
            u8_at(&first_continuations_guard.tag, 2),
            decoded(CodecError::InvalidTag("CrashRouteGuard", 2)),
        ),
        Leg::ContinuationGuardTagMax => (
            u8_at(&first_continuations_guard.tag, u8::MAX),
            decoded(CodecError::InvalidTag("CrashRouteGuard", u8::MAX)),
        ),
        Leg::ContinuationGuardRecastAsTruth => (
            u8_at(&first_continuations_guard.tag, 0),
            decoded(CodecError::InvalidTag("CrashCause", 0)),
        ),
        Leg::ContinuationPredicateRecastAsEquality => (
            u8_at(&first_continuations_predicate.tag, 4),
            continuations_mismatch(1),
        ),
        Leg::ContinuationOperandZero => (
            u64_at(&first_continuations_predicate.left.id, 0),
            decoded(CodecError::ZeroIdentity("ValueId")),
        ),
        // The actual namespace is not the formal telescope: rebinding the
        // continuation operand to the machine's other parameter or to a value
        // that exists only as an operation result each drifts from the
        // substituted publication.
        Leg::ContinuationOperandReboundToOtherParameter => (
            u64_at(&first_continuations_predicate.left.id, LEFT),
            continuations_mismatch(1),
        ),
        Leg::ContinuationOperandReboundToOperationResult => (
            u64_at(&first_continuations_predicate.left.id, COMPARISON),
            continuations_mismatch(1),
        ),
        Leg::ContinuationOperandReboundToFormal => (
            u64_at(&first_continuations_predicate.left.id, 2),
            continuations_mismatch(1),
        ),
        Leg::ContinuationBoundAboveZero => (
            i128_at(&first_continuations_predicate.right.value, 7),
            continuations_mismatch(1),
        ),

        // --- second row axes -----------------------------------------------
        //
        // The Abort row repeats the same field inventory under the aliased
        // telescope: bucket count lies wander into the machines section, cause
        // and guard recasts divorce the substitution, and the bound literal is
        // the same honest field.
        Leg::SecondPublishedBucketCountOver => (
            u32_at(&second.published.count, 2),
            decoded(CodecError::InvalidTag("Proposition", 0)),
        ),
        Leg::SecondPublishedCauseRecastAsTrap => (
            u8_at(&second_published_bucket.cause, 1),
            continuations_mismatch(2),
        ),
        Leg::SecondContinuationCauseRecastAsTrap => (
            u8_at(&second_continuations_bucket.cause, 1),
            continuations_mismatch(2),
        ),
        Leg::SecondContinuationOperandReboundToOtherActual => (
            u64_at(&second_continuations_predicate.left.id, RIGHT),
            continuations_mismatch(2),
        ),
        Leg::SecondContinuationOperandReboundToFormal => (
            u64_at(&second_continuations_predicate.left.id, 2),
            continuations_mismatch(2),
        ),
        Leg::SecondContinuationBoundBelowZero => (
            i128_at(&second_continuations_predicate.right.value, -1),
            continuations_mismatch(2),
        ),
    }
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

    // Every declared wire leg substitutes independently. A substitution that
    // still forms a canonical module honestly recomputes a divergent semantic
    // and artifact identity and is rejected by the retained manifest replay;
    // every other leg rejects inside the canonical decoder with an exact
    // error. The donor is the producer's own canonical encoding of the
    // fixture with only its first row retained.
    let mut donor_module = module.clone();
    donor_module.operation_crash_contracts.truncate(1);
    let donor = encode_module(&donor_module).expect("the single-row donor encodes canonically");
    let check = |bytes: &Vec<u8>| -> Result<Vec<u8>, CrashContractCheck> {
        let substituted = decode_module(bytes).map_err(CrashContractCheck::Decode)?;
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
        .map_err(CrashContractCheck::ManifestReplay)?;
        Ok(bytes.clone())
    };
    // A leg that still decodes must also re-encode canonically, keep the
    // substituted module verifiable under the retained bundle (the roster
    // carries no proof obligations), diverge the honestly recomputed
    // semantic and artifact identities, and reject at the sealed proof
    // subject join.
    let joined_replay = |bytes: &Vec<u8>, field: OperationCrashContractCustodyFieldForTest| {
        if crash_contract_leg(&encoded, field).1 != REPLAYED {
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
        family: "operation crash contract roster",
        fields: OperationCrashContractCustodyFieldForTest::INVENTORY,
        honest: &|| encoded.clone(),
        donor,
        custody: &|bytes: &Vec<u8>| bytes.clone(),
        substitute: &|bytes, field, _donor| *bytes = crash_contract_leg(bytes, field).0,
        check: &check,
        outcome: &|field| MutationOutcome::ExactError(crash_contract_leg(&encoded, field).1),
        joined_replay: Some(&joined_replay),
    });
    let replayed_legs = OperationCrashContractCustodyFieldForTest::INVENTORY
        .iter()
        .filter(|&&field| crash_contract_leg(&encoded, field).1 == REPLAYED)
        .count();
    assert_eq!(
        replayed_legs, 5,
        "the cleared roster, both dropped rows, the alias retarget, and the aliased formal rebind decode"
    );

    // A module-level mutation the producer can express rejects inside the
    // canonical encoder's semantic validation.
    let encode_rejected = |name: &'static str, changed: &TerminalModule, expected: CodecError| {
        assert_eq!(
            encode_module(changed),
            Err(expected.clone()),
            "{name} must reject at canonical encoding"
        );
    };

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
    // Continuation coverage is a supplied-certificate question now, not a
    // module-validity one: the uncovered module still encodes canonically,
    // and verification rejects it because the reconstructed continuation
    // obligations name no supplied roster rows.
    encode_module(&uncovered).expect("uncovered continuations still encode; coverage is proved");
    assert!(
        matches!(
            verify_module(&uncovered, &bundle, &AdmissionProfile::default()),
            Err(VerificationError::MissingCrashObligationEvidence(
                CrashObligationOwner::Continuation { .. }
            ))
        ),
        "producer continuations the caller never published"
    );
}
