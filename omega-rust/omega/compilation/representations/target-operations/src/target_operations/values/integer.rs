//! Integer expression vocabulary and exact proof-bearing arithmetic.

use crate::{ScalarParameterLocation, TargetByteView, TargetElementView};
use abstract_operations::{IntegerArithmetic, IntegerShiftDirection, IntegerShiftPolicy};
use calling_conventions::ValuePlacement;
use semantic_vocabulary::{
    IntegerType, IntegerValue, OperationId, PlaceId, StructuralFieldId, ValueId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetIntegerExpression {
    /// Read the exact value supplied on entry to its declaring block.
    BlockParameter(crate::TargetScalarBlockValue),
    /// Read one exact earlier scalar definition without repeating its producer.
    ScalarHome(crate::TargetUnitScalarHomeRequirement),
    /// Exact `u8` observation. The view retains its parameter placement or
    /// checked derivation. The index expression retains its Terminal
    /// identity; length and obligation retain proof custody, not a new check.
    ByteSequenceRead {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        view: Box<TargetByteView>,
        index: Box<TargetIntegerExpression>,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    ByteSequenceLength {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        view: Box<TargetByteView>,
        length_byte_offset: u32,
    },
    /// Exact element observation over an element-view descriptor. The index
    /// expression retains its Terminal identity; the element stride is fixed
    /// by the view's element declaration. The path selects the scalar leaf at
    /// element_byte_offset, independently replayed from that declaration.
    ElementViewRead {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        view: Box<TargetElementView>,
        index: Box<TargetIntegerExpression>,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        element_byte_offset: u32,
    },
    ElementViewLength {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        view: Box<TargetElementView>,
        length_byte_offset: u32,
    },
    Immediate {
        source_value: ValueId,
        value: IntegerValue,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    StructuralField {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        field: StructuralFieldId,
        source_placement: ValuePlacement,
        field_byte_offset: u32,
        integer_type: IntegerType,
    },
    BitwiseNot {
        psi_operation: OperationId,
        operand: Box<TargetIntegerExpression>,
    },
    IntegerWiden {
        psi_operation: OperationId,
        source_type: IntegerType,
        operand: Box<TargetIntegerExpression>,
    },
    IntegerExactCast {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        source_type: IntegerType,
        operand: Box<TargetIntegerExpression>,
    },
    BitwiseAnd {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    BitwiseOr {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    BitwiseXor {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    /// One fixed-width shift of `value` by an independently typed `count`.
    /// `policy` carries the verifier-discharged in-range count obligation
    /// exactly when the shift is exact.
    Shift {
        psi_operation: OperationId,
        policy: IntegerShiftPolicy,
        direction: IntegerShiftDirection,
        count_type: IntegerType,
        value: Box<TargetIntegerExpression>,
        count: Box<TargetIntegerExpression>,
    },
    /// One fixed-width binary arithmetic operation over two operands of the
    /// enclosing expression's carrier; `arithmetic` names its policy pair and
    /// the obligation that pair requires.
    Arithmetic {
        psi_operation: OperationId,
        arithmetic: IntegerArithmetic,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    /// A Terminal `Trapping` primitive over its operand expressions: the
    /// exact result, or a `Trap` crash at this operation when the settled
    /// Trapping predicate holds. The enclosing expression names the result
    /// carrier; `operand_type` is a shift count's or a conversion source's
    /// type (the result carrier for binary arithmetic). `right` is the second
    /// operand or shift count and is absent exactly for a conversion.
    Trapping {
        psi_operation: OperationId,
        primitive: terminal_psi::TrappingIntegerPrimitive,
        operand_type: IntegerType,
        left: Box<TargetIntegerExpression>,
        right: Option<Box<TargetIntegerExpression>>,
    },
}
