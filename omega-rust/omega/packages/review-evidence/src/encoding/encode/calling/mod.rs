//! Complete calling-policy framing and receipt-free semantic children.

mod application;
mod callbacks;
mod opaque;

pub(crate) use application::encode_application;

use super::encoder::Encoder;
pub(super) use super::type_identity;
use crate::encoding::PackageReviewEncodingError;

fn ordinal(encoder: &mut Encoder, ordinal: &u32) -> Result<(), PackageReviewEncodingError> {
    encoder.u32_field("ordinal", *ordinal)?;
    Ok(())
}
