/*
We define the target-fixed physical process-entry contract, its per-package
arrival table and boundary-entry-plan replay, the canonical normalized type
identities of closed hosted entry contracts, and the bundled package source
digests that authenticate toolchain-owned entry contracts.
*/

use calling_conventions::{
    BoundaryEntryPlan, CallPlan, CallSignature, CallingPolicy, EntryControl, EntryStack,
    MachineRegime, MachineRegister, MachineState, MachineStateSet, Preemption, StatePlan,
    ValidatedBoundaryEntryPlan, ValueLocation, ValuePlacement, ValueShape,
    evaluate_ordinary_boundary_entry_plan, validate_boundary_entry_plan,
};
use target::{
    ProgramEntryCallingConvention as Convention, ProgramEntryPhysicalContractPackage as Package,
    TargetEntryStackGuarantee, TargetProfile,
};

pub const WINDOWS_X86_64_U32_TYPE_IDENTITY: &str = "named(name(u32))";
pub const MACOS_ARM64_I32_TYPE_IDENTITY: &str = "named(name(i32))";
pub const MACOS_ARM64_ADDRESS_TYPE_IDENTITY: &str = "named(name(addr))";
pub const MACOS_X86_64_I32_TYPE_IDENTITY: &str = "named(name(i32))";
pub const MACOS_X86_64_ADDRESS_TYPE_IDENTITY: &str = "named(name(addr))";
pub const LINUX_X86_64_ADDRESS_TYPE_IDENTITY: &str = "named(name(addr))";
pub const LINUX_X86_64_I32_TYPE_IDENTITY: &str = "named(name(i32))";
pub const LINUX_ARM64_U64_TYPE_IDENTITY: &str = "named(name(u64))";
pub const LINUX_ARM64_I32_TYPE_IDENTITY: &str = "named(name(i32))";

const INTEGER32: ValueShape = ValueShape::integer(4, 4);
const INTEGER64: ValueShape = ValueShape::integer(8, 8);
const ADDRESS: ValueShape = ValueShape::integer(8, 8);

fn process_entry_state_plan(initial_regime: MachineRegime) -> StatePlan {
    StatePlan {
        initial_regime,
        interrupted_state: MachineStateSet::default(),
        saved_state: MachineStateSet::default(),
        restored_state: MachineStateSet::default(),
        permitted_transitive_use: MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::VectorRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            // A returning import's floating-control envelope restores the
            // callee-preserved MXCSR/FPCR controls.
            MachineState::ControlState,
        ]),
        stack: EntryStack::ProviderSelected,
        preemption: Preemption::NotApplicable,
    }
}

pub fn exact_hosted_physical_boundary_entry_plan(
    package: target::ProgramEntryPhysicalContractPackage,
) -> Option<ValidatedBoundaryEntryPlan> {
    physical_arrival(package).replay()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramEntryPhysicalContractPackageSourceDigest {
    package: target::ProgramEntryPhysicalContractPackage,
    bytes: [u8; 32],
}

pub fn exact_physical_contract_package_source_digest(
    package: target::ProgramEntryPhysicalContractPackage,
) -> ProgramEntryPhysicalContractPackageSourceDigest {
    ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
        package,
        exact_package_source(package),
    )
}

impl ProgramEntryPhysicalContractPackageSourceDigest {
    pub const fn from_retained_digest(
        package: target::ProgramEntryPhysicalContractPackage,
        bytes: [u8; 32],
    ) -> Self {
        Self { package, bytes }
    }

    pub fn from_package_source(
        package: target::ProgramEntryPhysicalContractPackage,
        source: &[u8],
    ) -> Self {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(b"");
        hash_digest_field(
            &mut digest,
            b"omega.program-entry-physical-contract-package-source.v1",
        );
        hash_digest_field(&mut digest, package.manifest_identity().as_bytes());
        hash_digest_field(&mut digest, package.package_relative_source().as_bytes());
        hash_digest_field(&mut digest, source);
        Self {
            package,
            bytes: digest.finish(),
        }
    }

