use psi_core::{
    CanonicalStructuralPathSegment, IeeeFloatFormat, IntegerSign, IntegerType, Proposition,
    ScalarTerm, StructuralFieldId,
};
use psi_proof_admission::{AdmissionProfile, EvidenceRoute, ProofRule};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{
    CrashPredicateTerm, CrashRouteGuard, OperationKind, StructuralFieldType, StructuralPathSegment,
    StructuralTypeShape,
};
use psi_terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use psi_terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use psi_terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalStructuralValue, interpret_terminal_artifact_with_effect_handler_measured,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

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
        data Root {}
        machine Root::enter(left: Transcendent, right: Transcendent)
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

    data BoundedRoot {}
    machine BoundedRoot::enter(left: Bounded, right: Bounded)
    crashes Abort
        left == right
    {}
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

#[test]
fn direct_boolean_member_crash_route_survives_source_call_codec_and_interpretation() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("direct Boolean member crash route lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 2);
    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [root_route] = root.contract.crash_routes.as_slice() else {
        panic!("caller publishes one member-guarded route")
    };
    let [helper_route] = helper.contract.crash_routes.as_slice() else {
        panic!("callee publishes one member-guarded route")
    };
    assert!(matches!(
        root_route.alternatives.as_slice(),
        [CrashRouteGuard::Predicate(_)]
    ));
    assert!(matches!(
        helper_route.alternatives.as_slice(),
        [CrashRouteGuard::Predicate(_)]
    ));
    let call = root.blocks[0]
        .operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::CallUnit {
                crash_continuations,
                ..
            } => Some(crash_continuations),
            _ => None,
        })
        .expect("caller emits the Unit call");
    assert_eq!(call, &root.contract.crash_routes);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs the exact member-root substitution");
    let bytes = encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );

    let packet = TerminalStructuralValue {
        opaque_identity: 7,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    assert_eq!(
        interpret_terminal_artifact_with_effect_handler_measured(
            &bytes,
            &encode_proof_bundle(&lowered.proof_bundle).expect("proof encode"),
            &AdmissionProfile::default(),
            &[],
            &[packet],
            &mut Accept,
        )
        .expect("member contracts do not reinterpret opaque aggregate runtime data")
        .into_value(),
        TerminalExecutionResult::Unit,
    );
}

#[test]
fn verifier_rejects_unknown_direct_boolean_member_identity() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let mut lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("direct Boolean member crash route lowers");
    let wrong_field = StructuralFieldId::new(u64::MAX).expect("nonzero field");
    let wrong_route = |root| {
        vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(root, wrong_field),
            ),
        ))]
    };
    let caller_root = lowered.semantic_module.machines[0].structural_parameters[0].place;
    lowered.semantic_module.machines[0].contract.crash_routes[0].alternatives =
        wrong_route(caller_root);
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut lowered.semantic_module.machines[0].blocks[0].operations[0].kind
    else {
        panic!("root operation is the Unit call")
    };
    crash_continuations[0].alternatives = wrong_route(caller_root);
    let helper_root = lowered.semantic_module.machines[1].structural_parameters[0].place;
    lowered.semantic_module.machines[1].contract.crash_routes[0].alternatives =
        wrong_route(helper_root);

    let result = psi_terminal_verifier::validate_module(&lowered.semantic_module);
    assert!(
        matches!(
            result,
            Err(psi_terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
        ),
        "unexpected verification result: {result:?}"
    );
}

#[test]
fn nested_boolean_member_path_survives_source_call_codec_verification_interpretation_and_fuel() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(NESTED_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("nested Boolean member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_predicate)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one nested member predicate")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            root: route_root,
            path,
        },
    ) = root_predicate.proposition()
    else {
        panic!("nested member route retains a structural Boolean path")
    };
    assert_eq!(*route_root, root.structural_parameters[0].place);
    let [
        CanonicalStructuralPathSegment::Field(outer_field),
        CanonicalStructuralPathSegment::Field(leaf_field),
    ] = path.as_slice()
    else {
        panic!("nested member route retains exactly two canonical field IDs")
    };
    let packet = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Packet type");
    let StructuralTypeShape::Record { fields } = &packet.shape else {
        panic!("Packet is a record")
    };
    let state = fields
        .iter()
        .find(|field| field.id == *outer_field)
        .expect("state field");
    assert_eq!(state.identity, "state");
    let StructuralFieldType::Structural(state_type) = state.field_type else {
        panic!("state field is structural")
    };
    let state = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == state_type)
        .expect("AbortState type");
    let StructuralTypeShape::Record { fields } = &state.shape else {
        panic!("AbortState is a record")
    };
    let leaf = fields
        .iter()
        .find(|field| field.id == *leaf_field)
        .expect("should_abort field");
    assert_eq!(leaf.identity, "should_abort");
    assert_eq!(
        leaf.field_type,
        StructuralFieldType::Scalar(psi_core::ScalarType::Boolean)
    );

    let [helper_route] = helper.contract.crash_routes.as_slice() else {
        panic!("callee publishes one nested member route")
    };
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(crash_continuations, &root.contract.crash_routes);
    assert_ne!(
        root.structural_parameters[0].place,
        helper.structural_parameters[0].place
    );
    assert_ne!(
        root.contract.crash_routes.as_slice(),
        std::slice::from_ref(helper_route)
    );

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier traverses and rebases the exact nested Boolean path");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("nested member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    let argument = TerminalStructuralValue {
        opaque_identity: 9,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("nested member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), 3);
}

#[test]
fn projected_structural_argument_prefix_rebases_member_crash_routes_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(PROJECTED_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected structural member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let envelope = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Envelope type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope is a record")
    };
    let packet_field = fields
        .iter()
        .find(|field| field.identity == "packet")
        .expect("packet field");

    let [CrashRouteGuard::Predicate(root_predicate)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one projected member predicate")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            root: route_root,
            path: caller_path,
        },
    ) = root_predicate.proposition()
    else {
        panic!("caller route retains its structural field path")
    };
    assert_eq!(*route_root, root.structural_parameters[0].place);
    assert_eq!(
        caller_path.first(),
        Some(&CanonicalStructuralPathSegment::Field(packet_field.id))
    );

    let [CrashRouteGuard::Predicate(helper_predicate)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one member predicate")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            path: helper_path, ..
        },
    ) = helper_predicate.proposition()
    else {
        panic!("callee route retains its parameter-relative field path")
    };
    assert_eq!(&caller_path[1..], helper_path);

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(
        structural_arguments[0].path,
        [StructuralPathSegment::Field("packet".into())]
    );
    assert_eq!(crash_continuations, &root.contract.crash_routes);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently composes the argument and callee field paths");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("projected member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    let argument = TerminalStructuralValue {
        opaque_identity: 11,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("projected member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), 3);
}

#[test]
fn composed_boolean_member_predicate_rebases_every_path_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn paths(
        proposition: &Proposition,
    ) -> (
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
    ) {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("composed member predicate is one conjunction")
        };
        let [equality, armed] = conjuncts.as_slice() else {
            panic!("conjunction retains equality then member assertion")
        };
        let Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::BooleanEqual { left, right }) =
            equality
        else {
            panic!("first conjunct retains Boolean equality")
        };
        let ScalarTerm::BooleanField {
            path: left_path, ..
        } = left.as_ref()
        else {
            panic!("equality left operand is a member path")
        };
        let ScalarTerm::BooleanNot { operand } = right.as_ref() else {
            panic!("equality right operand retains negation")
        };
        let ScalarTerm::BooleanField {
            path: right_path, ..
        } = operand.as_ref()
        else {
            panic!("negated operand is a member path")
        };
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::BooleanField {
                path: armed_path, ..
            },
        ) = armed
        else {
            panic!("second conjunct is the armed member assertion")
        };
        (left_path, right_path, armed_path)
    }

    let tokens = Lexer::new(COMPOSED_MEMBER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("composed Boolean member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one composed member route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one composed member route")
    };
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("callee composed route survives the projected call")
    };
    assert_eq!(continuation, root_route);

    let (root_left, root_right, root_armed) = paths(root_route.proposition());
    let (helper_left, helper_right, helper_armed) = paths(helper_route.proposition());
    assert_eq!(&root_left[1..], helper_left);
    assert_eq!(&root_right[1..], helper_right);
    assert_eq!(&root_armed[1..], helper_armed);
    assert_eq!(root_left[0], root_right[0]);
    assert_eq!(root_left[0], root_armed[0]);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently traverses every composed member path");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("composed member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 17,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("composed member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let Proposition::Conjunction(conjuncts) = predicate.proposition() else {
        unreachable!()
    };
    let Some(Proposition::Equal(_, ScalarTerm::BooleanField { path: armed, .. })) =
        conjuncts.iter().find(|conjunct| {
            matches!(
                conjunct,
                Proposition::Equal(_, ScalarTerm::BooleanField { .. })
            )
        })
    else {
        unreachable!()
    };
    let armed = armed.clone();
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::Equal(_, ScalarTerm::BooleanEqual { left, .. })) =
        conjuncts.iter_mut().find(|conjunct| {
            matches!(
                conjunct,
                Proposition::Equal(_, ScalarTerm::BooleanEqual { .. })
            )
        })
    else {
        unreachable!()
    };
    let ScalarTerm::BooleanField { path, .. } = left.as_mut() else {
        unreachable!()
    };
    *path = armed;
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected composed-member validation result: {invalid_result:?}"
    );
}

#[test]
fn integer_member_comparisons_rebase_and_validate_exact_leaf_types_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn paths(
        proposition: &Proposition,
    ) -> (
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
        IntegerType,
    ) {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("integer member route is one conjunction")
        };
        let [nonzero, ordered] = conjuncts.as_slice() else {
            panic!("integer route retains ordering then inequality")
        };
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            },
        ) = ordered
        else {
            panic!("first conjunct retains the ordered member comparison")
        };
        let ScalarTerm::IntegerField {
            path: limit_path,
            scalar_type: limit_type,
            ..
        } = left.as_ref()
        else {
            panic!("ordered left operand is the limit member")
        };
        let ScalarTerm::IntegerField {
            path: current_path,
            scalar_type: current_type,
            ..
        } = right.as_ref()
        else {
            panic!("ordered right operand is the current member")
        };
        assert_eq!(limit_type, scalar_type);
        assert_eq!(current_type, scalar_type);

        let Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::BooleanNot { operand }) =
            nonzero
        else {
            panic!("second conjunct retains integer inequality as negated equality")
        };
        let ScalarTerm::IntegerEqual { left, right, .. } = operand.as_ref() else {
            panic!("inequality retains one integer equality term")
        };
        let ScalarTerm::IntegerField {
            path: nonzero_path, ..
        } = left.as_ref()
        else {
            panic!("inequality left operand is the current member")
        };
        assert!(matches!(right.as_ref(), ScalarTerm::IntegerField { .. }));
        (limit_path, current_path, nonzero_path, *scalar_type)
    }

    let tokens = Lexer::new(INTEGER_MEMBER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("integer member crash comparisons lower");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one integer member route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one integer member route")
    };
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("integer member route survives the projected call")
    };
    assert_eq!(continuation, root_route);

    let (root_limit, root_current, root_nonzero, integer_type) = paths(root_route.proposition());
    let (helper_limit, helper_current, helper_nonzero, helper_type) =
        paths(helper_route.proposition());
    assert_eq!(
        integer_type,
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
    );
    assert_eq!(integer_type, helper_type);
    assert_eq!(root_limit, helper_limit);
    assert_eq!(root_current, helper_current);
    assert_eq!(root_nonzero, helper_nonzero);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently checks every integer member path and leaf type");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("integer member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 19,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("integer member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut mistyped = lowered.semantic_module.clone();
    let CrashRouteGuard::Predicate(predicate) =
        &mut mistyped.machines[1].contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::Equal(
        _,
        ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        },
    )) = conjuncts.iter_mut().find(|conjunct| {
        matches!(
            conjunct,
            Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { .. })
        )
    })
    else {
        unreachable!()
    };
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    *scalar_type = u32_type;
    for operand in [left, right] {
        let ScalarTerm::IntegerField { scalar_type, .. } = operand.as_mut() else {
            unreachable!()
        };
        *scalar_type = u32_type;
    }
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&mistyped);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected integer-member validation result: {invalid_result:?}"
    );
}

