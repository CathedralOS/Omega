use crate::encoding::encode::declarations::encode_type_identity;
use crate::encoding::{PackageReviewEncodingError, encode::encoder::Encoder};
use crate::record::PackageReviewBoundaryApplication;

pub(in crate::encoding::encode) fn encode_boundary_application(
    encoder: &mut Encoder,
    application: &PackageReviewBoundaryApplication,
) -> Result<(), PackageReviewEncodingError> {
    match application {
        PackageReviewBoundaryApplication::Empty => encoder.tag("empty", 0),
        PackageReviewBoundaryApplication::Exact(arguments) => {
            encoder.tag("exact", 1);
            encoder.field("arguments", |encoder| {
                encoder.sequence(arguments, |encoder, argument| {
                    encoder.field("argument", |encoder| {
                        match argument {
                            crate::record::PackageReviewBoundaryApplicationArgument::Type {
                                binder_ordinal,
                                type_identity,
                            } => {
                                encoder.tag("type", 0);
                                encoder.field("binder_ordinal", |encoder| {
                                    encoder.u32(*binder_ordinal);
                                    Ok(())
                                })?;
                                encoder.field("type_identity", |encoder| {
                                    encode_type_identity(encoder, type_identity)
                                })?;
                            }
                            crate::record::PackageReviewBoundaryApplicationArgument::Const {
                                binder_ordinal,
                                declared_carrier,
                                value_type,
                                value_encoding,
                            } => {
                                encoder.tag("const", 1);
                                encoder.field("binder_ordinal", |encoder| {
                                    encoder.u32(*binder_ordinal);
                                    Ok(())
                                })?;
                                encoder.field("declared_carrier", |encoder| {
                                    encode_type_identity(encoder, declared_carrier)
                                })?;
                                encoder
                                    .field("value_type", |encoder| encoder.string(value_type))?;
                                encoder.field("value_encoding", |encoder| {
                                    encoder.string(value_encoding)
                                })?;
                            }
                        };
                        Ok(())
                    })?;
                    Ok(())
                })
            })?;
        }
    }
    Ok(())
}
