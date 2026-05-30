use omega_machine_bytes::EncodedMachineSemanticSummary;

use crate::MachineEmissionInput;

pub(crate) fn build_encoded_machine_semantic_summary(
    input: &MachineEmissionInput<'_, '_>,
) -> EncodedMachineSemanticSummary {
    input.machine_instructions.semantics.clone()
}
