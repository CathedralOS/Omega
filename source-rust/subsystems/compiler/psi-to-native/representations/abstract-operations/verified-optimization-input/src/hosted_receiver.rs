//! Join a hosted receiver's sealed semantic entry and private RAM to its physical ABI.
use diagnostics::Diagnostic;
use program_entry_contracts::HostedReceiverCustody;
use program_entry_contracts::{ProgramEntryPhysicalContractPlan, SelectedProgramEntrySignature};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedHostedReceiver {
    source: SelectedProgramEntrySignature,
    checked_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    physical: ProgramEntryPhysicalContractPlan,
    private_entry_ram: Vec<psi_entry_memory::CheckedPrivateEntryRamRecipe>,
}

impl AdmittedHostedReceiver {
    pub fn admit(
        target: target::NativeTarget,
        profile: Option<target::TargetProfile>,
        admission: &crate::EmissionAdmission,
        function: Option<&target_vocabulary::publication::EmissionFunctionFacts>,
        abi: Option<&target_operations::LegalizedFunctionAbi>,
        entry: &crate::ValidatedNativeProgramEntrySettlement,
    ) -> Result<Self, Diagnostic> {
        let invalid = || invalid(target);
        let physical = entry
            .storage_entry()
            .and_then(|storage| storage.physical_contract())
            .ok_or_else(invalid)?;
        if !physical_contract_matches(physical, target)
            || profile != Some(physical.target_slot().owner)
            || physical.target_slot() != entry.source().target_slot()
        {
            return Err(invalid());
        }
        validate_entry_arguments(
            ReceiverArgumentInput {
                target,
                admission: admission.entry_facts(),
                function: function.ok_or_else(invalid)?,
                abi: abi.ok_or_else(invalid)?,
            },
            entry.source(),
            entry.checked_entry(),
            entry.private_entry_ram(),
        )?;
        Ok(Self {
            source: entry.source().clone(),
            checked_entry: entry.checked_entry().clone(),
            physical: physical.clone(),
            private_entry_ram: entry.private_entry_ram().to_vec(),
        })
    }
}

impl AdmittedHostedReceiver {
    pub fn source(&self) -> &SelectedProgramEntrySignature {
        &self.source
    }
    pub fn checked_entry(&self) -> &terminal_psi::CheckedProgramEntryTerminalReceipt {
        &self.checked_entry
    }
    pub fn physical_contract(&self) -> &ProgramEntryPhysicalContractPlan {
        &self.physical
    }
    pub fn private_entry_ram(&self) -> &[psi_entry_memory::CheckedPrivateEntryRamRecipe] {
        &self.private_entry_ram
    }
}

impl HostedReceiverCustody for AdmittedHostedReceiver {
    fn source(&self) -> &SelectedProgramEntrySignature {
        self.source()
    }
    fn checked_entry(&self) -> &terminal_psi::CheckedProgramEntryTerminalReceipt {
        self.checked_entry()
    }
    fn physical_contract(&self) -> &ProgramEntryPhysicalContractPlan {
        self.physical_contract()
    }
    fn private_entry_ram(&self) -> &[psi_entry_memory::CheckedPrivateEntryRamRecipe] {
        self.private_entry_ram()
    }
}

struct PhysicalContractRow {
    profile: target::TargetProfile,
    parameter_type_identities: &'static [&'static str],
    result_type_identity: &'static str,
}

