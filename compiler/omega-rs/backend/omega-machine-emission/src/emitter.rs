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
    use omega_calling_conventions::{
        MachineRegister, MachineStateSet, RegisterSet, StateFootprintEvidence,
    };
    use omega_core::arena::HandleSpan;
    use omega_machine_instructions::{
        AbstractBoundaryPolicyCheck, AbstractBoundaryPolicyVerdict, BoundaryFootprintFragment,
        BoundaryFootprintFragmentOrigin, MachineInstruction, MachineInstructionFunction,
        MachineInstructionKind, MachineInstructionPlan,
    };
    use omega_target::NativeTarget;

    #[test]
    fn copies_machine_semantic_summaries_to_encoded_plan() {
        let target = NativeTarget::host();
        let assigned_target_operations = AssignedTargetOperationPlan::default();
        let host_abi = build_host_abi_plan(target);
        let data = omega_target_operations::TargetDataPlan::default();
        let mut machine_instructions = MachineInstructionPlan::with_capacity(target, 1, 2);
        // Exercise both halves of the fixed ordinary frame so the semantic
        // summary test also crosses architecture-specific entry/return bytes.
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
            .boundaries
            .footprints
            .boundary_contract_fingerprint = Some(0x5678);
        machine_instructions
            .semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::DispatchScaffold,
                evidence: StateFootprintEvidence::new(
                    RegisterSet::new([MachineRegister::X86R12]),
                    MachineStateSet::empty(),
                ),
            });
        machine_instructions
            .semantics
            .ownership
            .permissions
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
            encoded.semantics.ownership.permissions.len(),
            machine_instructions.semantics.ownership.permissions.len()
        );
        assert_eq!(
            encoded.semantics.boundaries.footprints,
            machine_instructions.semantics.boundaries.footprints
        );
        assert_eq!(encoded.code.instructions.len(), 2);
        assert!(encoded.code.byte_count > 0);
        assert_ne!(instructions, HandleSpan::empty());
    }

    #[test]
    fn generated_idt_load_emits_only_the_pinned_x86_instruction() {
        let target = NativeTarget::linux_x64();
        let assigned_target_operations =
            AssignedTargetOperationPlan::with_capacity(target, 0, 0, 0, 0, 0);
        let host_abi = build_host_abi_plan(target);
        let data = omega_target_operations::TargetDataPlan::default();
        let mut machine_instructions = MachineInstructionPlan::with_capacity(target, 1, 1);
        let source_kind = SelectedInstructionKind::GeneratedIdtLoad {
            materialized: omega_external_roots::MaterializedIdtId::from_normalized_identity(1)
                .expect("materialized IDT identity"),
            descriptor: omega_external_roots::IdtDestinationId::from_normalized_identity(2)
                .expect("IDT destination identity"),
            content_fingerprint: 3,
            root_ledger_fingerprint: 4,
            control: omega_external_roots::IdtControlId::from_normalized_identity(5)
                .expect("IDT control identity"),
        };
        let instructions =
            machine_instructions
                .code
                .instructions
                .insert_many([MachineInstruction {
                    selected_instruction_index: 0,
                    source_kind,
                    kind: MachineInstructionKind::GeneratedIdtLoad,
                }]);
        machine_instructions
            .code
            .functions
            .insert(MachineInstructionFunction {
                source_key: Default::default(),
                instructions,
            });

        let encoded = emit_machine_bytes(MachineEmissionInput {
            target,
            assigned_target_operations: &assigned_target_operations,
            machine_instructions: &machine_instructions,
            host_abi: &host_abi,
            data: &data,
            terminal_dispatch_index: 0,
        })
        .expect("generated x86 IDT load should emit");

        assert_eq!(
            encoded.code.bytes.storage_slice(),
            omega_isa_x86_64::encode_lidt_from_r10_bytes()
        );
        assert_eq!(
            encoded.code.byte_count,
            omega_isa_x86_64::lidt_from_r10_width()
        );
    }

    #[test]
    fn generated_idt_load_refuses_aarch64_before_emission() {
        let target = NativeTarget::linux_arm64();
        let assigned_target_operations =
            AssignedTargetOperationPlan::with_capacity(target, 0, 0, 0, 0, 0);
        let host_abi = build_host_abi_plan(target);
        let data = omega_target_operations::TargetDataPlan::default();
        let mut machine_instructions = MachineInstructionPlan::with_capacity(target, 1, 1);
        let instructions =
            machine_instructions
                .code
                .instructions
                .insert_many([MachineInstruction {
                    selected_instruction_index: 0,
                    source_kind: SelectedInstructionKind::GeneratedIdtLoad {
                        materialized:
                            omega_external_roots::MaterializedIdtId::from_normalized_identity(1)
                                .expect("materialized IDT identity"),
                        descriptor:
                            omega_external_roots::IdtDestinationId::from_normalized_identity(2)
                                .expect("IDT destination identity"),
                        content_fingerprint: 3,
                        root_ledger_fingerprint: 4,
                        control: omega_external_roots::IdtControlId::from_normalized_identity(5)
                            .expect("IDT control identity"),
                    },
                    kind: MachineInstructionKind::GeneratedIdtLoad,
                }]);
        machine_instructions
            .code
            .functions
            .insert(MachineInstructionFunction {
                source_key: Default::default(),
                instructions,
            });

        let error = emit_machine_bytes(MachineEmissionInput {
            target,
            assigned_target_operations: &assigned_target_operations,
            machine_instructions: &machine_instructions,
            host_abi: &host_abi,
            data: &data,
            terminal_dispatch_index: 0,
        })
        .expect_err("AArch64 must not silently lower x86 IDT load");

        assert!(error.message.contains("x86_64-only"));
    }
}
