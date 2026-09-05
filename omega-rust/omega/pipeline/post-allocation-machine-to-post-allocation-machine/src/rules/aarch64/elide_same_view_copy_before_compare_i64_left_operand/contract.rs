use selected_instructions::{MachineAlternativeFamily, SelectedInstructionKind};

use crate::Aarch64SameViewCopyElisionPolicy;

use super::super::same_view_copy_before_compare::{
    CompareConsumerContract, CompareProvenanceContract,
};

pub(super) const CONTRACT: CompareConsumerContract = CompareConsumerContract {
    policy:
        Aarch64SameViewCopyElisionPolicy::Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1,
    kind: SelectedInstructionKind::CompareI64,
    family: MachineAlternativeFamily::CompareI64,
    operand_count: 2,
    external_reads: &[0, 1],
    consumed_operand: 0,
    provenance: CompareProvenanceContract::ConsumedOriginAndRetainedValue,
};
