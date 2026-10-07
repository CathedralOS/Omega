use super::{Error, reader::Reader};
use crate::record::{
    PackageReviewNominalIdentity, PackageReviewNominalOwner, PackageReviewOperatorCoordinate,
    PackageReviewToolchainSourceIdentity, PackageReviewTypeIdentity,
};
use semantic_vocabulary::PackageKeyIdentity;

pub(in crate::encoding::recovery) fn package(
    reader: &mut Reader<'_>,
) -> Result<PackageKeyIdentity, Error> {
    PackageKeyIdentity::from_digest(reader.digest()?).ok_or(Error::InvalidIdentity)
}

pub(in crate::encoding::recovery) fn nominal(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewNominalIdentity, Error> {
    let owner = owner(reader)?;
    Ok(PackageReviewNominalIdentity {
        owner,
        path: reader.string()?,
    })
}

pub(in crate::encoding::recovery) fn owner(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewNominalOwner, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewNominalOwner::Package(package(reader)?),
        1 => PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
            digest: reader.digest()?,
        }),
        _ => return Err(Error::InvalidIdentity),
    })
}

pub(in crate::encoding::recovery) fn type_identity(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewTypeIdentity, Error> {
    Ok(PackageReviewTypeIdentity {
        canonical: reader.string()?,
    })
}

pub(in crate::encoding::recovery) fn operator_coordinate(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewOperatorCoordinate, Error> {
    Ok(PackageReviewOperatorCoordinate {
        identity: nominal(reader)?,
        parameter_dispatch: reader.string()?,
        result_dispatch: reader.string()?,
    })
}

pub(in crate::encoding::recovery) fn target_context(
    reader: &mut Reader<'_>,
) -> Result<crate::record::PackagePolicyTargetContext, Error> {
    match reader.byte()? {
        0 => Ok(crate::record::PackagePolicyTargetContext::TargetIndependent),
        1 => {
            let target = reader.string()?;
            if target.is_empty() {
                return Err(Error::InvalidIdentity);
            }
            Ok(crate::record::PackagePolicyTargetContext::TargetMember { target })
        }
        _ => Err(Error::InvalidTag),
    }
}
