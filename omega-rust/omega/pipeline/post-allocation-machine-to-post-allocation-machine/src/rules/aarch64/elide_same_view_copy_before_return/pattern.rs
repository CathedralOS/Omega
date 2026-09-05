use register_model::RegisterOperandAccess;
use selected_instructions::{
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
const X30: UnitSetPattern = UnitSetPattern::named(&["x30"]);
const PC: UnitSetPattern = UnitSetPattern::named(&["pc"]);

const SOURCE: OperandCoordinate = OperandCoordinate {
    instruction: PairInstruction::First,
    operand: 0,
};
const DESTINATION: OperandCoordinate = OperandCoordinate {
    instruction: PairInstruction::First,
    operand: 1,
};
const RETURNED: OperandCoordinate = OperandCoordinate {
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

const RETURN_OPERANDS: [OperandPattern; 1] = [OperandPattern {
    operand: 0,
    access: RegisterOperandAccess::Use,
    read: OperandReadPattern::StorageUnits,
    write: OperandWritePattern::Empty,
    view: ViewPattern::Named {
        name: "x0",
        bits: 64,
    },
    fixed_view: FixedViewPattern::Named("x0"),
    tied_to: None,
    early_clobber: false,
}];

const RELATIONS: [OperandRelation; 2] = [
    OperandRelation::SamePhysicalViewAndStorageUnits(SOURCE, DESTINATION),
    OperandRelation::SameVirtualRegister(DESTINATION, RETURNED),
];

pub(crate) const AARCH64_SAME_VIEW_COPY_BEFORE_RETURN_V1: InstructionPairPattern =
    InstructionPairPattern::new(
        InstructionPairPatternId::Aarch64SameViewCopyI64BeforeReturnV1,
        InstructionPairTopology::BodyTailAndTerminatorV1,
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
            semantic: MachineSemanticKind::ReturnI64,
            selected_operand_count: 1,
            family: MachineAlternativeFamily::ReturnI64,
            variant: 0,
            external_reads: &[],
            external_writes: &[],
            implicit_uses: X30,
            implicit_defs: PC,
            implicit_clobbers: EMPTY,
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: ControlPattern::ReturnIndirectNamed("x30"),
            operands: &RETURN_OPERANDS,
        },
        EMPTY,
        EMPTY,
        &RELATIONS,
        &[RETURNED],
    );
