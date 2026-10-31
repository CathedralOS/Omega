//! Retained semantic operations, never an absence verdict or provider choice.

use super::{CodecError, Reader, Writer, decode_counted};
use crate::sections::semantic_module::wire::{decode_invocation_site, encode_invocation_site};
use terminal_psi::{UnoptimizedBehaviorOperation as Operation, UnoptimizedMachineBehavior};

pub(super) fn encode(
    writer: &mut Writer,
    body: &UnoptimizedMachineBehavior,
) -> Result<(), CodecError> {
    writer.id(body.machine);
    writer.counted(
        "unoptimized semantic operations",
        &body.operations,
        encode_operation,
    )
}

fn encode_operation(writer: &mut Writer, operation: &Operation) -> Result<(), CodecError> {
    match *operation {
        Operation::Invocation {
            block,
            site,
            callee,
        } => {
            writer.u8(0);
            writer.id(block);
            encode_invocation_site(writer, site);
            writer.id(callee);
        }
        Operation::BoundaryInvocation {
            block,
            site,
            boundary,
        } => {
            writer.u8(1);
            writer.id(block);
            encode_invocation_site(writer, site);
            writer.id(boundary);
        }
        Operation::InstructionService {
            block,
            operation,
            service,
        } => {
            writer.u8(2);
            writer.id(block);
            writer.id(operation);
            writer.id(service);
        }
        Operation::Crash { block, cause } => {
            writer.u8(3);
            writer.id(block);
            writer.u8(match cause {
                semantic_vocabulary::CrashCause::Trap => 0,
                semantic_vocabulary::CrashCause::Abort => 1,
            });
        }
        Operation::NominalCleanup { block, callee } => {
            writer.u8(4);
            writer.id(block);
            writer.id(callee);
        }
    }
    Ok(())
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<UnoptimizedMachineBehavior, CodecError> {
    Ok(UnoptimizedMachineBehavior {
        machine: reader.id("MachineId")?,
        operations: decode_counted(reader, decode_operation)?,
    })
}

fn decode_operation(reader: &mut Reader<'_>) -> Result<Operation, CodecError> {
    Ok(match reader.u8()? {
        0 => Operation::Invocation {
            block: reader.id("BlockId")?,
            site: decode_invocation_site(reader)?,
            callee: reader.id("MachineId")?,
        },
        1 => Operation::BoundaryInvocation {
            block: reader.id("BlockId")?,
            site: decode_invocation_site(reader)?,
            boundary: reader.id("BoundaryMachineId")?,
        },
        2 => Operation::InstructionService {
            block: reader.id("BlockId")?,
            operation: reader.id("OperationId")?,
            service: reader.id("ServiceId")?,
        },
        3 => Operation::Crash {
            block: reader.id("BlockId")?,
            cause: match reader.u8()? {
                0 => semantic_vocabulary::CrashCause::Trap,
                1 => semantic_vocabulary::CrashCause::Abort,
                tag => return Err(CodecError::InvalidTag("semantic crash cause", tag)),
            },
        },
        4 => Operation::NominalCleanup {
            block: reader.id("BlockId")?,
            callee: reader.id("MachineId")?,
        },
        tag => {
            return Err(CodecError::InvalidTag(
                "unoptimized semantic operation",
                tag,
            ));
        }
    })
}
