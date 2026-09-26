//! Complete calling-policy framing and receipt-free semantic children.

mod application;
mod callbacks;
mod opaque;

pub(crate) use application::encode_application;

use super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::PackageReviewTypeIdentity;

fn type_identity(
    encoder: &mut Encoder,
    identity: &PackageReviewTypeIdentity,
) -> Result<(), PackageReviewEncodingError> {
    encoder.observe_type_identity(identity.canonical())?;
    encoder.field("canonical", |encoder| encoder.string(identity.canonical()))
}

fn ordinal(encoder: &mut Encoder, ordinal: &u32) -> Result<(), PackageReviewEncodingError> {
    encoder.field("ordinal", |encoder| {
        encoder.u32(*ordinal);
        Ok(())
    })?;
    Ok(())
}
