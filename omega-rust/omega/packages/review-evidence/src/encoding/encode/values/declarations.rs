use super::super::declarations::{encode_data_properties, encode_type_identity};
use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::{
    PackageReviewConstShape, PackageReviewEvidenceInterface, PackageReviewOperatorCoordinate,
    PackageReviewPropositionBinder, PackageReviewPropositionBinderKind,
    PackageReviewPropositionShape, PackageReviewPublicPropositionBody,
};

use super::contracts::encode_contract_fact;
use super::identity::encode_nominal;
use language_core::OperatorSpelling;

pub(crate) fn encode_proposition_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewPropositionShape,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("identity", &shape.identity, encode_nominal)?;
    encoder.sequence_field("binders", &shape.binders, encode_proposition_binder)?;
    encoder.field("parameter_types", |encoder| {
        encoder.sequence(&shape.parameter_types, encode_type_identity)
    })?;
    encoder.field("body", |encoder| {
        match &shape.body {
            PackageReviewPublicPropositionBody::Primitive => encoder.tag("primitive", 0),
            PackageReviewPublicPropositionBody::Witness(interface) => {
                encoder.tag("witness", 1);
                encoder.field_by("interface", interface, encode_evidence_interface)?;
            }
            PackageReviewPublicPropositionBody::Transparent(expansion) => {
                encoder.tag("transparent", 2);
                encoder.field_by("expansion", expansion, encode_contract_fact)?;
            }
        };
        Ok(())
    })?;
    Ok(())
}

pub(crate) fn encode_const_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewConstShape,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("identity", &shape.identity, encode_nominal)?;
    encoder.field_by("declared_type", &shape.declared_type, encode_type_identity)?;
    encoder.string_field("canonical_value_encoding", &shape.canonical_value_encoding)
}

pub(crate) fn encode_operator_coordinate(
    encoder: &mut Encoder,
    coordinate: &PackageReviewOperatorCoordinate,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("identity", &coordinate.identity, encode_nominal)?;
    encoder.string_field("parameter_dispatch", &coordinate.parameter_dispatch)?;
    encoder.string_field("result_dispatch", &coordinate.result_dispatch)
}

pub(crate) const fn operator_spelling_tag(spelling: OperatorSpelling) -> u8 {
    match spelling {
        OperatorSpelling::Add => 0,
        OperatorSpelling::Subtract => 1,
        OperatorSpelling::Multiply => 2,
        OperatorSpelling::Divide => 3,
        OperatorSpelling::Modulo => 4,
        OperatorSpelling::Equal => 5,
        OperatorSpelling::NotEqual => 6,
        OperatorSpelling::Less => 7,
        OperatorSpelling::LessEqual => 8,
        OperatorSpelling::Greater => 9,
        OperatorSpelling::GreaterEqual => 10,
        OperatorSpelling::Index => 11,
        OperatorSpelling::Range => 12,
    }
}

pub(crate) const fn operator_spelling_name(spelling: OperatorSpelling) -> &'static str {
    match spelling {
        OperatorSpelling::Add => "add",
        OperatorSpelling::Subtract => "subtract",
        OperatorSpelling::Multiply => "multiply",
        OperatorSpelling::Divide => "divide",
        OperatorSpelling::Modulo => "modulo",
        OperatorSpelling::Equal => "equal",
        OperatorSpelling::NotEqual => "not_equal",
        OperatorSpelling::Less => "less",
        OperatorSpelling::LessEqual => "less_equal",
        OperatorSpelling::Greater => "greater",
        OperatorSpelling::GreaterEqual => "greater_equal",
        OperatorSpelling::Index => "index",
        OperatorSpelling::Range => "range",
    }
}

pub(crate) fn encode_proposition_binder(
    encoder: &mut Encoder,
    binder: &PackageReviewPropositionBinder,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("kind", |encoder| {
        match &binder.kind {
            PackageReviewPropositionBinderKind::Type => encoder.tag("type", 0),
            PackageReviewPropositionBinderKind::Const(type_identity) => {
                encoder.tag("const", 1);
                encoder.field_by("type_identity", type_identity, encode_type_identity)?;
            }
            PackageReviewPropositionBinderKind::Machine => encoder.tag("machine", 2),
        };
        Ok(())
    })?;
    encoder.field_by("bounds", binder.bounds, encode_data_properties)?;
    Ok(())
}

pub(crate) fn encode_evidence_interface(
    encoder: &mut Encoder,
    interface: &PackageReviewEvidenceInterface,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("trait_identity", &interface.trait_identity, encode_nominal)?;
    encoder.field("lifetime_arguments", |encoder| {
        encoder.sequence(&interface.lifetime_arguments, |encoder, argument| {
            encoder.u32_field("argument", *argument)?;
            Ok(())
        })
    })?;
    encoder.sequence_field("arguments", &interface.arguments, encode_type_identity)?;
    encoder.field("requirements", |encoder| {
        encoder.sequence(&interface.requirements, |encoder, requirement| {
            encoder.field_by(
                "declaring_trait",
                &requirement.declaring_trait,
                encode_nominal,
            )?;
            encoder.field("declaring_trait_lifetime_arguments", |encoder| {
                encoder.sequence(
                    &requirement.declaring_trait_lifetime_arguments,
                    |encoder, argument| {
                        encoder.u32_field("argument", *argument)?;
                        Ok(())
                    },
                )
            })?;
            encoder.field("declaring_trait_arguments", |encoder| {
                encoder.sequence(&requirement.declaring_trait_arguments, encode_type_identity)
            })?;
            encoder.field_by("requirement", &requirement.requirement, encode_nominal)
        })
    })
}