#[test]
fn projected_argument_prefix_rebases_every_integer_member_path_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn paths(
        proposition: &Proposition,
    ) -> (
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
        &[CanonicalStructuralPathSegment],
    ) {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("projected integer member route is one conjunction")
        };
        let [inequality, ordered] = conjuncts.as_slice() else {
            panic!("projected integer route retains both comparisons")
        };
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual { left, right, .. },
        ) = ordered
        else {
            panic!("ordered comparison remains terminal")
        };
        let (
            ScalarTerm::IntegerField {
                path: ordered_left, ..
            },
            ScalarTerm::IntegerField {
                path: ordered_right,
                ..
            },
        ) = (left.as_ref(), right.as_ref())
        else {
            panic!("ordered operands remain integer member paths")
        };
        let Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::BooleanNot { operand }) =
            inequality
        else {
            panic!("inequality remains a negated equality")
        };
        let ScalarTerm::IntegerEqual { left, right, .. } = operand.as_ref() else {
            panic!("inequality retains its integer equality")
        };
        let (
            ScalarTerm::IntegerField {
                path: unequal_left, ..
            },
            ScalarTerm::IntegerField {
                path: unequal_right,
                ..
            },
        ) = (left.as_ref(), right.as_ref())
        else {
            panic!("inequality operands remain integer member paths")
        };
        (ordered_left, ordered_right, unequal_left, unequal_right)
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected integer member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let envelope = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Envelope type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope is a record")
    };
    let batch = fields
        .iter()
        .find(|field| field.identity == "batch")
        .expect("batch field");
    let StructuralFieldType::Structural(batch_type) = batch.field_type else {
        panic!("batch has a structural type")
    };
    let batch_declaration = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == batch_type)
        .expect("Batch type");
    let StructuralTypeShape::Record {
        fields: batch_fields,
    } = &batch_declaration.shape
    else {
        panic!("Batch is a record")
    };
    let metrics = batch_fields
        .iter()
        .find(|field| field.identity == "metrics")
        .expect("metrics field");
    let shadow = batch_fields
        .iter()
        .find(|field| field.identity == "shadow")
        .expect("shadow field");

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected structural Unit call")
    };
    assert_eq!(
        structural_arguments[0].path,
        [
            StructuralPathSegment::Field("batch".into()),
            StructuralPathSegment::Field("metrics".into())
        ]
    );
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one integer member route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one integer member route")
    };
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains one guarded crash continuation")
    };
    assert_eq!(continuation, root_route);
    let (root_ordered_left, root_ordered_right, root_unequal_left, root_unequal_right) =
        paths(root_route.proposition());
    let (helper_ordered_left, helper_ordered_right, helper_unequal_left, helper_unequal_right) =
        paths(helper_route.proposition());
    for (caller_path, callee_path) in [
        root_ordered_left,
        root_ordered_right,
        root_unequal_left,
        root_unequal_right,
    ]
    .into_iter()
    .zip([
        helper_ordered_left,
        helper_ordered_right,
        helper_unequal_left,
        helper_unequal_right,
    ]) {
        assert_eq!(
            &caller_path[..2],
            [
                CanonicalStructuralPathSegment::Field(batch.id),
                CanonicalStructuralPathSegment::Field(metrics.id),
            ]
        );
        assert_eq!(&caller_path[2..], callee_path);
    }

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently composes every projected integer member path");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("projected integer route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 23,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("projected integer contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { left, .. })) =
        conjuncts.iter_mut().find(|conjunct| {
            matches!(
                conjunct,
                Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { .. })
            )
        })
    else {
        unreachable!()
    };
    let ScalarTerm::IntegerField { path, .. } = left.as_mut() else {
        unreachable!()
    };
    path[1] = CanonicalStructuralPathSegment::Field(shadow.id);
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected projected integer validation result: {invalid_result:?}"
    );
}

#[test]
fn exact_member_addition_rebases_every_operand_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn addition_fields(
        proposition: &Proposition,
    ) -> (&ScalarTerm, &ScalarTerm, &ScalarTerm, IntegerType) {
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            },
        ) = proposition
        else {
            panic!("member arithmetic route retains its ordered comparison")
        };
        let ScalarTerm::ExactIntegerAdd {
            scalar_type: addition_type,
            left: add_left,
            right: add_right,
        } = left.as_ref()
        else {
            panic!("comparison left operand retains exact addition")
        };
        assert_eq!(addition_type, scalar_type);
        (add_left, add_right, right, *scalar_type)
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_ARITHMETIC_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected exact member addition lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one arithmetic member route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one arithmetic member route")
    };

    let (root_current, root_delta, root_limit, root_type) =
        addition_fields(root_route.proposition());
    let (helper_current, helper_delta, helper_limit, helper_type) =
        addition_fields(helper_route.proposition());
    assert_eq!(
        root_type,
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
    );
    assert_eq!(root_type, helper_type);
    for term in [root_current, root_delta, root_limit] {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("caller arithmetic operand is a typed member path")
        };
        assert_eq!(*field_root, root.structural_parameters[0].place);
        assert_eq!(path.len(), 3);
        assert_eq!(*scalar_type, root_type);
    }
    for term in [helper_current, helper_delta, helper_limit] {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("callee arithmetic operand is a typed member path")
        };
        assert_eq!(*field_root, helper.structural_parameters[0].place);
        assert_eq!(path.len(), 1);
        assert_eq!(*scalar_type, helper_type);
    }

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].path.len(), 2);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the arithmetic member continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently substitutes every exact-add member operand");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("arithmetic member route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 41,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("member arithmetic remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { left, .. }) = &mut proposition
    else {
        unreachable!()
    };
    let ScalarTerm::ExactIntegerAdd { left, right, .. } = left.as_mut() else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: left_path, ..
    } = left.as_mut()
    else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: right_path, ..
    } = right.as_ref()
    else {
        unreachable!()
    };
    *left_path = right_path.clone();
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected arithmetic-member validation result: {invalid_result:?}"
    );
}

#[test]
fn exact_member_subtraction_rebases_every_operand_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn subtraction_fields(
        proposition: &Proposition,
    ) -> (&ScalarTerm, &ScalarTerm, &ScalarTerm, IntegerType) {
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            },
        ) = proposition
        else {
            panic!("member subtraction route retains its ordered comparison")
        };
        let ScalarTerm::ExactIntegerSubtract {
            scalar_type: subtraction_type,
            left: minuend,
            right: subtrahend,
        } = right.as_ref()
        else {
            panic!("comparison right operand retains exact subtraction")
        };
        assert_eq!(subtraction_type, scalar_type);
        (left, minuend, subtrahend, *scalar_type)
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_SUBTRACTION_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected exact member subtraction lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one subtraction member route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one subtraction member route")
    };

    let (root_floor, root_current, root_delta, root_type) =
        subtraction_fields(root_route.proposition());
    let (helper_floor, helper_current, helper_delta, helper_type) =
        subtraction_fields(helper_route.proposition());
    assert_eq!(
        root_type,
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
    );
    assert_eq!(root_type, helper_type);
    for term in [root_floor, root_current, root_delta] {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("caller subtraction operand is a typed member path")
        };
        assert_eq!(*field_root, root.structural_parameters[0].place);
        assert_eq!(path.len(), 3);
        assert_eq!(*scalar_type, root_type);
    }
    for term in [helper_floor, helper_current, helper_delta] {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("callee subtraction operand is a typed member path")
        };
        assert_eq!(*field_root, helper.structural_parameters[0].place);
        assert_eq!(path.len(), 1);
        assert_eq!(*scalar_type, helper_type);
    }

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].path.len(), 2);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the subtraction member continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently substitutes every exact-subtract member operand");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("subtraction member route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 43,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("member subtraction remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { right, .. }) = &mut proposition
    else {
        unreachable!()
    };
    let ScalarTerm::ExactIntegerSubtract { left, right, .. } = right.as_mut() else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: left_path, ..
    } = left.as_mut()
    else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: right_path, ..
    } = right.as_ref()
    else {
        unreachable!()
    };
    *left_path = right_path.clone();
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected subtraction-member validation result: {invalid_result:?}"
    );
}

#[test]
fn exact_member_multiplication_rebases_every_operand_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn multiplication_fields(proposition: &Proposition) -> [&ScalarTerm; 3] {
        let Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            },
        ) = proposition
        else {
            panic!("member multiplication route retains its ordered comparison")
        };
        let ScalarTerm::ExactIntegerMultiply {
            scalar_type: multiplication_type,
            left: multiplicand,
            right: multiplier,
        } = left.as_ref()
        else {
            panic!("comparison left operand retains exact multiplication")
        };
        assert_eq!(multiplication_type, scalar_type);
        [multiplicand, multiplier, right]
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_MULTIPLICATION_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected exact member multiplication lowers");

    let root = &lowered.semantic_module.machines[0];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one multiplication member route")
    };
    for term in multiplication_fields(root_route.proposition()) {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("caller multiplication operand is a typed member path")
        };
        assert_eq!(*field_root, root.structural_parameters[0].place);
        assert_eq!(path.len(), 3);
        assert_eq!(
            *scalar_type,
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
        );
    }

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].path.len(), 2);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the multiplication member continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently substitutes every exact-multiply member operand");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("multiplication member route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 47,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("member multiplication remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { left, .. }) = &mut proposition
    else {
        unreachable!()
    };
    let ScalarTerm::ExactIntegerMultiply { left, right, .. } = left.as_mut() else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: left_path, ..
    } = left.as_mut()
    else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        path: right_path, ..
    } = right.as_ref()
    else {
        unreachable!()
    };
    *left_path = right_path.clone();
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected multiplication-member validation result: {invalid_result:?}"
    );
}

