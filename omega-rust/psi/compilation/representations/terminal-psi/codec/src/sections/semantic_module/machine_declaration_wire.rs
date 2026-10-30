//! The same portable callable identity is used by bodies and demands.

use super::{CodecError, Reader, Writer, decode_counted};
use terminal_psi::{
    CheckedMachineDeclaration, CheckedMachineSpecialization, NominalDeclarationIdentity,
    TerminalIdentityIndex,
};

pub(super) fn encode(
    writer: &mut Writer,
    body: &CheckedMachineDeclaration,
) -> Result<(), CodecError> {
    validate(body)?;
    writer.u32(body.machine.identity().0);
    writer.u32(body.state.identity().0);
    writer.u32(body.semantic_declaration.0);
    writer.len("machine parameter identities", body.parameter_types.len())?;
    for identity in &body.parameter_types {
        writer.u32(identity.0);
    }
    writer.u32(body.callable.0);
    writer.u32(body.result_type.0);
    writer.bytes(&body.contract);
    writer.boolean(body.specialization.is_some());
    if let Some(specialization) = &body.specialization {
        writer.u32(specialization.template.identity().0);
        writer.u32(specialization.template_callable.0);
        writer.u32(specialization.template_semantic_declaration.0);
        writer.bytes(&specialization.commitment);
        writer.boolean(specialization.capture.is_some());
        if let Some(capture) = &specialization.capture {
            writer.u32(capture.template_identity.0);
            writer.bytes(&capture.template_commitment);
            writer.bytes(&capture.specialization_commitment);
        }
    }
    Ok(())
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<CheckedMachineDeclaration, CodecError> {
    let machine = NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?));
    let state = NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?));
    let semantic_declaration = TerminalIdentityIndex(reader.u32()?);
    let parameter_types =
        decode_counted(reader, |reader| Ok(TerminalIdentityIndex(reader.u32()?)))?;
    let callable = TerminalIdentityIndex(reader.u32()?);
    let result_type = TerminalIdentityIndex(reader.u32()?);
    let contract = reader.array()?;
    let specialization = if reader.boolean()? {
        Some(CheckedMachineSpecialization {
            template: NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?)),
            template_callable: TerminalIdentityIndex(reader.u32()?),
            template_semantic_declaration: TerminalIdentityIndex(reader.u32()?),
            commitment: reader.array()?,
            capture: if reader.boolean()? {
                Some(terminal_psi::CheckedMachineCapture {
                    template_identity: TerminalIdentityIndex(reader.u32()?),
                    template_commitment: reader.array()?,
                    specialization_commitment: reader.array()?,
                })
            } else {
                None
            },
        })
    } else {
        None
    };
    let body = CheckedMachineDeclaration {
        machine,
        state,
        semantic_declaration,
        parameter_types,
        callable,
        result_type,
        contract,
        specialization,
    };
    validate(&body)?;
    Ok(body)
}

fn validate(body: &CheckedMachineDeclaration) -> Result<(), CodecError> {
    if body.contract == [0; 32]
        || body.specialization.as_ref().is_some_and(|row| {
            row.commitment == [0; 32]
                || row.template_callable.is_absent()
                || row.template_semantic_declaration.is_absent()
                || row.capture.as_ref().is_some_and(|capture| {
                    capture.template_identity.is_absent()
                        || capture.template_commitment == [0; 32]
                        || capture.specialization_commitment == [0; 32]
                })
        })
    {
        return Err(CodecError::MalformedStructuralFoundation(
            "empty checked machine commitment",
        ));
    }
    Ok(())
}
