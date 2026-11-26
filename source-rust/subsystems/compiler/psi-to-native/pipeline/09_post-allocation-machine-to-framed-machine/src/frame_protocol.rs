//! Choose every frame register and save/restore recipe before encoding.
use frame_admission::frame_layout::{
    ValidatedTargetFrameLayout, committed_extent, frame_unwind_policy,
};
use frame_admission::frame_protocol::{
    FrameProtocolCodec, TargetFrameProtocolEncodingError, frame_protocol_codec,
};
use machine_code::TargetFrameProtocolEncodingPolicy;
use physical_recipes::frame_protocol as recipes;
use recipes::FrameProtocolInstructions;
pub use recipes::{FrameRecipe, PlannedFrameFunction};
use register_environment::ValidatedTargetRegisterEnvironment;
use target::Architecture;
pub type PlannedTargetFrameProtocols =
    recipes::PlannedTargetFrameProtocols<crate::FrameRecipeOrigin>;
pub(crate) fn plan_target_frame_protocols(
    frame: &ValidatedTargetFrameLayout,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: TargetFrameProtocolEncodingPolicy,
) -> Result<PlannedTargetFrameProtocols, TargetFrameProtocolEncodingError> {
    if policy != TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1 {
        return Err(TargetFrameProtocolEncodingError::UnsupportedPolicy);
    }
    if frame.plan().target != environment.target()
        || frame.plan().register_environment != environment.identity()
        || frame.plan().physical_register_model != environment.physical().identity()
    {
        return Err(TargetFrameProtocolEncodingError::RootMismatch);
    }
    let codec = frame_protocol_codec(environment.target())
        .ok_or(TargetFrameProtocolEncodingError::UnsupportedTarget)?;
    let continuation = frame_unwind_policy(environment.target())
        .ok_or(TargetFrameProtocolEncodingError::UnsupportedTarget)?
        .continuation;
    let mut functions = Vec::with_capacity(frame.plan().functions.len());
    for function in &frame.plan().functions {
        let committed_bytes = committed_extent(function)
            .ok_or(TargetFrameProtocolEncodingError::NonCanonicalEncoding)?;
        if !continuation.admits(function.return_address) {
            return Err(TargetFrameProtocolEncodingError::UnsupportedReturnAddressCustody);
        }
        let recipe = match codec {
            FrameProtocolCodec::X86_64 => {
                let slots = function
                    .unwind
                    .restores
                    .iter()
                    .rev()
                    .map(|restore| target_x86_64::X86_64FrameSlot {
                        view: restore.view,
                        offset_bytes: restore.frame_offset_bytes,
                        size_bytes: restore.size_bytes,
                    })
                    .collect::<Vec<_>>();
                FrameRecipe::X86(
                    target_x86_64::plan_system_v_amd64_frame_protocol(
                        environment.physical(),
                        committed_bytes,
                        target_x86_64::X86_64StackProbe {
                            interval_bytes: function.stack_probe.interval_bytes,
                            touches: function.stack_probe.touches,
                        },
                        &slots,
                    )
                    .map_err(TargetFrameProtocolEncodingError::X86)?,
                )
            }
            FrameProtocolCodec::Aarch64 => {
                let slots = function
                    .unwind
                    .restores
                    .iter()
                    .rev()
                    .map(|restore| target_aarch64::Aarch64FrameSlot {
                        view: restore.view,
                        offset_bytes: restore.frame_offset_bytes,
                        size_bytes: restore.size_bytes,
                    })
                    .collect::<Vec<_>>();
                FrameRecipe::Aarch64(
                    target_aarch64::plan_aapcs64_frame_protocol(
                        environment.physical(),
                        committed_bytes,
                        target_aarch64::Aarch64StackProbe {
                            interval_bytes: function.stack_probe.interval_bytes,
                            touches: function.stack_probe.touches,
                        },
                        &slots,
                    )
                    .map_err(TargetFrameProtocolEncodingError::Aarch64)?,
                )
            }
        };
        let scratch = match &recipe {
            FrameRecipe::X86(_) => None,
            FrameRecipe::Aarch64(recipe) => recipe.address_scratch(),
        };
        let effects = plan_frame_protocol_effects(function, environment, scratch)
            .map_err(|_| TargetFrameProtocolEncodingError::NonCanonicalEncoding)?;
        functions.push(PlannedFrameFunction {
            machine: function.machine,
            effects,
            recipe,
        });
    }
    Ok(PlannedTargetFrameProtocols::from_planner(
        crate::FrameRecipeOrigin(()),
        FrameProtocolInstructions {
            frame_layout: frame.receipt().identity(),
            register_environment: environment.identity(),
            physical_register_model: environment.physical().identity(),
            target: environment.target(),
            policy,
            functions,
        },
    ))
}

