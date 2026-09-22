//! Fixtures shared by the crash member source tests.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::TerminalStructuralInputs;
#[path = "crash_member_source/aggregate_equality.rs"]
mod aggregate_equality;
#[path = "crash_member_source/bounded_inputs.rs"]
mod bounded_inputs;
#[path = "crash_member_source/byte_entries.rs"]
mod byte_entries;
#[path = "crash_member_source/fenced_and_float_equality.rs"]
mod fenced_and_float_equality;
#[path = "crash_member_source/member_crash_routes.rs"]
mod member_crash_routes;
#[path = "crash_member_source/projected_arithmetic.rs"]
mod projected_arithmetic;

use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, Proposition, ScalarTerm, StructuralFieldId,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalStructuralValue, interpret_terminal_artifact_measured,
};
use terminal_psi::{CrashRouteGuard, OperationKind, StructuralFieldType, StructuralTypeShape};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data Packet { should_abort: bool; }
    data Helper {}
    machine Helper::inspect(packet: Packet)
    crashes Abort
        packet.should_abort
    {}

    data Root {}
    machine Root::enter(packet: Packet)
    crashes Abort
        packet.should_abort
    {
        Helper::inspect(packet);
    }
"#;

const NESTED_SOURCE: &str = r#"
    data AbortState { should_abort: bool; }
    data Packet { state: AbortState; }
    data Helper {}
    machine Helper::inspect(packet: Packet)
    crashes Abort
        packet.state.should_abort
    {}

    data Root {}
    machine Root::enter(packet: Packet)
    crashes Abort
        packet.state.should_abort
    {
        Helper::inspect(packet);
    }
"#;

const PROJECTED_SOURCE: &str = r#"
    data AbortState { should_abort: bool; }
    data Packet { state: AbortState; }
    data Spare { value: u64; }
    data Envelope { packet: Packet; spare: Spare; }
    data Helper {}
    machine Helper::inspect(packet: Packet)
    crashes Abort
        packet.state.should_abort
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.packet.state.should_abort
    {
        Helper::inspect(envelope.packet);
    }
"#;

const FIXED_INDEX_SOURCE: &str = r#"
    boundary trait PortIo {}
    pub data Receipt [linear] { should_abort: bool; }

    boundary machine Receipt::settle(self)
    reaches PortIo
    ensures true;

    data Helper {}
    machine Helper::inspect(receipt: Receipt)
    reaches PortIo
    crashes Abort
        receipt.should_abort
    {
        Receipt::settle(receipt);
    }

    data Root {}
    machine Root::enter(receipts: [Receipt; 1])
    reaches PortIo
    crashes Abort
    {
        Helper::inspect(receipts[0]);
    }
"#;

const COMPOSED_MEMBER_SOURCE: &str = r#"
    data Flag { active: bool; }
    data Pair { left: Flag; right: Flag; armed: bool; }
    data Spare {}
    data Envelope { pair: Pair; spare: Spare; }

    data Helper {}
    machine Helper::inspect(pair: Pair)
    crashes Abort
        pair.left.active == !pair.right.active && pair.armed
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.pair.left.active == !envelope.pair.right.active && envelope.pair.armed
    {
        Helper::inspect(envelope.pair);
    }
"#;