#[test]
fn exact_member_division_and_remainder_rebase_safe_literals_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn arithmetic_fields(proposition: &Proposition) -> [&ScalarTerm; 3] {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("division and remainder route is one conjunction")
        };
        let mut division = None;
        let mut remainder = None;
        let mut parity = None;
        for conjunct in conjuncts {
            let Proposition::Equal(ScalarTerm::Boolean(true), predicate) = conjunct else {
                continue;
            };
            match predicate {
                ScalarTerm::IntegerLessOrEqual { left, right, .. } => {
                    let ScalarTerm::ExactIntegerDivide {
                        left: dividend,
                        right: divisor,
                        ..
                    } = left.as_ref()
                    else {
                        continue;
                    };
                    assert!(matches!(
                        divisor.as_ref(),
                        ScalarTerm::Integer {
                            value: psi_core::IntegerValue::Unsigned(2),
                            ..
                        }
                    ));
                    division = Some(dividend.as_ref());
                    assert!(matches!(right.as_ref(), ScalarTerm::IntegerField { .. }));
                }
                ScalarTerm::IntegerEqual { left, right, .. } => {
                    let ScalarTerm::ExactIntegerRemainder {
                        left: dividend,
                        right: divisor,
                        ..
                    } = left.as_ref()
                    else {
                        continue;
                    };
                    assert!(matches!(
                        divisor.as_ref(),
                        ScalarTerm::Integer {
                            value: psi_core::IntegerValue::Unsigned(2),
                            ..
                        }
                    ));
                    remainder = Some(dividend.as_ref());
                    parity = Some(right.as_ref());
                }
                _ => {}
            }
        }
        [
            division.expect("division member"),
            remainder.expect("remainder member"),
            parity.expect("remainder comparison member"),
        ]
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_DIVISION_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected exact member division and remainder lower");

    let root = &lowered.semantic_module.machines[0];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one division/remainder member route")
    };
    for term in arithmetic_fields(root_route.proposition()) {
        let ScalarTerm::IntegerField {
            root: field_root,
            path,
            scalar_type,
        } = term
        else {
            panic!("caller division operand is a typed member path")
        };
        assert_eq!(*field_root, root.structural_parameters[0].place);
        assert_eq!(path.len(), 3);
        assert_eq!(
            *scalar_type,
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
        );
    }

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].path.len(), 2);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the division/remainder member continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently substitutes division/remainder member operands");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("division/remainder member route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 53,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("member division remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut unsafe_divisor =
        psi_checked_trees_to_terminal::lower_machine(&checked, "Helper::inspect")
            .expect("standalone helper division lowers")
            .semantic_module;
    let CrashRouteGuard::Predicate(predicate) =
        &mut unsafe_divisor.machines[0].contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(ScalarTerm::ExactIntegerDivide {
        scalar_type, right, ..
    }) = conjuncts.iter_mut().find_map(|conjunct| {
        let Proposition::Equal(_, ScalarTerm::IntegerLessOrEqual { left, .. }) = conjunct else {
            return None;
        };
        Some(left.as_mut())
    })
    else {
        unreachable!()
    };
    **right = ScalarTerm::integer(*scalar_type, psi_core::IntegerValue::Unsigned(0)).unwrap();
    *predicate = CrashPredicateTerm::new(proposition);
    let unsafe_result = psi_terminal_verifier::validate_module(&unsafe_divisor);
    assert!(
        matches!(
            unsafe_result,
            Err(psi_terminal_verifier::ModuleError::UnsafeStructuralCrashExactDivisor { .. })
        ),
        "unexpected unsafe-divisor validation result: {unsafe_result:?}"
    );

    let tokens = Lexer::new(RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE)
        .tokenize()
        .expect("runtime-divisor tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("runtime-divisor parse");
    let resolved = lower_syntax_trees(&syntax).expect("runtime-divisor resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("runtime-divisor type");
    let checked = lower_typed_trees(typed).expect("runtime-divisor check");
    let runtime_divisor = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("a positive runtime-divisor requirement is explicit terminal safety evidence");
    let runtime_machine = &runtime_divisor.semantic_module.machines[0];
    assert_eq!(runtime_machine.contract.requires.len(), 1);
    assert!(matches!(
        &runtime_machine.contract.requires[0],
        Proposition::LessOrEqual(
            ScalarTerm::Integer {
                value: psi_core::IntegerValue::Unsigned(1),
                ..
            },
            ScalarTerm::IntegerField { .. }
        )
    ));
    psi_terminal_verifier::verify_module(
        &runtime_divisor.semantic_module,
        &runtime_divisor.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the verifier independently accepts the runtime-divisor requirement");
    let encoded = encode_module(&runtime_divisor.semantic_module)
        .expect("runtime-divisor semantic module encodes");
    assert_eq!(
        decode_module(&encoded),
        Ok(runtime_divisor.semantic_module.clone()),
        "the exact runtime safety requirement survives canonical encoding"
    );

    let mut missing_requirement = runtime_divisor.semantic_module.clone();
    missing_requirement.machines[0].contract.requires.clear();
    assert!(matches!(
        psi_terminal_verifier::validate_module(&missing_requirement),
        Err(psi_terminal_verifier::ModuleError::UnsafeStructuralCrashExactDivisor { .. })
    ));

    let mut redirected_requirement = runtime_divisor.semantic_module.clone();
    let StructuralTypeShape::Record { fields } = &redirected_requirement.structural_types[0].shape
    else {
        unreachable!()
    };
    let limit = fields[2].id;
    let Proposition::LessOrEqual(_, ScalarTerm::IntegerField { path, .. }) =
        &mut redirected_requirement.machines[0].contract.requires[0]
    else {
        unreachable!()
    };
    *path = vec![CanonicalStructuralPathSegment::Field(limit)];
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected_requirement),
        Err(psi_terminal_verifier::ModuleError::UnsafeStructuralCrashExactDivisor { .. })
    ));

    let tokens = Lexer::new(UNPROVEN_RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE)
        .tokenize()
        .expect("unproven-runtime-divisor tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("unproven-runtime-divisor parse");
    let resolved = lower_syntax_trees(&syntax).expect("unproven-runtime-divisor resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("unproven-runtime-divisor type");
    let diagnostics =
        lower_typed_trees(typed).expect_err("an unproven runtime divisor must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("divisor must be proven nonzero")),
        "unexpected unproven-runtime-divisor diagnostics: {diagnostics:?}"
    );
}

#[test]
fn bitwise_member_terms_rebase_across_projected_calls_and_codecs() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn inspect_term<'a>(
        term: &'a ScalarTerm,
        bitwise_counts: &mut [usize; 4],
        paths: &mut Vec<&'a [CanonicalStructuralPathSegment]>,
    ) {
        match term {
            ScalarTerm::IntegerField { path, .. } => paths.push(path),
            ScalarTerm::BooleanNot { operand } => inspect_term(operand, bitwise_counts, paths),
            ScalarTerm::IntegerBitwiseNot { operand, .. } => {
                bitwise_counts[3] += 1;
                inspect_term(operand, bitwise_counts, paths);
            }
            ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
                match term {
                    ScalarTerm::IntegerBitwiseAnd { .. } => bitwise_counts[0] += 1,
                    ScalarTerm::IntegerBitwiseOr { .. } => bitwise_counts[1] += 1,
                    ScalarTerm::IntegerBitwiseXor { .. } => bitwise_counts[2] += 1,
                    _ => unreachable!(),
                }
                inspect_term(left, bitwise_counts, paths);
                inspect_term(right, bitwise_counts, paths);
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. } => {
                inspect_term(left, bitwise_counts, paths);
                inspect_term(right, bitwise_counts, paths);
            }
            _ => {}
        }
    }

    fn inspect_proposition(
        proposition: &Proposition,
    ) -> ([usize; 4], Vec<&[CanonicalStructuralPathSegment]>) {
        fn inspect<'a>(
            proposition: &'a Proposition,
            bitwise_counts: &mut [usize; 4],
            paths: &mut Vec<&'a [CanonicalStructuralPathSegment]>,
        ) {
            match proposition {
                Proposition::Equal(left, right)
                | Proposition::LessThan(left, right)
                | Proposition::LessOrEqual(left, right) => {
                    inspect_term(left, bitwise_counts, paths);
                    inspect_term(right, bitwise_counts, paths);
                }
                Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                    for proposition in propositions {
                        inspect(proposition, bitwise_counts, paths);
                    }
                }
                _ => {}
            }
        }

        let mut bitwise_counts = [0; 4];
        let mut paths = Vec::new();
        inspect(proposition, &mut bitwise_counts, &mut paths);
        (bitwise_counts, paths)
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_BITWISE_SOURCE)
        .tokenize()
        .expect("bitwise tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("bitwise parse");
    let resolved = lower_syntax_trees(&syntax).expect("bitwise resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("bitwise type");
    let checked = lower_typed_trees(typed).expect("bitwise check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected bitwise member predicates lower");
    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one bitwise route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one bitwise route")
    };
    let (root_counts, root_paths) = inspect_proposition(root_route.proposition());
    let (helper_counts, helper_paths) = inspect_proposition(helper_route.proposition());
    assert_eq!(root_counts, [1, 1, 1, 1]);
    assert_eq!(helper_counts, root_counts);
    assert_eq!(root_paths.len(), helper_paths.len());

    let envelope = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Envelope type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope is a record")
    };
    let bits = fields
        .iter()
        .find(|field| field.identity == "bits")
        .expect("bits field");
    assert!(
        root_paths
            .iter()
            .all(|path| { path.first() == Some(&CanonicalStructuralPathSegment::Field(bits.id)) })
    );
    assert!(helper_paths.iter().all(|path| path.len() == 1));

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected Unit call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [StructuralPathSegment::Field(identity)] if identity == "bits"
    ));
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call carries one bitwise crash continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the verifier independently rebases every nested bitwise member");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("bitwise member route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("bitwise fixed fuel recomputes");
    let semantics = encode_module(&lowered.semantic_module).expect("bitwise semantics encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("bitwise proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 59,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("bitwise crash predicates remain verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("spare".to_owned())];
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected),
        Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn total_policy_arithmetic_rebases_across_projected_calls_and_codecs() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn inspect_policy_terms(
        proposition: &Proposition,
    ) -> ([usize; 6], Vec<&[CanonicalStructuralPathSegment]>) {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("policy arithmetic route is one conjunction")
        };
        let mut counts = [0; 6];
        let mut paths = Vec::new();
        for conjunct in conjuncts {
            let Proposition::Equal(
                ScalarTerm::Boolean(true),
                ScalarTerm::IntegerEqual { left, right, .. },
            ) = conjunct
            else {
                panic!("each policy arithmetic clause remains an integer equality")
            };
            let (index, operation_left, operation_right) = match left.as_ref() {
                ScalarTerm::WrappingIntegerAdd { left, right, .. } => (0, left, right),
                ScalarTerm::WrappingIntegerSubtract { left, right, .. } => (1, left, right),
                ScalarTerm::WrappingIntegerMultiply { left, right, .. } => (2, left, right),
                ScalarTerm::SaturatingIntegerAdd { left, right, .. } => (3, left, right),
                ScalarTerm::SaturatingIntegerSubtract { left, right, .. } => (4, left, right),
                ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => (5, left, right),
                _ => panic!("unexpected policy arithmetic term"),
            };
            counts[index] += 1;
            for term in [
                operation_left.as_ref(),
                operation_right.as_ref(),
                right.as_ref(),
            ] {
                let ScalarTerm::IntegerField { path, .. } = term else {
                    panic!("policy arithmetic operand remains a typed member path")
                };
                paths.push(path.as_slice());
            }
        }
        (counts, paths)
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_POLICY_ARITHMETIC_SOURCE)
        .tokenize()
        .expect("policy arithmetic tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("policy arithmetic parse");
    let resolved = lower_syntax_trees(&syntax).expect("policy arithmetic resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("policy arithmetic type");
    let checked = lower_typed_trees(typed).expect("policy arithmetic check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected wrapping and saturating member arithmetic lowers");
    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one policy arithmetic route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one policy arithmetic route")
    };
    let (root_counts, root_paths) = inspect_policy_terms(root_route.proposition());
    let (helper_counts, helper_paths) = inspect_policy_terms(helper_route.proposition());
    assert_eq!(root_counts, [1; 6]);
    assert_eq!(helper_counts, root_counts);
    assert_eq!(root_paths.len(), 18);
    assert_eq!(helper_paths.len(), root_paths.len());

    let envelope = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Envelope type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope is a record")
    };
    let values = fields
        .iter()
        .find(|field| field.identity == "values")
        .expect("values field");
    assert!(
        root_paths.iter().all(|path| {
            path.first() == Some(&CanonicalStructuralPathSegment::Field(values.id))
        })
    );
    assert!(helper_paths.iter().all(|path| path.len() == 1));

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected Unit call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [StructuralPathSegment::Field(identity)] if identity == "values"
    ));
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call carries one policy arithmetic continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently rebases every policy arithmetic operand");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("policy arithmetic route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("policy arithmetic fixed fuel recomputes");
    let semantics = encode_module(&lowered.semantic_module).expect("policy semantics encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("policy proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 67,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("policy arithmetic predicates remain verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("spare".to_owned())];
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected),
        Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn wrapping_shifts_rebase_distinct_count_carriers_across_projected_calls() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn inspect_shift_terms(proposition: &Proposition) -> ([usize; 2], Vec<usize>) {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("wrapping shift route is one conjunction")
        };
        let value_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let count_type = IntegerType::new(IntegerSign::Signed, 16).unwrap();
        let mut counts = [0; 2];
        let mut path_lengths = Vec::new();
        for conjunct in conjuncts {
            let Proposition::Equal(
                ScalarTerm::Boolean(true),
                ScalarTerm::IntegerEqual { left, right, .. },
            ) = conjunct
            else {
                panic!("each wrapping shift clause remains an integer equality")
            };
            let (index, value, count) = match left.as_ref() {
                ScalarTerm::WrappingIntegerShiftLeft {
                    value_type: actual_value,
                    count_type: actual_count,
                    value,
                    count,
                } => {
                    assert_eq!((*actual_value, *actual_count), (value_type, count_type));
                    (0, value, count)
                }
                ScalarTerm::WrappingIntegerShiftRight {
                    value_type: actual_value,
                    count_type: actual_count,
                    value,
                    count,
                } => {
                    assert_eq!((*actual_value, *actual_count), (value_type, count_type));
                    (1, value, count)
                }
                _ => panic!("unexpected wrapping shift term"),
            };
            counts[index] += 1;
            for term in [value.as_ref(), count.as_ref(), right.as_ref()] {
                let ScalarTerm::IntegerField { path, .. } = term else {
                    panic!("wrapping shift operand remains a typed member path")
                };
                path_lengths.push(path.len());
            }
        }
        (counts, path_lengths)
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_WRAPPING_SHIFT_SOURCE)
        .tokenize()
        .expect("wrapping shifts tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("wrapping shifts parse");
    let resolved = lower_syntax_trees(&syntax).expect("wrapping shifts resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("wrapping shifts type");
    let checked = lower_typed_trees(typed).expect("wrapping shifts check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected wrapping shifts lower without count requirements");
    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    assert!(root.contract.requires.is_empty());
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one wrapping shift route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one wrapping shift route")
    };
    let (root_counts, root_path_lengths) = inspect_shift_terms(root_route.proposition());
    let (helper_counts, helper_path_lengths) = inspect_shift_terms(helper_route.proposition());
    assert_eq!(root_counts, [1; 2]);
    assert_eq!(helper_counts, root_counts);
    assert!(root_path_lengths.iter().all(|length| *length == 2));
    assert!(helper_path_lengths.iter().all(|length| *length == 1));

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected wrapping shift call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [StructuralPathSegment::Field(identity)] if identity == "values"
    ));
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call carries one wrapping shift continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently rebases wrapping shift value and count paths");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("wrapping shift route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("wrapping shift fixed fuel recomputes");
    let semantics =
        encode_module(&lowered.semantic_module).expect("wrapping shift semantics encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("wrapping shift proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 73,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("wrapping shifts remain verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut forged_exact =
        psi_checked_trees_to_terminal::lower_machine(&checked, "Helper::inspect")
            .expect("standalone wrapping shift helper lowers")
            .semantic_module;
    let CrashRouteGuard::Predicate(predicate) =
        &mut forged_exact.machines[0].contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(ScalarTerm::WrappingIntegerShiftLeft {
        value_type,
        count_type,
        value,
        count,
    }) = conjuncts.iter_mut().find_map(|conjunct| {
        let Proposition::Equal(_, ScalarTerm::IntegerEqual { left, .. }) = conjunct else {
            return None;
        };
        matches!(left.as_ref(), ScalarTerm::WrappingIntegerShiftLeft { .. })
            .then_some(left.as_mut())
    })
    else {
        unreachable!()
    };
    let exact = ScalarTerm::ExactIntegerShiftLeft {
        value_type: *value_type,
        count_type: *count_type,
        value: value.clone(),
        count: count.clone(),
    };
    **conjuncts
        .iter_mut()
        .find_map(|conjunct| {
            let Proposition::Equal(_, ScalarTerm::IntegerEqual { left, .. }) = conjunct else {
                return None;
            };
            matches!(left.as_ref(), ScalarTerm::WrappingIntegerShiftLeft { .. }).then_some(left)
        })
        .expect("wrapping shift term") = exact;
    *predicate = CrashPredicateTerm::new(proposition);
    assert!(matches!(
        psi_terminal_verifier::validate_module(&forged_exact),
        Err(psi_terminal_verifier::ModuleError::UnsafeStructuralCrashExactShift { .. })
    ));

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("spare".to_owned())];
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected),
        Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn exact_shifts_rebase_complete_count_and_overflow_requirements() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_EXACT_SHIFT_SOURCE)
        .tokenize()
        .expect("Exact shifts tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("Exact shifts parse");
    let resolved = lower_syntax_trees(&syntax).expect("Exact shifts resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("Exact shifts type");
    let checked = lower_typed_trees(typed).expect("Exact shifts check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected Exact shifts retain complete safety requirements");
    let root = &lowered.semantic_module.machines[0];
    assert_eq!(root.contract.requires.len(), 3);
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one Exact shift route")
    };
    let Proposition::Conjunction(conjuncts) = root_route.proposition() else {
        panic!("Exact shift route is one conjunction")
    };
    let mut counts = [0; 2];
    for conjunct in conjuncts {
        let Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::IntegerEqual { left, .. }) =
            conjunct
        else {
            panic!("each Exact shift clause remains an integer equality")
        };
        match left.as_ref() {
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                ..
            } => {
                assert_eq!(value_type.bits(), 8);
                assert_eq!(count_type.bits(), 16);
                counts[0] += 1;
            }
            ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                ..
            } => {
                assert_eq!(value_type.bits(), 8);
                assert_eq!(count_type.bits(), 16);
                counts[1] += 1;
            }
            _ => panic!("unexpected Exact shift term"),
        }
    }
    assert_eq!(counts, [1; 2]);

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        requirement_obligations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected Exact shift call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [StructuralPathSegment::Field(identity)] if identity == "values"
    ));
    assert_eq!(requirement_obligations.len(), 3);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call carries one Exact shift continuation")
    };
    assert_eq!(continuation, root_route);
    let reconstructed =
        psi_terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("verifier reconstructs projected Exact shift requirements");
    assert_eq!(reconstructed.len(), 3);
    assert!(reconstructed.iter().all(|item| {
        root.contract
            .requires
            .contains(&item.obligation.proposition)
    }));

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently checks Exact shift count and overflow bounds");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("Exact shift route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("Exact shift fixed fuel recomputes");
    let semantics = encode_module(&lowered.semantic_module).expect("Exact shift semantics encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("Exact shift proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 79,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("Exact shifts remain verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut missing_requirements = lowered.semantic_module.clone();
    missing_requirements.machines[0].contract.requires.clear();
    assert!(matches!(
        psi_terminal_verifier::validate_module(&missing_requirements),
        Err(psi_terminal_verifier::ModuleError::UnsafeStructuralCrashExactShift { .. })
    ));

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("spare".to_owned())];
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected),
        Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn policy_division_rebases_nonzero_requirements_across_projected_calls() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(PROJECTED_INTEGER_MEMBER_POLICY_DIVISION_SOURCE)
        .tokenize()
        .expect("policy division tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("policy division parse");
    let resolved = lower_syntax_trees(&syntax).expect("policy division resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("policy division type");
    let checked = lower_typed_trees(typed).expect("policy division check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("projected policy division retains exact nonzero requirements");
    let root = &lowered.semantic_module.machines[0];
    assert_eq!(root.contract.requires.len(), 2);
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one policy division route")
    };
    let Proposition::Conjunction(conjuncts) = root_route.proposition() else {
        panic!("policy division route is one conjunction")
    };
    let mut counts = [0; 4];
    for conjunct in conjuncts {
        let Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::IntegerEqual { left, .. }) =
            conjunct
        else {
            panic!("each policy division clause remains an integer equality")
        };
        match left.as_ref() {
            ScalarTerm::WrappingIntegerDivide { .. } => counts[0] += 1,
            ScalarTerm::WrappingIntegerRemainder { .. } => counts[1] += 1,
            ScalarTerm::SaturatingIntegerDivide { .. } => counts[2] += 1,
            ScalarTerm::SaturatingIntegerRemainder { .. } => counts[3] += 1,
            _ => panic!("unexpected policy division term"),
        }
    }
    assert_eq!(counts, [1; 4]);

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        requirement_obligations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected policy division call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [StructuralPathSegment::Field(identity)] if identity == "values"
    ));
    assert_eq!(requirement_obligations.len(), 2);
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call carries one policy division continuation")
    };
    assert_eq!(continuation, root_route);
    let reconstructed =
        psi_terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("verifier reconstructs projected policy divisor requirements");
    assert_eq!(reconstructed.len(), 2);
    assert!(reconstructed.iter().all(|item| {
        root.contract
            .requires
            .contains(&item.obligation.proposition)
    }));

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts both independently safe policy divisors");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("policy division route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("policy division fixed fuel recomputes");
    let semantics = encode_module(&lowered.semantic_module).expect("policy division encodes");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("policy proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 71,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("policy division predicates remain verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut missing_requirement = lowered.semantic_module.clone();
    missing_requirement.machines[0].contract.requires.clear();
    assert!(matches!(
        psi_terminal_verifier::validate_module(&missing_requirement),
        Err(psi_terminal_verifier::ModuleError::UnsafeStructuralCrashPolicyDivisor { .. })
    ));

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("spare".to_owned())];
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected),
        Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn wrapping_negative_one_literal_divisor_is_self_proving() {
    let tokens = Lexer::new(POLICY_NEGATIVE_ONE_LITERAL_DIVISION_SOURCE)
        .tokenize()
        .expect("negative-one policy division tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("negative-one policy division parse");
    let resolved = lower_syntax_trees(&syntax).expect("negative-one policy division resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("negative-one policy division type");
    let checked = lower_typed_trees(typed).expect("negative-one policy division check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("Wrapping defines signed MIN divided or remaindered by negative one");
    assert!(
        lowered.semantic_module.machines[0]
            .contract
            .requires
            .is_empty()
    );
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("negative one is independently nonzero under Wrapping");

    let mut zero = lowered.semantic_module.clone();
    let CrashRouteGuard::Predicate(predicate) =
        &mut zero.machines[0].contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(ScalarTerm::WrappingIntegerDivide {
        scalar_type, right, ..
    }) = conjuncts.iter_mut().find_map(|conjunct| {
        let Proposition::Equal(_, ScalarTerm::IntegerEqual { left, .. }) = conjunct else {
            return None;
        };
        matches!(left.as_ref(), ScalarTerm::WrappingIntegerDivide { .. }).then_some(left.as_mut())
    })
    else {
        unreachable!()
    };
    **right = ScalarTerm::integer(*scalar_type, psi_core::IntegerValue::Signed(0)).unwrap();
    *predicate = CrashPredicateTerm::new(proposition);
    assert!(matches!(
        psi_terminal_verifier::validate_module(&zero),
        Err(psi_terminal_verifier::ModuleError::UnsafeStructuralCrashPolicyDivisor { .. })
    ));
}

#[test]
fn signed_runtime_member_divisor_requires_an_overflow_safe_bound() {
    let tokens = Lexer::new(NEGATIVE_RUNTIME_INTEGER_MEMBER_DIVISOR_SOURCE)
        .tokenize()
        .expect("negative-runtime-divisor tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("negative-runtime-divisor parse");
    let resolved = lower_syntax_trees(&syntax).expect("negative-runtime-divisor resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("negative-runtime-divisor type");
    let checked = lower_typed_trees(typed).expect("negative-runtime-divisor check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("a divisor bounded at or below negative two is total for every dividend");
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the verifier independently accepts the negative runtime-divisor bound");

    let mut overflow_permitting = lowered.semantic_module.clone();
    let Proposition::LessOrEqual(_, ScalarTerm::Integer { value, .. }) =
        &mut overflow_permitting.machines[0].contract.requires[0]
    else {
        unreachable!()
    };
    *value = psi_core::IntegerValue::Signed(-1);
    assert!(matches!(
        psi_terminal_verifier::validate_module(&overflow_permitting),
        Err(psi_terminal_verifier::ModuleError::UnsafeStructuralCrashExactDivisor { .. })
    ));
}

#[test]
fn runtime_divisor_call_requirements_rebase_and_verify_exact_obligations() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(RUNTIME_DIVISOR_CALL_SOURCE)
        .tokenize()
        .expect("runtime-divisor-call tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("runtime-divisor-call parse");
    let resolved = lower_syntax_trees(&syntax).expect("runtime-divisor-call resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("runtime-divisor-call type");
    let checked = lower_typed_trees(typed).expect("runtime-divisor-call check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("a whole-root Unit call carries its exact runtime-divisor requirement");
    let root = &lowered.semantic_module.machines[0];
    let OperationKind::CallUnit {
        requirement_obligations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("root emits one structural Unit call")
    };
    let [obligation] = requirement_obligations.as_slice() else {
        panic!("the call owns one exact requirement obligation")
    };
    assert_eq!(lowered.proof_bundle.evidence.len(), 1);
    assert_eq!(lowered.proof_bundle.evidence[0].obligation, *obligation);
    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the verifier independently rebases and proves the call requirement");
    assert_eq!(verified.accepted_facts().len(), 1);

    let semantics = encode_module(&lowered.semantic_module).expect("call semantics encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("call proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 61,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("verified runtime-divisor call executes as erased proof metadata");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);

    let mut missing = lowered.proof_bundle.clone();
    missing.evidence.clear();
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &missing,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(id)) if id == *obligation
    ));

    let mut wrong_assumption = lowered.proof_bundle.clone();
    let EvidenceRoute::CertificateDerived(certificate) = &mut wrong_assumption.evidence[0].route
    else {
        unreachable!()
    };
    certificate.proof.rule = ProofRule::Assumption { index: 1 };
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &wrong_assumption,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { .. })
    ));
}

#[test]
fn projected_runtime_divisor_call_rebases_requirement_through_canonical_prefix() {
    let tokens = Lexer::new(PROJECTED_RUNTIME_DIVISOR_CALL_SOURCE)
        .tokenize()
        .expect("projected-runtime-divisor-call tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("projected-runtime-divisor-call parse");
    let resolved = lower_syntax_trees(&syntax).expect("projected-runtime-divisor-call resolve");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("projected-runtime-divisor-call type");
    let checked = lower_typed_trees(typed).expect("projected-runtime-divisor-call check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("a projected Unit call rebases its runtime-divisor requirement");
    let root = &lowered.semantic_module.machines[0];
    let OperationKind::CallUnit {
        structural_arguments,
        requirement_obligations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("root emits one projected structural Unit call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [StructuralPathSegment::Field(identity)] if identity == "metrics"
    ));
    assert_eq!(requirement_obligations.len(), 1);
    let reconstructed =
        psi_terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("the verifier reconstructs the projected call obligation");
    assert_eq!(reconstructed.len(), 1);
    assert_eq!(
        reconstructed[0].obligation.proposition, root.contract.requires[0],
        "the canonical argument prefix rebases the callee premise to the caller path"
    );
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the projected call proof cites the exact rebased caller assumption");

    let semantics = encode_module(&lowered.semantic_module).expect("projected call encodes");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("projected proof encodes");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );

    let mut wrong_prefix = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut wrong_prefix.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("decoy".to_owned())];
    assert!(matches!(
        psi_terminal_verifier::validate_module(&wrong_prefix),
        Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn proposition_disjunction_rebases_and_verifies_each_member_path_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn field_paths(proposition: &Proposition) -> Vec<&[CanonicalStructuralPathSegment]> {
        fn collect_term<'a>(
            term: &'a ScalarTerm,
            paths: &mut Vec<&'a [CanonicalStructuralPathSegment]>,
        ) {
            match term {
                ScalarTerm::BooleanField { path, .. } => paths.push(path),
                ScalarTerm::BooleanNot { operand } => collect_term(operand, paths),
                _ => {}
            }
        }
        fn collect<'a>(
            proposition: &'a Proposition,
            paths: &mut Vec<&'a [CanonicalStructuralPathSegment]>,
        ) {
            match proposition {
                Proposition::Equal(left, right) => {
                    collect_term(left, paths);
                    collect_term(right, paths);
                }
                Proposition::Disjunction(disjuncts) => {
                    for disjunct in disjuncts {
                        collect(disjunct, paths);
                    }
                }
                _ => {}
            }
        }
        let mut paths = Vec::new();
        collect(proposition, &mut paths);
        paths
    }

    let tokens = Lexer::new(DISJUNCTIVE_MEMBER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("disjunctive projected member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let envelope = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Envelope type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope is a record")
    };
    let pair = fields
        .iter()
        .find(|field| field.identity == "pair")
        .expect("pair field");
    let StructuralFieldType::Structural(pair_type) = pair.field_type else {
        panic!("pair has a structural type")
    };
    let pair_declaration = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == pair_type)
        .expect("Pair type");
    let StructuralTypeShape::Record {
        fields: pair_fields,
    } = &pair_declaration.shape
    else {
        panic!("Pair is a record")
    };
    let decoy = pair_fields
        .iter()
        .find(|field| field.identity == "decoy")
        .expect("decoy field");

    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one disjunctive route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one disjunctive route")
    };
    let Proposition::Disjunction(root_disjuncts) = root_route.proposition() else {
        panic!("caller retains terminal proposition disjunction")
    };
    assert_eq!(root_disjuncts.len(), 2);
    let Proposition::Disjunction(helper_disjuncts) = helper_route.proposition() else {
        panic!("callee retains terminal proposition disjunction")
    };
    assert_eq!(helper_disjuncts.len(), 2);

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one projected structural Unit call")
    };
    assert_eq!(
        structural_arguments[0].path,
        [StructuralPathSegment::Field("pair".into())]
    );
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains one disjunctive continuation")
    };
    assert_eq!(continuation, root_route);
    let root_paths = field_paths(root_route.proposition());
    let helper_paths = field_paths(helper_route.proposition());
    assert_eq!(root_paths.len(), 2);
    assert_eq!(helper_paths.len(), 2);
    for root_path in root_paths {
        assert_eq!(
            root_path.first(),
            Some(&CanonicalStructuralPathSegment::Field(pair.id))
        );
        assert!(helper_paths.contains(&&root_path[1..]));
    }

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently reconstructs the disjunctive continuation");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("disjunctive route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let argument = TerminalStructuralValue {
        opaque_identity: 29,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("disjunctive member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Disjunction(disjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::Equal(_, ScalarTerm::BooleanField { path, .. })) =
        disjuncts.iter_mut().find(|disjunct| {
            matches!(
                disjunct,
                Proposition::Equal(_, ScalarTerm::BooleanField { .. })
            )
        })
    else {
        unreachable!()
    };
    path[1] = CanonicalStructuralPathSegment::Field(decoy.id);
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected disjunctive validation result: {invalid_result:?}"
    );
}

