use omega_machine_bytes::EncodedMachineSemanticSummary;

use crate::MachineEmissionInput;

pub(crate) fn build_encoded_machine_semantic_summary(
    input: &MachineEmissionInput<'_, '_>,
) -> EncodedMachineSemanticSummary {
    EncodedMachineSemanticSummary::with_roots(
        input.machine_instructions.semantics.values.clone(),
        input.machine_instructions.semantics.boundaries.clone(),
        input.machine_instructions.semantics.ownership.clone(),
    )
}
