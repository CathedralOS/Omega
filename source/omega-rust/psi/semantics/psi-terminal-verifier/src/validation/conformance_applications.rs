use std::collections::{BTreeMap, BTreeSet};

use psi_terminal::{
    TerminalModule, closed_conformance_application_commitment,
    closed_conformance_application_report_fingerprint,
};

use super::ModuleError;

pub(super) fn validate_closed_conformance_applications(
    module: &TerminalModule,
) -> Result<(), ModuleError> {
    let mut seen = BTreeSet::new();
    let mut callable_machines = BTreeMap::new();
    let mut machine_callables = BTreeMap::new();
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
            || application
                .trait_lifetime_arguments
                .iter()
                .any(String::is_empty)
            || application.trait_arguments.iter().any(String::is_empty)
            || application
                .subject_identity
                .as_ref()
                .is_some_and(String::is_empty)
            || application.realization_callables.iter().any(|callable| {
                callable.source_callable_identity.is_empty()
                    || !module
                        .machines
                        .iter()
                        .any(|machine| machine.id == callable.machine)
            })
            || application
                .realization_callables
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || application.rows.iter().any(|row| {
                row.declaring_trait_identity.is_empty()
                    || row.public_requirement_identity.is_empty()
                    || row.requirement_identity.is_empty()
                    || row.realization_identity.is_empty()
                    || row
                        .realization_callable_identity
                        .as_ref()
                        .is_some_and(String::is_empty)
            })
        {
            return Err(ModuleError::InvalidClosedConformanceApplication {
                owner: application.owner,
                declaration: application.declaration_identity.clone(),
            });
        }
        for callable in &application.realization_callables {
            if callable_machines
                .insert(callable.source_callable_identity.as_str(), callable.machine)
                .is_some_and(|existing| existing != callable.machine)
                || machine_callables
                    .insert(callable.machine, callable.source_callable_identity.as_str())
                    .is_some_and(|existing| existing != callable.source_callable_identity)
            {
                return Err(ModuleError::InvalidClosedConformanceApplication {
                    owner: application.owner,
                    declaration: application.declaration_identity.clone(),
                });
            }
        }
        let mut used_callables = BTreeSet::new();
        for row in &application.rows {
            let Some(identity) = &row.realization_callable_identity else {
                continue;
            };
            let mut entries = application
                .realization_callables
                .iter()
                .filter(|callable| callable.source_callable_identity == *identity);
            if entries.next().is_none() || entries.next().is_some() {
                return Err(ModuleError::InvalidClosedConformanceApplication {
                    owner: application.owner,
                    declaration: application.declaration_identity.clone(),
                });
            }
            used_callables.insert(identity.as_str());
        }
        if application
            .realization_callables
            .iter()
            .any(|callable| !used_callables.contains(callable.source_callable_identity.as_str()))
        {
            return Err(ModuleError::InvalidClosedConformanceApplication {
                owner: application.owner,
                declaration: application.declaration_identity.clone(),
            });
        }
        let mapped_rows = application
            .rows
            .iter()
            .filter(|row| row.realization_callable_identity.is_some())
            .collect::<Vec<_>>();
        match (
            application.realization_callables.as_slice(),
            mapped_rows.as_slice(),
        ) {
            ([], []) => {}
            ([callable], [row]) => {
                let consumed = module.proof_output_calls.iter().any(|invocation| {
                    invocation.caller == application.owner
                        && invocation
                            .static_requirement_dispatch
                            .as_ref()
                            .is_some_and(|dispatch| {
                                dispatch.conformance_application_report_fingerprint
                                    == application.report_fingerprint
                                    && dispatch.conformance_application_commitment
                                        == application.commitment
                                    && dispatch.declaring_trait_identity
                                        == row.declaring_trait_identity
                                    && dispatch.public_requirement_identity
                                        == row.public_requirement_identity
                                    && dispatch.requirement_identity == row.requirement_identity
                                    && dispatch.realization_identity == row.realization_identity
                                    && dispatch.realization_callable_identity
                                        == callable.source_callable_identity
                                    && dispatch.realization == callable.machine
                            })
                });
                if !consumed {
                    return Err(ModuleError::InvalidClosedConformanceApplication {
                        owner: application.owner,
                        declaration: application.declaration_identity.clone(),
                    });
                }
            }
            _ => {
                return Err(ModuleError::InvalidClosedConformanceApplication {
                    owner: application.owner,
                    declaration: application.declaration_identity.clone(),
                });
            }
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
                report_fingerprint: application.report_fingerprint,
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
