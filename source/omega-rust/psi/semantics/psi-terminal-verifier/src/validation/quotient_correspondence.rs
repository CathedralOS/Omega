//! Representation-only validation for module-owned quotient correspondence.

use std::collections::BTreeSet;

use psi_terminal::TerminalModule;

use super::{ModuleError, ValidationPolicy};
use crate::replay_non_executable_quotient_correspondence;

pub(super) fn validate_quotient_correspondences(
    module: &TerminalModule,
    policy: ValidationPolicy,
) -> Result<(), ModuleError> {
    let mut previous_identity: Option<&[u8]> = None;
    let mut identities = BTreeSet::new();
    let mut owners = BTreeSet::new();

    for (index, retained) in module.quotient_correspondences.iter().enumerate() {
        let identity = retained.identity.0.as_slice();
        if previous_identity.is_some_and(|previous| previous > identity) {
            return Err(ModuleError::NonCanonicalQuotientCorrespondenceOrder);
        }
        previous_identity = Some(identity);
        if !identities.insert(identity) {
            return Err(ModuleError::DuplicateQuotientCorrespondenceIdentity);
        }
        if !owners.insert(&retained.certificate.public_operation) {
            return Err(ModuleError::DuplicateQuotientCorrespondenceOwner);
        }
        replay_non_executable_quotient_correspondence(retained)
            .map_err(|error| ModuleError::InvalidQuotientCorrespondence { index, error })?;
    }

    if policy != ValidationPolicy::Representation && !module.quotient_correspondences.is_empty() {
        return Err(ModuleError::NonExecutableQuotientCorrespondence);
    }
    Ok(())
}
