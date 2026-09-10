use isa_aarch64::aarch64_preservation_convention_for_target;
use isa_x86_64::x86_64_preservation_convention_for_target;
use register_model::{PreservationConvention, ValidatedPhysicalRegisterModel};
use target::{Architecture, NativeTarget, ObjectFormat};

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
) -> Result<&'model register_model::RegisterView, WholeFunctionExitContractError> {
    physical.model().view_named(name).ok_or(
        WholeFunctionExitContractError::MissingArchitecturalView(name),
    )
}

pub(in crate::exit_contract) fn scalar_return_view(
    physical: &ValidatedPhysicalRegisterModel,
    target: NativeTarget,
    registers: &[selected_instructions::VirtualRegister],
    instruction: &selected_instructions::SelectedInstruction,
    integer_result: register_model::RegisterViewId,
) -> Result<register_model::RegisterViewId, WholeFunctionExitContractError> {
    if instruction.kind != selected_instructions::SelectedInstructionKind::ReturnScalar {
        return Ok(integer_result);
    }
    let invalid = || WholeFunctionExitContractError::ReturnOperandMismatch(instruction.id);
    let [operand] = instruction.operands.as_slice() else {
        return Err(invalid());
    };
    let register = registers
        .iter()
        .find(|register| register.id == operand.virtual_register)
        .ok_or_else(invalid)?;
    // Preservation metadata names the integer result convention. Each scalar
    // return additionally rejoins its retained type to the exact ABI bank.
    let expected = if matches!(
        register.scalar_type,
        semantic_vocabulary::ScalarType::IeeeFloat(_)
    ) {
        view(
            physical,
            match target.architecture {
                Architecture::X86_64 => "xmm0",
                Architecture::Aarch64 => "d0",
            },
        )?
    } else {
        physical
            .model()
            .views
            .iter()
            .find(|view| view.id == integer_result)
            .ok_or_else(invalid)?
    };
    if register.class != expected.class
        || operand.class != expected.class
        || operand.fixed_view != Some(expected.id)
        || operand.access != register_model::RegisterOperandAccess::Use
    {
        return Err(invalid());
    }
    Ok(expected.id)
}
#[cfg(test)]
mod tests {
    use super::*;
    use register_model::RegisterOperandAccess;
    use selected_instructions::{
        SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedOperand,
        VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{IeeeFloatFormat, ScalarType, ValueId};

    #[test]
    fn scalar_exit_rejoins_float_type_class_and_abi_home() {
        for target in [NativeTarget::linux_x64(), NativeTarget::macos_arm64()] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let physical = environment.physical();
            let integer = physical
                .model()
                .view_named(if target.architecture == Architecture::X86_64 {
                    "rax"
                } else {
                    "x0"
                })
                .unwrap();
            let floating = physical
                .model()
                .view_named(if target.architecture == Architecture::X86_64 {
                    "xmm0"
                } else {
                    "d0"
                })
                .unwrap();
            for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
                let mut registers = vec![VirtualRegister {
                    id: VirtualRegisterId(0),
                    scalar_type: ScalarType::IeeeFloat(format),
                    class: floating.class,
                    definition_site: None,
                    entry_fixed_view: None,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(0),
                        source_value: ValueId::new(1).unwrap(),
                    },
                }];
                let mut instruction = SelectedInstruction {
                    id: SelectedInstructionId(1),
                    kind: SelectedInstructionKind::ReturnScalar,
                    constraint: environment.selected_keys().return_float[0],
                    operands: vec![SelectedOperand {
                        operand: 0,
                        virtual_register: VirtualRegisterId(0),
                        access: RegisterOperandAccess::Use,
                        class: floating.class,
                        fixed_view: Some(floating.id),
                        tied_to: None,
                        early_clobber: false,
                    }],
                    implicit_uses: Vec::new(),
                    implicit_defs: Vec::new(),
                    clobbers: Vec::new(),
                    provenance: Default::default(),
                };
                assert_eq!(
                    scalar_return_view(physical, target, &registers, &instruction, integer.id)
                        .unwrap(),
                    floating.id
                );
                instruction.operands[0].fixed_view = Some(integer.id);
                assert!(
                    scalar_return_view(physical, target, &registers, &instruction, integer.id)
                        .is_err()
                );
                instruction.operands[0].fixed_view = Some(floating.id);
                registers[0].scalar_type = ScalarType::Boolean;
                assert!(
                    scalar_return_view(physical, target, &registers, &instruction, integer.id)
                        .is_err()
                );
            }
        }
    }
}
