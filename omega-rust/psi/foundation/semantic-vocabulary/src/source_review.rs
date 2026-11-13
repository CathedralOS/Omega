/*
Portable source-review vocabulary shared by records and source representations.
These kinds describe data; they neither run a checker nor establish a verdict.
Variant order and classification remain the existing encoded vocabulary.
*/

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum IntegerBinaryKind {
    ExactAdd,
    ExactSubtract,
    ExactMultiply,
    ExactDivide,
    ExactRemainder,
    WrappingDivide,
    WrappingRemainder,
    SaturatingDivide,
    SaturatingRemainder,
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    WrappingShiftLeft,
    WrappingShiftRight,
    ExactShiftLeft,
    ExactShiftRight,
    SaturatingShiftLeft,
    SaturatingShiftRight,
    TrappingShiftLeft,
    TrappingShiftRight,
    TrappingAdd,
    TrappingSubtract,
    TrappingMultiply,
    TrappingDivide,
    TrappingRemainder,
}

impl IntegerBinaryKind {
    pub const fn is_trapping(self) -> bool {
        matches!(
            self,
            Self::TrappingShiftLeft
                | Self::TrappingShiftRight
                | Self::TrappingAdd
                | Self::TrappingSubtract
                | Self::TrappingMultiply
                | Self::TrappingDivide
                | Self::TrappingRemainder
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, compiler_storage::Record)]
pub enum SemanticDependencyKind {
    NominalIdentity,
    Layout,
    OwnershipBehavior,
    AutomaticCleanup,
    AutomaticCleanupMachine,
}
