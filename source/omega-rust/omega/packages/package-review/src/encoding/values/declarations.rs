use crate::encoding::PackageReviewEncodingError;
use crate::encoding::canonical::declarations::{
    encode_data_properties, encode_type_identity, encode_type_parameter,
};
use crate::encoding::canonical::encoder::Encoder;
use crate::evidence::{
    PackageReviewConstShape, PackageReviewEvidenceInterface, PackageReviewOperatorCoordinate,
    PackageReviewOperatorShape, PackageReviewPropositionBinder, PackageReviewPropositionBinderKind,
    PackageReviewPropositionShape, PackageReviewPublicPropositionBody,
};

use super::contracts::{encode_callable_contract, encode_contract_fact};
use super::crashes::encode_crash_route;
use super::identity::encode_nominal;

pub(crate) fn encode_proposition_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewPropositionShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encoder.sequence(&shape.binders, encode_proposition_binder)?;
    encoder.sequence(&shape.parameter_types, encode_type_identity)?;
    match &shape.body {
        PackageReviewPublicPropositionBody::Primitive => encoder.byte(0),
        PackageReviewPublicPropositionBody::Witness(interface) => {
            encoder.byte(1);
            encode_evidence_interface(encoder, interface)?;
        }
        PackageReviewPublicPropositionBody::Transparent(expansion) => {
            encoder.byte(2);
            encode_contract_fact(encoder, expansion)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_const_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewConstShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encode_type_identity(encoder, &shape.declared_type)?;
    encoder.string(&shape.canonical_value_encoding)
}

pub(crate) fn encode_operator_coordinate(
    encoder: &mut Encoder,
    coordinate: &PackageReviewOperatorCoordinate,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &coordinate.identity)?;
    encoder.string(&coordinate.parameter_dispatch)?;
    encoder.string(&coordinate.result_dispatch)
}

pub(crate) fn encode_operator_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewOperatorShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_operator_coordinate(encoder, &shape.coordinate)?;
    encoder.boolean(shape.is_boundary);
    encoder.option(shape.spelling.as_ref(), |encoder, spelling| {
        encoder.byte(operator_spelling_tag(*spelling));
        Ok(())
    })?;
    encoder.usize(shape.lifetime_parameter_count)?;
    encoder.sequence(&shape.type_parameters, encode_type_parameter)?;
    encoder.sequence(&shape.parameters, |encoder, parameter| {
        encoder.string(&parameter.name)?;
        encode_type_identity(encoder, &parameter.type_identity)?;
        encoder.boolean(parameter.is_const);
        encoder.boolean(parameter.is_mutable);
        encoder.boolean(parameter.is_self);
        Ok(())
    })?;
    encode_type_identity(encoder, &shape.return_type)?;
    encoder.sequence(&shape.contracts, encode_callable_contract)?;
    encoder.sequence(&shape.published_crash, encode_crash_route)
}

pub(crate) const fn operator_spelling_tag(spelling: psi_language_core::OperatorSpelling) -> u8 {
    match spelling {
        psi_language_core::OperatorSpelling::Add => 0,
        psi_language_core::OperatorSpelling::Subtract => 1,
        psi_language_core::OperatorSpelling::Multiply => 2,
        psi_language_core::OperatorSpelling::Divide => 3,
        psi_language_core::OperatorSpelling::Modulo => 4,
        psi_language_core::OperatorSpelling::Equal => 5,
        psi_language_core::OperatorSpelling::NotEqual => 6,
        psi_language_core::OperatorSpelling::Less => 7,
        psi_language_core::OperatorSpelling::LessEqual => 8,
        psi_language_core::OperatorSpelling::Greater => 9,
        psi_language_core::OperatorSpelling::GreaterEqual => 10,
        psi_language_core::OperatorSpelling::Index => 11,
        psi_language_core::OperatorSpelling::Range => 12,
    }
}

pub(crate) fn encode_proposition_binder(
    encoder: &mut Encoder,
    binder: &PackageReviewPropositionBinder,
) -> Result<(), PackageReviewEncodingError> {
    match &binder.kind {
        PackageReviewPropositionBinderKind::Type => encoder.byte(0),
        PackageReviewPropositionBinderKind::Const(type_identity) => {
            encoder.byte(1);
            encode_type_identity(encoder, type_identity)?;
        }
        PackageReviewPropositionBinderKind::Machine => encoder.byte(2),
    }
    encode_data_properties(encoder, binder.bounds);
    Ok(())
}

pub(crate) fn encode_evidence_interface(
    encoder: &mut Encoder,
    interface: &PackageReviewEvidenceInterface,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &interface.trait_identity)?;
    encoder.sequence(&interface.arguments, encode_type_identity)?;
    encoder.sequence(&interface.requirements, |encoder, requirement| {
        encode_nominal(encoder, &requirement.declaring_trait)?;
        encoder.sequence(&requirement.declaring_trait_arguments, encode_type_identity)?;
        encode_nominal(encoder, &requirement.requirement)
    })
}
