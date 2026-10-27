//! Canonical closed reach relations. These are ordinary semantic inputs, not
//! optional proof evidence; decoding replays their finite substitution and
//! operation joins through the common representation verifier.

use terminal_psi::{
    ClosedReachApplication, ClosedReachArgument, ClosedReachCall, ClosedReachCallApplication,
    ClosedReachCapture, ClosedReachMachineBinding, ClosedReachParameter, ClosedReachSchema,
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
    encode_kinds(writer, &application.parameter_kinds)?;
    encode_telescope(writer, &application.telescope)?;
    writer.boolean(application.capture.is_some());
    if let Some(capture) = &application.capture {
        writer.u32(capture.template_identity.0);
        writer.bytes(&capture.template_commitment);
        writer.bytes(&capture.specialization_commitment);
        encode_kinds(writer, &capture.parameter_kinds)?;
        encode_telescope(writer, &capture.telescope)?;
    }
    encode_service_ceiling(writer, &application.fixed)?;
    writer.len("reach dependencies", application.dependencies.len())?;
    for binder in &application.dependencies {
        writer.u32(*binder);
    }
    writer.len("reach calls", application.calls.len())?;
    for call in &application.calls {
        super::wire::encode_invocation_site(writer, call.site);
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

fn encode_kinds(writer: &mut Writer, kinds: &[u8]) -> Result<(), CodecError> {
    writer.len("reach parameter kinds", kinds.len())?;
    writer.bytes(kinds);
    Ok(())
}

fn decode_kinds(reader: &mut Reader<'_>) -> Result<Vec<u8>, CodecError> {
    decode_counted(reader, |reader| {
        let kind = reader.u8()?;
        if kind > 5 {
            return Err(CodecError::InvalidTag("ReachParameterKind", kind));
        }
        Ok(kind)
    })
}

fn encode_telescope(
    writer: &mut Writer,
    telescope: &[ClosedReachParameter],
) -> Result<(), CodecError> {
    writer.len("reach telescope", telescope.len())?;
    for parameter in telescope {
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
                encode_kinds(writer, &binding.requirement_parameter_kinds)?;
                writer.len(
                    "reach requirement arguments",
                    binding.requirement_arguments.len(),
                )?;
                for argument in &binding.requirement_arguments {
                    writer.u32(argument.0);
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
    let parameter_kinds = decode_kinds(reader)?;
    let telescope = decode_telescope(reader)?;
    let capture = if reader.boolean()? {
        Some(ClosedReachCapture {
            template_identity: TerminalIdentityIndex(reader.u32()?),
            template_commitment: reader.array()?,
            specialization_commitment: reader.array()?,
            parameter_kinds: decode_kinds(reader)?,
            telescope: decode_telescope(reader)?,
        })
    } else {
        None
    };
    Ok(Some(ClosedReachApplication {
        template_identity,
        template_commitment,
        specialization_commitment,
        parameter_kinds,
        telescope,
        capture,
        fixed: decode_ids(reader, "ServiceId")?,
        dependencies: decode_counted(reader, |reader| reader.u32())?,
        calls: decode_counted(reader, |reader| {
            Ok(ClosedReachCall {
                site: super::wire::decode_invocation_site(reader)?,
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

fn decode_telescope(reader: &mut Reader<'_>) -> Result<Vec<ClosedReachParameter>, CodecError> {
    decode_counted(reader, |reader| {
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
                    requirement_parameter_kinds: decode_kinds(reader)?,
                    requirement_arguments: decode_counted(reader, |reader| {
                        Ok(TerminalIdentityIndex(reader.u32()?))
                    })?,
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
    })
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
