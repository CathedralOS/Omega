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
/// `OperationKind::ScalarStructuralField`.
pub(super) const SCALAR_STRUCTURAL_FIELD: u8 = 47;
/// `OperationKind::CallStructuralWithScalarArguments`.
pub(super) const CALL_STRUCTURAL_WITH_SCALAR_ARGUMENTS: u8 = 50;
/// `OperationKind::ByteSequenceLength`.
// Tag 55 is retired: its whole-place payload must never be reinterpreted.
pub(super) const BYTE_SEQUENCE_LENGTH: u8 = 115;
/// `OperationKind::ByteSequenceRead`.
pub(super) const BYTE_SEQUENCE_READ: u8 = 56;
/// `OperationKind::ByteSequenceSubslice`.
// Tag 57 is retired: its whole-place payload must never be reinterpreted.
pub(super) const BYTE_SEQUENCE_SUBSLICE: u8 = 113;
/// `OperationKind::StructuralByteSequenceFieldStore`.
pub(super) const STRUCTURAL_BYTE_SEQUENCE_FIELD_STORE: u8 = 58;
/// `OperationKind::StructuralByteSequenceFieldLength`.
pub(super) const STRUCTURAL_BYTE_SEQUENCE_FIELD_LENGTH: u8 = 59;
/// `OperationKind::StructuralByteSequenceFieldByteStore`.
pub(super) const STRUCTURAL_BYTE_SEQUENCE_FIELD_BYTE_STORE: u8 = 60;
/// `OperationKind::EstablishPrimitiveLocal`.
// Tag 61 is retired: it omitted declaration-owned receiving evidence.
pub(super) const ESTABLISH_PRIMITIVE_LOCAL: u8 = 117;
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
// Tags 73 and 74 carried the canonical-path primitive read and store, 76 the
// runtime-indexed store and 87 the runtime-indexed read. A primitive leaf
// projection is now a structural path whose elements may be runtime-selected
// (tags 89 and 90); the retired tags are refused rather than reread.
/// `OperationKind::StructuralScalarFieldStore` (with range obligation).
pub(super) const RANGE_CHECKED_STRUCTURAL_SCALAR_FIELD_STORE: u8 = 75;
/// `OperationKind::MoveStructuralField`.
pub(super) const MOVE_STRUCTURAL_FIELD: u8 = 77;
/// `OperationKind::StoreStructuralField`.
pub(super) const STORE_STRUCTURAL_FIELD: u8 = 78;
/// `OperationKind::EstablishElementView`.
pub(super) const ESTABLISH_ELEMENT_VIEW: u8 = 79;
// Tag 80 retired: its whole-place payload cannot represent a projected
// descriptor. Reject old bytes instead of reinterpreting their source identity.
/// `OperationKind::ElementViewRead`.
pub(super) const ELEMENT_VIEW_READ: u8 = 81;
/// `OperationKind::ElementViewSubslice`.
// Tag 82 is retired: its whole-place payload must never be reinterpreted.
pub(super) const ELEMENT_VIEW_SUBSLICE: u8 = 114;
/// `OperationKind::EstablishStructuralCase`.
pub(super) const ESTABLISH_STRUCTURAL_CASE: u8 = 83;
/// `OperationKind::StructuralLeafCopy`.
pub(super) const STRUCTURAL_LEAF_COPY: u8 = 84;

/// `OperationKind::StructuralCaseLeafCopy`.
pub(super) const STRUCTURAL_CASE_LEAF_COPY: u8 = 85;

/// `OperationKind::StructuralByteSequenceFieldRead`.
pub(super) const STRUCTURAL_BYTE_SEQUENCE_FIELD_READ: u8 = 86;

/// `OperationKind::PrimitiveScalarRead` over a nonempty structural path.
pub(super) const PROJECTED_PRIMITIVE_SCALAR_READ: u8 = 89;

/// `OperationKind::WriteOnlyPrimitiveStore` over a nonempty structural path.
pub(super) const PROJECTED_WRITE_ONLY_PRIMITIVE_STORE: u8 = 90;

