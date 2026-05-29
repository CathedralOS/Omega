use omega_machine_bytes::{
    EncodedMachineBoundarySummary, EncodedMachineOwnershipSummary, EncodedMachinePlan,
    EncodedMachineSemanticSummary, EncodedMachineValueSummary,
};
use omega_machine_instructions::MachineInstructionPlan;
use omega_object_file::{ObjectPlan, RelocationPlan};
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendArtifactRoots {
    pub machine_instructions: MachineInstructionPlan,
    pub encoded_machine: EncodedMachinePlan,
    pub object: ObjectPlan,
    pub relocations: RelocationPlan,
}

impl BackendArtifactRoots {
    pub fn empty_for_target(target: NativeTarget) -> Self {
        Self {
            machine_instructions: MachineInstructionPlan::with_capacity(target, 0, 0),
            encoded_machine: EncodedMachinePlan::with_capacity(target, 0, 0, 0),
            object: ObjectPlan::with_capacity(target, 0, 0),
            relocations: RelocationPlan::with_record_capacity(target, 0),
        }
    }

    pub fn semantic_summary(&self) -> &EncodedMachineSemanticSummary {
        &self.encoded_machine.semantics
    }

    pub fn value_summary(&self) -> &EncodedMachineValueSummary {
        &self.encoded_machine.semantics.values
    }

    pub fn boundary_summary(&self) -> &EncodedMachineBoundarySummary {
        &self.encoded_machine.semantics.boundary_edges
    }

    pub fn ownership_summary(&self) -> &EncodedMachineOwnershipSummary {
        &self.encoded_machine.semantics.ownership
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_abstract_operations::{AbstractBoundaryPolicyCheck, AbstractBoundaryPolicyVerdict};
    use std::sync::Arc;

    #[test]
    fn exposes_encoded_semantic_spine_from_backend_artifact_roots() {
        let target = NativeTarget::host();
        let mut artifacts = BackendArtifactRoots::empty_for_target(target);

        artifacts
            .encoded_machine
            .semantics
            .boundary_edges
            .policy_checks
            .insert(AbstractBoundaryPolicyCheck {
                boundary_policy: Arc::from("omega::core::Slice::Index"),
                verdict: AbstractBoundaryPolicyVerdict::Accepted,
                ..AbstractBoundaryPolicyCheck::default()
            });

        assert_eq!(
            artifacts
                .semantic_summary()
                .boundary_edges
                .policy_checks
                .len(),
            1
        );
        assert_eq!(artifacts.boundary_summary().policy_checks.len(), 1);
        assert_eq!(artifacts.value_summary().values.len(), 0);
        assert_eq!(artifacts.ownership_summary().moves.len(), 0);

        let (_, check) = artifacts
            .boundary_summary()
            .policy_checks
            .iter()
            .next()
            .expect("boundary policy check should stay visible at artifact root");
        assert_eq!(check.boundary_policy.as_ref(), "omega::core::Slice::Index");
        assert_eq!(check.verdict, AbstractBoundaryPolicyVerdict::Accepted);
    }
}
