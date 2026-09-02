use omega_selected_instructions::{MachineAlternativeFamily, SelectedInstructionKind};

use crate::Aarch64SameViewCopyElisionPolicy;

use super::super::same_view_copy_before_compare::{
    CompareConsumerContract, CompareProvenanceContract,
};

pub(super) const CONTRACT: CompareConsumerContract = CompareConsumerContract {
    policy: Aarch64SameViewCopyElisionPolicy::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1,
    kind: SelectedInstructionKind::CompareI64Zero,
    family: MachineAlternativeFamily::CompareI64Zero,
    operand_count: 1,
    external_reads: &[0],
    consumed_operand: 0,
    provenance: CompareProvenanceContract::ExactCopyValue,
};