const INTEGER_MEMBER_SOURCE: &str = r#"
    data Metrics { current: u64; limit: u64; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    crashes Abort
        metrics.limit <= metrics.current && metrics.current != metrics.limit
    {}

    data Root {}
    machine Root::enter(metrics: Metrics)
    crashes Abort
        metrics.limit <= metrics.current && metrics.current != metrics.limit
    {
        Helper::inspect(metrics);
    }
"#;

const PROJECTED_INTEGER_MEMBER_SOURCE: &str = r#"
    data Metrics { current: u64; limit: u64; }
    data Batch { metrics: Metrics; shadow: Metrics; }
    data Envelope { batch: Batch; spare: Batch; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    crashes Abort
        metrics.limit <= metrics.current && metrics.current != metrics.limit
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.batch.metrics.limit <= envelope.batch.metrics.current
            && envelope.batch.metrics.current != envelope.batch.metrics.limit
    {
        Helper::inspect(envelope.batch.metrics);
    }
"#;

const PROJECTED_INTEGER_MEMBER_ARITHMETIC_SOURCE: &str = r#"
    data Metrics {
        current: u64 [0..=100];
        delta: u64 [0..=100];
        limit: u64;
    }
    data Batch { metrics: Metrics; shadow: Metrics; }
    data Envelope { batch: Batch; spare: Batch; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    crashes Abort
        metrics.current + metrics.delta <= metrics.limit
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.batch.metrics.current + envelope.batch.metrics.delta
            <= envelope.batch.metrics.limit
    {
        Helper::inspect(envelope.batch.metrics);
    }
"#;

const PROJECTED_INTEGER_MEMBER_SUBTRACTION_SOURCE: &str = r#"
    data Metrics {
        current: u64 [100..=200];
        delta: u64 [0..=100];
        floor: u64;
    }
    data Batch { metrics: Metrics; shadow: Metrics; }
    data Envelope { batch: Batch; spare: Batch; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    crashes Abort
        metrics.floor <= metrics.current - metrics.delta
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.batch.metrics.floor
            <= envelope.batch.metrics.current - envelope.batch.metrics.delta
    {
        Helper::inspect(envelope.batch.metrics);
    }
"#;

const PROJECTED_INTEGER_MEMBER_MULTIPLICATION_SOURCE: &str = r#"
    data Metrics {
        current: u64 [0..=10];
        factor: u64 [0..=10];
        limit: u64;
    }
    data Batch { metrics: Metrics; shadow: Metrics; }
    data Envelope { batch: Batch; spare: Batch; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    crashes Abort
        metrics.current * metrics.factor <= metrics.limit
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.batch.metrics.current * envelope.batch.metrics.factor
            <= envelope.batch.metrics.limit
    {
        Helper::inspect(envelope.batch.metrics);
    }
"#;

const PROJECTED_INTEGER_MEMBER_DIVISION_SOURCE: &str = r#"
    data Metrics { current: u64; limit: u64; parity: u64; }
    data Batch { metrics: Metrics; shadow: Metrics; }
    data Envelope { batch: Batch; spare: Batch; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    crashes Abort
        metrics.current / 2u64 <= metrics.limit
            && metrics.current % 2u64 == metrics.parity
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.batch.metrics.current / 2u64 <= envelope.batch.metrics.limit
            && envelope.batch.metrics.current % 2u64
                == envelope.batch.metrics.parity
    {
        Helper::inspect(envelope.batch.metrics);
    }
"#;

const PROJECTED_INTEGER_MEMBER_BITWISE_SOURCE: &str = r#"
    data Bits { value: u8; other: u8; mask: u8; expected: u8; }
    data Envelope { bits: Bits; spare: Bits; }
    data Helper {}
    machine Helper::inspect(bits: Bits)
    crashes Abort
        (bits.value & bits.mask) == bits.expected
            && (bits.value | bits.other) != bits.expected
            && (bits.value ^ bits.other) <= bits.expected
            && ~bits.value == bits.other
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        (envelope.bits.value & envelope.bits.mask) == envelope.bits.expected
            && (envelope.bits.value | envelope.bits.other) != envelope.bits.expected
            && (envelope.bits.value ^ envelope.bits.other) <= envelope.bits.expected
            && ~envelope.bits.value == envelope.bits.other
    {
        Helper::inspect(envelope.bits);
    }
"#;

const PROJECTED_INTEGER_MEMBER_POLICY_ARITHMETIC_SOURCE: &str = r#"
    data PolicyValues {
        wrapping_left: u8 in Wrapping;
        wrapping_right: u8 in Wrapping;
        wrapping_expected: u8 in Wrapping;
        saturating_left: i8 in Saturating;
        saturating_right: i8 in Saturating;
        saturating_expected: i8 in Saturating;
    }
    data Envelope { values: PolicyValues; spare: PolicyValues; }
    data Helper {}
    machine Helper::inspect(values: PolicyValues)
    crashes Abort
        values.wrapping_left + values.wrapping_right == values.wrapping_expected
            && values.wrapping_left - values.wrapping_right == values.wrapping_expected
            && values.wrapping_left * values.wrapping_right == values.wrapping_expected
            && values.saturating_left + values.saturating_right == values.saturating_expected
            && values.saturating_left - values.saturating_right == values.saturating_expected
            && values.saturating_left * values.saturating_right == values.saturating_expected
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.values.wrapping_left + envelope.values.wrapping_right
                == envelope.values.wrapping_expected
            && envelope.values.wrapping_left - envelope.values.wrapping_right
                == envelope.values.wrapping_expected
            && envelope.values.wrapping_left * envelope.values.wrapping_right
                == envelope.values.wrapping_expected
            && envelope.values.saturating_left + envelope.values.saturating_right
                == envelope.values.saturating_expected
            && envelope.values.saturating_left - envelope.values.saturating_right
                == envelope.values.saturating_expected
            && envelope.values.saturating_left * envelope.values.saturating_right
                == envelope.values.saturating_expected
    {
        Helper::inspect(envelope.values);
    }
"#;

const PROJECTED_INTEGER_MEMBER_POLICY_DIVISION_SOURCE: &str = r#"
    data PolicyValues {
        wrapping_dividend: u8 in Wrapping;
        wrapping_divisor: u8 in Wrapping;
        wrapping_quotient: u8 in Wrapping;
        wrapping_remainder: u8 in Wrapping;
        saturating_dividend: i8 in Saturating;
        saturating_divisor: i8 in Saturating;
        saturating_quotient: i8 in Saturating;
        saturating_remainder: i8 in Saturating;
    }
    data Envelope { values: PolicyValues; spare: PolicyValues; }
    data Helper {}
    machine Helper::inspect(values: PolicyValues)
    requires
        1 <= values.wrapping_divisor,
        values.saturating_divisor <= -1
    crashes Abort
        values.wrapping_dividend / values.wrapping_divisor == values.wrapping_quotient
            && values.wrapping_dividend % values.wrapping_divisor == values.wrapping_remainder
            && values.saturating_dividend / values.saturating_divisor
                == values.saturating_quotient
            && values.saturating_dividend % values.saturating_divisor
                == values.saturating_remainder
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    requires
        1 <= envelope.values.wrapping_divisor,
        envelope.values.saturating_divisor <= -1
    crashes Abort
        envelope.values.wrapping_dividend / envelope.values.wrapping_divisor
                == envelope.values.wrapping_quotient
            && envelope.values.wrapping_dividend % envelope.values.wrapping_divisor
                == envelope.values.wrapping_remainder
            && envelope.values.saturating_dividend / envelope.values.saturating_divisor
                == envelope.values.saturating_quotient
            && envelope.values.saturating_dividend % envelope.values.saturating_divisor
                == envelope.values.saturating_remainder
    {
        Helper::inspect(envelope.values);
    }
"#;

const PROJECTED_INTEGER_MEMBER_WRAPPING_SHIFT_SOURCE: &str = r#"
    data ShiftValues {
        value: u8 in Wrapping;
        count: i16;
        shifted_left: u8 in Wrapping;
        shifted_right: u8 in Wrapping;
    }
    data Envelope { values: ShiftValues; spare: ShiftValues; }
    data Helper {}
    machine Helper::inspect(values: ShiftValues)
    crashes Abort
        values.value << values.count == values.shifted_left
            && values.value >> values.count == values.shifted_right
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.values.value << envelope.values.count == envelope.values.shifted_left
            && envelope.values.value >> envelope.values.count
                == envelope.values.shifted_right
    {
        Helper::inspect(envelope.values);
    }
"#;

const PROJECTED_INTEGER_MEMBER_EXACT_SHIFT_SOURCE: &str = r#"
    data ShiftValues {
        value: u8;
        count: i16;
        shifted_left: u8;
        shifted_right: u8;
    }
    data Envelope { values: ShiftValues; spare: ShiftValues; }
    data Helper {}
    machine Helper::inspect(values: ShiftValues)
    requires
        0i16 <= values.count,
        values.count < 8i16,
        values.value <= 1u8
    crashes Abort
        values.value << values.count == values.shifted_left
            && values.value >> values.count == values.shifted_right
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    requires
        0i16 <= envelope.values.count,
        envelope.values.count < 8i16,
        envelope.values.value <= 1u8
    crashes Abort
        envelope.values.value << envelope.values.count == envelope.values.shifted_left
            && envelope.values.value >> envelope.values.count
                == envelope.values.shifted_right
    {
        Helper::inspect(envelope.values);
    }
"#;

const POLICY_NEGATIVE_ONE_LITERAL_DIVISION_SOURCE: &str = r#"
    data Values {
        dividend: i8 in Wrapping;
        quotient: i8 in Wrapping;
        remainder: i8 in Wrapping;
    }
    data Root {}
    machine Root::enter(values: Values)
    crashes Abort
        values.dividend / -1i8 == values.quotient
            && values.dividend % -1i8 == values.remainder
    {}
"#;

const RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE: &str = r#"
    data Metrics { current: u64; divisor: u64; limit: u64; }
    data Root {}
    machine Root::enter(metrics: Metrics)
    requires
        1 <= metrics.divisor
    crashes Abort
        metrics.current / metrics.divisor <= metrics.limit
    {}
"#;

const UNPROVEN_RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE: &str = r#"
    data Metrics { current: u64; divisor: u64; limit: u64; }
    data Root {}
    machine Root::enter(metrics: Metrics)
    crashes Abort
        metrics.current / metrics.divisor <= metrics.limit
    {}
"#;

const NEGATIVE_RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE: &str = r#"
    data Metrics { current: i64; divisor: i64; limit: i64; }
    data Root {}
    machine Root::enter(metrics: Metrics)
    requires
        metrics.divisor <= -2
    crashes Abort
        metrics.current % metrics.divisor <= metrics.limit
    {}
"#;

const RUNTIME_DIVISOR_CALL_SOURCE: &str = r#"
    data Metrics { current: u64; divisor: u64; limit: u64; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    requires
        1 <= metrics.divisor
    crashes Abort
        metrics.current / metrics.divisor <= metrics.limit
    {}

    data Root {}
    machine Root::enter(metrics: Metrics)
    requires
        1 <= metrics.divisor
    crashes Abort
        metrics.current / metrics.divisor <= metrics.limit
    {
        Helper::inspect(metrics);
    }
"#;

const PROJECTED_RUNTIME_DIVISOR_CALL_SOURCE: &str = r#"
    data Metrics { current: u64; divisor: u64; limit: u64; }
    data Envelope { metrics: Metrics; decoy: Metrics; }
    data Helper {}
    machine Helper::inspect(metrics: Metrics)
    requires
        1 <= metrics.divisor
    crashes Abort
        metrics.current / metrics.divisor <= metrics.limit
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    requires
        1 <= envelope.metrics.divisor
    crashes Abort
        envelope.metrics.current / envelope.metrics.divisor <= envelope.metrics.limit
    {
        Helper::inspect(envelope.metrics);
    }
"#;

const DISJUNCTIVE_MEMBER_SOURCE: &str = r#"
    data Flag { active: bool; }
    data Pair { left: Flag; right: Flag; decoy: Flag; }
    data Envelope { pair: Pair; spare: Pair; }
    data Helper {}
    machine Helper::inspect(pair: Pair)
    crashes Abort
        pair.left.active || !pair.right.active
    {}

    data Root {}
    machine Root::enter(envelope: Envelope)
    crashes Abort
        envelope.pair.left.active || !envelope.pair.right.active
    {
        Helper::inspect(envelope.pair);
    }
"#;

const WHOLE_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Counts { current: u64; limit: u64; }
    CountsEquatable: Counts satisfies Equatable;
    data Pair { active: bool; counts: Counts; }
    PairEquatable: Pair satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Pair, right: Pair)
    crashes Abort
        left == right
    {}

    data Root {}
    machine Root::enter(left: Pair, right: Pair)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }
"#;

const NESTED_PAYLOAD_SUM_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        case Empty;
        case Data(value: i32, checksum: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Envelope { active: bool; message: Message; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    data Whole {}
    machine Whole::enter(left: Envelope, right: Envelope)
    crashes Abort
        left == right
    {}
"#;

const MIXED_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        active: bool;
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Message, right: Message)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Message, right: Message)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Message, right: Message)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Message, right: Message)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        active: bool;
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Envelope { selected: bool; message: Message; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Envelope, right: Envelope)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Envelope, right: Envelope)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Envelope, right: Envelope)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Envelope, right: Envelope)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const TWO_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        active: bool;
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Inner { message: Message; }
    InnerEquatable: Inner satisfies Equatable;

    data Envelope { inner: Inner; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Envelope, right: Envelope)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Envelope, right: Envelope)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Envelope, right: Envelope)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Envelope, right: Envelope)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const THREE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        active: bool;
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Inner { message: Message; }
    InnerEquatable: Inner satisfies Equatable;

    data Middle { inner: Inner; }
    MiddleEquatable: Middle satisfies Equatable;

    data Envelope { middle: Middle; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Envelope, right: Envelope)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Envelope, right: Envelope)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Envelope, right: Envelope)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Envelope, right: Envelope)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const FOUR_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        active: bool;
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Inner { message: Message; }
    InnerEquatable: Inner satisfies Equatable;

    data Middle { inner: Inner; }
    MiddleEquatable: Middle satisfies Equatable;

    data Envelope { middle: Middle; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    data Exterior { envelope: Envelope; }
    ExteriorEquatable: Exterior satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Exterior, right: Exterior)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Exterior, right: Exterior)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Exterior, right: Exterior)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Exterior, right: Exterior)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const FIVE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        active: bool;
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Inner { message: Message; }
    InnerEquatable: Inner satisfies Equatable;

    data Middle { inner: Inner; }
    MiddleEquatable: Middle satisfies Equatable;

    data Envelope { middle: Middle; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    data Exterior { envelope: Envelope; }
    ExteriorEquatable: Exterior satisfies Equatable;

    data Outside { exterior: Exterior; }
    OutsideEquatable: Outside satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Outside, right: Outside)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Outside, right: Outside)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Outside, right: Outside)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Outside, right: Outside)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const SIX_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        active: bool;
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Inner { message: Message; }
    InnerEquatable: Inner satisfies Equatable;

    data Middle { inner: Inner; }
    MiddleEquatable: Middle satisfies Equatable;

    data Envelope { middle: Middle; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    data Exterior { envelope: Envelope; }
    ExteriorEquatable: Exterior satisfies Equatable;

    data Outside { exterior: Exterior; }
    OutsideEquatable: Outside satisfies Equatable;

    data Beyond { outside: Outside; }
    BeyondEquatable: Beyond satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Beyond, right: Beyond)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Beyond, right: Beyond)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Beyond, right: Beyond)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Beyond, right: Beyond)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const SEVEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        active: bool;
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Inner { message: Message; }
    InnerEquatable: Inner satisfies Equatable;

    data Middle { inner: Inner; }
    MiddleEquatable: Middle satisfies Equatable;

    data Envelope { middle: Middle; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    data Exterior { envelope: Envelope; }
    ExteriorEquatable: Exterior satisfies Equatable;

    data Outside { exterior: Exterior; }
    OutsideEquatable: Outside satisfies Equatable;

    data Beyond { outside: Outside; }
    BeyondEquatable: Beyond satisfies Equatable;

    data Further { beyond: Beyond; }
    FurtherEquatable: Further satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Further, right: Further)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Further, right: Further)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Further, right: Further)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Further, right: Further)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const EIGHT_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        active: bool;
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Inner { message: Message; }
    InnerEquatable: Inner satisfies Equatable;

    data Middle { inner: Inner; }
    MiddleEquatable: Middle satisfies Equatable;

    data Envelope { middle: Middle; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    data Exterior { envelope: Envelope; }
    ExteriorEquatable: Exterior satisfies Equatable;

    data Outside { exterior: Exterior; }
    OutsideEquatable: Outside satisfies Equatable;

    data Beyond { outside: Outside; }
    BeyondEquatable: Beyond satisfies Equatable;

    data Further { beyond: Beyond; }
    FurtherEquatable: Further satisfies Equatable;

    data Furthest { further: Further; }
    FurthestEquatable: Furthest satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Furthest, right: Furthest)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Furthest, right: Furthest)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Furthest, right: Furthest)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Furthest, right: Furthest)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const NINE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        active: bool;
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Inner { message: Message; }
    InnerEquatable: Inner satisfies Equatable;

    data Middle { inner: Inner; }
    MiddleEquatable: Middle satisfies Equatable;

    data Envelope { middle: Middle; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    data Exterior { envelope: Envelope; }
    ExteriorEquatable: Exterior satisfies Equatable;

    data Outside { exterior: Exterior; }
    OutsideEquatable: Outside satisfies Equatable;

    data Beyond { outside: Outside; }
    BeyondEquatable: Beyond satisfies Equatable;

    data Further { beyond: Beyond; }
    FurtherEquatable: Further satisfies Equatable;

    data Furthest { further: Further; }
    FurthestEquatable: Furthest satisfies Equatable;

    data Ultimate { furthest: Furthest; }
    UltimateEquatable: Ultimate satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Ultimate, right: Ultimate)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Ultimate, right: Ultimate)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Ultimate, right: Ultimate)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Ultimate, right: Ultimate)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const TEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        active: bool;
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Inner { message: Message; }
    InnerEquatable: Inner satisfies Equatable;

    data Middle { inner: Inner; }
    MiddleEquatable: Middle satisfies Equatable;

    data Envelope { middle: Middle; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    data Exterior { envelope: Envelope; }
    ExteriorEquatable: Exterior satisfies Equatable;

    data Outside { exterior: Exterior; }
    OutsideEquatable: Outside satisfies Equatable;

    data Beyond { outside: Outside; }
    BeyondEquatable: Beyond satisfies Equatable;

    data Further { beyond: Beyond; }
    FurtherEquatable: Further satisfies Equatable;

    data Furthest { further: Further; }
    FurthestEquatable: Furthest satisfies Equatable;

    data Ultimate { furthest: Furthest; }
    UltimateEquatable: Ultimate satisfies Equatable;

    data Outermost { ultimate: Ultimate; }
    OutermostEquatable: Outermost satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Outermost, right: Outermost)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Outermost, right: Outermost)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Outermost, right: Outermost)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Outermost, right: Outermost)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const ELEVEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        active: bool;
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Inner { message: Message; }
    InnerEquatable: Inner satisfies Equatable;

    data Middle { inner: Inner; }
    MiddleEquatable: Middle satisfies Equatable;

    data Envelope { middle: Middle; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    data Exterior { envelope: Envelope; }
    ExteriorEquatable: Exterior satisfies Equatable;

    data Outside { exterior: Exterior; }
    OutsideEquatable: Outside satisfies Equatable;

    data Beyond { outside: Outside; }
    BeyondEquatable: Beyond satisfies Equatable;

    data Further { beyond: Beyond; }
    FurtherEquatable: Further satisfies Equatable;

    data Furthest { further: Further; }
    FurthestEquatable: Furthest satisfies Equatable;

    data Ultimate { furthest: Furthest; }
    UltimateEquatable: Ultimate satisfies Equatable;

    data Outermost { ultimate: Ultimate; }
    OutermostEquatable: Outermost satisfies Equatable;

    data Final { outermost: Outermost; }
    FinalEquatable: Final satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Final, right: Final)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Final, right: Final)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Final, right: Final)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Final, right: Final)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const TWELVE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        active: bool;
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Inner { message: Message; }
    InnerEquatable: Inner satisfies Equatable;

    data Middle { inner: Inner; }
    MiddleEquatable: Middle satisfies Equatable;

    data Envelope { middle: Middle; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    data Exterior { envelope: Envelope; }
    ExteriorEquatable: Exterior satisfies Equatable;

    data Outside { exterior: Exterior; }
    OutsideEquatable: Outside satisfies Equatable;

    data Beyond { outside: Outside; }
    BeyondEquatable: Beyond satisfies Equatable;

    data Further { beyond: Beyond; }
    FurtherEquatable: Further satisfies Equatable;

    data Furthest { further: Further; }
    FurthestEquatable: Furthest satisfies Equatable;

    data Ultimate { furthest: Furthest; }
    UltimateEquatable: Ultimate satisfies Equatable;

    data Outermost { ultimate: Ultimate; }
    OutermostEquatable: Outermost satisfies Equatable;

    data Final { outermost: Outermost; }
    FinalEquatable: Final satisfies Equatable;

    data Absolute { final: Final; }
    AbsoluteEquatable: Absolute satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Absolute, right: Absolute)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Absolute, right: Absolute)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Absolute, right: Absolute)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Absolute, right: Absolute)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const THIRTEEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        active: bool;
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Inner { message: Message; }
    InnerEquatable: Inner satisfies Equatable;

    data Middle { inner: Inner; }
    MiddleEquatable: Middle satisfies Equatable;

    data Envelope { middle: Middle; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    data Exterior { envelope: Envelope; }
    ExteriorEquatable: Exterior satisfies Equatable;

    data Outside { exterior: Exterior; }
    OutsideEquatable: Outside satisfies Equatable;

    data Beyond { outside: Outside; }
    BeyondEquatable: Beyond satisfies Equatable;

    data Further { beyond: Beyond; }
    FurtherEquatable: Further satisfies Equatable;

    data Furthest { further: Further; }
    FurthestEquatable: Furthest satisfies Equatable;

    data Ultimate { furthest: Furthest; }
    UltimateEquatable: Ultimate satisfies Equatable;

    data Outermost { ultimate: Ultimate; }
    OutermostEquatable: Outermost satisfies Equatable;

    data Final { outermost: Outermost; }
    FinalEquatable: Final satisfies Equatable;

    data Absolute { final: Final; }
    AbsoluteEquatable: Absolute satisfies Equatable;

    data Supreme { absolute: Absolute; }
    SupremeEquatable: Supreme satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Supreme, right: Supreme)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Supreme, right: Supreme)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Supreme, right: Supreme)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Supreme, right: Supreme)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const FOURTEEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        active: bool;
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Inner { message: Message; }
    InnerEquatable: Inner satisfies Equatable;

    data Middle { inner: Inner; }
    MiddleEquatable: Middle satisfies Equatable;

    data Envelope { middle: Middle; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    data Exterior { envelope: Envelope; }
    ExteriorEquatable: Exterior satisfies Equatable;

    data Outside { exterior: Exterior; }
    OutsideEquatable: Outside satisfies Equatable;

    data Beyond { outside: Outside; }
    BeyondEquatable: Beyond satisfies Equatable;

    data Further { beyond: Beyond; }
    FurtherEquatable: Further satisfies Equatable;

    data Furthest { further: Further; }
    FurthestEquatable: Furthest satisfies Equatable;

    data Ultimate { furthest: Furthest; }
    UltimateEquatable: Ultimate satisfies Equatable;

    data Outermost { ultimate: Ultimate; }
    OutermostEquatable: Outermost satisfies Equatable;

    data Final { outermost: Outermost; }
    FinalEquatable: Final satisfies Equatable;

    data Absolute { final: Final; }
    AbsoluteEquatable: Absolute satisfies Equatable;

    data Supreme { absolute: Absolute; }
    SupremeEquatable: Supreme satisfies Equatable;

    data Transcendent { supreme: Supreme; }
    TranscendentEquatable: Transcendent satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Transcendent, right: Transcendent)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Transcendent, right: Transcendent)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Transcendent, right: Transcendent)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Transcendent, right: Transcendent)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const MIXED_AGGREGATE_EQUALITY_FENCE_SOURCES: [&str; 3] = [
    r#"
        trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
        data Message { pointer: addr; case Empty; case Data(value: i32); }
        MessageEquatable: Message satisfies Equatable;
        data Root {}
        machine Root::enter(left: Message, right: Message)
        crashes Abort left == right {}
    "#,
    r#"
        trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
        data Message { proof [erased]: i32; case Empty; case Data(value: i32); }
        MessageEquatable: Message satisfies Equatable;
        data Root {}
        machine Root::enter(left: Message, right: Message)
        crashes Abort left == right {}
    "#,
    r#"
        trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
        data Message { active: bool; case Empty; case More(next: Message); }
        MessageEquatable: Message satisfies Equatable;
        data Root {}
        machine Root::enter(left: Message, right: Message)
        crashes Abort left == right {}
    "#,
];

