// Canonical encoding for data properties, members, fields, type identities, and binding relevance.

use super::super::PackageReviewEncodingError;
use super::super::encoder::Encoder;
use super::super::type_identity_in_scope;
use crate::record::{PackageReviewDataField, PackageReviewDataMember, PackageReviewDataProperties};

pub(crate) fn encode_data_properties(
    encoder: &mut Encoder,
    properties: PackageReviewDataProperties,
) {
    let _ = encoder.field("multiplicity", |encoder| {
        match properties.multiplicity {
            language_core::Multiplicity::Unrestricted => encoder.tag("unrestricted", 0),
            language_core::Multiplicity::Affine => encoder.tag("affine", 1),
            language_core::Multiplicity::Linear => encoder.tag("linear", 2),
        };
        Ok(())
    });
    // Existing package evidence encodes an absent policy after the multiplicity.
    let _ = encoder.field("carry", |encoder| {
        encoder.tag("none", 0);
        Ok(())
    });
}

pub(crate) fn encode_data_member(
    encoder: &mut Encoder,
    member: &PackageReviewDataMember,
    lifetimes: usize,
) -> Result<(), PackageReviewEncodingError> {
    match member {
        PackageReviewDataMember::Field(field) => {
            encoder.tag("field", 0);
            encoder.field("field", |encoder| {
                encode_data_field(encoder, field, lifetimes)
            })?;
        }
        PackageReviewDataMember::Variant {
            identity,
            name,
            payload,
            retired_payload_identities,
        } => {
            encoder.tag("variant", 1);
            encoder.field_by("identity", *identity, encode_optional_u64)?;
            encoder.field("name", |encoder| encoder.string(name))?;
            encoder.sequence_field("payload", payload, |encoder, field| {
                encode_data_field(encoder, field, lifetimes)
            })?;
            encoder.field("retired_payload_identities", |encoder| {
                encoder.sequence(retired_payload_identities, |encoder, identity| {
                    encoder.u64_field("identity", *identity)?;
                    Ok(())
                })
            })?;
        }
    }
    Ok(())
}

pub(crate) fn encode_data_field(
    encoder: &mut Encoder,
    field: &PackageReviewDataField,
    lifetimes: usize,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("identity", field.identity, encode_optional_u64)?;
    encoder.field("name", |encoder| encoder.string(&field.name))?;
    encoder.field_by("relevance", field.relevance, encode_relevance)?;
    encoder.field("type_identity", |encoder| {
        type_identity_in_scope(encoder, &field.type_identity, Some(lifetimes))
    })
}

pub(crate) fn encode_relevance(encoder: &mut Encoder, relevance: language_core::BindingRelevance) {
    let _ = encoder.field("relevance", |encoder| {
        match relevance {
            language_core::BindingRelevance::Relevant => encoder.tag("relevant", 0),
            language_core::BindingRelevance::Erased => encoder.tag("erased", 1),
        };
        Ok(())
    });
}

pub(crate) fn encode_optional_u64(encoder: &mut Encoder, value: Option<u64>) {
    match value {
        None => encoder.tag("none", 0),
        Some(value) => {
            encoder.tag("some", 1);
            let _ = encoder.u64_field("value", value);
        }
    }
}