fn physical_contract_row(target: target::NativeTarget) -> Option<PhysicalContractRow> {
    use target::entry_contract::{
        LINUX_ARM64_I32_TYPE_IDENTITY, LINUX_ARM64_U64_TYPE_IDENTITY,
        LINUX_X86_64_ADDRESS_TYPE_IDENTITY, LINUX_X86_64_I32_TYPE_IDENTITY,
        MACOS_ARM64_ADDRESS_TYPE_IDENTITY, MACOS_ARM64_I32_TYPE_IDENTITY,
        MACOS_X86_64_ADDRESS_TYPE_IDENTITY, MACOS_X86_64_I32_TYPE_IDENTITY,
        WINDOWS_X86_64_U32_TYPE_IDENTITY,
    };
    Some(if target == target::NativeTarget::macos_arm64() {
        PhysicalContractRow {
            profile: target::TargetProfile::MacosArm64,
            parameter_type_identities: &[
                MACOS_ARM64_I32_TYPE_IDENTITY,
                MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
                MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
                MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
            ],
            result_type_identity: MACOS_ARM64_I32_TYPE_IDENTITY,
        }
    } else if target == target::NativeTarget::macos_x64() {
        PhysicalContractRow {
            profile: target::TargetProfile::MacosX64,
            parameter_type_identities: &[
                MACOS_X86_64_I32_TYPE_IDENTITY,
                MACOS_X86_64_ADDRESS_TYPE_IDENTITY,
                MACOS_X86_64_ADDRESS_TYPE_IDENTITY,
                MACOS_X86_64_ADDRESS_TYPE_IDENTITY,
            ],
            result_type_identity: MACOS_X86_64_I32_TYPE_IDENTITY,
        }
    } else if target == target::NativeTarget::linux_x64() {
        PhysicalContractRow {
            profile: target::TargetProfile::LinuxX64,
            parameter_type_identities: &[LINUX_X86_64_ADDRESS_TYPE_IDENTITY],
            result_type_identity: LINUX_X86_64_I32_TYPE_IDENTITY,
        }
    } else if target == target::NativeTarget::linux_arm64() {
        PhysicalContractRow {
            profile: target::TargetProfile::LinuxArm64,
            parameter_type_identities: &[LINUX_ARM64_U64_TYPE_IDENTITY],
            result_type_identity: LINUX_ARM64_I32_TYPE_IDENTITY,
        }
    } else if target == target::NativeTarget::windows_x64() {
        PhysicalContractRow {
            profile: target::TargetProfile::WindowsX64,
            parameter_type_identities: &[],
            result_type_identity: WINDOWS_X86_64_U32_TYPE_IDENTITY,
        }
    } else {
        return None;
    })
}

pub fn physical_contract_matches(
    physical: &ProgramEntryPhysicalContractPlan,
    target: target::NativeTarget,
) -> bool {
    let Some(row) = physical_contract_row(target) else {
        return false;
    };
    let slot = row.profile.program_entry_slot();
    let Some(package) = slot.physical_contract_package else {
        return false;
    };
    let Some(expected) =
        calling_conventions::process_entry::exact_hosted_physical_boundary_entry_plan(package)
    else {
        return false;
    };
    let identities = physical.identities();
    physical.target_slot() == slot
        && physical.target_package() == package
        && physical.target_package_source_digest()
            == target::entry_contract::exact_physical_contract_package_source_digest(package)
        && !physical.requirement_identity().is_absent()
        && physical.parameter_type_identities().len() == row.parameter_type_identities.len()
        && physical
            .parameter_type_identities()
            .iter()
            .zip(row.parameter_type_identities.iter())
            .all(|(left, right)| Some(*left) == identities.index_of(right))
        && Some(physical.result_type_identity()) == identities.index_of(row.result_type_identity)
        && physical.boundary_entry_plan() == expected.plan()
        && physical.calling_plan_report_fingerprint() == expected.contract_report_fingerprint()
        && physical.guaranteed_entry_stack().is_none()
        && physical.guaranteed_entry_stack_application().is_none()
}

pub struct ReceiverArgumentInput<'a> {
    pub target: target::NativeTarget,
    pub admission: &'a target_vocabulary::publication::NativeEntryFacts,
    pub function: &'a target_vocabulary::publication::EmissionFunctionFacts,
    pub abi: &'a target_operations::LegalizedFunctionAbi,
}