#[test]
fn whole_aggregate_equality_expands_and_reconstructs_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn field_roots(proposition: &Proposition) -> Vec<(psi_core::PlaceId, psi_core::PlaceId)> {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("aggregate equality is one flat conjunction")
        };
        let mut roots = Vec::new();
        for conjunct in conjuncts {
            let Proposition::Equal(ScalarTerm::Boolean(true), term) = conjunct else {
                panic!("aggregate field compare is asserted true")
            };
            match term {
                ScalarTerm::BooleanEqual { left, right } => {
                    let (
                        ScalarTerm::BooleanField {
                            root: left_root, ..
                        },
                        ScalarTerm::BooleanField {
                            root: right_root, ..
                        },
                    ) = (left.as_ref(), right.as_ref())
                    else {
                        panic!("Boolean aggregate fields retain paths")
                    };
                    roots.push((*left_root, *right_root));
                }
                ScalarTerm::IntegerEqual { left, right, .. } => {
                    let (
                        ScalarTerm::IntegerField {
                            root: left_root, ..
                        },
                        ScalarTerm::IntegerField {
                            root: right_root, ..
                        },
                    ) = (left.as_ref(), right.as_ref())
                    else {
                        panic!("integer aggregate fields retain paths")
                    };
                    roots.push((*left_root, *right_root));
                }
                _ => panic!("aggregate equality uses only member equality terms"),
            }
        }
        roots
    }

    let tokens = Lexer::new(WHOLE_AGGREGATE_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("whole aggregate equality lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one aggregate equality route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one aggregate equality route")
    };
    let root_field_roots = field_roots(root_route.proposition());
    let helper_field_roots = field_roots(helper_route.proposition());
    assert_eq!(root_field_roots.len(), 3);
    assert!(root_field_roots.iter().all(|roots| {
        *roots
            == (
                root.structural_parameters[0].place,
                root.structural_parameters[1].place,
            )
    }));
    assert_eq!(helper_field_roots.len(), 3);
    assert!(helper_field_roots.iter().all(|roots| {
        *roots
            == (
                helper.structural_parameters[0].place,
                helper.structural_parameters[1].place,
            )
    }));

    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(structural_arguments.len(), 2);
    assert!(
        structural_arguments
            .iter()
            .all(|argument| argument.path.is_empty())
    );
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains aggregate equality continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently substitutes both aggregate roots");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("aggregate equality route has an acyclic fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let arguments = root
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 31 + u64::try_from(index).unwrap(),
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
        &mut Accept,
    )
    .expect("aggregate equality remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::Equal(_, ScalarTerm::IntegerEqual { right, .. })) =
        conjuncts.iter_mut().find(|conjunct| {
            matches!(
                conjunct,
                Proposition::Equal(_, ScalarTerm::IntegerEqual { .. })
            )
        })
    else {
        unreachable!()
    };
    let ScalarTerm::IntegerField {
        root: right_root, ..
    } = right.as_mut()
    else {
        unreachable!()
    };
    *right_root = root.structural_parameters[0].place;
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected aggregate equality validation result: {invalid_result:?}"
    );
}

