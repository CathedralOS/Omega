//! Call payloads outside the fixed-tag rows: the module invocation inputs that
//! a no-normal invocation edge shares with calls, and register access, whose
//! tag follows its service authority. Call operations are rows of
//! `operation_wire!`.

use super::super::CodecError;
use super::super::contract_wire::encode_crash_routes;
use super::super::proof_term_wire::encode_proof_terms;
use super::super::scalar_term_wire::encode_scalar_terms;
use super::super::structural_place_wire::{encode_structural_arguments, encode_structural_path};
use super::super::wire::{Reader, Writer};
use super::operation_tags;
use semantic_vocabulary::ServiceId;
use terminal_psi::OperationKind;

/// The same entry payload crosses ordinary calls and no-normal invocation edges.
pub(super) fn encode_module_invocation_inputs(
    writer: &mut Writer,
    invocation: terminal_psi::ModuleInvocationInputs<'_>,
) -> Result<(), CodecError> {
    writer.id(invocation.callee);
    writer.ids("module invocation scalar arguments", invocation.arguments)?;
    encode_scalar_terms(writer, invocation.erased_arguments)?;
    encode_proof_terms(writer, invocation.erased_proof_arguments)?;
    encode_structural_arguments(writer, invocation.structural_arguments)?;
    writer.len("call claim transfers", invocation.claim_transfers.len())?;
    for transfer in invocation.claim_transfers {
        writer.id(transfer.claim);
        writer.u32(transfer.argument_index);
        encode_structural_path(writer, "call claim transfer path", &transfer.path)?;
    }
    writer.ids(
        "requirement obligations",
        invocation.requirement_obligations,
    )?;
    encode_crash_routes(writer, invocation.crash_continuations)
}

pub(super) fn encode_register_access(
    writer: &mut Writer,
    service: Option<ServiceId>,
    event: terminal_psi::RegisterAccessEvent,
) -> Result<(), CodecError> {
    if !event.operation().is_admitted()
        || service.is_some() != event.operation().requires_service_authority()
    {
        return Err(CodecError::MalformedStructuralFoundation(
            "register access disagrees with its catalog service authority",
        ));
    }
    if let Some(service) = service {
        writer.u8(operation_tags::REGISTER_ACCESS);
        writer.id(service);
    } else {
        writer.u8(operation_tags::REGISTER_OBSERVATION);
    }
    writer.u16(event.operation().catalog_tag());
    for value in event.inputs() {
        writer.id(*value);
    }
    Ok(())
}

pub(super) fn decode_register_access(
    reader: &mut Reader<'_>,
    requires_service: bool,
) -> Result<OperationKind, CodecError> {
    let service = if requires_service {
        Some(reader.id("ServiceId")?)
    } else {
        None
    };
    let tag = reader.u16()?;
    let operation = terminal_psi::RegisterOperation::from_catalog_tag(tag).ok_or(
        CodecError::MalformedStructuralFoundation("unknown register access catalog identity"),
    )?;
    if service.is_some() != operation.requires_service_authority() {
        return Err(CodecError::MalformedStructuralFoundation(
            "register access wire tag disagrees with catalog service authority",
        ));
    }
    let event = match operation {
        terminal_psi::RegisterOperation::MemoryFence(kind) => {
            terminal_psi::RegisterAccessEvent::MemoryFence { kind }
        }
        terminal_psi::RegisterOperation::ProcessorHalt => {
            terminal_psi::RegisterAccessEvent::ProcessorHalt
        }
        terminal_psi::RegisterOperation::InterruptControl(kind) => {
            terminal_psi::RegisterAccessEvent::InterruptControl { kind }
        }
        terminal_psi::RegisterOperation::SystemRead(register) => {
            terminal_psi::RegisterAccessEvent::Read { register }
        }
        terminal_psi::RegisterOperation::SystemWrite(register) => {
            terminal_psi::RegisterAccessEvent::Write {
                register,
                value: reader.id("ValueId")?,
            }
        }
        terminal_psi::RegisterOperation::PortRead => terminal_psi::RegisterAccessEvent::PortRead {
            port: reader.id("ValueId")?,
        },
        terminal_psi::RegisterOperation::PortWrite => {
            terminal_psi::RegisterAccessEvent::PortWrite {
                port: reader.id("ValueId")?,
                value: reader.id("ValueId")?,
            }
        }
        terminal_psi::RegisterOperation::ModelSpecificRead => {
            terminal_psi::RegisterAccessEvent::ModelSpecificRead {
                index: reader.id("ValueId")?,
            }
        }
        terminal_psi::RegisterOperation::ModelSpecificWrite => {
            terminal_psi::RegisterAccessEvent::ModelSpecificWrite {
                index: reader.id("ValueId")?,
                value: reader.id("ValueId")?,
            }
        }
    };
    Ok(OperationKind::RegisterAccess { service, event })
}
