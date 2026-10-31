use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::{
    PackageReviewCallableSupply, PackageReviewNominalIdentity, PackageReviewNominalOwner,
};

pub(crate) fn encode_nominal(
    encoder: &mut Encoder,
    identity: &PackageReviewNominalIdentity,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("owner", |encoder| {
        match identity.owner {
            PackageReviewNominalOwner::Package(package) => {
                encoder.tag("package", 0);
                encoder.package_identity_field("package", package)?;
            }
            PackageReviewNominalOwner::ToolchainSource(source) => {
                encoder.tag("toolchain_source", 1);
                encoder.fixed_bytes_field("source_digest", &source.digest())?;
            }
            PackageReviewNominalOwner::Unresolved => {
                return Err(PackageReviewEncodingError::new(
                    "package review cannot encode unresolved nominal ownership",
                ));
            }
        };
        Ok(())
    })?;
    encoder.observe_nominal_path(&identity.path)?;
    encoder.field("path", |encoder| encoder.string(&identity.path))
}

/// The `selection_owner` wire shape shared by calling-opaque and
/// representation policies: the same owner arms under identical tags; only
/// the caller's unresolved-ownership context label differs.
pub(in crate::encoding) fn encode_selection_owner(
    encoder: &mut Encoder,
    owner: &PackageReviewNominalOwner,
    unresolved_context: &'static str,
) -> Result<(), PackageReviewEncodingError> {
    match owner {
        PackageReviewNominalOwner::Package(package) => {
            encoder.tag("package", 0);
            encoder.package_identity_field("package", *package)?;
        }
        PackageReviewNominalOwner::ToolchainSource(source) => {
            encoder.tag("toolchain_source", 1);
            encoder.fixed_bytes_field("digest", &source.digest())?;
        }
        PackageReviewNominalOwner::Unresolved => {
            return Err(PackageReviewEncodingError::new(unresolved_context));
        }
    };
    Ok(())
}

pub(crate) fn encode_supply(
    encoder: &mut Encoder,
    supply: PackageReviewCallableSupply,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("supply", |encoder| {
        match supply {
            PackageReviewCallableSupply::CheckedBody => encoder.tag("checked_body", 0),
            PackageReviewCallableSupply::Requirement => encoder.tag("requirement", 1),
            PackageReviewCallableSupply::Boundary => encoder.tag("boundary", 2),
            PackageReviewCallableSupply::AdmissionClaim => encoder.tag("admission_claim", 3),
            PackageReviewCallableSupply::CompilerPrimitive => encoder.tag("compiler_primitive", 6),
            PackageReviewCallableSupply::ExternalRealization => {
                encoder.tag("external_realization", 4)
            }
            PackageReviewCallableSupply::TopLevelRequirement => {
                encoder.tag("top_level_requirement", 5)
            }
        };
        Ok(())
    })?;
    Ok(())
}