#[test]
fn nested_payload_sum_equality_retains_exact_record_case_payload_paths_end_to_end() {
    fn collect_paths(
        proposition: &Proposition,
        memberships: &mut Vec<(
            psi_core::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
            psi_core::StructuralCaseId,
        )>,
        integer_fields: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        match proposition {
            Proposition::StructuralCaseMembership { subject, case } => {
                memberships.push((subject.root(), subject.path().to_vec(), *case));
            }
            Proposition::Equal(_, ScalarTerm::IntegerEqual { left, right, .. }) => {
                for operand in [left.as_ref(), right.as_ref()] {
                    if let ScalarTerm::IntegerField { root, path, .. } = operand {
                        integer_fields.push((*root, path.clone()));
                    }
                }
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                for child in children {
                    collect_paths(child, memberships, integer_fields);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                collect_paths(premise, memberships, integer_fields);
                collect_paths(conclusion, memberships, integer_fields);
            }
            Proposition::Truth
            | Proposition::Falsehood
            | Proposition::Atom(_)
            | Proposition::Equal(_, _)
            | Proposition::LessThan(_, _)
            | Proposition::LessOrEqual(_, _)
            | Proposition::IntegerMathEqual(_, _)
            | Proposition::IntegerMathLessThan(_, _)
            | Proposition::IntegerMathLessOrEqual(_, _)
            | Proposition::IeeeFloatComparison { .. }
            | Proposition::ByteSequenceEqual { .. }
            | Proposition::ContentConservation(_) => {}
        }
    }

    fn redirect_right_payload_field(
        proposition: &mut Proposition,
        from: StructuralFieldId,
        to: StructuralFieldId,
    ) -> bool {
        match proposition {
            Proposition::Equal(_, ScalarTerm::IntegerEqual { right, .. }) => {
                let ScalarTerm::IntegerField { path, .. } = right.as_mut() else {
                    return false;
                };
                let Some(CanonicalStructuralPathSegment::Field(field)) = path.last_mut() else {
                    return false;
                };
                if *field != from {
                    return false;
                }
                *field = to;
                true
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => children
                .iter_mut()
                .any(|child| redirect_right_payload_field(child, from, to)),
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                redirect_right_payload_field(premise, from, to)
                    || redirect_right_payload_field(conclusion, from, to)
            }
            _ => false,
        }
    }

    let tokens = Lexer::new(NESTED_PAYLOAD_SUM_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Whole::enter")
        .expect("whole Envelope equality lowers through its sum field");

    let root = &lowered.semantic_module.machines[0];
    let envelope = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Envelope structural type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope is a record")
    };
    let message_field = fields
        .iter()
        .find(|field| field.identity == "message")
        .expect("message field");
    let StructuralFieldType::Structural(message_type) = message_field.field_type else {
        panic!("message field names the nested structural sum")
    };
    let message = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == message_type)
        .expect("Message structural type");
    let StructuralTypeShape::Sum { cases } = &message.shape else {
        panic!("Message is a sum")
    };
    let data_case = cases
        .iter()
        .find(|case| case.identity == "Data")
        .expect("Data case");
    let value_field = data_case
        .fields
        .iter()
        .find(|field| field.identity == "value")
        .expect("value field");
    let checksum_field = data_case
        .fields
        .iter()
        .find(|field| field.identity == "checksum")
        .expect("checksum field");

    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("whole Envelope equality publishes one predicate")
    };
    let mut root_memberships = Vec::new();
    let mut root_fields = Vec::new();
    collect_paths(
        root_route.proposition(),
        &mut root_memberships,
        &mut root_fields,
    );
    assert_eq!(root_memberships.len(), 4);
    assert!(root_memberships.iter().all(|(root_place, path, _)| {
        [
            root.structural_parameters[0].place,
            root.structural_parameters[1].place,
        ]
        .contains(root_place)
            && path == &[CanonicalStructuralPathSegment::Field(message_field.id)]
    }));
    assert_eq!(root_fields.len(), 4);
    assert!(root_fields.iter().all(|(root_place, path)| {
        [
            root.structural_parameters[0].place,
            root.structural_parameters[1].place,
        ]
        .contains(root_place)
            && matches!(
                path.as_slice(),
                [
                    CanonicalStructuralPathSegment::Field(message),
                    CanonicalStructuralPathSegment::Case(case),
                    CanonicalStructuralPathSegment::Field(field),
                ] if *message == message_field.id
                    && *case == data_case.id
                    && [value_field.id, checksum_field.id].contains(field)
            )
    }));

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts exact nested record-to-sum paths");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("nested equality has fixed fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");
    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );

    let mut redirected = lowered.semantic_module.clone();
    let CrashRouteGuard::Predicate(predicate) =
        &mut redirected.machines[0].contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let wrong_field = StructuralFieldId::new(u64::MAX).expect("nonzero redirected field");
    assert!(redirect_right_payload_field(
        &mut proposition,
        value_field.id,
        wrong_field,
    ));
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::InvalidIntegerFieldTerm { .. })
        ),
        "unexpected redirected nested payload validation result: {invalid_result:?}"
    );
}

#[test]
fn mixed_aggregate_equality_retains_common_fields_cases_and_call_rebasing_end_to_end() {
    fn collect_scalar_paths(
        term: &ScalarTerm,
        boolean: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
        integer: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
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
            psi_core::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
            psi_core::StructuralCaseId,
        )>,
        boolean: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
        integer: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        match proposition {
            Proposition::StructuralCaseMembership { subject, case } => {
                memberships.push((subject.root(), subject.path().to_vec(), *case));
            }
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
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

    let tokens = Lexer::new(MIXED_AGGREGATE_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let equal = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("mixed equality lowers through the direct call");
    let different = psi_checked_trees_to_terminal::lower_machine(&checked, "Different::enter")
        .expect("mixed inequality lowers through the direct call");

    for (lowered, is_different) in [(&equal, false), (&different, true)] {
        let machine = &lowered.semantic_module.machines[0];
        let declaration = lowered
            .semantic_module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == machine.structural_parameters[0].structural_type)
            .expect("Message structural type");
        let StructuralTypeShape::Mixed { fields, cases } = &declaration.shape else {
            panic!("Message retains its mixed shape")
        };
        let active = fields
            .iter()
            .find(|field| field.identity == "active")
            .expect("common active field");
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
            panic!("mixed equality publishes one predicate")
        };
        let equality = if is_different {
            let Proposition::Implication {
                premise,
                conclusion,
            } = route.proposition()
            else {
                panic!("mixed inequality is an implication")
            };
            assert!(matches!(conclusion.as_ref(), Proposition::Falsehood));
            premise.as_ref()
        } else {
            route.proposition()
        };
        let Proposition::Conjunction(canonical) = equality else {
            panic!("mixed equality is one canonical conjunction")
        };
        assert_eq!(canonical.len(), 2);
        assert!(matches!(
            &canonical[0],
            Proposition::Equal(_, ScalarTerm::BooleanEqual { .. })
        ));
        assert!(matches!(
            &canonical[1],
            Proposition::Disjunction(arms) if arms.len() == 2
        ));
        if is_different {
            assert!(matches!(
                route.proposition(),
                Proposition::Implication { conclusion, .. }
                    if matches!(conclusion.as_ref(), Proposition::Falsehood)
            ));
        }
        let mut memberships = Vec::new();
        let mut boolean = Vec::new();
        let mut integer = Vec::new();
        collect(
            route.proposition(),
            &mut memberships,
            &mut boolean,
            &mut integer,
        );
        assert_eq!(memberships.len(), 4);
        assert!(memberships.iter().all(|(root, path, case)| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *root)
                && path.is_empty()
                && cases.iter().any(|candidate| candidate.id == *case)
        }));
        assert_eq!(boolean.len(), 2);
        assert!(boolean.iter().all(|(root, path)| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *root)
                && path == &[CanonicalStructuralPathSegment::Field(active.id)]
        }));
        assert_eq!(integer.len(), 2);
        assert!(integer.iter().all(|(root, path)| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *root)
                && path
                    == &[
                        CanonicalStructuralPathSegment::Case(data.id),
                        CanonicalStructuralPathSegment::Field(value.id),
                    ]
        }));
        let verified = psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("verifier replays the exact mixed paths");
        let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("mixed equality has fixed fuel");
        validate_fixed_entry_fuel(&verified, &fixed).expect("mixed equality fuel recomputes");
        let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
        assert_eq!(
            decode_module(&semantics),
            Ok(lowered.semantic_module.clone())
        );
    }

    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }
    let verified = psi_terminal_verifier::verify_module(
        &equal.semantic_module,
        &equal.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed equality verifies before interpretation");
    let fixed = derive_fixed_entry_fuel(&verified, equal.semantic_module.entry)
        .expect("mixed equality has fixed fuel");
    let semantics = encode_module(&equal.semantic_module).expect("semantic encode");
    let proof = encode_proof_bundle(&equal.proof_bundle).expect("proof encode");
    let arguments = equal.semantic_module.machines[0]
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 701 + u64::try_from(index).expect("small parameter index"),
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
        &mut Accept,
    )
    .expect("verified mixed equality remains executable metadata");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = equal.semantic_module.clone();
    let cases = redirected
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Mixed { cases, .. } => Some(cases),
            _ => None,
        })
        .expect("Message remains a mixed structural type");
    cases[0].id = psi_core::StructuralCaseId::new(u64::MAX).expect("nonzero case");
    let redirected_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            redirected_result,
            Err(psi_terminal_verifier::ModuleError::InvalidStructuralCaseMembership { .. })
        ),
        "unexpected redirected mixed case result: {redirected_result:?}"
    );

    let mut common_field_drift = equal.semantic_module.clone();
    let fields = common_field_drift
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Mixed { fields, .. } => Some(fields),
            _ => None,
        })
        .expect("Message common fields");
    fields[0].id = StructuralFieldId::new(u64::MAX).expect("nonzero field");
    assert!(matches!(
        psi_terminal_verifier::validate_module(&common_field_drift),
        Err(psi_terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
    ));

    let mut payload_field_drift = equal.semantic_module.clone();
    let cases = payload_field_drift
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Mixed { cases, .. } => Some(cases),
            _ => None,
        })
        .expect("Message cases");
    let data = cases
        .iter_mut()
        .find(|case| case.identity == "Data")
        .expect("Data case");
    data.fields[0].id = StructuralFieldId::new(u64::MAX).expect("nonzero field");
    assert!(matches!(
        psi_terminal_verifier::validate_module(&payload_field_drift),
        Err(psi_terminal_verifier::ModuleError::InvalidIntegerFieldTerm { .. })
    ));

    let mut noncanonical = equal.semantic_module.clone();
    let cases = noncanonical
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Mixed { cases, .. } => Some(cases),
            _ => None,
        })
        .expect("Message cases");
    cases.swap(0, 1);
    assert!(matches!(
        encode_module(&noncanonical),
        Err(psi_terminal_codec::CodecError::NonCanonicalOrder(
            "mixed structural cases by StructuralCaseId"
        ))
    ));
}

