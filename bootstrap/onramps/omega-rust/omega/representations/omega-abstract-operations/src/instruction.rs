mod function;
mod operand;
mod operation;
mod operation_kind;
mod place;
mod storage;
mod value_operand;

pub use function::{AbstractFunctionPlan, FunctionInstructionPlan};
pub use operand::{InstructionOperand, InstructionOperandKind};
pub use operation::{AbstractOperation, SelectedInstruction};
pub use operation_kind::{
    AbstractOperationDomain, AbstractOperationKind, CopyPlacesRole, SelectedInstructionKind,
};
pub use place::{PLACE_MAX_STEPS, Place, PlaceStep};
pub use storage::RuntimeStorageRegion;
pub use value_operand::{
    AbstractValueOperand, AbstractValueOperandHandle, RuntimeBitFieldFragment, RuntimeValueOperand,
    RuntimeValueOperandHandle, ValueOperand, ValueOperandHandle,
};

/// Storage shape receiving a runtime text line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTextReadTarget {
    /// A `{ptr, len}` String descriptor backed by a separate data object.
    StringDescriptor,
    /// An owned domain-qualified `[u8; N]` carrier laid out as `{len, bytes}`.
    BoundedByteBuffer,
    /// A raw always-full `[u8; N]` array used as disposable input scratch.
    FixedByteArray,
}