const NESTED_MIXED_AGGREGATE_EQUALITY_FENCE_SOURCES: [&str; 6] = [
    r#"
        trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
        data Message { active: bool; case Empty; case Data(value: i32); }
        MessageEquatable: Message satisfies Equatable;
        data Inner { message: Message; }
        InnerEquatable: Inner satisfies Equatable;
        data Middle { inner: Inner; }
        MiddleEquatable: Middle satisfies Equatable;
        data Envelope { middle: Middle; }
        EnvelopeEquatable: Envelope satisfies Equatable;
        data Exterior { envelope: Envelope; }
        ExteriorEquatable: Exterior satisfies Equatable;
        data Outside { exterior: Exterior; }
        OutsideEquatable: Outside satisfies Equatable;
        data Beyond { outside: Outside; }
        BeyondEquatable: Beyond satisfies Equatable;
        data Further { beyond: Beyond; }
        FurtherEquatable: Further satisfies Equatable;
        data Furthest { further: Further; }
        FurthestEquatable: Furthest satisfies Equatable;
        data Ultimate { furthest: Furthest; }
        UltimateEquatable: Ultimate satisfies Equatable;
        data Outermost { ultimate: Ultimate; }
        OutermostEquatable: Outermost satisfies Equatable;
        data Final { outermost: Outermost; }
        FinalEquatable: Final satisfies Equatable;
        data Absolute { final: Final; }
        AbsoluteEquatable: Absolute satisfies Equatable;
        data Supreme { absolute: Absolute; }
        SupremeEquatable: Supreme satisfies Equatable;
        data Transcendent { supreme: Supreme; }
        TranscendentEquatable: Transcendent satisfies Equatable;
        data Infinite { transcendent: Transcendent; }
        InfiniteEquatable: Infinite satisfies Equatable;
        data Root {}
        machine Root::enter(left: Infinite, right: Infinite)
        crashes Abort left == right {}
    "#,
    r#"
        trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
        data Message { active: bool; case Empty; case Data(value: i32); }
        MessageEquatable: Message satisfies Equatable;
        data Envelope { message: Message; }
        EnvelopeEquatable: Envelope satisfies Equatable;
        data Boxed { envelope: Envelope; }
        BoxedEquatable: Boxed satisfies Equatable;
        data Root {}
        machine Root::enter(left: Boxed, right: Boxed)
        crashes Abort left.envelope == right.envelope {}
    "#,
    r#"
        trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
        data Message { active: bool; case Empty; case Data(value: i32); }
        MessageEquatable: Message satisfies Equatable;
        data Envelope { first: Message; second: Message; }
        EnvelopeEquatable: Envelope satisfies Equatable;
        data Root {}
        machine Root::enter(left: Envelope, right: Envelope)
        crashes Abort left == right {}
    "#,
    r#"
        trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
        data Message { active: bool; case Empty; case Data(value: i32); }
        MessageEquatable: Message satisfies Equatable;
        data Outer { active: bool; case Empty; case Nested(message: Message); }
        OuterEquatable: Outer satisfies Equatable;
        data Root {}
        machine Root::enter(left: Outer, right: Outer)
        crashes Abort left == right {}
    "#,
    r#"
        trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
        data Message { active: bool; case Empty; case Data(value: i32); }
        MessageEquatable: Message satisfies Equatable;
        data Outer { case Empty; case Nested(message: Message); }
        OuterEquatable: Outer satisfies Equatable;
        data Root {}
        machine Root::enter(left: Outer, right: Outer)
        crashes Abort left == right {}
    "#,
    r#"
        trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
        data Message { active: bool; case Empty; case Data(value: i32); }
        MessageEquatable: Message satisfies Equatable;
        data Outer { message: Message; case Empty; case Value(value: i32); }
        OuterEquatable: Outer satisfies Equatable;
        data Root {}
        machine Root::enter(left: Outer, right: Outer)
        crashes Abort left == right {}
    "#,
];

