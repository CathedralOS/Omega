use super::super::{
    Error,
    identity::{nominal, type_identity},
    reader::Reader,
};
use crate::record::{PackageReviewTraitCompositionKind, PackageReviewTraitParent};

pub(in crate::encoding::recovery) fn trait_parent(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewTraitParent, Error> {
    Ok(PackageReviewTraitParent {
        kind: match reader.byte()? {
            0 => PackageReviewTraitCompositionKind::Policy,
            1 => PackageReviewTraitCompositionKind::ServiceReach,
            _ => return Err(Error::InvalidTag),
        },
        identity: nominal(reader)?,
        lifetime_arguments: reader.sequence(4, |reader| reader.u32())?,
        arguments: reader.sequence(8, type_identity)?,
    })
}
