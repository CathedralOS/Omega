//! Exact Linux x86-64 replay owned beside the physical entry-plan carrier.

use calling_conventions::{
    BoundaryEntryPlan, CallPlan, CallSignature, EntryControl, EntryStack, MachineRegime,
    MachineRegister, MachineState, MachineStateSet, Preemption, RegisterSet, StatePlan,
    ValidatedBoundaryEntryPlan, ValueLocation, ValuePlacement, ValueShape,
    validate_boundary_entry_plan,
};

use super::ProgramEntryPhysicalContractPlan;

const LINUX_X86_64_TARGET_PACKAGE_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/std/targets/linux_x86_64/entry.omg"
));

/// Canonical checked-tree overload identity for `LinuxPhysicalEntry::enter`.
pub const LINUX_X86_64_PHYSICAL_REQUIREMENT_IDENTITY: &str = concat!(
    "named-callable(path(LinuxPhysicalEntry::enter),parameters(",
    "parameter\\(self\\(no\\)\\,mutable\\(no\\)\\,const\\(no\\)\\,",
    "named\\(name\\(addr\\)\\)\\)",
    "),result-dispatch())",
);
/// Canonical normalized type identity of the `initial_stack` arrival address.
pub const LINUX_X86_64_ADDRESS_TYPE_IDENTITY: &str = "named(name(addr))";
/// Canonical normalized type identity of the process-completion status.
pub const LINUX_X86_64_I32_TYPE_IDENTITY: &str = "named(name(i32))";

/// System V AMD64 caller-saved bank: the nine integer argument/return
/// registers plus all sixteen vector registers. The bridge may also observe
/// the incoming stack pointer before switching to its private stack.
const LINUX_X86_64_ORDINARY_CLOBBERS: [MachineRegister; 25] = [
    MachineRegister::X86Rax,
    MachineRegister::X86Rcx,
    MachineRegister::X86Rdx,
    MachineRegister::X86Rsi,
    MachineRegister::X86Rdi,
    MachineRegister::X86R8,
    MachineRegister::X86R9,
    MachineRegister::X86R10,
    MachineRegister::X86R11,
    MachineRegister::X86Xmm(0),
    MachineRegister::X86Xmm(1),
    MachineRegister::X86Xmm(2),
    MachineRegister::X86Xmm(3),
    MachineRegister::X86Xmm(4),
    MachineRegister::X86Xmm(5),
    MachineRegister::X86Xmm(6),
    MachineRegister::X86Xmm(7),
    MachineRegister::X86Xmm(8),
    MachineRegister::X86Xmm(9),
    MachineRegister::X86Xmm(10),
    MachineRegister::X86Xmm(11),
    MachineRegister::X86Xmm(12),
    MachineRegister::X86Xmm(13),
    MachineRegister::X86Xmm(14),
    MachineRegister::X86Xmm(15),
];

/// Strong commitment to the exact closed Linux x86-64 target-package source
/// compiled into this plan owner.
pub fn exact_linux_x86_64_physical_contract_package_source_digest()
-> super::ProgramEntryPhysicalContractPackageSourceDigest {
    super::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
        target::ProgramEntryPhysicalContractPackage::LinuxX86_64,
        LINUX_X86_64_TARGET_PACKAGE_SOURCE,
    )
}

/// Independently reconstruct and validate the exact plan authored by the
/// closed Linux x86-64 target package. The kernel enters a static ELF
/// executable with rsp holding the initial process-stack image and supplies
/// no return continuation, so the authored `LinuxX86_64` policy is a
/// dedicated physical surface rather than the ordinary System V evaluator:
/// the lone `addr` input is the arriving stack pointer itself, and the `i32`
/// result completes through `exit_group` (rax = 231) with status in edi.
pub fn exact_linux_x86_64_physical_boundary_entry_plan() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: Some(ValueShape::integer(4, 4)),
    };
    let plan = BoundaryEntryPlan {
        call: CallPlan {
            policy: calling_conventions::CallingPolicy::SystemVAMD64,
            parameters: vec![ValuePlacement {
                shape: ValueShape::integer(8, 8),
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::X86Rsp,
                    value_byte_offset: 0,
                    byte_size: 8,
                }],
            }],
            result: Some(ValuePlacement {
                shape: ValueShape::integer(4, 4),
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::X86Rdi,
                    value_byte_offset: 0,
                    byte_size: 4,
                }],
            }),
            callback_materializations: Vec::new(),
            ordinary_clobbers: RegisterSet::new(LINUX_X86_64_ORDINARY_CLOBBERS),
            stack_alignment: 16,
            shadow_bytes: 0,
            entry_control: EntryControl::SupervisorCall {
                number_register: MachineRegister::X86Rax,
                immediate: 231,
            },
        },
        state: StatePlan {
            initial_regime: MachineRegime::X86Long64,
            interrupted_state: MachineStateSet::default(),
            saved_state: MachineStateSet::default(),
            restored_state: MachineStateSet::default(),
            permitted_transitive_use: MachineStateSet::new([
                MachineState::GeneralRegisters,
                MachineState::VectorRegisters,
                MachineState::Flags,
            ]),
            stack: EntryStack::ProviderSelected,
            preemption: Preemption::NotApplicable,
        },
    };
    validate_boundary_entry_plan(plan, &signature)
        .expect("the closed target-authored Linux x86-64 physical entry plan must remain valid")
}

