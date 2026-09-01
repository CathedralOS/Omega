use omega_register_model::RegisterOperandAccess;
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineSemanticKind,
};

use crate::rules::peephole_matching::{
    ControlPattern, FixedViewPattern, InstructionPairPattern, InstructionPairPatternId,
    InstructionPairTopology, InstructionPattern, OperandCoordinate, OperandPattern,
    OperandReadPattern, OperandRelation, OperandWritePattern, PairInstruction, UnitSetPattern,
    ViewPattern,
};

const EMPTY: UnitSetPattern = UnitSetPattern::EMPTY;
const NZCV: UnitSetPattern = UnitSetPattern::named(&["nzcv"]);
const SOURCE: OperandCoordinate = OperandCoordinate {
    instruction: PairInstruction::First,
    operand: 0,
};
const DESTINATION: OperandCoordinate = OperandCoordinate {
    instruction: PairInstruction::First,
    operand: 1,
};
const COMPARED: OperandCoordinate = OperandCoordinate {
    instruction: PairInstruction::Second,
    operand: 0,
};
const X_REGISTER: ViewPattern = ViewPattern::IndexedAllocatable {
    prefix: 'x',
    maximum_index: 30,
    bits: 64,
};
const COPY_OPERANDS: [OperandPattern; 2] = [
    OperandPattern {
        operand: 0,
        access: RegisterOperandAccess::Use,
        read: OperandReadPattern::StorageUnits,
        write: OperandWritePattern::Empty,
        view: X_REGISTER,
        fixed_view: FixedViewPattern::None,
        tied_to: None,
        early_clobber: false,
    },
    OperandPattern {
        operand: 1,
        access: RegisterOperandAccess::Def,
        read: OperandReadPattern::Empty,
        write: OperandWritePattern::ViewWrite,
        view: X_REGISTER,
        fixed_view: FixedViewPattern::None,
        tied_to: None,
        early_clobber: false,
    },
];
const COMPARE_OPERANDS: [OperandPattern; 1] = [OperandPattern {
    operand: 0,
    access: RegisterOperandAccess::Use,
    read: OperandReadPattern::StorageUnits,
    write: OperandWritePattern::Empty,
    view: X_REGISTER,
    fixed_view: FixedViewPattern::None,
    tied_to: None,
    early_clobber: false,
}];
const RELATIONS: [OperandRelation; 2] = [
    OperandRelation::SamePhysicalViewAndStorageUnits(SOURCE, DESTINATION),
    OperandRelation::SameVirtualRegister(DESTINATION, COMPARED),
];

pub(crate) const AARCH64_SAME_VIEW_COPY_BEFORE_COMPARE_ZERO_V1: InstructionPairPattern =
    InstructionPairPattern::new(
        InstructionPairPatternId::Aarch64SameViewCopyI64BeforeCompareZeroV1,
        InstructionPairTopology::AdjacentBodyInstructionsV1,
        InstructionPattern {
            semantic: MachineSemanticKind::CopyI64,
            selected_operand_count: 2,
            family: MachineAlternativeFamily::CopyI64,
            variant: 0,
            external_reads: &[0],
            external_writes: &[1],
            implicit_uses: EMPTY,
            implicit_defs: EMPTY,
            implicit_clobbers: EMPTY,
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::NeverV1,
            control: ControlPattern::Exact(MachineEncodedControlEffect::FallThroughV1),
            operands: &COPY_OPERANDS,
        },
        InstructionPattern {
            semantic: MachineSemanticKind::CompareI64Zero,
            selected_operand_count: 1,
            family: MachineAlternativeFamily::CompareI64Zero,
            variant: 0,
            external_reads: &[0],
            external_writes: &[],
            implicit_uses: EMPTY,
            implicit_defs: NZCV,
            implicit_clobbers: EMPTY,
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::NeverV1,
            control: ControlPattern::Exact(MachineEncodedControlEffect::FallThroughV1),
            operands: &COMPARE_OPERANDS,
        },
        EMPTY,
        EMPTY,
        &RELATIONS,
        &[COMPARED],
    );