#[test]
fn nested_mixed_aggregate_equality_prefixes_every_path_and_rebases_whole_root_calls() {
    fn collect_scalar_paths(
        term: &ScalarTerm,
        boolean: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
        integer: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
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
            psi_core::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
            psi_core::StructuralCaseId,
        )>,
        boolean: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
        integer: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        match proposition {
            Proposition::StructuralCaseMembership { subject, case } => {
                memberships.push((subject.root(), subject.path().to_vec(), *case));
            }
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
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

    let tokens = Lexer::new(NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let equal = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("nested mixed equality lowers through the whole-root call");
    let different = psi_checked_trees_to_terminal::lower_machine(&checked, "Different::enter")
        .expect("nested mixed inequality lowers through the whole-root call");

    for (lowered, is_different) in [(&equal, false), (&different, true)] {
        let machine = &lowered.semantic_module.machines[0];
        let envelope = lowered
            .semantic_module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == machine.structural_parameters[0].structural_type)
            .expect("Envelope structural type");
        let StructuralTypeShape::Record { fields } = &envelope.shape else {
            panic!("Envelope remains a record")
        };
        let selected = fields
            .iter()
            .find(|field| field.identity == "selected")
            .expect("selected field");
        let message = fields
            .iter()
            .find(|field| field.identity == "message")
            .expect("message field");
        let StructuralFieldType::Structural(message_type) = message.field_type else {
            panic!("message field retains its structural type")
        };
        let message_declaration = lowered
            .semantic_module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == message_type)
            .expect("Message structural type");
        let StructuralTypeShape::Mixed { fields, cases } = &message_declaration.shape else {
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
        assert_eq!(canonical.len(), 3);
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
        assert_eq!(memberships.len(), 4);
        assert!(memberships.iter().all(|(root, path, case)| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *root)
                && path == &[CanonicalStructuralPathSegment::Field(message.id)]
                && cases.iter().any(|candidate| candidate.id == *case)
        }));
        assert_eq!(boolean.len(), 4);
        assert_eq!(
            boolean
                .iter()
                .filter(|(_, path)| {
                    path.as_slice() == [CanonicalStructuralPathSegment::Field(selected.id)]
                })
                .count(),
            2
        );
        assert_eq!(
            boolean
                .iter()
                .filter(|(_, path)| {
                    path.as_slice()
                        == [
                            CanonicalStructuralPathSegment::Field(message.id),
                            CanonicalStructuralPathSegment::Field(active.id),
                        ]
                })
                .count(),
            2
        );
        assert_eq!(integer.len(), 2);
        assert!(integer.iter().all(|(root, path)| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *root)
                && path
                    == &[
                        CanonicalStructuralPathSegment::Field(message.id),
                        CanonicalStructuralPathSegment::Case(data.id),
                        CanonicalStructuralPathSegment::Field(value.id),
                    ]
        }));

        let OperationKind::CallUnit {
            crash_continuations,
            ..
        } = &machine.blocks[0].operations[0].kind
        else {
            panic!("nested mixed caller emits one Unit call")
        };
        assert_eq!(crash_continuations, &machine.contract.crash_routes);

        let verified = psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("verifier independently replays every prefixed mixed path");
        let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("nested mixed equality has fixed fuel");
        validate_fixed_entry_fuel(&verified, &fixed).expect("nested mixed fixed fuel recomputes");
        let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
        assert_eq!(
            decode_module(&semantics),
            Ok(lowered.semantic_module.clone())
        );
    }

    let verified = psi_terminal_verifier::verify_module(
        &equal.semantic_module,
        &equal.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("nested mixed equality verifies before interpretation");
    let fixed = derive_fixed_entry_fuel(&verified, equal.semantic_module.entry)
        .expect("nested mixed equality has fixed fuel");
    let semantics = encode_module(&equal.semantic_module).expect("semantic encode");
    let proof = encode_proof_bundle(&equal.proof_bundle).expect("proof encode");
    let arguments = equal.semantic_module.machines[0]
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 801 + u64::try_from(index).expect("small parameter index"),
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
        &mut Accept,
    )
    .expect("verified nested mixed equality remains executable metadata");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = equal.semantic_module.clone();
    let envelope = redirected
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Record { fields }
                if fields.iter().any(|field| {
                    field.identity == "message"
                        && matches!(field.field_type, StructuralFieldType::Structural(_))
                }) =>
            {
                Some(fields)
            }
            _ => None,
        })
        .expect("Envelope fields");
    envelope
        .iter_mut()
        .find(|field| field.identity == "message")
        .expect("message field")
        .id = StructuralFieldId::new(u64::MAX).expect("nonzero redirected field");
    let redirected_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            redirected_result,
            Err(psi_terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
                | Err(psi_terminal_verifier::ModuleError::InvalidStructuralCaseMembership { .. })
        ),
        "unexpected redirected nested-mixed prefix result: {redirected_result:?}"
    );
}

#[test]
fn two_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    fn collect_scalar_paths(
        term: &ScalarTerm,
        boolean: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
        integer: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
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
            psi_core::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
            psi_core::StructuralCaseId,
        )>,
        boolean: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
        integer: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        match proposition {
            Proposition::StructuralCaseMembership { subject, case } => {
                memberships.push((subject.root(), subject.path().to_vec(), *case));
            }
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
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

    let tokens = Lexer::new(TWO_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let equal = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("two-field nested mixed equality lowers through the whole-root call");
    let different = psi_checked_trees_to_terminal::lower_machine(&checked, "Different::enter")
        .expect("two-field nested mixed inequality lowers through the whole-root call");

    for (lowered, is_different) in [(&equal, false), (&different, true)] {
        let machine = &lowered.semantic_module.machines[0];
        let envelope = lowered
            .semantic_module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == machine.structural_parameters[0].structural_type)
            .expect("Envelope structural type");
        let StructuralTypeShape::Record { fields } = &envelope.shape else {
            panic!("Envelope remains a record")
        };
        let inner = fields
            .iter()
            .find(|field| field.identity == "inner")
            .expect("inner field");
        let StructuralFieldType::Structural(inner_type) = inner.field_type else {
            panic!("inner field retains its structural type")
        };
        let inner_declaration = lowered
            .semantic_module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == inner_type)
            .expect("Inner structural type");
        let StructuralTypeShape::Record { fields } = &inner_declaration.shape else {
            panic!("Inner remains a record")
        };
        let message = fields
            .iter()
            .find(|field| field.identity == "message")
            .expect("message field");
        let StructuralFieldType::Structural(message_type) = message.field_type else {
            panic!("message field retains its structural type")
        };
        let message_declaration = lowered
            .semantic_module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == message_type)
            .expect("Message structural type");
        let StructuralTypeShape::Mixed { fields, cases } = &message_declaration.shape else {
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
            panic!("two-field nested mixed equality publishes one predicate")
        };
        let equality = if is_different {
            let Proposition::Implication {
                premise,
                conclusion,
            } = route.proposition()
            else {
                panic!("two-field nested mixed inequality is an implication")
            };
            assert!(matches!(conclusion.as_ref(), Proposition::Falsehood));
            premise.as_ref()
        } else {
            route.proposition()
        };
        let Proposition::Conjunction(canonical) = equality else {
            panic!("two-field nested mixed equality is one canonical conjunction")
        };
        assert_eq!(canonical.len(), 2);
        assert!(matches!(
            canonical.last(),
            Some(Proposition::Disjunction(_))
        ));

        let mixed_prefix = [
            CanonicalStructuralPathSegment::Field(inner.id),
            CanonicalStructuralPathSegment::Field(message.id),
        ];
        let mut memberships = Vec::new();
        let mut boolean = Vec::new();
        let mut integer = Vec::new();
        collect(
            route.proposition(),
            &mut memberships,
            &mut boolean,
            &mut integer,
        );
        assert_eq!(memberships.len(), 4);
        assert!(memberships.iter().all(|(root, path, case)| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *root)
                && path == &mixed_prefix
                && cases.iter().any(|candidate| candidate.id == *case)
        }));
        assert_eq!(boolean.len(), 2);
        assert!(boolean.iter().all(|(root, path)| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *root)
                && path
                    == &[
                        mixed_prefix[0],
                        mixed_prefix[1],
                        CanonicalStructuralPathSegment::Field(active.id),
                    ]
        }));
        assert_eq!(integer.len(), 2);
        assert!(integer.iter().all(|(root, path)| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *root)
                && path
                    == &[
                        mixed_prefix[0],
                        mixed_prefix[1],
                        CanonicalStructuralPathSegment::Case(data.id),
                        CanonicalStructuralPathSegment::Field(value.id),
                    ]
        }));

        let OperationKind::CallUnit {
            crash_continuations,
            ..
        } = &machine.blocks[0].operations[0].kind
        else {
            panic!("two-field nested mixed caller emits one Unit call")
        };
        assert_eq!(crash_continuations, &machine.contract.crash_routes);

        let verified = psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("verifier replays every two-field-prefixed mixed path");
        let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
            .expect("two-field nested mixed equality has fixed fuel");
        validate_fixed_entry_fuel(&verified, &fixed)
            .expect("two-field nested mixed fixed fuel recomputes");
        let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
        assert_eq!(
            decode_module(&semantics),
            Ok(lowered.semantic_module.clone())
        );
        let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
        assert_eq!(
            decode_proof_bundle(&proof),
            Ok(lowered.proof_bundle.clone())
        );
        let arguments = machine
            .structural_parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| TerminalStructuralValue {
                opaque_identity: 901 + u64::try_from(index).expect("small parameter index"),
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            })
            .collect::<Vec<_>>();
        let measured = interpret_terminal_artifact_with_effect_handler_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &arguments,
            &mut Accept,
        )
        .expect("verified two-field nested mixed equality remains executable metadata");
        assert_eq!(measured.value(), TerminalExecutionResult::Unit);
        assert_eq!(measured.usage().total_units(), fixed.ceiling_units());
    }

    let machine = &equal.semantic_module.machines[0];
    let envelope_type = machine.structural_parameters[0].structural_type;
    let envelope = equal
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == envelope_type)
        .expect("Envelope structural type");
    let StructuralTypeShape::Record { fields } = &envelope.shape else {
        panic!("Envelope remains a record")
    };
    let StructuralFieldType::Structural(inner_type) = fields[0].field_type else {
        panic!("inner field retains its structural type")
    };

    let mut outer_field_drift = equal.semantic_module.clone();
    let StructuralTypeShape::Record { fields } = &mut outer_field_drift
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.id == envelope_type)
        .expect("Envelope structural type")
        .shape
    else {
        panic!("Envelope remains a record")
    };
    fields[0].id = StructuralFieldId::new(u64::MAX).expect("nonzero outer field");
    let outer_result = psi_terminal_verifier::validate_module(&outer_field_drift);
    assert!(
        matches!(
            outer_result,
            Err(psi_terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
                | Err(psi_terminal_verifier::ModuleError::InvalidStructuralCaseMembership { .. })
        ),
        "unexpected outer-field drift result: {outer_result:?}"
    );

    let mut inner_field_drift = equal.semantic_module.clone();
    let StructuralTypeShape::Record { fields } = &mut inner_field_drift
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.id == inner_type)
        .expect("Inner structural type")
        .shape
    else {
        panic!("Inner remains a record")
    };
    fields[0].id = StructuralFieldId::new(u64::MAX - 1).expect("nonzero inner field");
    let inner_result = psi_terminal_verifier::validate_module(&inner_field_drift);
    assert!(
        matches!(
            inner_result,
            Err(psi_terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
                | Err(psi_terminal_verifier::ModuleError::InvalidStructuralCaseMembership { .. })
        ),
        "unexpected inner-field drift result: {inner_result:?}"
    );
}

fn assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
    source: &str,
    prefix_identities: &[&str],
) {
    fn collect_scalar_paths(
        term: &ScalarTerm,
        boolean: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
        integer: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
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
            psi_core::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
            psi_core::StructuralCaseId,
        )>,
        boolean: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
        integer: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        match proposition {
            Proposition::StructuralCaseMembership { subject, case } => {
                memberships.push((subject.root(), subject.path().to_vec(), *case));
            }
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let equal = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("nested mixed equality lowers through the whole-root call");
    let different = psi_checked_trees_to_terminal::lower_machine(&checked, "Different::enter")
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

        let verified = psi_terminal_verifier::verify_module(
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
        let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
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
        let measured = interpret_terminal_artifact_with_effect_handler_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &arguments,
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
        let result = psi_terminal_verifier::validate_module(&redirected);
        assert!(
            matches!(
                result,
                Err(psi_terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
                    | Err(
                        psi_terminal_verifier::ModuleError::InvalidStructuralCaseMembership { .. }
                    )
            ),
            "unexpected enclosing-field {index} drift result: {result:?}"
        );
    }
}

#[test]
fn three_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        THREE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &["middle", "inner", "message"],
    );
}

#[test]
fn four_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        FOUR_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &["envelope", "middle", "inner", "message"],
    );
}

#[test]
fn five_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        FIVE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &["exterior", "envelope", "middle", "inner", "message"],
    );
}

#[test]
fn six_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        SIX_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "outside", "exterior", "envelope", "middle", "inner", "message",
        ],
    );
}

#[test]
fn seven_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        SEVEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "beyond", "outside", "exterior", "envelope", "middle", "inner", "message",
        ],
    );
}

#[test]
fn eight_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        EIGHT_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "further", "beyond", "outside", "exterior", "envelope", "middle", "inner", "message",
        ],
    );
}

#[test]
fn nine_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        NINE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "furthest", "further", "beyond", "outside", "exterior", "envelope", "middle", "inner",
            "message",
        ],
    );
}

#[test]
fn ten_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        TEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "ultimate", "furthest", "further", "beyond", "outside", "exterior", "envelope",
            "middle", "inner", "message",
        ],
    );
}

#[test]
fn eleven_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        ELEVEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "outermost",
            "ultimate",
            "furthest",
            "further",
            "beyond",
            "outside",
            "exterior",
            "envelope",
            "middle",
            "inner",
            "message",
        ],
    );
}

#[test]
fn twelve_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        TWELVE_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "final",
            "outermost",
            "ultimate",
            "furthest",
            "further",
            "beyond",
            "outside",
            "exterior",
            "envelope",
            "middle",
            "inner",
            "message",
        ],
    );
}

#[test]
fn thirteen_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        THIRTEEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "absolute",
            "final",
            "outermost",
            "ultimate",
            "furthest",
            "further",
            "beyond",
            "outside",
            "exterior",
            "envelope",
            "middle",
            "inner",
            "message",
        ],
    );
}

#[test]
fn unsupported_mixed_aggregate_equality_shapes_remain_fenced() {
    for source in MIXED_AGGREGATE_EQUALITY_FENCE_SOURCES
        .into_iter()
        .chain(NESTED_MIXED_AGGREGATE_EQUALITY_FENCE_SOURCES)
    {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = match lower_symbol_resolved_trees(&resolved) {
            Ok(typed) => typed,
            Err(_) => continue,
        };
        let checked = match lower_typed_trees(typed) {
            Ok(checked) => checked,
            Err(diagnostics) => {
                assert!(!diagnostics.is_empty());
                continue;
            }
        };
        let result = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter");
        assert!(matches!(
            result,
            Err(psi_checked_trees_to_terminal::LoweringError::Unsupported(
                "structural crash route is outside checked Boolean member lowering"
            ))
        ));
    }
}

