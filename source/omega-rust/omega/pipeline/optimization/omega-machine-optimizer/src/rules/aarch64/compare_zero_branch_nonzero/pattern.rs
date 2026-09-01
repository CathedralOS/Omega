use omega_register_model::RegisterOperandAccess;
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineSemanticKind,
};

use crate::rules::peephole_matching::{
    InstructionPattern, OperandPattern, TerminalPairPattern, TerminalPairPatternId, UnitSetPattern,
    ViewPattern,
};

const EMPTY: UnitSetPattern = UnitSetPattern::EMPTY;
const NZCV: UnitSetPattern = UnitSetPattern::named(&["nzcv"]);
const PC: UnitSetPattern = UnitSetPattern::named(&["pc"]);
const NZCV_AND_PC: UnitSetPattern = UnitSetPattern::named(&["nzcv", "pc"]);

const COMPARE_OPERANDS: [OperandPattern; 1] = [OperandPattern {
    operand: 0,
    access: RegisterOperandAccess::Use,
    read_equals_storage: true,
    writes_empty: true,
    no_write_semantics: true,
    view: ViewPattern::IndexedAllocatable {
        prefix: 'x',
        maximum_index: 30,
        bits: 64,
    },
}];

pub(crate) const AARCH64_CBNZ_TERMINAL_PAIR_V1: TerminalPairPattern = TerminalPairPattern::new(
    TerminalPairPatternId::Aarch64CompareI64ZeroBranchNonZeroV1,
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
        control: MachineEncodedControlEffect::FallThroughV1,
        operands: &COMPARE_OPERANDS,
    },
    InstructionPattern {
        semantic: MachineSemanticKind::ConditionalBranchNonZero,
        selected_operand_count: 0,
        family: MachineAlternativeFamily::ConditionalBranchNonZero,
        variant: 0,
        external_reads: &[],
        external_writes: &[],
        implicit_uses: NZCV_AND_PC,
        implicit_defs: PC,
        implicit_clobbers: EMPTY,
        memory: MachineEncodedMemoryEffect::NoneV1,
        stack: MachineEncodedStackEffect::UnchangedV1,
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        control: MachineEncodedControlEffect::ConditionalRelativeBranchV1,
        operands: &[],
    },
    NZCV,
    NZCV,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_names_the_complete_terminal_pair_contract() {
        let pattern = AARCH64_CBNZ_TERMINAL_PAIR_V1;
        assert_eq!(
            pattern.id,
            TerminalPairPatternId::Aarch64CompareI64ZeroBranchNonZeroV1
        );
        let compare = pattern.first();
        assert_eq!(compare.semantic, MachineSemanticKind::CompareI64Zero);
        assert_eq!(compare.family, MachineAlternativeFamily::CompareI64Zero);
        assert_eq!(compare.variant, 0);
        assert_eq!(compare.external_reads, [0]);
        assert!(compare.external_writes.is_empty());
        assert_eq!(compare.implicit_uses, EMPTY);
        assert_eq!(compare.implicit_defs, NZCV);
        assert_eq!(compare.implicit_clobbers, EMPTY);
        assert_eq!(compare.memory, MachineEncodedMemoryEffect::NoneV1);
        assert_eq!(compare.stack, MachineEncodedStackEffect::UnchangedV1);
        assert_eq!(compare.trap, MachineEncodedTrapBehavior::NeverV1);
        assert_eq!(compare.control, MachineEncodedControlEffect::FallThroughV1);
        assert_eq!(compare.operands, COMPARE_OPERANDS);

        let branch = pattern.second();
        assert_eq!(
            branch.semantic,
            MachineSemanticKind::ConditionalBranchNonZero
        );
        assert_eq!(
            branch.family,
            MachineAlternativeFamily::ConditionalBranchNonZero
        );
        assert_eq!(branch.variant, 0);
        assert!(branch.external_reads.is_empty());
        assert!(branch.external_writes.is_empty());
        assert_eq!(branch.implicit_uses, NZCV_AND_PC);
        assert_eq!(branch.implicit_defs, PC);
        assert_eq!(branch.implicit_clobbers, EMPTY);
        assert_eq!(branch.memory, MachineEncodedMemoryEffect::NoneV1);
        assert_eq!(branch.stack, MachineEncodedStackEffect::UnchangedV1);
        assert_eq!(
            branch.trap,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        );
        assert_eq!(
            branch.control,
            MachineEncodedControlEffect::ConditionalRelativeBranchV1
        );
        assert!(branch.operands.is_empty());
        assert_eq!(pattern.live_through(), NZCV);
        assert_eq!(pattern.dead_after(), NZCV);
    }
}
