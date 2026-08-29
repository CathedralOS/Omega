use std::collections::BTreeSet;

use psi_terminal::{
    TerminalModule, closed_conformance_application_commitment,
    closed_conformance_application_report_fingerprint,
};

use super::ModuleError;

pub(super) fn validate_closed_conformance_applications(
    module: &TerminalModule,
) -> Result<(), ModuleError> {
    let mut seen = BTreeSet::new();
    for application in &module.closed_conformance_applications {
        if !module
            .machines
            .iter()
            .any(|machine| machine.id == application.owner)
        {
            return Err(ModuleError::UnknownClosedConformanceOwner(
                application.owner,
            ));
        }
        if application.declaration_identity.is_empty()
            || application.trait_identity.is_empty()
            || application
                .telescope
                .iter()
                .any(|binding| binding.parameter.is_empty() || binding.argument.is_empty())
            || application.trait_arguments.iter().any(String::is_empty)
            || application
                .subject_identity
                .as_ref()
                .is_some_and(String::is_empty)
            || application.rows.iter().any(|row| {
                row.declaring_trait_identity.is_empty()
                    || row.public_requirement_identity.is_empty()
                    || row.requirement_identity.is_empty()
                    || row.realization_identity.is_empty()
            })
        {
            return Err(ModuleError::InvalidClosedConformanceApplication {
                owner: application.owner,
                declaration: application.declaration_identity.clone(),
            });
        }
        let mut parameters = BTreeSet::new();
        if application
            .telescope
            .iter()
            .any(|binding| !parameters.insert(binding.parameter.as_str()))
        {
            return Err(ModuleError::InvalidClosedConformanceApplication {
                owner: application.owner,
                declaration: application.declaration_identity.clone(),
            });
        }
        if !seen.insert((application.owner, application.commitment)) {
            return Err(ModuleError::DuplicateClosedConformanceApplication {
                owner: application.owner,
                fingerprint: application.report_fingerprint,
            });
        }
        let expected = closed_conformance_application_report_fingerprint(application);
        if application.report_fingerprint == 0 || application.report_fingerprint != expected {
            return Err(ModuleError::ClosedConformanceFingerprintMismatch {
                owner: application.owner,
                expected,
                actual: application.report_fingerprint,
            });
        }
        let expected_commitment = closed_conformance_application_commitment(application);
        if application.commitment.is_zero() || application.commitment != expected_commitment {
            return Err(ModuleError::ClosedConformanceCommitmentMismatch {
                owner: application.owner,
            });
        }
    }
    Ok(())
}