const NESTED_RECORD_PAYLOAD_SUM_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Counter { count: i32; }
    CounterEquatable: Counter satisfies Equatable;

    data Detail { active: bool; counter: Counter; }
    DetailEquatable: Detail satisfies Equatable;

    data Message {
        case Empty;
        case Data(detail: Detail);
    }
    MessageEquatable: Message satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Message, right: Message)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Message, right: Message)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Message, right: Message)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Different {}
    machine Different::enter(left: Message, right: Message)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }
"#;

const NESTED_SUM_PAYLOAD_SUM_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Detail {
        case Missing;
        case Count(value: i32);
    }
    DetailEquatable: Detail satisfies Equatable;

    data Message {
        case Empty;
        case Data(detail: Detail);
    }
    MessageEquatable: Message satisfies Equatable;

    data Root {}
    machine Root::enter(left: Message, right: Message)
    crashes Abort
        left == right
    {}
"#;

const IEEE_FLOAT_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Samples { narrow: f32; wide: f64; }
    SamplesEquatable: Samples satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Samples, right: Samples)
    crashes Abort
        left == right
    {}

    machine Helper::different(left: Samples, right: Samples)
    crashes Abort
        left != right
    {}

    data Root {}
    machine Root::enter(left: Samples, right: Samples)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data Reverse {}
    machine Reverse::enter(left: Samples, right: Samples)
    crashes Abort
        right == left
    {}

    data Different {}
    machine Different::enter(left: Samples, right: Samples)
    crashes Abort
        left.narrow != right.narrow
    {}

    data AggregateDifferent {}
    machine AggregateDifferent::enter(left: Samples, right: Samples)
    crashes Abort
        left != right
    {
        Helper::different(left, right);
    }

    data Pair { left: Samples; right: Samples; }
    data ProjectedHelper {}
    machine ProjectedHelper::different(pair: Pair)
    crashes Abort
        !(pair.left.narrow == pair.right.narrow && pair.left.wide == pair.right.wide)
    {}

    data Envelope { pair: Pair; shadow: Pair; }
    data ProjectedDifferent {}
    machine ProjectedDifferent::enter(envelope: Envelope)
    crashes Abort
        !(envelope.pair.left.narrow == envelope.pair.right.narrow
            && envelope.pair.left.wide == envelope.pair.right.wide)
    {
        ProjectedHelper::different(envelope.pair);
    }
