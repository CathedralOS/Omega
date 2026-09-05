use selected_instructions::{MachineAlternativeFamily, MachineSemanticKind};

use crate::rules::peephole_matching::{
    InstructionPairPatternId, InstructionPairTopology, OperandCoordinate, OperandRelation,
    PairInstruction,
};

#[test]
fn descriptor_names_the_exact_adjacent_pair_and_right_operand_relation() {
    let pattern = super::super::pattern::AARCH64_SAME_VIEW_COPY_BEFORE_COMPARE_I64_RIGHT_OPERAND_V1;
    assert_eq!(
        pattern.id,
        InstructionPairPatternId::Aarch64SameViewCopyI64BeforeCompareI64RightOperandV1
    );
    assert_eq!(
        pattern.topology(),
        InstructionPairTopology::AdjacentBodyInstructionsV1
    );
    assert_eq!(pattern.first().semantic, MachineSemanticKind::CopyI64);
    assert_eq!(pattern.first().family, MachineAlternativeFamily::CopyI64);
    assert_eq!(pattern.second().semantic, MachineSemanticKind::CompareI64);
    assert_eq!(
        pattern.second().family,
        MachineAlternativeFamily::CompareI64
    );
    assert_eq!(pattern.second().selected_operand_count, 2);
    assert_eq!(pattern.second().external_reads, [0, 1]);
    assert_eq!(pattern.relations().len(), 2);
    assert!(matches!(
        pattern.relations()[0],
        OperandRelation::SamePhysicalViewAndStorageUnits(_, _)
    ));
    assert_eq!(
        pattern.relations()[1],
        OperandRelation::SameVirtualRegister(
            OperandCoordinate {
                instruction: PairInstruction::First,
                operand: 1,
            },
            OperandCoordinate {
                instruction: PairInstruction::Second,
                operand: 1,
            },
        )
    );
    assert_eq!(
        pattern.live_through_operands(),
        &[OperandCoordinate {
            instruction: PairInstruction::Second,
            operand: 1,
        }]
    );
}
