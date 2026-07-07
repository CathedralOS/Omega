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
    Subtract,
    Multiply,
    Divide,
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
    /// `BranchArmsEnd` marker.
    ForwardBranchSkip,
    /// A zero-byte marker placed after all arms of a multi-arm guarded transition;
    /// the target of every `ForwardBranchSkip` for that transition.
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
}
