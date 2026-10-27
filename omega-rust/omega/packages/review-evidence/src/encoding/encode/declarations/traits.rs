use super::super::PackageReviewEncodingError;
use super::super::encoder::Encoder;
use super::super::values::identity::encode_nominal;
use super::data::encode_type_identity;
use crate::record::{PackageReviewTraitCompositionKind, PackageReviewTraitParent};

pub(crate) fn encode_trait_parent(
    encoder: &mut Encoder,
    parent: &PackageReviewTraitParent,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("kind", |encoder| {
        match parent.kind {
            PackageReviewTraitCompositionKind::Policy => encoder.tag("policy", 0),
            PackageReviewTraitCompositionKind::ServiceReach => encoder.tag("service_reach", 1),
        };
        Ok(())
    })?;
    encoder.field_by("identity", &parent.identity, encode_nominal)?;
    encoder.field("lifetime_arguments", |encoder| {
        encoder.sequence(&parent.lifetime_arguments, |encoder, argument| {
            encoder.field("argument", |encoder| {
                encoder.u32(*argument);
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("arguments", |encoder| {
        encoder.sequence(&parent.arguments, encode_type_identity)
    })
}
