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
    Modulo,
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
}
