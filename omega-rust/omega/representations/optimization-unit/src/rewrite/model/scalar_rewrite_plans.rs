use super::super::*;
use super::foundations::NodeLocation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IntegerConstantRewrite {
    pub location: NodeLocation,
    pub source_operation: OperationId,
    pub result: ValueId,
    pub scalar_type: IntegerType,
    pub constant: IntegerValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BooleanConstantRewrite {
    pub location: NodeLocation,
    pub source_operation: OperationId,
    pub result: ValueId,
    pub constant: bool,
}

/// Remove one unused, independently total scalar-producing node. Its source
/// occurrence and fuel are fused into the immediately following direct node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeadScalarNodeRewrite {
    pub location: NodeLocation,
    pub source_operation: OperationId,
    pub result: ValueId,
    pub scalar_type: ScalarType,
}

/// Closed, obligation-free scalar identities whose result is exactly an
/// existing operand for every value of the declared integer and count types.
/// Operation policies remain distinct rows and rule identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TotalScalarIdentityKind {
    WrappingIntegerAddZeroLeft,
    WrappingIntegerAddZeroRight,
    WrappingIntegerSubtractZeroRight,
    WrappingIntegerMultiplyOneLeft,
    WrappingIntegerMultiplyOneRight,
    WrappingIntegerShiftLeftZeroCount,
    WrappingIntegerShiftRightZeroCount,
    WrappingIntegerMultiplyZeroLeft,
    WrappingIntegerMultiplyZeroRight,
    SaturatingIntegerAddZeroLeft,
    SaturatingIntegerAddZeroRight,
    SaturatingIntegerSubtractZeroRight,
    SaturatingIntegerMultiplyOneLeft,
    SaturatingIntegerMultiplyOneRight,
    SaturatingIntegerMultiplyZeroLeft,
    SaturatingIntegerMultiplyZeroRight,
    IntegerBitwiseAndAllOnesLeft,
    IntegerBitwiseAndAllOnesRight,
    IntegerBitwiseOrZeroLeft,
    IntegerBitwiseOrZeroRight,
    IntegerBitwiseXorZeroLeft,
    IntegerBitwiseXorZeroRight,
    IntegerBitwiseAndZeroLeft,
    IntegerBitwiseAndZeroRight,
    IntegerBitwiseOrAllOnesLeft,
    IntegerBitwiseOrAllOnesRight,
}

/// Remove one total integer identity and replace every use of its live result
/// with the equivalent existing value operand. The source operation policy is
/// retained by the identity kind, including any distinct shift-count type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TotalScalarIdentityRewrite {
    pub location: NodeLocation,
    pub source_operation: OperationId,
    pub result: ValueId,
    pub replacement: ValueId,
    pub scalar_type: IntegerType,
    pub identity: TotalScalarIdentityKind,
}

/// The closed integer identities whose verifier-accepted obligation permits
/// the operation to disappear while its live result is replaced by an
/// existing operand. Exact, wrapping, and saturating operation identities are
/// never reclassified across policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofCertifiedScalarIdentityKind {
    ExactIntegerAddZeroLeft,
    ExactIntegerAddZeroRight,
    ExactIntegerSubtractZeroRight,
    ExactIntegerMultiplyOneLeft,
    ExactIntegerMultiplyOneRight,
    ExactIntegerShiftLeftZeroCount,
    ExactIntegerShiftRightZeroCount,
    ExactIntegerDivideOneRight,
    WrappingIntegerDivideOneRight,
    SaturatingIntegerDivideOneRight,
    ExactIntegerMultiplyZeroLeft,
    ExactIntegerMultiplyZeroRight,
    ExactIntegerDivideZeroLeft,
    WrappingIntegerDivideZeroLeft,
    SaturatingIntegerDivideZeroLeft,
    ExactIntegerRemainderZeroLeft,
    WrappingIntegerRemainderZeroLeft,
    SaturatingIntegerRemainderZeroLeft,
    ExactIntegerShiftLeftZeroValue,
    ExactIntegerShiftRightZeroValue,
    ExactIntegerShiftRightNegativeOneValue,
}

/// Remove one proof-certified integer identity and replace every use of its
/// live result with the equivalent existing operand. The removed occurrence
/// and fuel remain realized at the next co-executed node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofCertifiedScalarIdentityRewrite {
    pub location: NodeLocation,
    pub source_operation: OperationId,
    pub result: ValueId,
    pub replacement: ValueId,
    pub scalar_type: IntegerType,
    pub identity: ProofCertifiedScalarIdentityKind,
}

/// Replace every use of a later same-block scalar result with the result of an
/// earlier, independently identical total scalar expression, then remove the
/// redundant node. The later source occurrence remains realized at its next
/// co-executed node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalScalarCommonSubexpressionRewrite {
    pub leader: NodeLocation,
    pub redundant: NodeLocation,
    pub leader_operation: OperationId,
    pub redundant_operation: OperationId,
    pub leader_result: ValueId,
    pub redundant_result: ValueId,
    pub scalar_type: ScalarType,
}

/// Replace every use of a scalar result with an equivalent result defined in
/// a different block that independently dominates the redundant definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DominatingScalarCommonSubexpressionRewrite {
    pub leader: NodeLocation,
    pub redundant: NodeLocation,
    pub leader_operation: OperationId,
    pub redundant_operation: OperationId,
    pub leader_result: ValueId,
    pub redundant_result: ValueId,
    pub scalar_type: ScalarType,
}

/// One incoming control-flow arm supplying a value already computed for a
/// phi-translated total scalar expression at the target block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhiTranslatedScalarIncoming {
    pub source: BlockId,
    pub edge: EdgeId,
    pub leader: NodeLocation,
    pub leader_operation: OperationId,
    pub leader_result: ValueId,
}

/// Preserve the redundant result identity as a new target-block parameter,
/// bind every incoming edge to its available translated leader, and remove the
/// now-redundant target-block computation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhiTranslatedScalarGvnRewrite {
    pub redundant: NodeLocation,
    pub redundant_operation: OperationId,
    pub redundant_result: ValueId,
    pub scalar_type: ScalarType,
    pub parameter_position: u32,
    pub incoming: Vec<PhiTranslatedScalarIncoming>,
}
