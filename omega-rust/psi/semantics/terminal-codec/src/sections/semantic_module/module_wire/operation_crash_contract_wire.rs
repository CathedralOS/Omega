//! Operation crash contracts on the wire: the owning machine and operation,
//! the published routes in the operation's formal namespace, then the
//! surviving continuations in the machine's actual-value namespace.

use super::super::CodecError;
use super::super::contract_wire::{decode_crash_routes, encode_crash_routes};
use super::super::wire::{Reader, Writer};

pub(super) fn encode_operation_crash_contract(
    writer: &mut Writer,
    contract: &terminal_psi::TerminalOperationCrashContract,
) -> Result<(), CodecError> {
    writer.id(contract.machine);
    writer.id(contract.operation);
    encode_crash_routes(writer, &contract.published_routes)?;
    encode_crash_routes(writer, &contract.crash_continuations)
}

pub(super) fn decode_operation_crash_contract(
    reader: &mut Reader<'_>,
) -> Result<terminal_psi::TerminalOperationCrashContract, CodecError> {
    Ok(terminal_psi::TerminalOperationCrashContract {
        machine: reader.id("MachineId")?,
        operation: reader.id("OperationId")?,
        published_routes: decode_crash_routes(reader)?,
        crash_continuations: decode_crash_routes(reader)?,
    })
}
