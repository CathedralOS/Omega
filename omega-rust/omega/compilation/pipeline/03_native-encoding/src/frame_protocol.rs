//! Encode the immutable frame recipes completed by physical planning.
use frame_admission::frame_protocol::TargetFrameProtocolEncodingError;
use machine_code::{
    FrameProtocolByteSpan, FramedMachineProgram, FunctionTargetFrameProtocolEncoding,
    TargetFrameProtocolEncodingPlan,
};
use physical_planning::{
    PhysicalFrameProgram,
    frame_protocol::{FrameRecipe, PlannedTargetFrameProtocols},
};
use register_environment::ValidatedTargetRegisterEnvironment;
use std::sync::Arc;
pub(crate) fn encode_target_frame_protocols(
    prepared: PlannedTargetFrameProtocols,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<TargetFrameProtocolEncodingPlan, TargetFrameProtocolEncodingError> {
    if prepared.target() != environment.target()
        || prepared.register_environment() != environment.identity()
        || prepared.physical_register_model() != environment.physical().identity()
    {
        return Err(TargetFrameProtocolEncodingError::RootMismatch);
    }
    let mut bytes = Vec::new();
    let mut functions = Vec::with_capacity(prepared.functions().len());
    for function in prepared.functions() {
        let (prologue, epilogue) = match function.recipe() {
            FrameRecipe::X86(recipe) => isa_x86_64::encode_planned_system_v_amd64_frame_protocol(
                environment.physical(),
                recipe,
            )
            .map_err(TargetFrameProtocolEncodingError::X86)?,
            FrameRecipe::Aarch64(recipe) => {
                isa_aarch64::encode_planned_aapcs64_frame_protocol(environment.physical(), recipe)
                    .map_err(TargetFrameProtocolEncodingError::Aarch64)?
            }
        };
        let prologue = append(&mut bytes, &prologue)?;
        let epilogue = append(&mut bytes, &epilogue)?;
        functions.push(FunctionTargetFrameProtocolEncoding {
            machine: function.machine(),
            prologue,
            epilogue,
        });
    }
    let plan = TargetFrameProtocolEncodingPlan {
        frame_layout: prepared.frame_layout(),
        register_environment: prepared.register_environment(),
        physical_register_model: prepared.physical_register_model(),
        target: prepared.target(),
        policy: prepared.policy(),
        functions,
        bytes,
    };
    Ok(plan)
}

fn append(
    arena: &mut Vec<u8>,
    value: &[u8],
) -> Result<FrameProtocolByteSpan, TargetFrameProtocolEncodingError> {
    let offset = u32::try_from(arena.len())
        .map_err(|_| TargetFrameProtocolEncodingError::ByteArenaOverflow)?;
    let length = u32::try_from(value.len())
        .map_err(|_| TargetFrameProtocolEncodingError::ByteArenaOverflow)?;
    arena
        .len()
        .checked_add(value.len())
        .ok_or(TargetFrameProtocolEncodingError::ByteArenaOverflow)?;
    arena.extend_from_slice(value);
    Ok(FrameProtocolByteSpan { offset, length })
}

pub fn encode_function_frame(
    planned: PhysicalFrameProgram,
) -> Result<FramedMachineProgram, TargetFrameProtocolEncodingError> {
    let PhysicalFrameProgram {
        instructions: program,
        storage,
        frame: layout,
        protocols,
    } = planned;
    let environment = &program.environment;
    let protocol = encode_target_frame_protocols(protocols, environment)?;
    Ok(FramedMachineProgram {
        fragment_origins: Arc::new(
            encoding_admission::fragment_origins::prepare_fragment_origins(&program),
        ),
        frame_demands: layout.shared_demands(),
        frame_identity: layout.receipt().identity(),
        protocol_identity: machine_code::target_frame_protocol_encoding_identity(&protocol),
        instructions: program,
        storage,
        frame: layout.shared_plan(),
        protocol: Arc::new(protocol),
    })
}
