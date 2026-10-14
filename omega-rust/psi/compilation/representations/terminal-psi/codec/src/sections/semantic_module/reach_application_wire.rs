//! Canonical closed reach relations. These are ordinary semantic inputs, not
//! optional proof evidence; decoding replays their finite substitution and
//! operation joins through the common representation verifier.

use terminal_psi::{
    ClosedReachApplication, ClosedReachArgument, ClosedReachCall, ClosedReachCallApplication,
    ClosedReachMachineBinding, ClosedReachParameter, ClosedReachSchema,
};

use super::CodecError;
use super::structural_signature_wire::encode_service_ceiling;
use super::wire::{Reader, Writer};
use crate::sections::semantic_module::wire::{
    decode_counted, decode_ids, decode_optional_id, encode_optional_id,
};
use terminal_psi::TerminalIdentityIndex;

pub(crate) fn encode(
    writer: &mut Writer,
    application: Option<&ClosedReachApplication>,
) -> Result<(), CodecError> {
    let Some(application) = application else {
        writer.boolean(false);
        return Ok(());
    };
    writer.boolean(true);
    writer.u32(application.template_identity.0);
    writer.bytes(&application.template_commitment);
    writer.bytes(&application.specialization_commitment);
    writer.len("reach telescope", application.telescope.len())?;
    for parameter in &application.telescope {
        match parameter {
            ClosedReachParameter::Type { argument } => {
                writer.u8(0);
                writer.string("reach type argument", argument)?;
            }
            ClosedReachParameter::Const { argument } => {
                writer.u8(1);
                writer.string("reach const argument", argument)?;
            }
            ClosedReachParameter::Proposition { argument } => {
                writer.u8(2);
                writer.string("reach proposition argument", argument)?;
            }
            ClosedReachParameter::Machine(binding) => {
                writer.u8(3);
                writer.boolean(binding.nominal_requirement.is_some());
                if let Some(requirement) = &binding.nominal_requirement {
                    writer.string("reach requirement", requirement)?;
                }
                encode_service_ceiling(writer, &binding.upper_bound)?;
                writer.u32(binding.selected_identity.0);
                writer.bytes(&binding.selected_contract_commitment);
                encode_service_ceiling(writer, &binding.selected_reach)?;
                encode_optional_id(writer, binding.callee);
                writer.boolean(binding.schema.is_some());
                if let Some(schema) = &binding.schema {
                    writer.u32(schema.template_identity.0);
                    writer.bytes(&schema.template_commitment);
                }
            }
        }
    }
    encode_service_ceiling(writer, &application.fixed)?;
    writer.len("reach dependencies", application.dependencies.len())?;
    for binder in &application.dependencies {
        writer.u32(*binder);
    }
    writer.len("reach calls", application.calls.len())?;
    for call in &application.calls {
        writer.id(call.operation);
        writer.u32(call.binder);
        writer.boolean(call.application.is_some());
        if let Some(application) = &call.application {
            writer.id(application.callee);
            writer.bytes(&application.specialization_commitment);
            writer.len("reach call arguments", application.arguments.len())?;
            for argument in &application.arguments {
                encode_argument(writer, argument);
            }
        }
    }
    Ok(())
}

pub(crate) fn decode(
    reader: &mut Reader<'_>,
) -> Result<Option<ClosedReachApplication>, CodecError> {
    if !reader.boolean()? {
        return Ok(None);
    }
    let template_identity = TerminalIdentityIndex(reader.u32()?);
    let template_commitment = reader.array()?;
    let specialization_commitment = reader.array()?;
    let telescope = decode_counted(reader, |reader| {
        Ok(match reader.u8()? {
            0 => ClosedReachParameter::Type {
                argument: reader.string("reach const argument")?,
            },
            1 => ClosedReachParameter::Const {
                argument: reader.string("reach proposition argument")?,
            },
            2 => ClosedReachParameter::Proposition {
                argument: reader.string("reach requirement")?,
            },
            3 => {
                let nominal_requirement = if reader.boolean()? {
                    Some(reader.string("reach selected identity")?)
                } else {
                    None
                };
                ClosedReachParameter::Machine(ClosedReachMachineBinding {
                    nominal_requirement,
                    upper_bound: decode_ids(reader, "ServiceId")?,
                    selected_identity: TerminalIdentityIndex(reader.u32()?),
                    selected_contract_commitment: reader.array()?,
                    selected_reach: decode_ids(reader, "ServiceId")?,
                    callee: decode_optional_id(reader, "MachineId")?,
                    schema: if reader.boolean()? {
                        Some(ClosedReachSchema {
                            template_identity: TerminalIdentityIndex(reader.u32()?),
                            template_commitment: reader.array()?,
                        })
                    } else {
                        None
                    },
                })
            }
            tag => return Err(CodecError::InvalidTag("ClosedReachParameter", tag)),
        })
    })?;
    Ok(Some(ClosedReachApplication {
        template_identity,
        template_commitment,
        specialization_commitment,
        telescope,
        fixed: decode_ids(reader, "ServiceId")?,
        dependencies: decode_counted(reader, |reader| reader.u32())?,
        calls: decode_counted(reader, |reader| {
            Ok(ClosedReachCall {
                operation: reader.id("OperationId")?,
                binder: reader.u32()?,
                application: if reader.boolean()? {
                    Some(ClosedReachCallApplication {
                        callee: reader.id("MachineId")?,
                        specialization_commitment: reader.array()?,
                        arguments: decode_counted(reader, decode_argument)?,
                    })
                } else {
                    None
                },
            })
        })?,
    }))
}

/// One closed argument uses the same canonical wire across method applications
/// and reach applications; the receiving context owns its tuple role.
pub(crate) fn encode_argument(writer: &mut Writer, argument: &ClosedReachArgument) {
    match argument {
        ClosedReachArgument::Type(identity) => {
            writer.u8(0);
            writer.u32(identity.0);
        }
        ClosedReachArgument::Const(identity) => {
            writer.u8(1);
            writer.u32(identity.0);
        }
        ClosedReachArgument::Proposition(identity) => {
            writer.u8(2);
            writer.u32(identity.0);
        }
        ClosedReachArgument::Machine {
            identity,
            contract_commitment,
        } => {
            writer.u8(3);
            writer.u32(identity.0);
            writer.bytes(contract_commitment);
        }
    }
}
pub(crate) fn decode_argument(reader: &mut Reader<'_>) -> Result<ClosedReachArgument, CodecError> {
    Ok(match reader.u8()? {
        0 => ClosedReachArgument::Type(TerminalIdentityIndex(reader.u32()?)),
        1 => ClosedReachArgument::Const(TerminalIdentityIndex(reader.u32()?)),
        2 => ClosedReachArgument::Proposition(TerminalIdentityIndex(reader.u32()?)),
        3 => ClosedReachArgument::Machine {
            identity: TerminalIdentityIndex(reader.u32()?),
            contract_commitment: reader.array()?,
        },
        tag => return Err(CodecError::InvalidTag("ClosedReachArgument", tag)),
    })
}
