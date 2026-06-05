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
    Max,
    Min,
    And,
    Or,
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
}
