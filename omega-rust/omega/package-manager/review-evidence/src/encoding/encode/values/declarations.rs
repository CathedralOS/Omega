use super::super::declarations::{
    encode_data_properties, encode_type_identity, encode_type_parameter,
};
use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::{
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
    encoder.field("identity", |encoder| {
        encode_nominal(encoder, &shape.identity)
    })?;
    encoder.field("binders", |encoder| {
        encoder.sequence(&shape.binders, encode_proposition_binder)
    })?;
    encoder.field("parameter_types", |encoder| {
        encoder.sequence(&shape.parameter_types, encode_type_identity)
    })?;
    encoder.field("body", |encoder| {
        match &shape.body {
            PackageReviewPublicPropositionBody::Primitive => encoder.tag("primitive", 0),
            PackageReviewPublicPropositionBody::Witness(interface) => {
                encoder.tag("witness", 1);
                encoder.field("interface", |encoder| {
                    encode_evidence_interface(encoder, interface)
                })?;
            }
            PackageReviewPublicPropositionBody::Transparent(expansion) => {
                encoder.tag("transparent", 2);
                encoder.field("expansion", |encoder| {
                    encode_contract_fact(encoder, expansion)
                })?;
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
    encoder.field("identity", |encoder| {
        encode_nominal(encoder, &shape.identity)
    })?;
    encoder.field("declared_type", |encoder| {
        encode_type_identity(encoder, &shape.declared_type)
    })?;
    encoder.field("canonical_value_encoding", |encoder| {
        encoder.string(&shape.canonical_value_encoding)
    })
}

pub(crate) fn encode_operator_coordinate(
    encoder: &mut Encoder,
    coordinate: &PackageReviewOperatorCoordinate,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("identity", |encoder| {
        encode_nominal(encoder, &coordinate.identity)
    })?;
    encoder.field("parameter_dispatch", |encoder| {
        encoder.string(&coordinate.parameter_dispatch)
    })?;
    encoder.field("result_dispatch", |encoder| {
        encoder.string(&coordinate.result_dispatch)
    })
}

pub(crate) fn encode_operator_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewOperatorShape,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("coordinate", |encoder| {
        encode_operator_coordinate(encoder, &shape.coordinate)
    })?;
    encoder.field("is_boundary", |encoder| {
        encoder.boolean(shape.is_boundary);
        Ok(())
    })?;
    encoder.field("spelling", |encoder| {
        encoder.option(shape.spelling.as_ref(), |encoder, spelling| {
            encoder.tag(
                operator_spelling_name(*spelling),
                operator_spelling_tag(*spelling),
            );
            Ok(())
        })
    })?;
    encoder.field("lifetime_parameter_count", |encoder| {
        encoder.usize(shape.lifetime_parameter_count)
    })?;
    encoder.field("type_parameters", |encoder| {
        encoder.sequence(&shape.type_parameters, encode_type_parameter)
    })?;
    encoder.field("parameters", |encoder| {
        encoder.sequence(&shape.parameters, |encoder, parameter| {
            encoder.field("name", |encoder| encoder.string(&parameter.name))?;
            encoder.field("type_identity", |encoder| {
                encode_type_identity(encoder, &parameter.type_identity)
            })?;
            encoder.field("is_const", |encoder| {
                encoder.boolean(parameter.is_const);
                Ok(())
            })?;
            encoder.field("is_mutable", |encoder| {
                encoder.boolean(parameter.is_mutable);
                Ok(())
            })?;
            encoder.field("is_self", |encoder| {
                encoder.boolean(parameter.is_self);
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("return_type", |encoder| {
        encode_type_identity(encoder, &shape.return_type)
    })?;
    encoder.field("contracts", |encoder| {
        encoder.sequence(&shape.contracts, encode_callable_contract)
    })?;
    encoder.field("published_crash", |encoder| {
        encoder.sequence(&shape.published_crash, encode_crash_route)
    })
}

pub(crate) const fn operator_spelling_tag(spelling: language_core::OperatorSpelling) -> u8 {
    match spelling {
        language_core::OperatorSpelling::Add => 0,
        language_core::OperatorSpelling::Subtract => 1,
        language_core::OperatorSpelling::Multiply => 2,
        language_core::OperatorSpelling::Divide => 3,
        language_core::OperatorSpelling::Modulo => 4,
        language_core::OperatorSpelling::Equal => 5,
        language_core::OperatorSpelling::NotEqual => 6,
        language_core::OperatorSpelling::Less => 7,
        language_core::OperatorSpelling::LessEqual => 8,
        language_core::OperatorSpelling::Greater => 9,
        language_core::OperatorSpelling::GreaterEqual => 10,
        language_core::OperatorSpelling::Index => 11,
        language_core::OperatorSpelling::Range => 12,
    }
}

pub(crate) const fn operator_spelling_name(
    spelling: language_core::OperatorSpelling,
) -> &'static str {
    match spelling {
        language_core::OperatorSpelling::Add => "add",
        language_core::OperatorSpelling::Subtract => "subtract",
        language_core::OperatorSpelling::Multiply => "multiply",
        language_core::OperatorSpelling::Divide => "divide",
        language_core::OperatorSpelling::Modulo => "modulo",
        language_core::OperatorSpelling::Equal => "equal",
        language_core::OperatorSpelling::NotEqual => "not_equal",
        language_core::OperatorSpelling::Less => "less",
        language_core::OperatorSpelling::LessEqual => "less_equal",
        language_core::OperatorSpelling::Greater => "greater",
        language_core::OperatorSpelling::GreaterEqual => "greater_equal",
        language_core::OperatorSpelling::Index => "index",
        language_core::OperatorSpelling::Range => "range",
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
                encoder.field("type_identity", |encoder| {
                    encode_type_identity(encoder, type_identity)
                })?;
            }
            PackageReviewPropositionBinderKind::Machine => encoder.tag("machine", 2),
        };
        Ok(())
    })?;
    encoder.field("bounds", |encoder| {
        encode_data_properties(encoder, binder.bounds);
        Ok(())
    })?;
    Ok(())
}

pub(crate) fn encode_evidence_interface(
    encoder: &mut Encoder,
    interface: &PackageReviewEvidenceInterface,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("trait_identity", |encoder| {
        encode_nominal(encoder, &interface.trait_identity)
    })?;
    encoder.field("lifetime_arguments", |encoder| {
        encoder.sequence(&interface.lifetime_arguments, |encoder, argument| {
            encoder.field("argument", |encoder| {
                encoder.u32(*argument);
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("arguments", |encoder| {
        encoder.sequence(&interface.arguments, encode_type_identity)
    })?;
    encoder.field("requirements", |encoder| {
        encoder.sequence(&interface.requirements, |encoder, requirement| {
            encoder.field("declaring_trait", |encoder| {
                encode_nominal(encoder, &requirement.declaring_trait)
            })?;
            encoder.field("declaring_trait_lifetime_arguments", |encoder| {
                encoder.sequence(
                    &requirement.declaring_trait_lifetime_arguments,
                    |encoder, argument| {
                        encoder.field("argument", |encoder| {
                            encoder.u32(*argument);
                            Ok(())
                        })?;
                        Ok(())
                    },
                )
            })?;
            encoder.field("declaring_trait_arguments", |encoder| {
                encoder.sequence(&requirement.declaring_trait_arguments, encode_type_identity)
            })?;
            encoder.field("requirement", |encoder| {
                encode_nominal(encoder, &requirement.requirement)
            })
        })
    })
}
