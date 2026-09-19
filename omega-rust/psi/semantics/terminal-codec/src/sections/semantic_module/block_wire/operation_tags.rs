//! The wire tag of every operation kind, in tag order. A new kind takes
//! the next free tag; the family files name these constants, never the
//! numbers, so the encoder and decoder cannot disagree.

/// `OperationKind::IntegerConstant`.
pub(super) const INTEGER_CONSTANT: u8 = 1;
/// `OperationKind::BooleanConstant`.
pub(super) const BOOLEAN_CONSTANT: u8 = 2;
/// `OperationKind::WrappingIntegerAdd`.
pub(super) const WRAPPING_INTEGER_ADD: u8 = 3;
/// `OperationKind::SaturatingIntegerAdd`.
pub(super) const SATURATING_INTEGER_ADD: u8 = 4;
/// `OperationKind::WrappingIntegerSubtract`.
pub(super) const WRAPPING_INTEGER_SUBTRACT: u8 = 5;
/// `OperationKind::SaturatingIntegerSubtract`.
pub(super) const SATURATING_INTEGER_SUBTRACT: u8 = 6;
/// `OperationKind::WrappingIntegerMultiply`.
pub(super) const WRAPPING_INTEGER_MULTIPLY: u8 = 7;
/// `OperationKind::SaturatingIntegerMultiply`.
pub(super) const SATURATING_INTEGER_MULTIPLY: u8 = 8;
/// `OperationKind::BooleanNot`.
pub(super) const BOOLEAN_NOT: u8 = 9;
/// `OperationKind::BooleanEqual`.
pub(super) const BOOLEAN_EQUAL: u8 = 10;
/// `OperationKind::IntegerEqual`.
pub(super) const INTEGER_EQUAL: u8 = 11;
/// `OperationKind::IntegerLessThan`.
pub(super) const INTEGER_LESS_THAN: u8 = 12;
/// `OperationKind::IntegerLessOrEqual`.
pub(super) const INTEGER_LESS_OR_EQUAL: u8 = 13;
/// `OperationKind::IntegerBitwiseAnd`.
pub(super) const INTEGER_BITWISE_AND: u8 = 14;
/// `OperationKind::IntegerBitwiseOr`.
pub(super) const INTEGER_BITWISE_OR: u8 = 15;
/// `OperationKind::IntegerBitwiseXor`.
pub(super) const INTEGER_BITWISE_XOR: u8 = 16;
/// `OperationKind::WrappingIntegerShiftLeft`.
pub(super) const WRAPPING_INTEGER_SHIFT_LEFT: u8 = 17;
/// `OperationKind::WrappingIntegerShiftRight`.
pub(super) const WRAPPING_INTEGER_SHIFT_RIGHT: u8 = 18;
/// `OperationKind::IntegerBitwiseNot`.
pub(super) const INTEGER_BITWISE_NOT: u8 = 19;
/// `OperationKind::IntegerWiden`.
pub(super) const INTEGER_WIDEN: u8 = 20;
/// `OperationKind::IntegerExactCast`.
pub(super) const INTEGER_EXACT_CAST: u8 = 21;
/// `OperationKind::ExactIntegerShiftRight`.
pub(super) const EXACT_INTEGER_SHIFT_RIGHT: u8 = 22;
/// `OperationKind::ExactIntegerShiftLeft`.
pub(super) const EXACT_INTEGER_SHIFT_LEFT: u8 = 23;
/// `OperationKind::ExactIntegerAdd`.
pub(super) const EXACT_INTEGER_ADD: u8 = 24;
/// `OperationKind::ExactIntegerSubtract`.
pub(super) const EXACT_INTEGER_SUBTRACT: u8 = 25;
/// `OperationKind::ExactIntegerMultiply`.
pub(super) const EXACT_INTEGER_MULTIPLY: u8 = 26;
/// `OperationKind::ExactIntegerDivide`.
pub(super) const EXACT_INTEGER_DIVIDE: u8 = 27;
/// `OperationKind::ExactIntegerRemainder`.
pub(super) const EXACT_INTEGER_REMAINDER: u8 = 28;
/// `OperationKind::WrappingIntegerDivide`.
pub(super) const WRAPPING_INTEGER_DIVIDE: u8 = 29;
/// `OperationKind::WrappingIntegerRemainder`.
pub(super) const WRAPPING_INTEGER_REMAINDER: u8 = 30;
/// `OperationKind::SaturatingIntegerDivide`.
pub(super) const SATURATING_INTEGER_DIVIDE: u8 = 31;
/// `OperationKind::SaturatingIntegerRemainder`.
pub(super) const SATURATING_INTEGER_REMAINDER: u8 = 32;
/// `OperationKind::Call`.
pub(super) const CALL: u8 = 33;
/// `OperationKind::CallUnit`.
pub(super) const CALL_UNIT: u8 = 34;
/// `OperationKind::BoundaryCall`.
pub(super) const BOUNDARY_CALL: u8 = 35;
/// `OperationKind::PortWrite`.
pub(super) const PORT_WRITE: u8 = 36;
/// `OperationKind::EstablishTrivialAffineLocal`.
pub(super) const ESTABLISH_TRIVIAL_AFFINE_LOCAL: u8 = 37;
/// `OperationKind::BooleanStructuralField`.
pub(super) const BOOLEAN_STRUCTURAL_FIELD: u8 = 38;
/// `OperationKind::CallStructuralScalar`.
pub(super) const CALL_STRUCTURAL_SCALAR: u8 = 39;
/// `OperationKind::EstablishByteSequenceLiteral`.
pub(super) const ESTABLISH_BYTE_SEQUENCE_LITERAL: u8 = 40;
/// `OperationKind::CallStructural`.
pub(super) const CALL_STRUCTURAL: u8 = 41;
/// `OperationKind::EstablishScalarCase`.
pub(super) const ESTABLISH_SCALAR_CASE: u8 = 42;
/// `OperationKind::WriteOnlyPrimitiveStore`.
pub(super) const WRITE_ONLY_PRIMITIVE_STORE: u8 = 43;
/// `OperationKind::IeeeFloatConstant`.
pub(super) const IEEE_FLOAT_CONSTANT: u8 = 44;
/// `OperationKind::NearestIeeeFloatFusedMultiplyAdd`.
pub(super) const NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD: u8 = 45;
/// `OperationKind::StructuralScalarFieldStore`.
pub(super) const STRUCTURAL_SCALAR_FIELD_STORE: u8 = 46;
/// `OperationKind::IntegerStructuralField`.
pub(super) const INTEGER_STRUCTURAL_FIELD: u8 = 47;
/// `OperationKind::CallDynamicScalar`.
pub(super) const CALL_DYNAMIC_SCALAR: u8 = 48;
/// `OperationKind::CallDynamicParameterScalar`.
pub(super) const CALL_DYNAMIC_PARAMETER_SCALAR: u8 = 49;
/// `OperationKind::CallStructuralWithScalarArguments`.
pub(super) const CALL_STRUCTURAL_WITH_SCALAR_ARGUMENTS: u8 = 50;
/// `OperationKind::CallDynamicUnit`.
pub(super) const CALL_DYNAMIC_UNIT: u8 = 52;
/// `OperationKind::CallDynamicParameterUnit`.
pub(super) const CALL_DYNAMIC_PARAMETER_UNIT: u8 = 53;
/// `OperationKind::StoreDynamicDescriptor`.
pub(super) const STORE_DYNAMIC_DESCRIPTOR: u8 = 54;
/// `OperationKind::ByteSequenceLength`.
pub(super) const BYTE_SEQUENCE_LENGTH: u8 = 55;
/// `OperationKind::ByteSequenceRead`.
pub(super) const BYTE_SEQUENCE_READ: u8 = 56;
/// `OperationKind::ByteSequenceSubslice`.
pub(super) const BYTE_SEQUENCE_SUBSLICE: u8 = 57;
/// `OperationKind::StructuralByteSequenceFieldStore`.
pub(super) const STRUCTURAL_BYTE_SEQUENCE_FIELD_STORE: u8 = 58;
/// `OperationKind::StructuralByteSequenceFieldLength`.
pub(super) const STRUCTURAL_BYTE_SEQUENCE_FIELD_LENGTH: u8 = 59;
/// `OperationKind::StructuralByteSequenceFieldByteStore`.
pub(super) const STRUCTURAL_BYTE_SEQUENCE_FIELD_BYTE_STORE: u8 = 60;
/// `OperationKind::EstablishPrimitiveLocal`.
pub(super) const ESTABLISH_PRIMITIVE_LOCAL: u8 = 61;
/// `OperationKind::PrimitiveScalarRead`.
pub(super) const PRIMITIVE_SCALAR_READ: u8 = 62;
/// `OperationKind::ByteSequenceWrite`.
pub(super) const BYTE_SEQUENCE_WRITE: u8 = 63;
/// `OperationKind::EstablishScalarArray`.
pub(super) const ESTABLISH_SCALAR_ARRAY: u8 = 64;
/// `OperationKind::IeeeFloatCompare`.
pub(super) const IEEE_FLOAT_COMPARE: u8 = 65;
/// `OperationKind::StructuralCaseMembership`.
pub(super) const STRUCTURAL_CASE_MEMBERSHIP: u8 = 66;
/// `OperationKind::EstablishRecord`.
pub(super) const ESTABLISH_RECORD: u8 = 68;
/// `OperationKind::EstablishReference`.
pub(super) const ESTABLISH_REFERENCE: u8 = 69;
/// `OperationKind::ReleaseReference`.
pub(super) const RELEASE_REFERENCE: u8 = 70;
/// `OperationKind::PrimitiveScalarRead` (projected path).
pub(super) const PROJECTED_PRIMITIVE_SCALAR_READ: u8 = 73;
/// `OperationKind::WriteOnlyPrimitiveStore` (projected path).
pub(super) const PROJECTED_WRITE_ONLY_PRIMITIVE_STORE: u8 = 74;
/// `OperationKind::StructuralScalarFieldStore` (with range obligation).
pub(super) const RANGE_CHECKED_STRUCTURAL_SCALAR_FIELD_STORE: u8 = 75;
/// `OperationKind::WriteOnlyIndexedPrimitiveStore`.
pub(super) const WRITE_ONLY_INDEXED_PRIMITIVE_STORE: u8 = 76;
/// `OperationKind::MoveStructuralField`.
pub(super) const MOVE_STRUCTURAL_FIELD: u8 = 77;
/// `OperationKind::StoreStructuralField`.
pub(super) const STORE_STRUCTURAL_FIELD: u8 = 78;