    pub const fn package(&self) -> target::ProgramEntryPhysicalContractPackage {
        self.package
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntryPhysicalContractPlan {
    target_slot: target::ProgramEntrySlotDeclaration,
    requirement_identity: terminal_psi::TerminalIdentityIndex,
    identities: terminal_psi::TerminalIdentityTable,
    target_package: target::ProgramEntryPhysicalContractPackage,
    target_package_source_digest: ProgramEntryPhysicalContractPackageSourceDigest,
    non_authoritative_target_package_source_report_fingerprint: u64,
    parameter_type_identities: Vec<terminal_psi::TerminalIdentityIndex>,
    result_type_identity: terminal_psi::TerminalIdentityIndex,
    calling_plan_report_fingerprint: u64,
    boundary_entry_plan: BoundaryEntryPlan,
    guaranteed_entry_stack: Option<target::TargetEntryStackGuarantee>,
}

impl ProgramEntryPhysicalContractPlan {
    pub fn new(
        target_slot: target::ProgramEntrySlotDeclaration,
        requirement_identity: String,
        target_package: target::ProgramEntryPhysicalContractPackage,
        target_package_source_digest: ProgramEntryPhysicalContractPackageSourceDigest,
        non_authoritative_target_package_source_report_fingerprint: u64,
        parameter_type_identities: Vec<String>,
        result_type_identity: String,
        calling_plan_report_fingerprint: u64,
        boundary_entry_plan: BoundaryEntryPlan,
    ) -> Result<Self, String> {
        let Some(physical_requirement) = target_slot.physical_arrival_requirement else {
            return Err("program-entry physical contract has no target-fixed requirement".into());
        };
        if target_slot != target_slot.owner.program_entry_slot()
            || physical_requirement.is_empty()
            || target_slot.physical_contract_package != Some(target_package)
            || target_package_source_digest.package() != target_package
        {
            return Err(
                "physical entry contract drifted from its exact target slot declaration".into(),
            );
        }
        let arrival = physical_arrival(target_package);
        if target_slot.physical_calling_convention != Some(arrival.calling_convention) {
            return Err(format!(
                "physical {physical_requirement} entry contract requires a target declaration with the package's authored physical calling convention"
            ));
        }
        let (expected_policy, parameter_count) = (arrival.policy, arrival.parameter_count());
        let guaranteed_entry_stack = arrival.guaranteed_entry_stack(target_slot.owner)?;
        if requirement_identity.is_empty()
            || parameter_type_identities.len() != parameter_count
            || parameter_type_identities
                .iter()
                .any(|identity| identity.is_empty())
            || result_type_identity.is_empty()
            || calling_plan_report_fingerprint == 0
        {
            return Err(format!(
                "physical {physical_requirement} entry contract lost its exact {parameter_count} parameters, result, or calling-plan identity"
            ));
        }
        if boundary_entry_plan.call.policy != expected_policy
            || boundary_entry_plan.call.parameters.len() != parameter_count
            || boundary_entry_plan.call.result.is_none()
        {
            return Err(format!(
                "physical {physical_requirement} entry contract does not realize {parameter_count} {expected_policy:?} inputs and one result"
            ));
        }
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        Ok(Self {
            target_slot,
            requirement_identity: identities.intern(&requirement_identity),
            target_package,
            target_package_source_digest,
            non_authoritative_target_package_source_report_fingerprint,
            parameter_type_identities: parameter_type_identities
                .iter()
                .map(|identity| identities.intern(identity))
                .collect(),
            result_type_identity: identities.intern(&result_type_identity),
            calling_plan_report_fingerprint,
            boundary_entry_plan,
            guaranteed_entry_stack,
            identities: identities.build(),
        })
    }

    pub const fn identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.identities
    }

    pub const fn target_slot(&self) -> target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub const fn requirement_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.requirement_identity
    }

    pub const fn target_package(&self) -> target::ProgramEntryPhysicalContractPackage {
        self.target_package
    }

    pub const fn target_package_source_digest(
        &self,
    ) -> ProgramEntryPhysicalContractPackageSourceDigest {
        self.target_package_source_digest
    }

    pub const fn non_authoritative_target_package_source_report_fingerprint(&self) -> u64 {
        self.non_authoritative_target_package_source_report_fingerprint
    }

