//! Typed recovery of normalized policy, not recovery of compiler authority.
//!
//! `external` owns the component envelope; `reader` accounts hostile framing
//! and allocations. The other leaves mirror stable signature, contract,
//! expression, identity, and behavior vocabulary without inspecting compiler
//! representations or reconstructing proofs.

pub(in crate::encoding::recovery) mod baseline;
pub(in crate::encoding::recovery) mod behavior;
pub(in crate::encoding::recovery) mod callable_policy;
pub(in crate::encoding::recovery) mod calling_application;
pub(in crate::encoding::recovery) mod conformance;
pub(in crate::encoding::recovery) mod contracts;
pub(in crate::encoding::recovery) mod declarations;
pub(in crate::encoding::recovery) mod expressions;
pub(in crate::encoding::recovery) mod external;
pub(in crate::encoding::recovery) mod identity;
pub(in crate::encoding::recovery) mod intrinsic;
pub(in crate::encoding::recovery) mod physical_calling_policy;
pub(in crate::encoding::recovery) mod policy_recovery_accounting;
pub(in crate::encoding::recovery) mod public_api;
pub(in crate::encoding::recovery) mod reader;
pub(in crate::encoding::recovery) mod representation;
pub(in crate::encoding::recovery) mod selected_providers;
pub(in crate::encoding::recovery) mod signatures;
pub(in crate::encoding::recovery) mod structural_expressions;
pub(in crate::encoding::recovery) mod terminal_permissions;
pub(in crate::encoding::recovery) mod text;

use PackagePolicyRecoveryError as Error;
pub use policy_recovery_accounting::{
    PackagePolicyRecoveryError, PackagePolicyRecoveryLimits, PackagePolicyRecoveryUsage,
};

/// Keep component framing and scratch charges on the decoder's shared budget.
/// Validation stays component-specific; canonical encoding is an independent
/// format check after scratch accounting, never recovered compiler authority.
fn recover_component<Policy>(
    bytes: &[u8],
    limits: PackagePolicyRecoveryLimits,
    magic: &[u8],
    version: u16,
    decode: impl FnOnce(&mut reader::Reader<'_>) -> Result<Policy, Error>,
    validate: impl FnOnce(&Policy) -> Result<(), Error>,
    encode: impl FnOnce(&Policy) -> Result<Vec<u8>, crate::encoding::PackageReviewEncodingError>,
) -> Result<Policy, Error> {
    let mut reader = reader::Reader::new(bytes, limits)?;
    reader.literal(magic)?;
    if reader.u16()? != version {
        return Err(Error::UnsupportedVersion);
    }
    let policy = decode(&mut reader)?;
    reader.finish()?;
    validate(&policy)?;
    reader.canonical_scratch(bytes.len())?;
    if encode(&policy).map_err(|_| Error::NonCanonicalEncoding)? != bytes {
        return Err(Error::NonCanonicalEncoding);
    }
    Ok(policy)
}