"#;

const BYTE_SEQUENCE_AGGREGATE_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    domain [u8]::Utf8
    requires
        valid_utf8(self);
    domain [u8; 8]::Utf8
    requires
        valid_utf8(self);

    data Borrowed { active: bool; text: &[u8] in Utf8; }
    BorrowedEquatable: Borrowed satisfies Equatable;
    data Bounded { active: bool; text: [u8; 8] in Utf8; }
    BoundedEquatable: Bounded satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Borrowed, right: Borrowed)
    crashes Abort
        left == right
    {}

    data Root {}
    machine Root::enter(left: Borrowed, right: Borrowed)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }

    data BoundedHelper {}
    machine BoundedHelper::inspect(left: Bounded, right: Bounded)
    crashes Abort
        left == right
    {}

    data BoundedRoot {}
    machine BoundedRoot::enter(left: Bounded, right: Bounded)
    crashes Abort
        left == right
    { BoundedHelper::inspect(left, right); }
"#;

const EMPTY_RECORD_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Empty {}
    EmptyEquatable: Empty satisfies Equatable;

    data Helper {}
    machine Helper::inspect(left: Empty, right: Empty)
    crashes Abort
        left == right
    {}

    data Root {}
    machine Root::enter(left: Empty, right: Empty)
    crashes Abort
        left == right
    {
        Helper::inspect(left, right);
    }
