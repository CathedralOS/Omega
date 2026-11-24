//! Encode the immutable frame recipes completed by physical planning.
use calling_conventions::frame_unwind::FrameProtocolStep;
use frame_admission::frame_protocol::TargetFrameProtocolEncodingError;
use machine_code::{
    FrameProtocolByteSpan, FrameProtocolStepSpan, FramedMachineProgram,
    FunctionTargetFrameProtocolEncoding, TargetFrameProtocolEncodingPlan,
};
use physical_recipes::frame_protocol::{
    FrameRecipe, PhysicalFrameProgram, PlannedTargetFrameProtocols,
};
use register_environment::ValidatedTargetRegisterEnvironment;
use std::sync::Arc;
pub(crate) fn encode_target_frame_protocols<Origin>(
    prepared: PlannedTargetFrameProtocols<Origin>,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<TargetFrameProtocolEncodingPlan, TargetFrameProtocolEncodingError> {
    if prepared.target() != environment.target()
        || prepared.register_environment() != environment.identity()
        || prepared.physical_register_model() != environment.physical().identity()
    {
        return Err(TargetFrameProtocolEncodingError::RootMismatch);
    }
    let mut bytes = Vec::new();
    let mut steps = Vec::new();
    let mut functions = Vec::with_capacity(prepared.functions().len());
    for function in prepared.functions() {
        let encoded = match function.recipe() {
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
        functions.push(FunctionTargetFrameProtocolEncoding {
            machine: function.machine(),
            prologue: append(&mut bytes, &encoded.prologue)?,
            epilogue: append(&mut bytes, &encoded.epilogue)?,
            prologue_steps: append_steps(&mut steps, &encoded.prologue_steps)?,
            epilogue_steps: append_steps(&mut steps, &encoded.epilogue_steps)?,
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
        steps,
    };
    Ok(plan)
}

fn append_steps(
    arena: &mut Vec<FrameProtocolStep>,
    value: &[FrameProtocolStep],
) -> Result<FrameProtocolStepSpan, TargetFrameProtocolEncodingError> {
    let offset = u32::try_from(arena.len())
        .map_err(|_| TargetFrameProtocolEncodingError::ByteArenaOverflow)?;
    let length = u32::try_from(value.len())
        .map_err(|_| TargetFrameProtocolEncodingError::ByteArenaOverflow)?;
    arena.extend_from_slice(value);
    Ok(FrameProtocolStepSpan { offset, length })
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

#[derive(Debug)]
pub enum FunctionBodyEncodingError {
    Encoding(crate::OptimizedSelectedFormEncodingError),
    BodyContracts(&'static str),
    FrameProtocol(TargetFrameProtocolEncodingError),
    WindowsInternet(&'static str),
}

pub(super) fn encode_function_frame<FrameOrigin, ImportOrigin>(
    body: physical_recipes::frame_protocol::PlannedFunctionBody<FrameOrigin, ImportOrigin>,
    audit: bool,
) -> Result<FramedMachineProgram, FunctionBodyEncodingError> {
    let bindings = machine_code::import_code::EncodedBodyBindings {
        private_references: body.private_references.into(),
        imports: Arc::new(crate::imports::encode_import_program(body.imports)),
        windows_internet: body
            .windows_internet
            .map(crate::windows_internet::encode_windows_internet_provider)
            .transpose()
            .map_err(FunctionBodyEncodingError::WindowsInternet)?
            .map(Arc::new),
    };
    let PhysicalFrameProgram {
        instructions: program,
        storage,
        frame: layout,
        protocols,
    } = body.frame;
    let environment = &program.environment;
    let protocol = encode_target_frame_protocols(protocols, environment)
        .map_err(FunctionBodyEncodingError::FrameProtocol)?;
    let protocol_identity = if audit {
        encoding_admission::frame_protocol::validate_target_frame_protocol_encoding(
            &layout,
            environment,
            &protocol,
        )
        .map_err(FunctionBodyEncodingError::FrameProtocol)?
    } else {
        machine_code::target_frame_protocol_encoding_identity(&protocol)
    };
    Ok(FramedMachineProgram {
        bindings,
        fragment_origins: Arc::new(
            encoding_admission::fragment_origins::prepare_fragment_origins(&program),
        ),
        frame_demands: layout.shared_demands(),
        frame_identity: layout.receipt().identity(),
        protocol_identity,
        instructions: program,
        storage,
        frame: layout.shared_plan(),
        protocol: Arc::new(protocol),
    })
}
