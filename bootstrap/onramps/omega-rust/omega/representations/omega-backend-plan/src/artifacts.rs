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
    pub fn with_roots(
        machine_instructions: MachineInstructionPlan,
        encoded_machine: EncodedMachinePlan,
        object: ObjectPlan,
        relocations: RelocationPlan,
    ) -> Self {
        Self {
            machine_instructions,
            encoded_machine,
            object,
            relocations,
        }
    }

    pub fn empty_for_target(target: NativeTarget) -> Self {
        Self::with_roots(
            MachineInstructionPlan::with_capacity(target, 0, 0),
            EncodedMachinePlan::with_capacity(target, 0, 0, 0),
            ObjectPlan::with_capacity(target, 0, 0),
            RelocationPlan::with_record_capacity(target, 0),
        )
    }

    pub fn semantic_summary(&self) -> &EncodedMachineSemanticSummary {
        &self.encoded_machine.semantics
    }

    pub fn value_summary(&self) -> &EncodedMachineValueSummary {
        &self.encoded_machine.semantics.values
    }

    pub fn boundary_summary(&self) -> &EncodedMachineBoundarySummary {
        &self.encoded_machine.semantics.boundaries
    }

    pub fn boundary_footprints(&self) -> &omega_abstract_operations::BoundaryFootprintPlan {
        &self.encoded_machine.semantics.boundaries.footprints
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
            .boundaries
            .policy_checks
            .insert(AbstractBoundaryPolicyCheck {
                boundary_policy: Arc::from("omega::core::Slice::Index"),
                verdict: AbstractBoundaryPolicyVerdict::Accepted,
                ..AbstractBoundaryPolicyCheck::default()
            });

        assert_eq!(
            artifacts.semantic_summary().boundaries.policy_checks.len(),
            1
        );
        assert_eq!(artifacts.boundary_summary().policy_checks.len(), 1);
        assert_eq!(
            artifacts.boundary_footprints(),
            &omega_abstract_operations::BoundaryFootprintPlan::default()
        );
        assert_eq!(artifacts.value_summary().values.len(), 0);
        assert_eq!(artifacts.ownership_summary().permissions.len(), 0);

        let (_, check) = artifacts
            .boundary_summary()
            .policy_checks
            .iter()
            .next()
            .expect("boundary policy check should stay visible at artifact root");
        assert_eq!(check.boundary_policy.as_ref(), "omega::core::Slice::Index");
        assert_eq!(check.verdict, AbstractBoundaryPolicyVerdict::Accepted);
    }

    #[test]
    fn backend_artifact_constructor_keeps_artifact_roots_explicit() {
        let target = NativeTarget::host();
        let machine_instructions = MachineInstructionPlan::with_capacity(target, 1, 2);
        let encoded_machine = EncodedMachinePlan::with_capacity(target, 3, 4, 5);
        let object = ObjectPlan::with_capacity(target, 6, 7);
        let relocations = RelocationPlan::with_record_capacity(target, 8);

        let artifacts = BackendArtifactRoots::with_roots(
            machine_instructions.clone(),
            encoded_machine.clone(),
            object.clone(),
            relocations.clone(),
        );

        assert_eq!(artifacts.machine_instructions, machine_instructions);
        assert_eq!(artifacts.encoded_machine, encoded_machine);
        assert_eq!(artifacts.object, object);
        assert_eq!(artifacts.relocations, relocations);
    }
}
