pub mod data;
pub mod instruction;

pub use data::{
    TargetDataObject, TargetDataObjectHandle, TargetDataObjectKind, TargetDataPlan,
    target_data_handle_from_abstract,
};
pub use instruction::{
    AbstractBoundaryPolicyCheck, AbstractBoundaryPolicyVerdict, AbstractDataObjectHandle,
    FunctionInstructionPlan, GENERATED_IDT_WRITER_CONTEXT_ABI_V1,
    GENERATED_IDT_WRITER_DESTINATION_OFFSET, GENERATED_IDT_WRITER_SOURCE_SLOT_WIDTH,
    GENERATED_IDT_WRITER_SOURCE_SLOTS_OFFSET, GeneratedIdtWriterStep, HostOperationKey,
    InstructionOperand, InstructionOperandKind, InstructionOperandLike, InstructionPlan,
    RuntimeStorageRegion, RuntimeTextReadSource, RuntimeValueOperand, RuntimeValueOperandHandle,
    RuntimeValueOperandSource, SelectedInstruction, SelectedInstructionKind, TargetBoundarySummary,
    TargetHostBinding, TargetInstructionOperand, TargetInstructionOperandKind, TargetOperation,
    TargetOperationCode, TargetOperationDomain, TargetOperationFunction, TargetOperationKind,
    TargetOperationPlan, TargetOwnershipSummary, TargetSemanticSummary, TargetValueOperand,
    TargetValueOperandHandle, TargetValueSummary, generated_idt_writer_context_byte_len,
};
pub use omega_abstract_operations::{
    BoundaryFootprintFragment, BoundaryFootprintFragmentOrigin, BoundaryFootprintPlan,
    PLACE_MAX_STEPS, Place, PlaceStep, StateGuardLowering, StateGuardOperator,
};
