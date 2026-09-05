use omega_isa_aarch64::aarch64_preservation_convention_for_target;
use omega_isa_x86_64::x86_64_preservation_convention_for_target;
use omega_register_model::{PreservationConvention, ValidatedPhysicalRegisterModel};
use omega_target::{Architecture, NativeTarget, ObjectFormat};

use super::super::{error::WholeFunctionExitContractError, model::WholeFunctionExitPolicy};

#[derive(Clone, Copy)]
pub(in crate::exit_contract) enum EntryAssumptionKind {
    ActivationStack,
    LinkRegister,
}

pub(in crate::exit_contract) fn target_contract_inputs(
    physical: &ValidatedPhysicalRegisterModel,
    target: NativeTarget,
) -> Result<
    (
        WholeFunctionExitPolicy,
        &PreservationConvention,
        &'static str,
        Option<&'static str>,
        EntryAssumptionKind,
    ),
    WholeFunctionExitContractError,
> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf) => Ok((
            WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1,
            x86_64_preservation_convention_for_target(physical, target)
                .ok_or(WholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "rsp",
            None,
            EntryAssumptionKind::ActivationStack,
        )),
        (Architecture::X86_64, ObjectFormat::Coff) => Ok((
            WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1,
            x86_64_preservation_convention_for_target(physical, target)
                .ok_or(WholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "rsp",
            None,
            EntryAssumptionKind::ActivationStack,
        )),
        (Architecture::Aarch64, ObjectFormat::Elf) => Ok((
            WholeFunctionExitPolicy::Aapcs64FramelessLeafV1,
            aarch64_preservation_convention_for_target(physical, target)
                .ok_or(WholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "sp",
            Some("x30"),
            EntryAssumptionKind::LinkRegister,
        )),
        (Architecture::Aarch64, ObjectFormat::MachO) => Ok((
            WholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1,
            aarch64_preservation_convention_for_target(physical, target)
                .ok_or(WholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "sp",
            Some("x30"),
            EntryAssumptionKind::LinkRegister,
        )),
        _ => Err(WholeFunctionExitContractError::UnsupportedTargetPolicy),
    }
}

pub(in crate::exit_contract) fn view<'model>(
    physical: &'model ValidatedPhysicalRegisterModel,
    name: &'static str,
) -> Result<&'model omega_register_model::RegisterView, WholeFunctionExitContractError> {
    physical.model().view_named(name).ok_or(
        WholeFunctionExitContractError::MissingArchitecturalView(name),
    )
}