/// `OperationKind::TrappingInteger`, followed by one primitive sub-tag.
pub(super) const TRAPPING_INTEGER: u8 = 91;

/// `OperationKind::ElementViewRead` over a nonempty element path.
pub(super) const PROJECTED_ELEMENT_VIEW_READ: u8 = 92;

/// `OperationKind::AtomicAccess`, followed by the place, its path and one
/// event sub-tag.
pub(super) const ATOMIC_ACCESS: u8 = 93;

/// `OperationKind::EstablishZeroedStructural`.
pub(super) const ESTABLISH_ZEROED_STRUCTURAL: u8 = 94;

/// `OperationKind::EstablishByteSequenceConcatenation`.
pub(super) const ESTABLISH_BYTE_SEQUENCE_CONCATENATION: u8 = 95;

/// Whole borrowed bounded-owner content replacement.
pub(super) const BYTE_SEQUENCE_STORE: u8 = 96;

/// `OperationKind::ElementViewWrite`.
pub(super) const ELEMENT_VIEW_WRITE: u8 = 97;

/// `OperationKind::EstablishMutableElementView`.
pub(super) const ESTABLISH_MUTABLE_ELEMENT_VIEW: u8 = 98;

/// `OperationKind::FloatResultPolicy`, followed by its policy and payload IDs.
pub(super) const FLOAT_RESULT_POLICY: u8 = 99;

/// `OperationKind::StructuralByteViewFieldStore`.
pub(super) const STRUCTURAL_BYTE_VIEW_FIELD_STORE: u8 = 100;

/// `OperationKind::ScalarRepresentationView` with no landing obligation:
/// the total bit re-views (IEEE float and integer, Boolean to byte).
pub(super) const SCALAR_REPRESENTATION_VIEW: u8 = 101;

/// `OperationKind::ScalarRepresentationView` with its `{0, 1}` landing
/// obligation: an 8-bit integer re-viewed as `bool`.
pub(super) const LANDED_SCALAR_REPRESENTATION_VIEW: u8 = 102;

/// `OperationKind::EstablishStructuralLocal`.
pub(super) const ESTABLISH_STRUCTURAL_LOCAL: u8 = 103;
/// `OperationKind::ByteSequencePredicate`.
pub(super) const BYTE_SEQUENCE_PREDICATE: u8 = 104;

/// `OperationKind::ByteSequenceEqual`.
pub(super) const BYTE_SEQUENCE_EQUAL: u8 = 105;
/// A shared-view field store with an exact current-source predicate obligation.
pub(super) const PREDICATE_CHECKED_STRUCTURAL_BYTE_VIEW_FIELD_STORE: u8 = 106;

/// Complete owned structural array construction.
pub(super) const ESTABLISH_STRUCTURAL_ARRAY: u8 = 107;

/// Reference release at a nonempty owned descriptor path.
pub(super) const PROJECTED_RELEASE_REFERENCE: u8 = 108;

/// `OperationKind::SelectImplementation`.
pub(super) const SELECT_IMPLEMENTATION: u8 = 109;
/// `OperationKind::CallDynamic`.
pub(super) const CALL_DYNAMIC: u8 = 110;

/// A scalar reference retaining source-selected byte storage interpretation.
pub(super) const ESTABLISH_STORAGE_VIEW_REFERENCE: u8 = 111;

/// `OperationKind::ElementViewLength` with the complete structural operand.
pub(super) const ELEMENT_VIEW_LENGTH: u8 = 112;
/// Primitive replacement with an exact declaration-owned receiving restriction.
pub(super) const RESTRICTED_WRITE_ONLY_PRIMITIVE_STORE: u8 = 116;

/// Whole-value replacement of intact unrestricted storage.
pub(super) const REPLACE_DISCARDABLE_STRUCTURAL_FIELD: u8 = 118;

/// Place-free atomic ordering event.
pub(super) const ATOMIC_FENCE: u8 = 119;

/// Explicit initialized-RAM placed interpretation; distinct from recasts.
pub(super) const ESTABLISH_PLACED_REFERENCE: u8 = 120;