"#;

const ADDRESS_RECORD_EQUALITY_SOURCE: &str = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Addressed { pointer: addr; }
    AddressedEquatable: Addressed satisfies Equatable;

    data Root {}
    machine Root::enter(left: Addressed, right: Addressed)
    crashes Abort
        left == right
    {}
"#;

fn assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
    source: &str,
    prefix_identities: &[&str],
) {
    fn collect_scalar_paths(
        term: &ScalarTerm,
        boolean: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
        )>,
        integer: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
        )>,
    ) {
        match term {
            ScalarTerm::BooleanField { root, path } => boolean.push((*root, path.clone())),
            ScalarTerm::IntegerField { root, path, .. } => integer.push((*root, path.clone())),
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. } => {
                collect_scalar_paths(left, boolean, integer);
                collect_scalar_paths(right, boolean, integer);
            }
            _ => {}
        }
    }

    fn collect(
        proposition: &Proposition,
        memberships: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
            semantic_vocabulary::StructuralCaseId,
        )>,
        boolean: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
        )>,
        integer: &mut Vec<(
            semantic_vocabulary::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
        )>,
    ) {
        match proposition {
            Proposition::StructuralCaseMembership { subject, case } => {
                memberships.push((subject.root(), subject.path().to_vec(), *case));
            }
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right)
            | Proposition::ScalarIeeeFloatComparison { left, right, .. } => {
                collect_scalar_paths(left, boolean, integer);
                collect_scalar_paths(right, boolean, integer);
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                for child in children {
                    collect(child, memberships, boolean, integer);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                collect(premise, memberships, boolean, integer);
                collect(conclusion, memberships, boolean, integer);
            }
            Proposition::Truth
            | Proposition::Falsehood
            | Proposition::Atom(_)
            | Proposition::IntegerMathEqual(_, _)
            | Proposition::IntegerMathLessThan(_, _)
            | Proposition::IntegerMathLessOrEqual(_, _)
            | Proposition::IeeeFloatComparison { .. }
            | Proposition::ByteSequenceEqual { .. }
            | Proposition::ContentConservation(_) => {}
        }
    }

    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");
    let equal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("nested mixed equality lowers through the whole-root call");
    let different = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Different::enter"),
    )
    .expect("nested mixed inequality lowers through the whole-root call");

    for (lowered, is_different) in [(&equal, false), (&different, true)] {
        let machine = &lowered.semantic_module.machines[0];
        let mut structural_type = machine.structural_parameters[0].structural_type;
        let mut mixed_prefix = Vec::new();
        for identity in prefix_identities {
            let declaration = lowered
                .semantic_module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
                .expect("enclosing structural type");
            let StructuralTypeShape::Record { fields } = &declaration.shape else {
                panic!("every enclosing type remains a record")
            };
            let field = fields
                .iter()
                .find(|field| field.identity == *identity)
                .expect("exact enclosing field");
            mixed_prefix.push(CanonicalStructuralPathSegment::Field(field.id));
            let StructuralFieldType::Structural(next) = field.field_type else {
                panic!("enclosing field retains its structural type")
            };
            structural_type = next;
        }
        let message = lowered
            .semantic_module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .expect("Message structural type");
        let StructuralTypeShape::Mixed { fields, cases } = &message.shape else {
            panic!("Message retains its mixed shape")
        };
        let active = fields
            .iter()
            .find(|field| field.identity == "active")
            .expect("message active field");
        let data = cases
            .iter()
            .find(|case| case.identity == "Data")
            .expect("Data case");
        let value = data
            .fields
            .iter()
            .find(|field| field.identity == "value")
            .expect("Data value field");
        let [CrashRouteGuard::Predicate(route)] =
            machine.contract.crash_routes[0].alternatives.as_slice()
        else {
            panic!("nested mixed equality publishes one predicate")
        };
        let equality = if is_different {
            let Proposition::Implication {
                premise,
                conclusion,
            } = route.proposition()
            else {
                panic!("nested mixed inequality is an implication")
            };
            assert!(matches!(conclusion.as_ref(), Proposition::Falsehood));
            premise.as_ref()
        } else {
            route.proposition()
        };
        let Proposition::Conjunction(canonical) = equality else {
            panic!("nested mixed equality is one canonical conjunction")
        };
        assert_eq!(canonical.len(), 2);
        assert!(matches!(
            canonical.last(),
            Some(Proposition::Disjunction(_))
        ));

        let mut memberships = Vec::new();
        let mut boolean = Vec::new();
        let mut integer = Vec::new();
        collect(
            route.proposition(),
            &mut memberships,
            &mut boolean,
            &mut integer,
        );
        let parameter_places = machine
            .structural_parameters
            .iter()
            .map(|parameter| parameter.place)
            .collect::<Vec<_>>();
        assert_eq!(parameter_places.len(), 2);
        assert_ne!(parameter_places[0], parameter_places[1]);
        assert_eq!(memberships.len(), 4);
        assert!(memberships.iter().all(|(_, path, case)| {
            path == &mixed_prefix && cases.iter().any(|candidate| candidate.id == *case)
        }));
        let mut active_path = mixed_prefix.clone();
        active_path.push(CanonicalStructuralPathSegment::Field(active.id));
        assert_eq!(boolean.len(), 2);
        assert!(boolean.iter().all(|(_, path)| path == &active_path));
        let mut value_path = mixed_prefix.clone();
        value_path.push(CanonicalStructuralPathSegment::Case(data.id));
        value_path.push(CanonicalStructuralPathSegment::Field(value.id));
        assert_eq!(integer.len(), 2);
        assert!(integer.iter().all(|(_, path)| path == &value_path));
        for place in parameter_places {
            assert_eq!(
                memberships
                    .iter()
                    .filter(|(root, _, _)| *root == place)
                    .count(),
                2
            );
            assert_eq!(boolean.iter().filter(|(root, _)| *root == place).count(), 1);
            assert_eq!(integer.iter().filter(|(root, _)| *root == place).count(), 1);
        }

        let OperationKind::CallUnit {
            crash_continuations,
            ..
        } = &machine.blocks[0].operations[0].kind
        else {
            panic!("nested mixed caller emits one Unit call")
        };
        assert_eq!(crash_continuations, &machine.contract.crash_routes);

        let verified = terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("verifier replays every nested mixed prefix");
        let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("nested mixed equality has fixed fuel");
        validate_fixed_entry_fuel(&verified, &fixed).expect("nested mixed fixed fuel recomputes");
        let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
        assert_eq!(
            decode_module(&semantics),
            Ok(lowered.semantic_module.clone())
        );
        let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("proof encode");
        assert_eq!(
            decode_proof_bundle(&proof),
            Ok(lowered.proof_bundle.clone())
        );
        let arguments = machine
            .structural_parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| TerminalStructuralValue {
                opaque_identity: 1001 + u64::try_from(index).expect("small parameter index"),
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            })
            .collect::<Vec<_>>();
        let measured = interpret_terminal_artifact_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: &arguments,
                ..Default::default()
            },
            &mut Accept,
        )
        .expect("verified nested mixed equality remains executable metadata");
        assert_eq!(measured.value(), TerminalExecutionResult::Unit);
        assert_eq!(measured.usage().total_units(), fixed.ceiling_units());
    }

    let machine = &equal.semantic_module.machines[0];
    let mut structural_type = machine.structural_parameters[0].structural_type;
    let mut enclosing_fields = Vec::new();
    for identity in prefix_identities {
        let declaration = equal
            .semantic_module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .expect("enclosing structural type");
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            panic!("every enclosing type remains a record")
        };
        let field = fields
            .iter()
            .find(|field| field.identity == *identity)
            .expect("exact enclosing field");
        enclosing_fields.push((structural_type, field.id));
        let StructuralFieldType::Structural(next) = field.field_type else {
            panic!("enclosing field retains its structural type")
        };
        structural_type = next;
    }
    for (index, (owner, field)) in enclosing_fields.into_iter().enumerate() {
        let mut redirected = equal.semantic_module.clone();
        let StructuralTypeShape::Record { fields } = &mut redirected
            .structural_types
            .iter_mut()
            .find(|declaration| declaration.id == owner)
            .expect("enclosing structural type")
            .shape
        else {
            panic!("every enclosing type remains a record")
        };
        fields
            .iter_mut()
            .find(|candidate| candidate.id == field)
            .expect("exact enclosing field")
            .id = StructuralFieldId::new(u64::MAX - u64::try_from(index).expect("small index"))
            .expect("nonzero redirected field");
        let result = terminal_verifier::validate_module(&redirected);
        assert!(
            matches!(
                result,
                Err(terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
                    | Err(terminal_verifier::ModuleError::InvalidStructuralCaseMembership { .. })
            ),
            "unexpected enclosing-field {index} drift result: {result:?}"
        );
    }
}
