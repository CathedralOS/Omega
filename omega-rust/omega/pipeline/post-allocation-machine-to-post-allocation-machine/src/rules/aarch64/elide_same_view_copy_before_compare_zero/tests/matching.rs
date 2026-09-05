use selected_instructions::{MachineAlternativeFamily, MachineSemanticKind};

use crate::rules::peephole_matching::{
    InstructionPairPatternId, InstructionPairTopology, OperandRelation,
};

#[test]
fn descriptor_names_the_exact_adjacent_pair_relations() {
    let pattern = super::super::pattern::AARCH64_SAME_VIEW_COPY_BEFORE_COMPARE_ZERO_V1;
    assert_eq!(
        pattern.id,
        InstructionPairPatternId::Aarch64SameViewCopyI64BeforeCompareZeroV1
    );
    assert_eq!(
        pattern.topology(),
        InstructionPairTopology::AdjacentBodyInstructionsV1
    );
    assert_eq!(pattern.first().semantic, MachineSemanticKind::CopyI64);
    assert_eq!(pattern.first().family, MachineAlternativeFamily::CopyI64);
    assert_eq!(
        pattern.second().semantic,
        MachineSemanticKind::CompareI64Zero
    );
    assert_eq!(
        pattern.second().family,
        MachineAlternativeFamily::CompareI64Zero
    );
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
