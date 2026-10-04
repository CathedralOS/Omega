//! Exact Linux ARM64 replay owned beside the physical entry-plan carrier.

use calling_conventions::{
    BoundaryEntryPlan, CallPlan, CallSignature, EntryControl, EntryStack, MachineRegime,
    MachineRegister, MachineState, MachineStateSet, Preemption, RegisterSet, StatePlan,
    ValidatedBoundaryEntryPlan, ValueLocation, ValuePlacement, ValueShape,
    validate_boundary_entry_plan,
};

use super::ProgramEntryPhysicalContractPlan;

const LINUX_ARM64_TARGET_PACKAGE_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/std/targets/linux_arm64/entry.omg"
));

/// Canonical checked-tree overload identity for `LinuxArm64PhysicalEntry::enter`.
pub const LINUX_ARM64_PHYSICAL_REQUIREMENT_IDENTITY: &str = concat!(
    "named-callable(path(LinuxArm64PhysicalEntry::enter),parameters(",
    "parameter\\(self\\(no\\)\\,mutable\\(no\\)\\,const\\(no\\)\\,",
    "named\\(name\\(u64\\)\\)\\)",
    "),result-dispatch())",
);
/// Canonical normalized type identity of the `argc` arrival count delivered as
/// the head word of the initial process-stack image.
pub const LINUX_ARM64_U64_TYPE_IDENTITY: &str = "named(name(u64))";
/// Canonical normalized type identity of the process-completion status.
pub const LINUX_ARM64_I32_TYPE_IDENTITY: &str = "named(name(i32))";

/// Standard AAPCS64 volatile banks: the eighteen integer argument/scratch
/// registers plus the sixteen volatile vector registers. x18 is
/// platform-reserved; the bridge may also observe the incoming stack pointer
/// before switching to its private stack.
const LINUX_ARM64_ORDINARY_CLOBBERS: [MachineRegister; 42] = [
    MachineRegister::Aarch64X(0),
    MachineRegister::Aarch64X(1),
    MachineRegister::Aarch64X(2),
    MachineRegister::Aarch64X(3),
    MachineRegister::Aarch64X(4),
    MachineRegister::Aarch64X(5),
    MachineRegister::Aarch64X(6),
    MachineRegister::Aarch64X(7),
    MachineRegister::Aarch64X(8),
    MachineRegister::Aarch64X(9),
    MachineRegister::Aarch64X(10),
    MachineRegister::Aarch64X(11),
    MachineRegister::Aarch64X(12),
    MachineRegister::Aarch64X(13),
    MachineRegister::Aarch64X(14),
    MachineRegister::Aarch64X(15),
    MachineRegister::Aarch64X(16),
    MachineRegister::Aarch64X(17),
    MachineRegister::Aarch64V(0),
    MachineRegister::Aarch64V(1),
    MachineRegister::Aarch64V(2),
    MachineRegister::Aarch64V(3),
    MachineRegister::Aarch64V(4),
    MachineRegister::Aarch64V(5),
    MachineRegister::Aarch64V(6),
    MachineRegister::Aarch64V(7),
    MachineRegister::Aarch64V(16),
    MachineRegister::Aarch64V(17),
    MachineRegister::Aarch64V(18),
    MachineRegister::Aarch64V(19),
    MachineRegister::Aarch64V(20),
    MachineRegister::Aarch64V(21),
    MachineRegister::Aarch64V(22),
    MachineRegister::Aarch64V(23),
    MachineRegister::Aarch64V(24),
    MachineRegister::Aarch64V(25),
    MachineRegister::Aarch64V(26),
    MachineRegister::Aarch64V(27),
    MachineRegister::Aarch64V(28),
    MachineRegister::Aarch64V(29),
    MachineRegister::Aarch64V(30),
    MachineRegister::Aarch64V(31),
];

/// Strong commitment to the exact closed Linux ARM64 target-package source
/// compiled into this plan owner.
pub fn exact_linux_arm64_physical_contract_package_source_digest()
-> super::ProgramEntryPhysicalContractPackageSourceDigest {
    super::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
        target::ProgramEntryPhysicalContractPackage::LinuxArm64,
        LINUX_ARM64_TARGET_PACKAGE_SOURCE,
    )
}

/// Independently reconstruct and validate the exact plan authored by the
/// closed Linux ARM64 target package. The kernel enters a static ELF
/// executable with the machine stack pointer at the initial process-stack
/// image and supplies no argument registers and no return continuation, so
/// the authored `LinuxArm64` policy is a dedicated physical surface rather
/// than the ordinary AAPCS64 evaluator: the lone `u64` input is the argument
/// count delivered as the image's head stack word, and the `i32` result
/// completes through `exit_group` (x8 = 94) with status in w0.
pub fn exact_linux_arm64_physical_boundary_entry_plan() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: Some(ValueShape::integer(4, 4)),
    };
    let plan = BoundaryEntryPlan {
        call: CallPlan {
            policy: calling_conventions::CallingPolicy::Aapcs64,
            parameters: vec![ValuePlacement {
                shape: ValueShape::integer(8, 8),
                locations: vec![ValueLocation::Stack {
                    stack_byte_offset: 0,
                    value_byte_offset: 0,
                    byte_size: 8,
                    alignment: 8,
                }],
            }],
            result: Some(ValuePlacement {
                shape: ValueShape::integer(4, 4),
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::Aarch64X(0),
                    value_byte_offset: 0,
                    byte_size: 4,
                }],
            }),
            callback_materializations: Vec::new(),
            ordinary_clobbers: RegisterSet::new(LINUX_ARM64_ORDINARY_CLOBBERS),
            stack_alignment: 16,
            shadow_bytes: 0,
            entry_control: EntryControl::SupervisorCall {
                number_register: MachineRegister::Aarch64X(8),
                immediate: 94,
            },
        },
        state: StatePlan {
            initial_regime: MachineRegime::Aarch64A64 { exception_level: 0 },
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
        .expect("the closed target-authored Linux ARM64 physical entry plan must remain valid")
}

impl ProgramEntryPhysicalContractPlan {
    /// Replay the complete target-owned Linux ARM64 physical-entry contract.
    /// Constructor compatibility remains deliberately broader for synthetic
    /// compiler fixtures; runtime custody must use this exact verdict. The
    /// authored contract retains no numeric entry-stack guarantee, so this
    /// verdict rejects any injected closure rather than comparing one.
    pub fn matches_exact_linux_arm64_physical_contract(&self) -> bool {
        let expected = exact_linux_arm64_physical_boundary_entry_plan();
        self.target_slot == target::TargetProfile::LinuxArm64.program_entry_slot()
            && self.target_package == target::ProgramEntryPhysicalContractPackage::LinuxArm64
            && self.target_package_source_digest
                == exact_linux_arm64_physical_contract_package_source_digest()
            && self.requirement_identity == LINUX_ARM64_PHYSICAL_REQUIREMENT_IDENTITY
            && self.parameter_type_identities.len() == 1
            && self.parameter_type_identities[0] == LINUX_ARM64_U64_TYPE_IDENTITY
            && self.result_type_identity == LINUX_ARM64_I32_TYPE_IDENTITY
            && self.calling_plan_report_fingerprint == expected.contract_report_fingerprint()
            && &self.boundary_entry_plan == expected.plan()
            && self.guaranteed_entry_stack.is_none()
    }
}
