use omega_selected_instructions::{MachineAlternativeFamily, MachineSemanticKind};

use crate::rules::peephole_matching::{InstructionPairPatternId, OperandRelation};

#[test]
fn descriptor_names_the_exact_pair_relations_and_dynamic_liveness() {
    let pattern = super::super::pattern::AARCH64_SAME_VIEW_COPY_BEFORE_RETURN_V1;
    assert_eq!(
        pattern.id,
        InstructionPairPatternId::Aarch64SameViewCopyI64BeforeReturnV1
    );
    assert_eq!(pattern.first().semantic, MachineSemanticKind::CopyI64);
    assert_eq!(pattern.first().family, MachineAlternativeFamily::CopyI64);
    assert_eq!(pattern.second().semantic, MachineSemanticKind::ReturnI64);
    assert_eq!(pattern.second().family, MachineAlternativeFamily::ReturnI64);
    assert_eq!(pattern.relations().len(), 2);
    assert!(matches!(
        pattern.relations()[0],
        OperandRelation::SamePhysicalViewAndStorageUnits(_, _)
    ));
    assert!(matches!(
        pattern.relations()[1],
        OperandRelation::SameVirtualRegister(_, _)
    ));
    assert_eq!(pattern.live_through_operands().len(), 1);
}
