use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::evidence::{
    PackageReviewCallableSupply, PackageReviewNominalIdentity, PackageReviewNominalOwner,
};

pub(crate) fn encode_nominal(
    encoder: &mut Encoder,
    identity: &PackageReviewNominalIdentity,
) -> Result<(), PackageReviewEncodingError> {
    match identity.owner {
        PackageReviewNominalOwner::Package(package) => {
            encoder.byte(0);
            encoder.package_identity(package);
        }
        PackageReviewNominalOwner::ToolchainSource(source) => {
            encoder.byte(1);
            encoder.fixed_bytes(&source.digest());
        }
        PackageReviewNominalOwner::Unresolved => {
            return Err(PackageReviewEncodingError::new(
                "package review cannot encode unresolved nominal ownership",
            ));
        }
    }
    encoder.string(&identity.path)
}

pub(crate) fn encode_supply(
    encoder: &mut Encoder,
    supply: PackageReviewCallableSupply,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match supply {
        PackageReviewCallableSupply::CheckedBody => 0,
        PackageReviewCallableSupply::Requirement => 1,
        PackageReviewCallableSupply::Boundary => 2,
        PackageReviewCallableSupply::Accepted => 3,
        PackageReviewCallableSupply::ExternalRealization => 4,
    });
    Ok(())
}
