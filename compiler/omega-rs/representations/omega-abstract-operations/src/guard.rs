#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateGuardOperator {
    #[default]
    None,
    Equal,
    NotEqual,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    Add,
    /// One-step floating add under an explicit rounding direction. These are
    /// distinct named operations, never ambient-mode variants of `Add`.
    AddTowardZero,
    AddTowardPositive,
    AddTowardNegative,
    Subtract,
    SubtractTowardZero,
    SubtractTowardPositive,
    SubtractTowardNegative,
    Multiply,
    MultiplyTowardZero,
    MultiplyTowardPositive,
    MultiplyTowardNegative,
    Divide,
    DivideTowardZero,
    DivideTowardPositive,
    DivideTowardNegative,
    Modulo,
    ShiftLeft,
    ShiftRight,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    /// Unsigned counterparts, selected when the operands are an unsigned integer
    /// type. They differ from the signed forms only in the machine encoding
    /// (`div`/`shr` vs `idiv`/`sar`).
    DivideUnsigned,
    ModuloUnsigned,
    ShiftRightLogical,
    MinUnsigned,
    MaxUnsigned,
    LessUnsigned,
    LessOrEqualUnsigned,
    GreaterUnsigned,
    GreaterOrEqualUnsigned,
    Max,
    Min,
    And,
    Or,
    /// `sqrt(x)`: a UNARY float op carried on the binary value-write path with
    /// both operands = `x`; the encoder reads the first SSE register only.
    Sqrt,
    SqrtTowardZero,
    SqrtTowardPositive,
    SqrtTowardNegative,
    /// Unary IEEE NaN predicate. It is carried on the binary value-write path
    /// with one real operand and one ignored zero placeholder so `x` is
    /// evaluated exactly once.
    IsNan,
    /// Unary IEEE finite predicate, carried like `IsNan` with one real
    /// operand and one ignored zero placeholder.
    IsFinite,
    /// Unary IEEE infinity predicate.
    IsInfinite,
    /// Unary IEEE normal predicate (finite, nonzero exponent field).
    IsNormal,
    /// Unary IEEE subnormal predicate (zero exponent, nonzero fraction).
    IsSubnormal,
    /// Unary IEEE classifier. The raw source float is carried on the left;
    /// native lowering returns the stable packed `FloatClass` enum carrier.
    FloatClassify,
    /// Internal structural carrier for the second and third operands of a
    /// ternary float realization. It returns the third operand while keeping
    /// the second in the architecture's pinned ternary-float scratch register.
    FloatPair,
    /// `round(round(a * b) + c)`, with all three original operands retained
    /// for exactly-once evaluation and final-result policy adaptation.
    MultiplyThenAdd,
    /// `round(a * b + c)`, with one target fused operation and all three
    /// original operands retained for exactly-once evaluation and final-result
    /// policy adaptation.
    FusedMultiplyAdd,
    FusedMultiplyAddTowardZero,
    FusedMultiplyAddTowardPositive,
    FusedMultiplyAddTowardNegative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateGuardLowering {
    NoOp,
    CompareStaticValue,
    CompareRuntimeValue,
    #[default]
    NeedsRuntimeExpression,
    /// An unconditional forward jump emitted after a MATCHED arm's body in a
    /// multi-arm guarded transition, to skip the remaining sibling arms (which
    /// would otherwise execute and clobber this arm's effect). Jumps to the next
    /// `BranchArmsEnd` marker carrying the same branch-scope identity.
    ForwardBranchSkip,
    /// A zero-byte marker placed after all arms of a multi-arm guarded transition;
    /// the target of every same-scoped `ForwardBranchSkip` for that transition.
    BranchArmsEnd,
    /// POISON: an inline-leaf VALUE arm whose guard selection could NOT
    /// resolve. The arm's compare and its result write would both have been
    /// silently dropped (the call would return a stale 0), so selection emits
    /// this marker instead and emission planning rejects it with a hard
    /// "needs lowering" diagnostic. It must never encode (zero bytes) and
    /// never reach a runnable image.
    ///
    /// Distinct from `NeedsRuntimeExpression`, which dispatch edges use as an
    /// intentional zero-width "unconditionally enter" fallthrough (e.g. the
    /// false arm of a string-equality transition).
    UnresolvedInlineArmGuard,
    /// POISON: a machine/state TERMINAL VALUE that is a bare CALL expression
    /// no write strategy could lower (a host-boundary call in value-return
    /// position: `machine close(..) -> i32 { self.host.close(fd) }`). The
    /// call would silently never be emitted and its result slot would read
    /// ZII 0 -- `Filesystem::close` reported rc 0 "success" while the fd
    /// stayed open. Selection emits this marker instead; emission planning
    /// rejects it with a bind-to-a-`let` diagnostic. Zero bytes; must never
    /// reach a runnable image.
    UnloweredTerminalHostCall,
    /// POISON: a CASE/STRUCT-LITERAL construction with a payload field whose
    /// VALUE no operand strategy could lower (`z: self.name == "omega"` --
    /// text equality has no branch-resolver arm, for one). The construction
    /// cascade writes the tag and each field independently and used to OR
    /// per-field success, so the one unresolvable field was silently DROPPED
    /// while its siblings landed and the field read ZII 0 (the
    /// cast-in-payload face was the first instance; this marker closes the
    /// hole for every future field shape). Selection emits this instead of
    /// skipping the field; emission planning rejects it with a
    /// bind-to-a-`let` diagnostic. Zero bytes; must never reach a runnable
    /// image.
    UnloweredCaseLiteralField,
}
