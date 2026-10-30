//! Exact Terminal callable identities.

use super::super::CodecError;
use super::super::wire::{Reader, Writer};
use terminal_psi::TerminalCallable;

pub(super) fn encode_callable(writer: &mut Writer, callable: TerminalCallable) {
    match callable {
        TerminalCallable::Machine(machine) => {
            writer.u8(1);
            writer.id(machine);
        }
        TerminalCallable::Boundary(boundary) => {
            writer.u8(2);
            writer.id(boundary);
        }
    }
}

pub(super) fn decode_callable(reader: &mut Reader<'_>) -> Result<TerminalCallable, CodecError> {
    Ok(match reader.u8()? {
        1 => TerminalCallable::Machine(reader.id("MachineId")?),
        2 => TerminalCallable::Boundary(reader.id("BoundaryMachineId")?),
        tag => return Err(CodecError::InvalidTag("TerminalCallable", tag)),
    })
}
