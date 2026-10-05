use super::super::PackageReviewEncodingError;
use super::super::encoder::Encoder;
use super::super::values::expressions::encode_contract_static_argument;
use super::super::values::identity::encode_nominal;
use super::data::encode_type_identity;
use crate::record::PackageReviewConformanceBound;

pub(crate) fn encode_conformance_bound(
    encoder: &mut Encoder,
    bound: &PackageReviewConformanceBound,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("binder_ordinal", |encoder| {
        match bound.binder_ordinal {
            None => encoder.tag("none", 0),
            Some(ordinal) => {
                encoder.tag("some", 1);
                encoder.field("value", |encoder| {
                    encoder.u32(ordinal);
                    Ok(())
                })?;
            }
        };
        Ok(())
    })?;
    encoder.field("subject_parameter", |encoder| {
        encoder.u32(bound.subject_parameter);
        Ok(())
    })?;
    encoder.field("selection", |encoder| {
        match (&bound.selected_conformance, &bound.selected_subject) {
            (None, None)
                if bound.selected_lifetime_arguments.is_empty()
                    && bound.selected_arguments.is_empty() =>
            {
                encoder.tag("none", 0)
            }
            (Some(conformance), Some(subject)) => {
                encoder.tag("some", 1);
                encoder.field("conformance", |encoder| {
                    encode_nominal(encoder, conformance)
                })?;
                encoder.field("selected_lifetime_arguments", |encoder| {
                    encoder.sequence(&bound.selected_lifetime_arguments, |encoder, argument| {
                        encoder.field("argument", |encoder| {
                            encoder.u32(*argument);
                            Ok(())
                        })?;
                        Ok(())
                    })
                })?;
                encoder.field("selected_arguments", |encoder| {
                    encoder.sequence(&bound.selected_arguments, encode_contract_static_argument)
                })?;
                encoder.field("subject", |encoder| {
                    encode_contract_static_argument(encoder, subject)
                })?;
            }
            _ => {
                return Err(PackageReviewEncodingError::new(
                    "selected conformance review row has an incomplete application identity",
                ));
            }
        };
        Ok(())
    })?;
    encoder.field("trait_identity", |encoder| {
        encode_nominal(encoder, &bound.trait_identity)
    })?;
    encoder.field("trait_lifetime_arguments", |encoder| {
        encoder.sequence(&bound.trait_lifetime_arguments, |encoder, argument| {
            encoder.field("argument", |encoder| {
                encoder.u32(*argument);
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("arguments", |encoder| {
        encoder.sequence(&bound.arguments, encode_type_identity)
    })
}
