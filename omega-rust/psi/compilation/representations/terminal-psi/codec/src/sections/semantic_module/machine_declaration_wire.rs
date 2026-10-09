//! The same portable callable identity is used by bodies and demands.

use super::{CodecError, Reader, Writer};
use terminal_psi::{
    CheckedMachineDeclaration, CheckedMachineSpecialization, NominalDeclarationIdentity,
};

pub(super) fn encode(
    writer: &mut Writer,
    body: &CheckedMachineDeclaration,
) -> Result<(), CodecError> {
    validate(body)?;
    writer.string("checked machine", body.machine.canonical())?;
    writer.string("checked state", body.state.canonical())?;
    writer.bytes(&body.contract);
    writer.boolean(body.specialization.is_some());
    if let Some(specialization) = &body.specialization {
        writer.string("checked template", specialization.template.canonical())?;
        writer.bytes(&specialization.commitment);
    }
    Ok(())
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<CheckedMachineDeclaration, CodecError> {
    let machine = NominalDeclarationIdentity::new(reader.string("checked machine")?)
        .map_err(CodecError::MalformedStructuralFoundation)?;
    let state = NominalDeclarationIdentity::new(reader.string("checked state")?)
        .map_err(CodecError::MalformedStructuralFoundation)?;
    let contract = reader.array()?;
    let specialization = if reader.boolean()? {
        Some(CheckedMachineSpecialization {
            template: NominalDeclarationIdentity::new(reader.string("checked template")?)
                .map_err(CodecError::MalformedStructuralFoundation)?,
            commitment: reader.array()?,
        })
    } else {
        None
    };
    let body = CheckedMachineDeclaration {
        machine,
        state,
        contract,
        specialization,
    };
    validate(&body)?;
    Ok(body)
}

fn validate(body: &CheckedMachineDeclaration) -> Result<(), CodecError> {
    if body.contract == [0; 32]
        || body
            .specialization
            .as_ref()
            .is_some_and(|row| row.commitment == [0; 32])
    {
        return Err(CodecError::MalformedStructuralFoundation(
            "empty checked machine commitment",
        ));
    }
    Ok(())
}