fn plan_frame_protocol_effects(
    function: &machine_code::FunctionTargetFrameLayout,
    environment: &ValidatedTargetRegisterEnvironment,
    address_scratch: Option<u8>,
) -> Result<calling_conventions::StateEffectsEvidence, &'static str> {
    use calling_conventions::MachineRegister as R;
    use post_allocation_machine::adapters::AdapterOperation as Op;
    let architecture = environment.target().architecture;
    let model = environment.physical().model();
    let mut registers = Vec::with_capacity(function.unwind.restores.len());
    for restore in &function.unwind.restores {
        let name = model
            .view(restore.view)
            .ok_or("frame effect names an unknown register")?
            .name
            .as_str();
        let register = match architecture {
            Architecture::X86_64 => {
                let names = [
                    "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10",
                    "r11", "r12", "r13", "r14", "r15",
                ];
                let values = [
                    R::X86Rax,
                    R::X86Rcx,
                    R::X86Rdx,
                    R::X86Rbx,
                    R::X86Rsp,
                    R::X86Rbp,
                    R::X86Rsi,
                    R::X86Rdi,
                    R::X86R8,
                    R::X86R9,
                    R::X86R10,
                    R::X86R11,
                    R::X86R12,
                    R::X86R13,
                    R::X86R14,
                    R::X86R15,
                ];
                names
                    .iter()
                    .position(|candidate| *candidate == name)
                    .map(|index| values[index])
                    .or_else(|| {
                        name.strip_prefix("xmm")
                            .and_then(|number| number.parse::<u8>().ok())
                            .filter(|number| *number < 16)
                            .map(R::X86Xmm)
                    })
            }
            Architecture::Aarch64 => name
                .strip_prefix('x')
                .and_then(|number| number.parse::<u8>().ok())
                .filter(|number| *number < 31)
                .map(R::Aarch64X)
                .or_else(|| {
                    name.strip_prefix('d')
                        .and_then(|number| number.parse::<u8>().ok())
                        .filter(|number| *number < 32)
                        .map(R::Aarch64V)
                }),
        }
        .ok_or("frame effect has an unsupported preserved register")?;
        registers.push(register);
    }
    let committed = committed_extent(function)
        .and_then(|bytes| u32::try_from(bytes).ok())
        .ok_or("frame effect has an invalid committed extent")?;
    let scratch = R::Aarch64X(address_scratch.unwrap_or(16));
    let mut effects = Op::Frame {
        bytes: committed,
        interval: function.stack_probe.interval_bytes,
        touches: function.stack_probe.touches,
        enter: true,
    }
    .effects(architecture);
    for (restore, register) in function.unwind.restores.iter().zip(registers) {
        let size = u8::try_from(restore.size_bytes).map_err(|_| "frame effect size exceeds u8")?;
        for load in [false, true] {
            effects = effects.compose(
                &Op::FrameMemory {
                    register,
                    offset: u32::try_from(restore.frame_offset_bytes)
                        .map_err(|_| "frame effect offset exceeds u32")?,
                    size,
                    load,
                    scratch,
                }
                .effects(architecture),
            );
        }
    }
    Ok(effects)
}

#[cfg(test)]
mod tests {
    use super::plan_frame_protocol_effects;
    use calling_conventions::MachineRegister;
    use machine_code::{
        FrameUnwindPlan, FrameUnwindRestore, FunctionTargetFrameLayout, OutgoingAbiFrameArea,
        ReturnAddressFrameCustody, StackProbePlan,
    };
    use semantic_vocabulary::MachineId;
    use target::NativeTarget;

    #[test]
    fn plans_frame_effects_for_gpr_and_xmm_callee_saves_on_windows_x64() {
        let environment =
            register_environment::baseline_target_register_environment(NativeTarget::windows_x64())
                .unwrap();
        let model = environment.physical().model();
        let rsp = model.view_named("rsp").unwrap().id;
        let rbx = model.view_named("rbx").unwrap().id;
        let xmm6 = model.view_named("xmm6").unwrap().id;
        let function = FunctionTargetFrameLayout {
            machine: MachineId::new(1).unwrap(),
            contains_call: false,
            stack_pointer: rsp,
            pre_call_stack_alignment: 16,
            frame_size_bytes: 40,
            red_zone_resident_bytes: 0,
            abi_stack_alignment_bytes: 16,
            outgoing_abi_area: OutgoingAbiFrameArea {
                byte_size: 0,
                shadow_bytes: 0,
            },
            local_storage_slots: Vec::new(),
            stable_address_loans: Vec::new(),
            callee_save_slots: Vec::new(),
            return_address: ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes: 40,
                size_bytes: 8,
            },
            stack_probe: StackProbePlan {
                interval_bytes: 4_096,
                touches: 0,
            },
            unwind: FrameUnwindPlan {
                restores: vec![
                    FrameUnwindRestore {
                        view: xmm6,
                        frame_offset_bytes: 16,
                        size_bytes: 16,
                    },
                    FrameUnwindRestore {
                        view: rbx,
                        frame_offset_bytes: 0,
                        size_bytes: 8,
                    },
                ],
                released_bytes: 40,
                return_address: ReturnAddressFrameCustody::CallerActivationStack {
                    post_prologue_offset_bytes: 40,
                    size_bytes: 8,
                },
            },
        };
        let effects = plan_frame_protocol_effects(&function, &environment, None).unwrap();
        assert!(effects.registers_read().contains(MachineRegister::X86Rbx));
        assert!(effects.registers().contains(MachineRegister::X86Rbx));
        assert!(
            effects
                .registers_read()
                .contains(MachineRegister::X86Xmm(6))
        );
        assert!(effects.registers().contains(MachineRegister::X86Xmm(6)));
    }
}
