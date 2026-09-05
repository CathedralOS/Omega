use selected_instructions::{MachineAlternativeFamily, MachineSemanticKind};

use crate::rules::peephole_matching::{
    InstructionPairPatternId, InstructionPairTopology, OperandRelation,
};

#[test]
fn descriptor_names_the_exact_adjacent_pair_relations() {
    let pattern = super::super::pattern::AARCH64_SAME_VIEW_COPY_BEFORE_COMPARE_I64_LEFT_OPERAND_V1;
    assert_eq!(
        pattern.id,
        InstructionPairPatternId::Aarch64SameViewCopyI64BeforeCompareI64LeftOperandV1
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
    assert!(matches!(
        pattern.relations()[1],
        OperandRelation::SameVirtualRegister(_, _)
    ));
}
