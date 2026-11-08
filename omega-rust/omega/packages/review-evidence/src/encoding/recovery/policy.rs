// Typed recovery of normalized policy without reconstructing compiler authority.
// `external` owns the component envelope while `reader` accounts for hostile framing and allocations; leaf decoders mirror stable signature, contract, expression, identity, and behavior vocabulary without inspecting compiler representations or reconstructing proofs.
// `recover_component` and `recover_canonical_component!` keep component framing, structural validation, and canonical round-trip scratch charges on one shared decoder budget, and `tag!` dispatches byte tags with a trailing invalid-tag rejection.

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
pub use policy_recovery_accounting::PackagePolicyRecoveryLimits;
pub(crate) use policy_recovery_accounting::{
    PackagePolicyRecoveryError, PackagePolicyRecoveryUsage,
};

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

macro_rules! recover_canonical_component {
    ($magic:expr, $version:expr, $decode:expr, validate $(,)?) => {
        pub fn recover_canonical(
            bytes: &[u8],
            limits: PackagePolicyRecoveryLimits,
        ) -> Result<Self, Error> {
            super::recover_component(
                bytes,
                limits,
                $magic,
                $version,
                $decode,
                |policy| {
                    policy
                        .validate_canonical_structure()
                        .map_err(|_| Error::InvalidValue)
                },
                Self::canonical_bytes,
            )
        }
    };
    ($magic:expr, $version:expr, $decode:expr $(,)?) => {
        pub fn recover_canonical(
            bytes: &[u8],
            limits: PackagePolicyRecoveryLimits,
        ) -> Result<Self, Error> {
            super::recover_component(
                bytes,
                limits,
                $magic,
                $version,
                $decode,
                |_| Ok(()),
                Self::canonical_bytes,
            )
        }
    };
}
pub(crate) use recover_canonical_component;

macro_rules! tag {
    ($reader:expr => { $($tag:literal => $variant:expr),+ $(,)? }) => {
        match $reader.byte()? {
            $($tag => $variant,)+
            _ => return Err(Error::InvalidTag),
        }
    };
}

pub(crate) use tag;
