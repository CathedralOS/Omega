//! Boundary application identity references at external module ingress.
//!
//! Decoding already validated the module table's structure. This join admits
//! the actual tuple and its directly associated declaration/body references
//! against that same issuer. Structural/category policy stays in its existing
//! owners; compilation-produced rows are not reverified by this operation.

use terminal_psi::{TerminalIdentityIndex, TerminalIdentityTable, TerminalModule};

use super::CodecError;

pub(super) fn validate_boundary_application_identities(
    module: &TerminalModule,
) -> Result<(), CodecError> {
    let identities = &module.identities;
    for demand in &module.boundary_application_demands {
        require_identity(identities, demand.requirement().declaration().identity())?;
        demand
            .application()
            .validate_in_identity_table(identities)
            .map_err(CodecError::MalformedStructuralFoundation)?;
    }
    for candidate in &module.boundary_application_candidates {
        require_identity(identities, candidate.requirement.declaration().identity())?;
        require_identity(identities, candidate.selection_identity)?;
        require_identity(identities, candidate.body.machine.identity())?;
        require_identity(identities, candidate.body.state.identity())?;
        if let Some(specialization) = &candidate.body.specialization {
            require_identity(identities, specialization.template.identity())?;
        }
        candidate
            .application
            .validate_in_identity_table(identities)
            .map_err(CodecError::MalformedStructuralFoundation)?;
    }
    Ok(())
}

fn require_identity(
    identities: &TerminalIdentityTable,
    identity: TerminalIdentityIndex,
) -> Result<(), CodecError> {
    if !identity.is_absent()
        && identities
            .text(identity)
            .is_some_and(|text| !text.is_empty())
    {
        Ok(())
    } else {
        Err(CodecError::MalformedStructuralFoundation(
            "boundary application declaration is not in its issuing table",
        ))
    }
}

#[cfg(test)]
mod tests;