impl ProgramEntryPhysicalContractPlan {
    /// Replay the complete target-owned Linux x86-64 physical-entry contract.
    /// Constructor compatibility remains deliberately broader for synthetic
    /// compiler fixtures; runtime custody must use this exact verdict. The
    /// authored contract retains no numeric entry-stack guarantee, so this
    /// verdict rejects any injected closure rather than comparing one.
    pub fn matches_exact_linux_x86_64_physical_contract(&self) -> bool {
        let expected = exact_linux_x86_64_physical_boundary_entry_plan();
        self.target_slot == target::TargetProfile::LinuxX64.program_entry_slot()
            && self.target_package == target::ProgramEntryPhysicalContractPackage::LinuxX86_64
            && self.target_package_source_digest
                == exact_linux_x86_64_physical_contract_package_source_digest()
            && self.requirement_identity == LINUX_X86_64_PHYSICAL_REQUIREMENT_IDENTITY
            && self.parameter_type_identities.len() == 1
            && self.parameter_type_identities[0] == LINUX_X86_64_ADDRESS_TYPE_IDENTITY
            && self.result_type_identity == LINUX_X86_64_I32_TYPE_IDENTITY
            && self.calling_plan_report_fingerprint == expected.contract_report_fingerprint()
            && &self.boundary_entry_plan == expected.plan()
            && self.guaranteed_entry_stack.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LINUX_X86_64_ADDRESS_TYPE_IDENTITY, LINUX_X86_64_I32_TYPE_IDENTITY,
        LINUX_X86_64_PHYSICAL_REQUIREMENT_IDENTITY, ProgramEntryPhysicalContractPlan,
        exact_linux_x86_64_physical_boundary_entry_plan,
        exact_linux_x86_64_physical_contract_package_source_digest,
    };
    use crate::ProgramEntryPhysicalContractPackageSourceDigest;

    fn exact_contract() -> ProgramEntryPhysicalContractPlan {
        let package = target::ProgramEntryPhysicalContractPackage::LinuxX86_64;
        let calling_plan = exact_linux_x86_64_physical_boundary_entry_plan();
        ProgramEntryPhysicalContractPlan::new(
            target::TargetProfile::LinuxX64.program_entry_slot(),
            LINUX_X86_64_PHYSICAL_REQUIREMENT_IDENTITY.into(),
            package,
            exact_linux_x86_64_physical_contract_package_source_digest(),
            1,
            vec![LINUX_X86_64_ADDRESS_TYPE_IDENTITY.into()],
            LINUX_X86_64_I32_TYPE_IDENTITY.into(),
            calling_plan.contract_report_fingerprint(),
            calling_plan.plan().clone(),
        )
        .expect("exact physical contract")
    }

    #[test]
    fn exact_runtime_contract_replays_all_owned_fields() {
        let exact = exact_contract();
        assert!(exact.matches_exact_linux_x86_64_physical_contract());
        assert!(exact.guaranteed_entry_stack().is_none());
        assert!(exact.guaranteed_entry_stack_application().is_none());

        let mut requirement_drift = exact.clone();
        requirement_drift.requirement_identity =
            "named-callable(path(ProgramStorageEntry::enter),parameters(),result-dispatch())"
                .into();
        assert!(!requirement_drift.matches_exact_linux_x86_64_physical_contract());

        let mut parameter_drift = exact.clone();
        parameter_drift
            .parameter_type_identities
            .push(LINUX_X86_64_ADDRESS_TYPE_IDENTITY.into());
        assert!(!parameter_drift.matches_exact_linux_x86_64_physical_contract());

        let mut result_drift = exact.clone();
        result_drift.result_type_identity = "named(name(Unit))".into();
        assert!(!result_drift.matches_exact_linux_x86_64_physical_contract());

        let mut source_drift = exact.clone();
        source_drift.target_package_source_digest =
            ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
                target::ProgramEntryPhysicalContractPackage::LinuxX86_64,
                b"substituted target package",
            );
        assert!(!source_drift.matches_exact_linux_x86_64_physical_contract());

        let mut placement_drift = exact;
        placement_drift.boundary_entry_plan.call.result = placement_drift
            .boundary_entry_plan
            .call
            .parameters
            .first()
            .cloned();
        assert!(!placement_drift.matches_exact_linux_x86_64_physical_contract());
    }

    #[test]
    fn physical_plan_is_exact_kernel_arrival_shape() {
        let plan = exact_linux_x86_64_physical_boundary_entry_plan();
        let call = &plan.plan().call;
        assert_eq!(
            call.policy,
            calling_conventions::CallingPolicy::SystemVAMD64
        );
        let [parameter] = call.parameters.as_slice() else {
            panic!("kernel arrival does not carry exactly one input");
        };
        assert_eq!(
            parameter.locations.as_slice(),
            &[calling_conventions::ValueLocation::Register {
                register: calling_conventions::MachineRegister::X86Rsp,
                value_byte_offset: 0,
                byte_size: 8,
            }]
        );
        let result = call.result.as_ref().expect("kernel arrival completes i32");
        assert_eq!(
            result.locations.as_slice(),
            &[calling_conventions::ValueLocation::Register {
                register: calling_conventions::MachineRegister::X86Rdi,
                value_byte_offset: 0,
                byte_size: 4,
            }]
        );
        assert_eq!(
            call.entry_control,
            calling_conventions::EntryControl::SupervisorCall {
                number_register: calling_conventions::MachineRegister::X86Rax,
                immediate: 231,
            }
        );
        assert_eq!(call.stack_alignment, 16);
        assert_eq!(
            plan.plan().state.stack,
            calling_conventions::EntryStack::ProviderSelected
        );
        assert_eq!(
            plan.plan().state.initial_regime,
            calling_conventions::MachineRegime::X86Long64
        );
    }
}
