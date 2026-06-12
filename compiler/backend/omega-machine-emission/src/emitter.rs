use crate::code::build_encoded_machine_code;
use crate::semantics::build_encoded_machine_semantic_summary;
use omega_core::diagnostics::Diagnostic;
use omega_machine_bytes::EncodedMachinePlan;
use omega_machine_instructions::MachineInstructionPlan;
use omega_target::NativeTarget;

#[derive(Debug)]
pub struct MachineEmissionInput<'plan, 'machine> {
    pub target: NativeTarget,
    pub assigned_target_operations:
        &'plan omega_assigned_target_operations::AssignedTargetOperationPlan,
    pub machine_instructions: &'machine MachineInstructionPlan,
    pub host_abi: &'plan omega_calling_conventions::HostAbiPlan,
    pub data: &'plan omega_target_operations::TargetDataPlan,
    pub terminal_dispatch_index: u32,
}

pub fn emit_machine_bytes(
    input: MachineEmissionInput<'_, '_>,
) -> Result<EncodedMachinePlan, Diagnostic> {
    Ok(EncodedMachinePlan::with_roots(
        input.target,
        build_encoded_machine_code(&input)?,
        build_encoded_machine_semantic_summary(&input),
    ))
}

#[cfg(test)]
mod tests {
    use super::{MachineEmissionInput, emit_machine_bytes};
    use omega_assigned_target_operations::{AssignedTargetOperationPlan, SelectedInstructionKind};
    use omega_calling_conventions::build_host_abi_plan;
    use omega_core::arena::HandleSpan;
    use omega_machine_instructions::{
        AbstractBoundaryPolicyCheck, AbstractBoundaryPolicyVerdict, MachineInstruction,
        MachineInstructionFunction, MachineInstructionKind, MachineInstructionPlan,
    };
    use omega_target::NativeTarget;

    #[test]
    fn copies_machine_semantic_summaries_to_encoded_plan() {
        let target = NativeTarget::host();
        let assigned_target_operations = AssignedTargetOperationPlan::default();
        let host_abi = build_host_abi_plan(target);
        let data = omega_target_operations::TargetDataPlan::default();
        let mut machine_instructions = MachineInstructionPlan::with_capacity(target, 1, 2);
        // EnterFunction is deliberately zero-width on x86_64 (the backend
        // manages frames without a prologue), so the function also needs a
        // LeaveFunction -- which encodes a real `ret` on every architecture --
        // for the encoded byte count to be observable on any host.
        let instructions = machine_instructions.code.instructions.insert_many([
            MachineInstruction {
                selected_instruction_index: 7,
                source_kind: SelectedInstructionKind::EnterFunction,
                kind: MachineInstructionKind::NoOp,
            },
            MachineInstruction {
                selected_instruction_index: 8,
                source_kind: SelectedInstructionKind::LeaveFunction,
                kind: MachineInstructionKind::NoOp,
            },
        ]);
        machine_instructions
            .code
            .functions
            .insert(MachineInstructionFunction {
                source_key: Default::default(),
                instructions,
            });
        machine_instructions
            .semantics
            .values
            .values
            .insert(Default::default());
        machine_instructions
            .semantics
            .boundaries
            .source_edges
            .insert(Default::default());
        machine_instructions
            .semantics
            .boundaries
            .edges
            .insert(Default::default());
        machine_instructions
            .semantics
            .boundaries
            .policy_checks
            .insert(AbstractBoundaryPolicyCheck {
                boundary_policy: "omega::host::targets::linux".into(),
                verdict: AbstractBoundaryPolicyVerdict::MissingHostBinding,
                ..Default::default()
            });
        machine_instructions
            .semantics
            .ownership
            .moves
            .insert(Default::default());

        let encoded = emit_machine_bytes(MachineEmissionInput {
            target,
            assigned_target_operations: &assigned_target_operations,
            machine_instructions: &machine_instructions,
            host_abi: &host_abi,
            data: &data,
            terminal_dispatch_index: 0,
        })
        .expect("machine emission should preserve semantic summaries");

        assert_eq!(
            encoded.semantics.values.values.len(),
            machine_instructions.semantics.values.values.len()
        );
        assert_eq!(
            encoded.semantics.boundaries.source_edges.len(),
            machine_instructions.semantics.boundaries.source_edges.len()
        );
        assert_eq!(
            encoded.semantics.boundaries.edges.len(),
            machine_instructions.semantics.boundaries.edges.len()
        );
        assert_eq!(
            encoded.semantics.boundaries.policy_checks.len(),
            machine_instructions
                .semantics
                .boundaries
                .policy_checks
                .len()
        );
        let check = encoded
            .semantics
            .boundaries
            .policy_checks
            .iter()
            .next()
            .map(|(_, check)| check)
            .expect("encoded boundary policy check");
        assert_eq!(
            check.verdict,
            AbstractBoundaryPolicyVerdict::MissingHostBinding
        );
        assert_eq!(
            encoded.semantics.ownership.moves.len(),
            machine_instructions.semantics.ownership.moves.len()
        );
        assert_eq!(encoded.code.instructions.len(), 2);
        assert!(encoded.code.byte_count > 0);
        assert_ne!(instructions, HandleSpan::empty());
    }
}
