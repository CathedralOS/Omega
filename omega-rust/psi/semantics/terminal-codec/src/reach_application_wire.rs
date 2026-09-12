//! Canonical closed reach relations. These are ordinary semantic inputs, not
//! optional proof evidence; decoding replays their finite substitution and
//! operation joins through the common representation verifier.

use terminal_psi::{
    ClosedReachApplication, ClosedReachCall, ClosedReachMachineBinding, ClosedReachParameter,
};

use super::structural_signature_wire::encode_service_ceiling;
use super::wire::{Reader, Writer};
use super::{CodecError, decode_counted, decode_ids, decode_optional_id, encode_optional_id};

pub(super) fn encode(
    writer: &mut Writer,
    application: Option<&ClosedReachApplication>,
) -> Result<(), CodecError> {
    let Some(application) = application else {
        writer.boolean(false);
        return Ok(());
    };
    writer.boolean(true);
    writer.string("reach template", &application.template_identity)?;
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
                writer.string("reach selected identity", &binding.selected_identity)?;
                writer.bytes(&binding.selected_contract_commitment);
                encode_service_ceiling(writer, &binding.selected_reach)?;
                encode_optional_id(writer, binding.callee);
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
    }
    Ok(())
}

pub(super) fn decode(
    reader: &mut Reader<'_>,
) -> Result<Option<ClosedReachApplication>, CodecError> {
    if !reader.boolean()? {
        return Ok(None);
    }
    let template_identity = reader.string("reach template")?;
    let template_commitment = reader.array()?;
    let specialization_commitment = reader.array()?;
    let telescope = decode_counted(reader, |reader| {
        Ok(match reader.u8()? {
            0 => ClosedReachParameter::Type {
                argument: reader.string("reach type argument")?,
            },
            1 => ClosedReachParameter::Const {
                argument: reader.string("reach const argument")?,
            },
            2 => ClosedReachParameter::Proposition {
                argument: reader.string("reach proposition argument")?,
            },
            3 => {
                let nominal_requirement = if reader.boolean()? {
                    Some(reader.string("reach requirement")?)
                } else {
                    None
                };
                ClosedReachParameter::Machine(ClosedReachMachineBinding {
                    nominal_requirement,
                    upper_bound: decode_ids(reader, "ServiceId")?,
                    selected_identity: reader.string("reach selected identity")?,
                    selected_contract_commitment: reader.array()?,
                    selected_reach: decode_ids(reader, "ServiceId")?,
                    callee: decode_optional_id(reader, "MachineId")?,
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
            })
        })?,
    }))
}