    pub fn parameter_type_identities(&self) -> &[terminal_psi::TerminalIdentityIndex] {
        &self.parameter_type_identities
    }

    pub const fn result_type_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.result_type_identity
    }

    pub const fn calling_plan_report_fingerprint(&self) -> u64 {
        self.calling_plan_report_fingerprint
    }

    pub const fn boundary_entry_plan(&self) -> &BoundaryEntryPlan {
        &self.boundary_entry_plan
    }

    pub const fn guaranteed_entry_stack_application(
        &self,
    ) -> Option<&target::SymbolicTargetObservationApplication> {
        match &self.guaranteed_entry_stack {
            Some(guarantee) => Some(guarantee.application()),
            None => None,
        }
    }

    pub const fn guaranteed_entry_stack(&self) -> Option<&target::TargetEntryStackGuarantee> {
        self.guaranteed_entry_stack.as_ref()
    }
}

fn hash_digest_field(digest: &mut crate::digest_encoder::DomainDigestEncoder, bytes: &[u8]) {
    digest.bytes(bytes);
}

struct PhysicalArrival {
    package: Package,
    calling_convention: Convention,
    policy: CallingPolicy,
    parameter_shapes: &'static [ValueShape],
    result_shape: ValueShape,
    replay: Option<fn(CallingPolicy, &CallSignature) -> ValidatedBoundaryEntryPlan>,
    entry_stack_closure: Option<fn(TargetProfile) -> Result<TargetEntryStackGuarantee, String>>,
}

impl PhysicalArrival {
    fn parameter_count(&self) -> usize {
        self.parameter_shapes.len()
    }

    fn replay(&self) -> Option<ValidatedBoundaryEntryPlan> {
        let signature = CallSignature {
            parameters: self.parameter_shapes.to_vec(),
            result: Some(self.result_shape),
        };
        Some((self.replay?)(self.policy, &signature))
    }

    fn guaranteed_entry_stack(
        &self,
        owner: TargetProfile,
    ) -> Result<Option<TargetEntryStackGuarantee>, String> {
        match self.entry_stack_closure {
            Some(close) => close(owner).map(Some),
            None => Ok(None),
        }
    }
}

const fn row(
    package: Package,
    calling_convention: Convention,
    policy: CallingPolicy,
    parameter_shapes: &'static [ValueShape],
    result_shape: ValueShape,
    replay: Option<fn(CallingPolicy, &CallSignature) -> ValidatedBoundaryEntryPlan>,
    entry_stack_closure: Option<fn(TargetProfile) -> Result<TargetEntryStackGuarantee, String>>,
) -> PhysicalArrival {
    PhysicalArrival {
        package,
        calling_convention,
        policy,
        parameter_shapes,
        result_shape,
        replay,
        entry_stack_closure,
    }
}

const ARRIVALS: [PhysicalArrival; 6] = [
    row(
        Package::WindowsX64,
        Convention::MicrosoftX64,
        CallingPolicy::MicrosoftX64,
        &[],
        INTEGER32,
        Some(ordinary_arrival),
        None,
    ),
    row(
        Package::MacosX64,
        Convention::SystemVAMD64,
        CallingPolicy::SystemVAMD64,
        &[INTEGER32, ADDRESS, ADDRESS, ADDRESS],
        INTEGER32,
        Some(ordinary_arrival),
        None,
    ),
    row(
        Package::MacosArm64,
        Convention::Aapcs64,
        CallingPolicy::Aapcs64,
        &[INTEGER32, ADDRESS, ADDRESS, ADDRESS],
        INTEGER32,
        Some(ordinary_arrival),
        None,
    ),
    row(
        Package::LinuxX86_64,
        Convention::SystemVAMD64,
        CallingPolicy::SystemVAMD64,
        &[ADDRESS],
        INTEGER32,
        Some(linux_x86_64_arrival),
        None,
    ),
    row(
        Package::LinuxArm64,
        Convention::Aapcs64,
        CallingPolicy::Aapcs64,
        &[ADDRESS],
        INTEGER32,
        Some(linux_arm64_arrival),
        None,
    ),
    row(
        Package::UefiX64,
        Convention::MicrosoftX64,
        CallingPolicy::MicrosoftX64,
        &[ADDRESS, ADDRESS],
        INTEGER64,
        None,
        Some(close_uefi_entry_stack),
    ),
];

