use super::{Error, reader::Reader};
use crate::record::{
    PackageReviewNominalIdentity, PackageReviewNominalOwner, PackageReviewOperatorCoordinate,
    PackageReviewToolchainSourceIdentity, PackageReviewTypeIdentity,
};
use psi_core::PackageKeyIdentity;

pub(super) fn package(reader: &mut Reader<'_>) -> Result<PackageKeyIdentity, Error> {
    PackageKeyIdentity::from_digest(reader.digest()?).ok_or(Error::InvalidIdentity)
}

pub(super) fn nominal(reader: &mut Reader<'_>) -> Result<PackageReviewNominalIdentity, Error> {
    let owner = match reader.byte()? {
        0 => PackageReviewNominalOwner::Package(package(reader)?),
        1 => PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
            digest: reader.digest()?,
        }),
        _ => return Err(Error::InvalidIdentity),
    };
    Ok(PackageReviewNominalIdentity {
        owner,
        path: reader.string()?,
    })
}

pub(super) fn type_identity(reader: &mut Reader<'_>) -> Result<PackageReviewTypeIdentity, Error> {
    Ok(PackageReviewTypeIdentity {
        canonical: reader.string()?,
    })
}

pub(super) fn operator_coordinate(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewOperatorCoordinate, Error> {
    Ok(PackageReviewOperatorCoordinate {
        identity: nominal(reader)?,
        parameter_dispatch: reader.string()?,
        result_dispatch: reader.string()?,
    })
}
