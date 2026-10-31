//! Native declaration telescopes; storage and contracts stay on the boundary.

use terminal_psi::{
    CallingCallbackBinder, CallingDirectCallback, CallingRuntimeFormal,
    CallingSignatureDeclaration, CallingSignatureTelescope, TerminalIdentityIndex,
};

use super::{CodecError, Reader, Writer, decode_counted};

pub(super) fn encode(
    writer: &mut Writer,
    signature: &CallingSignatureDeclaration,
) -> Result<(), CodecError> {
    writer.id(signature.boundary);
    writer.u32(signature.requirement_identity.0);
    match &signature.telescope {
        Err(reason) => {
            writer.boolean(false);
            writer.string("deferred calling signature refusal", reason)?;
        }
        Ok(telescope) => {
            writer.boolean(true);
            writer.counted(
                "calling runtime formals",
                &telescope.parameters,
                |writer, formal| {
                    writer.u64(formal.identity.get());
                    writer.u32(formal.native_ordinal);
                    Ok(())
                },
            )?;
            writer.counted(
                "calling callback binders",
                &telescope.binders,
                |writer, binder| {
                    writer.u64(binder.binder.get());
                    writer.u64(binder.requirement.get());
                    writer.u32(binder.requirement_identity.0);
                    writer.u32(binder.static_machine_ordinal);
                    writer.boolean(binder.invokes_synchronously);
                    Ok(())
                },
            )?;
            writer.counted(
                "calling direct callbacks",
                &telescope.direct_callbacks,
                |writer, callback| {
                    writer.u64(callback.identity.get());
                    writer.u32(callback.native_ordinal);
                    writer.u64(callback.binder.get());
                    writer.u64(callback.requirement.get());
                    Ok(())
                },
            )?;
        }
    }
    Ok(())
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<CallingSignatureDeclaration, CodecError> {
    Ok(CallingSignatureDeclaration {
        boundary: reader.id("BoundaryMachineId")?,
        requirement_identity: TerminalIdentityIndex(reader.u32()?),
        telescope: if reader.boolean()? {
            Ok(CallingSignatureTelescope {
                parameters: decode_counted(reader, |reader| {
                    Ok(CallingRuntimeFormal {
                        identity: semantic_vocabulary::NativeParameterId::new(reader.u64()?)
                            .ok_or(CodecError::MalformedStructuralFoundation(
                                "zero native parameter identity",
                            ))?,
                        native_ordinal: reader.u32()?,
                    })
                })?,
                binders: decode_counted(reader, |reader| {
                    Ok(CallingCallbackBinder {
                        binder: semantic_vocabulary::StaticMachineBinderId::new(reader.u64()?)
                            .ok_or(CodecError::MalformedStructuralFoundation(
                                "zero callback binder identity",
                            ))?,
                        requirement: semantic_vocabulary::CallbackRequirementId::new(reader.u64()?)
                            .ok_or(CodecError::MalformedStructuralFoundation(
                                "zero callback requirement identity",
                            ))?,
                        requirement_identity: TerminalIdentityIndex(reader.u32()?),
                        static_machine_ordinal: reader.u32()?,
                        invokes_synchronously: reader.boolean()?,
                    })
                })?,
                direct_callbacks: decode_counted(reader, |reader| {
                    Ok(CallingDirectCallback {
                        identity: semantic_vocabulary::NativeParameterId::new(reader.u64()?)
                            .ok_or(CodecError::MalformedStructuralFoundation(
                                "zero native parameter identity",
                            ))?,
                        native_ordinal: reader.u32()?,
                        binder: semantic_vocabulary::StaticMachineBinderId::new(reader.u64()?)
                            .ok_or(CodecError::MalformedStructuralFoundation(
                                "zero callback binder identity",
                            ))?,
                        requirement: semantic_vocabulary::CallbackRequirementId::new(reader.u64()?)
                            .ok_or(CodecError::MalformedStructuralFoundation(
                                "zero callback requirement identity",
                            ))?,
                    })
                })?,
            })
        } else {
            Err(reader.string("deferred calling signature refusal")?)
        },
    })
}
