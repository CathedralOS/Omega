use super::super::PackageReviewEncodingError;
use super::super::encoder::Encoder;
use super::super::values::declarations::encode_evidence_interface;
use super::super::values::expressions::encode_contract_static_argument;
use super::super::values::identity::encode_nominal;
use super::data::{encode_type_identity, encode_type_parameter};
use crate::record::{
    PackageReviewConformanceBound, PackageReviewConformanceShape, PackageReviewConformanceSubject,
};

pub(crate) fn encode_conformance_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewConformanceShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encoder.usize(shape.lifetime_parameter_count)?;
    encoder.sequence(&shape.type_parameters, encode_type_parameter)?;
    match &shape.subject {
        PackageReviewConformanceSubject::Subjectless => encoder.byte(0),
        PackageReviewConformanceSubject::TypeParameter(ordinal) => {
            encoder.byte(1);
            encoder.u32(*ordinal);
        }
        PackageReviewConformanceSubject::Nominal(identity) => {
            encoder.byte(2);
            encode_nominal(encoder, identity)?;
        }
    }
    encode_evidence_interface(encoder, &shape.interface)
}

pub(crate) fn encode_conformance_bound(
    encoder: &mut Encoder,
    bound: &PackageReviewConformanceBound,
) -> Result<(), PackageReviewEncodingError> {
    match bound.binder_ordinal {
        None => encoder.byte(0),
        Some(ordinal) => {
            encoder.byte(1);
            encoder.u32(ordinal);
        }
    }
    encoder.u32(bound.subject_parameter);
    match (&bound.selected_conformance, &bound.selected_subject) {
        (None, None)
            if bound.selected_lifetime_arguments.is_empty()
                && bound.selected_arguments.is_empty() =>
        {
            encoder.byte(0)
        }
        (Some(conformance), Some(subject)) => {
            encoder.byte(1);
            encode_nominal(encoder, conformance)?;
            encoder.sequence(&bound.selected_lifetime_arguments, |encoder, argument| {
                encoder.u32(*argument);
                Ok(())
            })?;
            encoder.sequence(&bound.selected_arguments, encode_contract_static_argument)?;
            encode_contract_static_argument(encoder, subject)?;
        }
        _ => {
            return Err(PackageReviewEncodingError::new(
                "selected conformance review row has an incomplete application identity",
            ));
        }
    }
    encode_nominal(encoder, &bound.trait_identity)?;
    encoder.sequence(&bound.trait_lifetime_arguments, |encoder, argument| {
        encoder.u32(*argument);
        Ok(())
    })?;
    encoder.sequence(&bound.arguments, encode_type_identity)
}
