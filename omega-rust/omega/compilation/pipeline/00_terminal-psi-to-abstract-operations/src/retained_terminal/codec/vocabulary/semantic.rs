//! Ordinary native-input vocabulary. Field and variant coverage is exhaustive.

use super::record_fields;

record_variants!(numerics::arithmetic::ArithmeticDomain {
    0 => Exact,
    1 => Wrapping,
    2 => Saturating,
    3 => Trapping,
});

record_variants!(semantic_vocabulary::BoundaryCompletion {
    0 => NormalReturn,
    1 => External(value0),
});

record_variants!(symbols::BuiltinFunction {
    0 => FloatMaximum,
    1 => FloatMinimum,
    2 => FloatSquareRoot,
    3 => AsmHlt,
    4 => AsmPortOut,
    5 => AsmPortIn,
    6 => AsmLoadFence,
    7 => AsmStoreFence,
    8 => AsmFullFence,
    9 => AsmAarch64LoadFence,
    10 => AsmAarch64StoreFence,
    11 => AsmAarch64FullFence,
    12 => AsmDisableInterrupts,
    13 => AsmEnableInterrupts,
    14 => AsmSnapshotFlags,
    15 => AsmRestoreFlags,
    16 => AsmReadMsr,
    17 => AsmWriteMsr,
    18 => AsmReadCr0,
    19 => AsmReadCr2,
    20 => AsmReadCr3,
    21 => AsmReadCr4,
    22 => AsmWriteCr0,
    23 => AsmWriteCr3,
    24 => AsmWriteCr4,
    25 => AsmSerialize,
    26 => AsmInstructionSyncBarrier,
    27 => AsmSpinPause,
    28 => AsmYieldHint,
    29 => AsmNop,
    30 => AsmWaitForEvent,
    31 => AsmWaitForInterrupt,
    32 => AsmSendEvent,
    33 => AsmSendEventLocal,
    34 => AsmWriteBackInvalidate,
    35 => AsmInvalidate,
    36 => AsmWriteBackNoInvalidate,
    37 => AsmReadSctlrEl1,
    38 => AsmReadTcrEl1,
    39 => AsmReadTtbr0El1,
    40 => AsmReadTtbr1El1,
    41 => AsmReadMairEl1,
    42 => AsmReadVbarEl1,
    43 => AsmReadTpidrEl1,
    44 => AsmReadEsrEl1,
    45 => AsmReadFarEl1,
    46 => AsmWriteSctlrEl1,
    47 => AsmWriteTcrEl1,
    48 => AsmWriteTtbr0El1,
    49 => AsmWriteTtbr1El1,
    50 => AsmWriteMairEl1,
    51 => AsmWriteVbarEl1,
    52 => AsmWriteTpidrEl1,
    53 => FloatIsNan,
    54 => FloatMultiplyThenAddF32,
    55 => FloatMultiplyThenAddF64,
    56 => FloatFusedMultiplyAddF32,
    57 => FloatFusedMultiplyAddF64,
    58 => FloatIsFinite,
    59 => FloatIsInfinite,
    60 => FloatIsNormal,
    61 => FloatIsSubnormal,
    62 => FloatClassifyF32,
    63 => FloatClassifyF64,
    64 => ContentOld,
    65 => ContentSeparate,
    66 => FloatAddTowardZeroF32,
    67 => FloatAddTowardZeroF64,
    68 => FloatAddTowardPositiveF32,
    69 => FloatAddTowardPositiveF64,
    70 => FloatAddTowardNegativeF32,
    71 => FloatAddTowardNegativeF64,
    72 => FloatSubtractTowardZeroF32,
    73 => FloatSubtractTowardZeroF64,
    74 => FloatSubtractTowardPositiveF32,
    75 => FloatSubtractTowardPositiveF64,
    76 => FloatSubtractTowardNegativeF32,
    77 => FloatSubtractTowardNegativeF64,
    78 => FloatMultiplyTowardZeroF32,
    79 => FloatMultiplyTowardZeroF64,
    80 => FloatMultiplyTowardPositiveF32,
    81 => FloatMultiplyTowardPositiveF64,
    82 => FloatMultiplyTowardNegativeF32,
    83 => FloatMultiplyTowardNegativeF64,
    84 => FloatDivideTowardZeroF32,
    85 => FloatDivideTowardZeroF64,
    86 => FloatDivideTowardPositiveF32,
    87 => FloatDivideTowardPositiveF64,
    88 => FloatDivideTowardNegativeF32,
    89 => FloatDivideTowardNegativeF64,
    90 => FloatSqrtTowardZeroF32,
    91 => FloatSqrtTowardZeroF64,
    92 => FloatSqrtTowardPositiveF32,
    93 => FloatSqrtTowardPositiveF64,
    94 => FloatSqrtTowardNegativeF32,
    95 => FloatSqrtTowardNegativeF64,
    96 => FloatFusedMultiplyAddTowardZeroF32,
    97 => FloatFusedMultiplyAddTowardZeroF64,
    98 => FloatFusedMultiplyAddTowardPositiveF32,
    99 => FloatFusedMultiplyAddTowardPositiveF64,
    100 => FloatFusedMultiplyAddTowardNegativeF32,
    101 => FloatFusedMultiplyAddTowardNegativeF64,
    102 => IntegerEmbed,
});

record_variants!(language_semantics::CarryAddress {
    0 => Stable,
    1 => Movable,
});

record_variants!(language_semantics::CarryCpu {
    0 => Origin,
    1 => Any,
});

record_variants!(language_semantics::CarryHostThread {
    0 => Origin,
    1 => Any,
});

record_fields!(language_semantics::CarryPolicy {
    suspension,
    cpu,
    host_thread,
    address
});

record_variants!(language_semantics::CarrySuspension {
    0 => Forbidden,
    1 => Allowed,
});

record_variants!(terminal_psi::CrashCause {
    0 => Trap,
    1 => Abort,
});

record_fields!(diagnostics::Diagnostic {
    severity,
    message,
    exceeded_provision,
    source_span,
    resolved_location
});

record_variants!(diagnostics::DiagnosticSeverity {
    0 => Error,
    1 => Warning,
});

record_variants!(language_semantics::DomainPredicateBody {
    0 => Bodyless,
    1 => Present,
});

record_variants!(semantic_vocabulary::ExternalCompletionKind {
    0 => ProcessExit,
});

record_variants!(numerics::literals::FloatFormat {
    0 => F32,
    1 => F64,
});

record_fields!(diagnostics::ResolvedSourceLocation {
    package,
    path,
    line,
    column
});

record_fields!(source::SourceSpan { source_id, span });

record_variants!(terminal_psi::TerminalPlacementLifecycleOperation {
    0 => Initialize,
    1 => MoveOut,
    2 => View,
});
