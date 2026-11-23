#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FloatFormat {
    Binary32,
    Binary64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FloatOperation {
    Negate,
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FloatSemantics {
    Ieee754NearestEvenV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FloatRealization {
    pub format: FloatFormat,
    pub operation: FloatOperation,
    pub semantics: FloatSemantics,
}