#[test]
fn payload_sum_nested_record_equality_rebases_and_replays_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    fn collect_scalar_fields(
        term: &ScalarTerm,
        fields: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        match term {
            ScalarTerm::BooleanField { root, path }
            | ScalarTerm::IntegerField { root, path, .. } => {
                fields.push((*root, path.clone()));
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. } => {
                collect_scalar_fields(left, fields);
                collect_scalar_fields(right, fields);
            }
            _ => {}
        }
    }

    fn collect_paths(
        proposition: &Proposition,
        memberships: &mut Vec<(
            psi_core::PlaceId,
            Vec<CanonicalStructuralPathSegment>,
            psi_core::StructuralCaseId,
        )>,
        fields: &mut Vec<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        match proposition {
            Proposition::StructuralCaseMembership { subject, case } => {
                memberships.push((subject.root(), subject.path().to_vec(), *case));
            }
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
                collect_scalar_fields(left, fields);
                collect_scalar_fields(right, fields);
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                for child in children {
                    collect_paths(child, memberships, fields);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                collect_paths(premise, memberships, fields);
                collect_paths(conclusion, memberships, fields);
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

    fn redirect_integer_leaf(
        proposition: &mut Proposition,
        from: StructuralFieldId,
        to: StructuralFieldId,
    ) -> bool {
        fn redirect_term(
            term: &mut ScalarTerm,
            from: StructuralFieldId,
            to: StructuralFieldId,
        ) -> bool {
            match term {
                ScalarTerm::IntegerField { path, .. } => {
                    let Some(CanonicalStructuralPathSegment::Field(field)) = path.last_mut() else {
                        return false;
                    };
                    if *field != from {
                        return false;
                    }
                    *field = to;
                    true
                }
                ScalarTerm::BooleanEqual { left, right }
                | ScalarTerm::IntegerEqual { left, right, .. } => {
                    redirect_term(left, from, to) || redirect_term(right, from, to)
                }
                _ => false,
            }
        }

        match proposition {
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
                redirect_term(left, from, to) || redirect_term(right, from, to)
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => children
                .iter_mut()
                .any(|child| redirect_integer_leaf(child, from, to)),
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                redirect_integer_leaf(premise, from, to)
                    || redirect_integer_leaf(conclusion, from, to)
            }
            _ => false,
        }
    }

    let tokens = Lexer::new(NESTED_RECORD_PAYLOAD_SUM_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("payload-sum equality expands the nested record and lowers");
    let different = psi_checked_trees_to_terminal::lower_machine(&checked, "Different::enter")
        .expect("payload-sum inequality expands the nested record and lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let message = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root.structural_parameters[0].structural_type)
        .expect("Message structural type");
    let StructuralTypeShape::Sum { cases } = &message.shape else {
        panic!("Message is a sum")
    };
    let data_case = cases
        .iter()
        .find(|case| case.identity == "Data")
        .expect("Data case");
    let detail_field = data_case
        .fields
        .iter()
        .find(|field| field.identity == "detail")
        .expect("detail payload field");
    let StructuralFieldType::Structural(detail_type) = detail_field.field_type else {
        panic!("detail payload retains its record type")
    };
    let detail = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == detail_type)
        .expect("Detail structural type");
    let StructuralTypeShape::Record {
        fields: detail_fields,
    } = &detail.shape
    else {
        panic!("Detail is a record")
    };
    let active_field = detail_fields
        .iter()
        .find(|field| field.identity == "active")
        .expect("active field");
    let counter_field = detail_fields
        .iter()
        .find(|field| field.identity == "counter")
        .expect("counter field");
    let StructuralFieldType::Structural(counter_type) = counter_field.field_type else {
        panic!("counter field retains its record type")
    };
    let counter = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == counter_type)
        .expect("Counter structural type");
    let StructuralTypeShape::Record {
        fields: counter_fields,
    } = &counter.shape
    else {
        panic!("Counter is a record")
    };
    let count_field = counter_fields
        .iter()
        .find(|field| field.identity == "count")
        .expect("count leaf");

    for module in [&lowered.semantic_module, &different.semantic_module] {
        let entry = &module.machines[0];
        let [CrashRouteGuard::Predicate(route)] =
            entry.contract.crash_routes[0].alternatives.as_slice()
        else {
            panic!("entry publishes one nested-record payload predicate")
        };
        let mut memberships = Vec::new();
        let mut field_paths = Vec::new();
        collect_paths(route.proposition(), &mut memberships, &mut field_paths);
        assert_eq!(memberships.len(), 4);
        assert!(memberships.iter().all(|(place, path, _)| {
            entry
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *place)
                && path.is_empty()
        }));
        assert_eq!(field_paths.len(), 4);
        assert!(field_paths.iter().all(|(place, path)| {
            entry
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == *place)
                && match path.as_slice() {
                    [
                        CanonicalStructuralPathSegment::Case(case),
                        CanonicalStructuralPathSegment::Field(payload),
                        CanonicalStructuralPathSegment::Field(leaf),
                    ] => {
                        *case == data_case.id
                            && *payload == detail_field.id
                            && *leaf == active_field.id
                    }
                    [
                        CanonicalStructuralPathSegment::Case(case),
                        CanonicalStructuralPathSegment::Field(payload),
                        CanonicalStructuralPathSegment::Field(record),
                        CanonicalStructuralPathSegment::Field(leaf),
                    ] => {
                        *case == data_case.id
                            && *payload == detail_field.id
                            && *record == counter_field.id
                            && *leaf == count_field.id
                    }
                    _ => false,
                }
        }));
    }

    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        unreachable!()
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        unreachable!()
    };
    let mut helper_memberships = Vec::new();
    let mut helper_fields = Vec::new();
    collect_paths(
        helper_route.proposition(),
        &mut helper_memberships,
        &mut helper_fields,
    );
    assert!(helper_fields.iter().all(|(place, _)| {
        helper
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == *place)
    }));
    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("Root emits the helper call")
    };
    assert_eq!(structural_arguments.len(), 2);
    assert!(
        structural_arguments
            .iter()
            .all(|argument| argument.path.is_empty())
    );
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains one rebased crash continuation")
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs nested payload-record paths through the helper call");
    psi_terminal_verifier::verify_module(
        &different.semantic_module,
        &different.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts nested payload-record inequality");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("nested payload-record equality has fixed fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");
    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let arguments = root
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 71 + u64::try_from(index).unwrap(),
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
        &mut Accept,
    )
    .expect("nested payload-record equality remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut redirected = lowered.semantic_module.clone();
    let CrashRouteGuard::Predicate(predicate) =
        &mut redirected.machines[0].contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    assert!(redirect_integer_leaf(
        &mut proposition,
        count_field.id,
        active_field.id,
    ));
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationUncovered { .. })
        ),
        "redirecting the exact nested leaf must break the independently reconstructed call continuation: {invalid_result:?}"
    );
}

#[test]
fn payload_sum_nested_sum_equality_replays_end_to_end() {
    fn collect_scalar_paths(
        term: &ScalarTerm,
        paths: &mut Vec<Vec<CanonicalStructuralPathSegment>>,
    ) {
        match term {
            ScalarTerm::BooleanField { path, .. } | ScalarTerm::IntegerField { path, .. } => {
                paths.push(path.clone())
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. } => {
                collect_scalar_paths(left, paths);
                collect_scalar_paths(right, paths);
            }
            _ => {}
        }
    }

    fn collect_paths(
        proposition: &Proposition,
        paths: &mut Vec<Vec<CanonicalStructuralPathSegment>>,
    ) {
        match proposition {
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
                collect_scalar_paths(left, paths);
                collect_scalar_paths(right, paths);
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                for child in children {
                    collect_paths(child, paths);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                collect_paths(premise, paths);
                collect_paths(conclusion, paths);
            }
            Proposition::Truth
            | Proposition::Falsehood
            | Proposition::Atom(_)
            | Proposition::IntegerMathEqual(_, _)
            | Proposition::IntegerMathLessThan(_, _)
            | Proposition::IntegerMathLessOrEqual(_, _)
            | Proposition::StructuralCaseMembership { .. }
            | Proposition::IeeeFloatComparison { .. }
            | Proposition::ByteSequenceEqual { .. }
            | Proposition::ContentConservation(_) => {}
        }
    }

    let tokens = Lexer::new(NESTED_SUM_PAYLOAD_SUM_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("payload-sum equality expands the nested sum and lowers");

    let root = &lowered.semantic_module.machines[0];
    let [CrashRouteGuard::Predicate(route)] = root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("nested sum equality publishes one predicate")
    };
    let mut paths = Vec::new();
    collect_paths(route.proposition(), &mut paths);
    assert_eq!(paths.len(), 2, "both integer roots remain explicit");
    assert!(paths.iter().all(|path| {
        matches!(
            path.as_slice(),
            [
                CanonicalStructuralPathSegment::Case(_),
                CanonicalStructuralPathSegment::Field(_),
                CanonicalStructuralPathSegment::Case(_),
                CanonicalStructuralPathSegment::Field(_),
            ]
        )
    }));

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts exact nested-sum payload paths");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("nested-sum equality has fixed fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");
    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
}

#[test]
fn ieee_float_aggregate_equality_is_atomic_and_canonical_end_to_end() {
    let tokens = Lexer::new(IEEE_FLOAT_AGGREGATE_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("IEEE aggregate equality lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("caller publishes one IEEE aggregate route")
    };
    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one IEEE aggregate route")
    };
    let inspect =
        |proposition: &Proposition, left_root: psi_core::PlaceId, right_root: psi_core::PlaceId| {
            let Proposition::Conjunction(conjuncts) = proposition else {
                panic!("two-float equality is one conjunction")
            };
            assert_eq!(conjuncts.len(), 2);
            let mut formats = Vec::new();
            for conjunct in conjuncts {
                let Proposition::IeeeFloatComparison {
                    format,
                    left,
                    right,
                    ..
                } = conjunct
                else {
                    panic!("float leaves remain atomic IEEE propositions")
                };
                assert_eq!((left.root(), right.root()), (left_root, right_root));
                assert_eq!(left.path().len(), 1);
                assert_eq!(right.path().len(), 1);
                formats.push(*format);
            }
            formats.sort();
            assert_eq!(
                formats,
                [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64]
            );
        };
    inspect(
        root_route.proposition(),
        root.structural_parameters[0].place,
        root.structural_parameters[1].place,
    );
    inspect(
        helper_route.proposition(),
        helper.structural_parameters[0].place,
        helper.structural_parameters[1].place,
    );

    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the IEEE continuation")
    };
    assert_eq!(continuation, root_route);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier checks IEEE leaf formats and substituted roots");
    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );

    let reversed = psi_checked_trees_to_terminal::lower_machine(&checked, "Reverse::enter")
        .expect("reversed IEEE operands lower canonically");
    let [CrashRouteGuard::Predicate(reversed_route)] =
        reversed.semantic_module.machines[0].contract.crash_routes[0]
            .alternatives
            .as_slice()
    else {
        unreachable!()
    };
    let Proposition::Conjunction(reversed_conjuncts) = reversed_route.proposition() else {
        unreachable!()
    };
    assert!(reversed_conjuncts.iter().all(|item| matches!(
        item,
        Proposition::IeeeFloatComparison { left, right, .. } if left <= right
    )));
    encode_module(&reversed.semantic_module).expect("canonical reversed semantic encode");

    let different = psi_checked_trees_to_terminal::lower_machine(&checked, "Different::enter")
        .expect("direct IEEE inequality lowers atomically");
    let [CrashRouteGuard::Predicate(different_route)] =
        different.semantic_module.machines[0].contract.crash_routes[0]
            .alternatives
            .as_slice()
    else {
        unreachable!()
    };
    assert!(matches!(
        different_route.proposition(),
        Proposition::IeeeFloatComparison {
            kind: psi_core::IeeeFloatComparisonKind::NotEqual,
            format: IeeeFloatFormat::Binary32,
            ..
        }
    ));
    psi_terminal_verifier::verify_module(
        &different.semantic_module,
        &different.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier retains direct IEEE inequality");
    let different_bytes = encode_module(&different.semantic_module).expect("inequality encode");
    assert_eq!(
        decode_module(&different_bytes),
        Ok(different.semantic_module)
    );

    let aggregate_different =
        psi_checked_trees_to_terminal::lower_machine(&checked, "AggregateDifferent::enter")
            .expect("aggregate IEEE inequality lowers as canonical negation");
    let aggregate_root = &aggregate_different.semantic_module.machines[0];
    let [CrashRouteGuard::Predicate(aggregate_route)] = aggregate_root.contract.crash_routes[0]
        .alternatives
        .as_slice()
    else {
        unreachable!()
    };
    let Proposition::Implication {
        premise,
        conclusion,
    } = aggregate_route.proposition()
    else {
        panic!("aggregate IEEE inequality is equality implying falsehood")
    };
    assert_eq!(conclusion.as_ref(), &Proposition::Falsehood);
    let Proposition::Conjunction(equalities) = premise.as_ref() else {
        panic!("two-field aggregate equality remains the negated conjunction")
    };
    assert!(equalities.iter().all(|proposition| matches!(
        proposition,
        Proposition::IeeeFloatComparison {
            kind: psi_core::IeeeFloatComparisonKind::Equal,
            ..
        }
    )));
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &aggregate_root.blocks[0].operations[0].kind
    else {
        panic!("aggregate inequality caller emits one structural Unit call")
    };
    let [CrashRouteGuard::Predicate(aggregate_continuation)] =
        crash_continuations[0].alternatives.as_slice()
    else {
        unreachable!()
    };
    assert_eq!(aggregate_continuation, aggregate_route);
    psi_terminal_verifier::verify_module(
        &aggregate_different.semantic_module,
        &aggregate_different.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs aggregate IEEE negation through the call");

    let mut redirected_aggregate = aggregate_different.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut redirected_aggregate.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Implication { premise, .. } = &mut proposition else {
        unreachable!()
    };
    let Proposition::Conjunction(conjuncts) = premise.as_mut() else {
        unreachable!()
    };
    let Some(Proposition::IeeeFloatComparison { left, right, .. }) = conjuncts
        .iter_mut()
        .find(|item| matches!(item, Proposition::IeeeFloatComparison { .. }))
    else {
        unreachable!()
    };
    *right = psi_core::IeeeFloatStructuralField::new(left.root(), right.path().to_vec())
        .expect("redirected IEEE field path remains nonempty");
    *predicate = CrashPredicateTerm::new(proposition);
    let invalid_result = psi_terminal_verifier::validate_module(&redirected_aggregate);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected aggregate IEEE inequality validation result: {invalid_result:?}"
    );

    let aggregate_bytes =
        encode_module(&aggregate_different.semantic_module).expect("aggregate inequality encode");
    assert_eq!(
        decode_module(&aggregate_bytes),
        Ok(aggregate_different.semantic_module)
    );

    let projected_different =
        psi_checked_trees_to_terminal::lower_machine(&checked, "ProjectedDifferent::enter")
            .expect("projected aggregate IEEE inequality lowers");
    let projected_root = &projected_different.semantic_module.machines[0];
    let [CrashRouteGuard::Predicate(projected_route)] = projected_root.contract.crash_routes[0]
        .alternatives
        .as_slice()
    else {
        unreachable!()
    };
    let Proposition::Implication { premise, .. } = projected_route.proposition() else {
        panic!("projected aggregate inequality retains canonical negation")
    };
    let Proposition::Conjunction(projected_equalities) = premise.as_ref() else {
        unreachable!()
    };
    assert!(projected_equalities.iter().all(|proposition| matches!(
        proposition,
        Proposition::IeeeFloatComparison { left, right, .. }
            if left.path().len() == 3 && right.path().len() == 3
    )));
    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &projected_root.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].path.len(), 1);
    let [CrashRouteGuard::Predicate(projected_continuation)] =
        crash_continuations[0].alternatives.as_slice()
    else {
        unreachable!()
    };
    assert_eq!(projected_continuation, projected_route);
    psi_terminal_verifier::verify_module(
        &projected_different.semantic_module,
        &projected_different.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs nonempty prefixes below aggregate IEEE negation");

    let mut wrong_format = reversed.semantic_module.clone();
    let [CrashRouteGuard::Predicate(predicate)] = wrong_format.machines[0].contract.crash_routes[0]
        .alternatives
        .as_mut_slice()
    else {
        unreachable!()
    };
    let mut proposition = predicate.proposition().clone();
    let Proposition::Conjunction(conjuncts) = &mut proposition else {
        unreachable!()
    };
    let Some(Proposition::IeeeFloatComparison { format, .. }) = conjuncts.iter_mut().find(|item| {
        matches!(
            item,
            Proposition::IeeeFloatComparison {
                format: IeeeFloatFormat::Binary32,
                ..
            }
        )
    }) else {
        unreachable!()
    };
    *format = IeeeFloatFormat::Binary64;
    *predicate = CrashPredicateTerm::new(proposition);
    assert!(matches!(
        psi_terminal_verifier::validate_module(&wrong_format),
        Err(psi_terminal_verifier::ModuleError::InvalidIeeeFloatFieldTerm { .. })
    ));
}