pub fn validate_entry_arguments(
    input: ReceiverArgumentInput<'_>,
    source: &SelectedProgramEntrySignature,
    checked_entry: &terminal_psi::CheckedProgramEntryTerminalReceipt,
    private_entry_ram: &[psi_entry_memory::CheckedPrivateEntryRamRecipe],
) -> Result<(), Diagnostic> {
    let ReceiverArgumentInput {
        target,
        admission,
        function,
        abi,
    } = input;
    use calling_conventions::CallingPolicy;
    let invalid = || invalid(target);
    if ![
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ]
    .contains(&target)
        || private_entry_ram.is_empty()
    {
        return Err(invalid());
    }
    let identity = admission.terminal();
    let selected = checked_entry.terminal_entry();
    if source.target_slot().owner.native_realization() != Some(target)
        || checked_entry.source_signature_identity() != source.identity().bytes()
        || checked_entry.terminal_psi_identity() != identity
        || checked_entry.terminal_entry() != function.machine
        || admission
            .identities()
            .text(checked_entry.declaration().identity())
            != Some(source.declaration())
        || admission.entry() != selected
        || admission
            .entry_declaration()
            .and_then(|declaration| admission.identities().text(declaration.machine.identity()))
            != Some(source.declaration())
        || !matches!(
            function.result,
            operation_operands::values::AbstractFunctionResult::Unit
        )
        || !function.parameters.is_empty()
        || !function.entry_claims.is_empty()
        || !source.visible_parameters().is_empty()
        || function.attachment.is_some()
        || abi.call_plan.policy != CallingPolicy::native_for_target(target)
        || function.structural_parameters.len() != private_entry_ram.len()
        || abi.structural_parameters.len() != private_entry_ram.len()
        || abi.call_plan.parameters.len() != private_entry_ram.len()
        || abi.call_plan.result.is_some()
    {
        return Err(invalid());
    }
    if !admission.admits_hosted_entry(identity, selected, source.target_slot().owner) {
        return Err(invalid());
    }
    for (ordinal, parameter) in function.structural_parameters.iter().enumerate() {
        let index =
            recipe_index_for_runtime_position(private_entry_ram, ordinal).ok_or_else(invalid)?;
        let recipe = private_entry_ram.get(index).ok_or_else(invalid)?;
        let actual = abi
            .structural_parameters
            .iter()
            .find(|native| native.place == parameter.place)
            .ok_or_else(invalid)?;
        if recipe.terminal() != identity
            || recipe.target() != source.target_slot().owner.target_name()
            || recipe.input().machine != selected
            || recipe.parameter() != parameter
            || actual.structural_type != parameter.structural_type
            || actual.access != parameter.access
            || actual.shape.byte_size as usize != recipe.contents().len()
            || (actual.shape.alignment as u64) < recipe.alignment()
            || !receiver_pointer_matches(
                actual.shape,
                &actual.placement,
                if target == target::NativeTarget::windows_x64() {
                    use calling_conventions::{IndirectPointerLocation, MachineRegister};
                    match ordinal {
                        0 => IndirectPointerLocation::Register(MachineRegister::X86Rcx),
                        1 => IndirectPointerLocation::Register(MachineRegister::X86Rdx),
                        2 => IndirectPointerLocation::Register(MachineRegister::X86R8),
                        3 => IndirectPointerLocation::Register(MachineRegister::X86R9),
                        4 => IndirectPointerLocation::Stack {
                            stack_byte_offset: 32,
                            alignment: 8,
                        },
                        _ => return Err(invalid()),
                    }
                } else {
                    calling_conventions::IndirectPointerLocation::Register(
                        if target.architecture == target::Architecture::Aarch64 {
                            (ordinal < 8).then_some(calling_conventions::MachineRegister::Aarch64X(
                                ordinal as u8,
                            ))
                        } else {
                            linux_argument_register(ordinal)
                        }
                        .ok_or_else(invalid)?,
                    )
                },
            )
            || abi.call_plan.parameters.get(ordinal) != Some(&actual.placement)
        {
            return Err(invalid());
        }
    }
    Ok(())
}

fn receiver_pointer_matches(
    shape: calling_conventions::ValueShape,
    placement: &calling_conventions::ValuePlacement,
    pointer: calling_conventions::IndirectPointerLocation,
) -> bool {
    use calling_conventions::{ValueClass, ValueLocation};
    shape.class == ValueClass::BorrowedReference
        && shape.alignment.is_power_of_two()
        && placement.shape == shape
        && matches!(
            placement.locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: actual,
                copy_stack_byte_offset: None,
                byte_size,
                alignment,
            }] if *actual == pointer && *byte_size == shape.byte_size && *alignment == shape.alignment
        )
}

fn linux_argument_register(ordinal: usize) -> Option<calling_conventions::MachineRegister> {
    use calling_conventions::MachineRegister::*;
    [X86Rdi, X86Rsi, X86Rdx, X86Rcx, X86R8, X86R9]
        .get(ordinal)
        .copied()
}

pub fn invalid(target: target::NativeTarget) -> Diagnostic {
    Diagnostic::error(format!(
        "installed private entry RAM lost exact contract, storage, or entry custody for {target:?}"
    ))
}

pub fn recipe_index_for_runtime_position(
    recipes: &[psi_entry_memory::CheckedPrivateEntryRamRecipe],
    ordinal: usize,
) -> Option<usize> {
    recipes
        .iter()
        .position(|recipe| recipe.input().runtime_position as usize == ordinal)
}
