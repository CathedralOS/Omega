// Inverse of the complete structural signature vocabulary without receipts.

use super::Error;
use super::identity::{nominal, type_identity};
use super::reader::Reader;
use crate::record::{PackageReviewConformanceBound, PackageReviewDataProperties};

pub(in crate::encoding::recovery) fn conformance_bound(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewConformanceBound, Error> {
    Ok(PackageReviewConformanceBound {
        binder_ordinal: reader.u32()?,
        trait_identity: nominal(reader)?,
        trait_lifetime_arguments: reader.sequence(4, |reader| reader.u32())?,
        arguments: reader.sequence(8, type_identity)?,
    })
}

pub(in crate::encoding::recovery) fn data_properties(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDataProperties, Error> {
    use language_core::Multiplicity;
    let multiplicity = super::tag!(reader => {
        0 => Multiplicity::Unrestricted,
        1 => Multiplicity::Affine,
        2 => Multiplicity::Linear,
    });
    super::tag!(reader => { 0 => () });
    Ok(PackageReviewDataProperties { multiplicity })
}

#[test]
fn package_data_properties_keep_the_multiplicity_and_require_the_absent_policy_tag() {
    use crate::encoding::encode::{declarations::encode_data_properties, encoder::Encoder};
    use language_core::Multiplicity;

    for (tag, multiplicity) in [
        (0, Multiplicity::Unrestricted),
        (1, Multiplicity::Affine),
        (2, Multiplicity::Linear),
    ] {
        let properties = PackageReviewDataProperties { multiplicity };
        let mut encoder = Encoder::bounded(2);
        encode_data_properties(&mut encoder, properties);
        assert_eq!(encoder.finish().unwrap(), [tag, 0]);

        let bytes = [tag, 0];
        let mut reader = Reader::new(&bytes, Default::default()).unwrap();
        assert_eq!(data_properties(&mut reader).unwrap(), properties);
        reader.finish().unwrap();
    }

    let mut reader = Reader::new(&[0, 1, 0, 0, 0, 0], Default::default()).unwrap();
    assert_eq!(data_properties(&mut reader), Err(Error::InvalidTag));
}