#[test]
fn byte_sequence_aggregate_equality_is_content_atomic_end_to_end() {
    let tokens = Lexer::new(BYTE_SEQUENCE_AGGREGATE_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("borrowed byte-sequence aggregate equality lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let byte_equality = |proposition: &Proposition| {
        let Proposition::Conjunction(conjuncts) = proposition else {
            panic!("Boolean and byte-sequence fields form one conjunction")
        };
        assert_eq!(conjuncts.len(), 2);
        conjuncts
            .iter()
            .find_map(|proposition| match proposition {
                Proposition::ByteSequenceEqual { left, right } => {
                    Some((left.clone(), right.clone()))
                }
                _ => None,
            })
            .expect("byte-sequence equality remains one semantic atom")
    };
    let [CrashRouteGuard::Predicate(root_route)] =
        root.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("root publishes one byte-sequence aggregate route")
    };
    let (left, right) = byte_equality(root_route.proposition());
    assert_eq!(
        (left.root(), right.root()),
        (
            root.structural_parameters[0].place,
            root.structural_parameters[1].place
        )
    );
    assert_eq!((left.path().len(), right.path().len()), (1, 1));

    let [CrashRouteGuard::Predicate(helper_route)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("helper publishes one byte-sequence aggregate route")
    };
    let (helper_left, helper_right) = byte_equality(helper_route.proposition());
    assert_eq!(
        (helper_left.root(), helper_right.root()),
        (
            helper.structural_parameters[0].place,
            helper.structural_parameters[1].place
        )
    );

    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("root emits one structural Unit call")
    };
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains one byte-sequence crash continuation")
    };
    assert_eq!(continuation, root_route);

    assert!(lowered
        .semantic_module
        .structural_types
        .iter()
        .any(|declaration| {
            matches!(&declaration.shape, StructuralTypeShape::Record { fields }
            if fields.iter().any(|field| matches!(
                field.field_type,
                StructuralFieldType::ByteSequence(psi_terminal::ByteSequenceCarrier::BorrowedView)
            )))
        }));
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier checks byte-sequence leaves and substituted roots");
    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );

    let bounded = psi_checked_trees_to_terminal::lower_machine(&checked, "BoundedRoot::enter")
        .expect("bounded byte-sequence aggregate equality lowers");
    assert!(
        bounded
            .semantic_module
            .structural_types
            .iter()
            .any(|declaration| {
                matches!(&declaration.shape, StructuralTypeShape::Record { fields }
                if fields.iter().any(|field| matches!(
                    field.field_type,
                    StructuralFieldType::ByteSequence(
                        psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity: 8 }
                    )
                )))
            })
    );
    psi_terminal_verifier::verify_module(
        &bounded.semantic_module,
        &bounded.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("bounded carrier verifies");
    let bounded_bytes = encode_module(&bounded.semantic_module).expect("bounded semantic encode");
    assert_eq!(decode_module(&bounded_bytes), Ok(bounded.semantic_module));

    let mut redirected = lowered.semantic_module;
    let field = redirected
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Record { fields } => fields
                .iter_mut()
                .find(|field| matches!(field.field_type, StructuralFieldType::ByteSequence(_))),
            StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::FixedArray { .. }
            | StructuralTypeShape::Sum { .. }
            | StructuralTypeShape::Mixed { .. } => None,
        })
        .expect("borrowed byte-sequence field");
    field.field_type = StructuralFieldType::Scalar(psi_core::ScalarType::Boolean);
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected),
        Err(psi_terminal_verifier::ModuleError::InvalidByteSequenceFieldTerm { .. })
    ));
}

#[test]
fn empty_record_equality_reuses_boolean_constants_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(EMPTY_RECORD_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("empty-record equality lowers through the existing Boolean constant carrier");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    for machine in [root, helper] {
        let [CrashRouteGuard::Predicate(predicate)] =
            machine.contract.crash_routes[0].alternatives.as_slice()
        else {
            panic!("empty-record equality should retain one predicate")
        };
        assert_eq!(
            predicate.proposition(),
            &Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::Boolean(true))
        );
    }

    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("call retains the empty-record equality continuation")
    };
    let CrashRouteGuard::Predicate(root_route) = &root.contract.crash_routes[0].alternatives[0]
    else {
        unreachable!()
    };
    assert_eq!(continuation, root_route);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs the root-free constant continuation");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("empty-record equality route has fixed fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let arguments = root
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 41 + u64::try_from(index).unwrap(),
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
        &mut Accept,
    )
    .expect("constant equality remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut tampered = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut tampered.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    crash_continuations[0].alternatives[0] = CrashRouteGuard::Predicate(CrashPredicateTerm::new(
        Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::Boolean(false)),
    ));
    let invalid_result = psi_terminal_verifier::validate_module(&tampered);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected empty-record continuation result: {invalid_result:?}"
    );
}

#[test]
fn address_record_equality_remains_fenced_before_terminal_lowering() {
    let tokens = Lexer::new(ADDRESS_RECORD_EQUALITY_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");

    let result = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter");
    assert!(
        matches!(
            &result,
            Err(psi_checked_trees_to_terminal::LoweringError::Unsupported(
                "structural crash route is outside checked Boolean member lowering"
            ))
        ),
        "unexpected lowering result: {result:?}"
    );
}

#[test]
fn fixed_index_argument_prefix_is_canonical_and_rebases_member_crash_routes_end_to_end() {
    struct Accept;
    impl TerminalEffectHandler for Accept {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Ok(())
        }
    }

    let tokens = Lexer::new(FIXED_INDEX_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("fixed-index structural member crash route lowers");

    let root = &lowered.semantic_module.machines[0];
    let helper = &lowered.semantic_module.machines[1];
    let OperationKind::CallUnit {
        structural_arguments,
        crash_continuations,
        ..
    } = &root.blocks[0].operations[0].kind
    else {
        panic!("caller emits one structural Unit call")
    };
    assert_eq!(
        structural_arguments[0].path,
        [StructuralPathSegment::FixedIndex(0)]
    );
    let [CrashRouteGuard::Predicate(continuation)] = crash_continuations[0].alternatives.as_slice()
    else {
        panic!("callee member route survives the fixed-index call")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            root: continuation_root,
            path: continuation_path,
        },
    ) = continuation.proposition()
    else {
        panic!("continuation is a canonical structural Boolean path")
    };
    assert_eq!(*continuation_root, root.structural_parameters[0].place);
    let [
        CanonicalStructuralPathSegment::FixedIndex(0),
        CanonicalStructuralPathSegment::Field(leaf),
    ] = continuation_path.as_slice()
    else {
        panic!("fixed index precedes the callee-relative Boolean field")
    };
    let [CrashRouteGuard::Predicate(helper_predicate)] =
        helper.contract.crash_routes[0].alternatives.as_slice()
    else {
        panic!("callee publishes one Boolean member route")
    };
    let Proposition::Equal(
        _,
        ScalarTerm::BooleanField {
            path: helper_path, ..
        },
    ) = helper_predicate.proposition()
    else {
        panic!("callee route retains its member")
    };
    assert_eq!(helper_path, &[CanonicalStructuralPathSegment::Field(*leaf)]);

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier independently composes the fixed index and callee member");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("fixed-index member route has an acyclic fixed-fuel certificate");
    assert_eq!(fixed.ceiling_units(), 4);
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");

    let semantics = encode_module(&lowered.semantic_module).expect("semantic encode");
    assert_eq!(
        decode_module(&semantics),
        Ok(lowered.semantic_module.clone())
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    let argument = TerminalStructuralValue {
        opaque_identity: 13,
        structural_type: root.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
        &mut Accept,
    )
    .expect("fixed-index member contract remains verified metadata at interpretation");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), fixed.ceiling_units());

    let mut out_of_bounds = lowered.semantic_module.clone();
    let OperationKind::CallUnit {
        crash_continuations,
        ..
    } = &mut out_of_bounds.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let CrashRouteGuard::Predicate(predicate) = &mut crash_continuations[0].alternatives[0] else {
        unreachable!()
    };
    let Proposition::Equal(_, ScalarTerm::BooleanField { root, path }) = predicate.proposition()
    else {
        unreachable!()
    };
    let root = *root;
    let mut path = path.clone();
    path[0] = CanonicalStructuralPathSegment::FixedIndex(1);
    *predicate = CrashPredicateTerm::new(Proposition::Equal(
        ScalarTerm::boolean(true),
        ScalarTerm::boolean_field_path(root, path),
    ));
    let invalid_result = psi_terminal_verifier::validate_module(&out_of_bounds);
    assert!(
        matches!(
            invalid_result,
            Err(psi_terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
        ),
        "unexpected fixed-index validation result: {invalid_result:?}"
    );
}

#[test]
fn verifier_rejects_empty_truncated_and_mistyped_boolean_field_paths() {
    let tokens = Lexer::new(NESTED_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("nested Boolean member crash route lowers");
    let CrashRouteGuard::Predicate(predicate) =
        &lowered.semantic_module.machines[0].contract.crash_routes[0].alternatives[0]
    else {
        panic!("nested route is a predicate")
    };
    let Proposition::Equal(_, ScalarTerm::BooleanField { path, .. }) = predicate.proposition()
    else {
        panic!("nested route is a Boolean field path")
    };
    let [outer, leaf] = path.as_slice() else {
        panic!("nested route has two fields")
    };

    for invalid_path in [
        Vec::new(),
        vec![*outer],
        vec![*leaf, *outer],
        vec![*outer, *leaf, *leaf],
    ] {
        let mut malformed = lowered.semantic_module.clone();
        let replace_path = |predicate: &mut CrashPredicateTerm| {
            let Proposition::Equal(_, ScalarTerm::BooleanField { root, .. }) =
                predicate.proposition()
            else {
                panic!("member route remains a Boolean field predicate")
            };
            let root = *root;
            *predicate = CrashPredicateTerm::new(Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field_path(root, invalid_path.clone()),
            ));
        };
        for machine in &mut malformed.machines {
            for route in &mut machine.contract.crash_routes {
                for alternative in &mut route.alternatives {
                    let CrashRouteGuard::Predicate(predicate) = alternative else {
                        continue;
                    };
                    replace_path(predicate);
                }
            }
            for operation in &mut machine.blocks[0].operations {
                let OperationKind::CallUnit {
                    crash_continuations,
                    ..
                } = &mut operation.kind
                else {
                    continue;
                };
                for route in crash_continuations {
                    for alternative in &mut route.alternatives {
                        let CrashRouteGuard::Predicate(predicate) = alternative else {
                            continue;
                        };
                        replace_path(predicate);
                    }
                }
            }
        }
        let result = psi_terminal_verifier::validate_module(&malformed);
        assert!(
            matches!(
                result,
                Err(psi_terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. })
            ),
            "unexpected validation result: {result:?}"
        );
    }
}
