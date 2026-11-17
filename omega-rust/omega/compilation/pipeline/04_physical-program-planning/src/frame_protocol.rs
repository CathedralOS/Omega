//! Choose every frame register and save/restore recipe before encoding.
use frame_admission::frame_layout::{
    ValidatedTargetFrameLayout, committed_extent, frame_unwind_policy,
};
use frame_admission::frame_protocol::{
    FrameProtocolCodec, TargetFrameProtocolEncodingError, frame_protocol_codec,
};
use machine_code::{TargetFrameLayoutIdentity, TargetFrameProtocolEncodingPolicy};
use register_environment::ValidatedTargetRegisterEnvironment;
use target::{Architecture, NativeTarget};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedTargetFrameProtocols {
    frame_layout: TargetFrameLayoutIdentity,
    register_environment: register_model::TargetRegisterEnvironmentIdentity,
    physical_register_model: register_model::PhysicalRegisterModelIdentity,
    target: NativeTarget,
    policy: TargetFrameProtocolEncodingPolicy,
    functions: Vec<PlannedFrameFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFrameFunction {
    machine: semantic_vocabulary::MachineId,
    effects: calling_conventions::StateEffectsEvidence,
    recipe: FrameRecipe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameRecipe {
    X86(target_x86_64::X86_64FrameProtocolPlan),
    Aarch64(target_aarch64::Aarch64FrameProtocolPlan),
}

impl PlannedTargetFrameProtocols {
    pub fn function_effects(
        &self,
        machine: semantic_vocabulary::MachineId,
    ) -> Option<&calling_conventions::StateEffectsEvidence> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
            .map(|function| &function.effects)
    }
}

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
    Ok(PlannedTargetFrameProtocols {
        frame_layout: frame.receipt().identity(),
        register_environment: environment.identity(),
        physical_register_model: environment.physical().identity(),
        target: environment.target(),
        policy,
        functions,
    })
}

impl PlannedTargetFrameProtocols {
    pub fn frame_layout(&self) -> TargetFrameLayoutIdentity {
        self.frame_layout
    }
    pub fn register_environment(&self) -> register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub fn physical_register_model(&self) -> register_model::PhysicalRegisterModelIdentity {
        self.physical_register_model
    }
    pub fn target(&self) -> NativeTarget {
        self.target
    }
    pub fn policy(&self) -> TargetFrameProtocolEncodingPolicy {
        self.policy
    }
    pub fn functions(&self) -> &[PlannedFrameFunction] {
        &self.functions
    }
}
impl PlannedFrameFunction {
    pub fn machine(&self) -> semantic_vocabulary::MachineId {
        self.machine
    }
    pub fn recipe(&self) -> &FrameRecipe {
        &self.recipe
    }
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
        for load in [false, true] {
            effects = effects.compose(
                &Op::FrameMemory {
                    register,
                    offset: u32::try_from(restore.frame_offset_bytes)
                        .map_err(|_| "frame effect offset exceeds u32")?,
                    size: 8,
                    load,
                    scratch,
                }
                .effects(architecture),
            );
        }
    }
    Ok(effects)
}