fn physical_arrival(package: Package) -> &'static PhysicalArrival {
    ARRIVALS
        .iter()
        .find(|arrival| arrival.package == package)
        .expect("every closed physical contract package has one arrival row")
}

fn ordinary_arrival(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> ValidatedBoundaryEntryPlan {
    evaluate_ordinary_boundary_entry_plan(policy, signature)
        .expect("the closed target-authored physical entry plan must remain valid")
}

fn linux_x86_64_arrival(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> ValidatedBoundaryEntryPlan {
    let plan = BoundaryEntryPlan {
        call: CallPlan {
            policy,
            parameters: vec![ValuePlacement {
                shape: ADDRESS,
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::X86Rsp,
                    value_byte_offset: 0,
                    byte_size: 8,
                }],
            }],
            result: Some(ValuePlacement {
                shape: INTEGER32,
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::X86Rdi,
                    value_byte_offset: 0,
                    byte_size: 4,
                }],
            }),
            callback_materializations: Vec::new(),
            ordinary_clobbers: policy.ordinary_clobbers(),
            stack_alignment: 16,
            shadow_bytes: 0,
            entry_control: EntryControl::SupervisorCall {
                number_register: MachineRegister::X86Rax,
                immediate: 231,
            },
        },
        state: process_entry_state_plan(MachineRegime::X86Long64),
    };
    validate_boundary_entry_plan(plan, signature)
        .expect("the closed target-authored Linux x86-64 physical entry plan must remain valid")
}

fn linux_arm64_arrival(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> ValidatedBoundaryEntryPlan {
    let plan = BoundaryEntryPlan {
        call: CallPlan {
            policy,
            parameters: vec![ValuePlacement {
                shape: ADDRESS,
                locations: vec![ValueLocation::Stack {
                    stack_byte_offset: 0,
                    value_byte_offset: 0,
                    byte_size: 8,
                    alignment: 8,
                }],
            }],
            result: Some(ValuePlacement {
                shape: INTEGER32,
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::Aarch64X(0),
                    value_byte_offset: 0,
                    byte_size: 4,
                }],
            }),
            callback_materializations: Vec::new(),
            ordinary_clobbers: policy.ordinary_clobbers(),
            stack_alignment: 16,
            shadow_bytes: 0,
            entry_control: EntryControl::SupervisorCall {
                number_register: MachineRegister::Aarch64X(8),
                immediate: 94,
            },
        },
        state: process_entry_state_plan(MachineRegime::Aarch64A64 { exception_level: 0 }),
    };
    validate_boundary_entry_plan(plan, signature)
        .expect("the closed target-authored Linux ARM64 physical entry plan must remain valid")
}

fn close_uefi_entry_stack(owner: TargetProfile) -> Result<TargetEntryStackGuarantee, String> {
    let application = target::TargetSemantics::guaranteed_entry_stack::<target::UefiX86_64>(owner)
        .map_err(|error| error.to_string())?;
    let guarantee =
        target::TargetSemantics::close_guaranteed_entry_stack::<target::UefiX86_64>(owner)
            .map_err(|error| error.to_string())?;
    if guarantee.application() != &application {
        return Err(
            "physical UEFI entry contract target-stack application and numeric closure drifted"
                .into(),
        );
    }
    Ok(guarantee)
}

fn exact_package_source(package: Package) -> &'static [u8] {
    match package {
        Package::UefiX64 => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/std/targets/uefi_x86_64/entry.omg"
        )),
        Package::MacosArm64 => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/std/targets/macos_arm64/entry.omg"
        )),
        Package::MacosX64 => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/std/targets/macos_x86_64/entry.omg"
        )),
        Package::LinuxX86_64 => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/std/targets/linux_x86_64/entry.omg"
        )),
        Package::LinuxArm64 => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/std/targets/linux_arm64/entry.omg"
        )),
        Package::WindowsX64 => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/std/targets/windows_x86_64/entry.omg"
        )),
    }
}
