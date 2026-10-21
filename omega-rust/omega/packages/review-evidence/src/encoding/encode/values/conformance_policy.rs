//! Canonical semantic conformance applications, without compiler receipts.

use super::super::type_identity;
use super::identity::encode_nominal;

use crate::encoding::PackageReviewEncodingError;
use crate::encoding::encode::encoder::Encoder;
use crate::record::{
    PackagePolicyClosedConformanceApplication, PackagePolicyConformanceConstArgument,
};

pub(crate) fn encode_application(
    encoder: &mut Encoder,
    application: &PackagePolicyClosedConformanceApplication,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("declaration", |encoder| {
        encode_nominal(encoder, &application.declaration)
    })?;
    encoder.field("lifetime_arguments", |encoder| {
        encoder.sequence(&application.lifetime_arguments, ordinal)
    })?;
    encoder.field("type_arguments", |encoder| {
        encoder.sequence(&application.type_arguments, type_identity)
    })?;
    encoder.field("const_arguments", |encoder| {
        encoder.sequence(&application.const_arguments, const_argument)
    })?;
    encoder.field("machine_arguments", |encoder| {
        encoder.sequence(&application.machine_arguments, encode_nominal)
    })?;
    encoder.field("trait_identity", |encoder| {
        encode_nominal(encoder, &application.trait_identity)
    })?;
    encoder.field("trait_lifetime_arguments", |encoder| {
        encoder.sequence(&application.trait_lifetime_arguments, ordinal)
    })?;
    encoder.field("trait_arguments", |encoder| {
        encoder.sequence(&application.trait_arguments, type_identity)
    })?;
    encoder.field("rows", |encoder| {
        encoder.sequence(&application.rows, |encoder, row| {
            encoder.field("declaring_trait", |encoder| {
                encode_nominal(encoder, &row.declaring_trait)
            })?;
            encoder.field("requirement", |encoder| {
                encode_nominal(encoder, &row.requirement)
            })?;
            encoder.field("realization_machine", |encoder| {
                encode_nominal(encoder, &row.realization_machine)
            })?;
            encoder.field("realization_state", |encoder| {
                encode_nominal(encoder, &row.realization_state)
            })
        })
    })
}

fn ordinal(encoder: &mut Encoder, ordinal: &u32) -> Result<(), PackageReviewEncodingError> {
    encoder.field("value", |encoder| {
        encoder.u32(*ordinal);
        Ok(())
    })?;
    Ok(())
}

fn const_argument(
    encoder: &mut Encoder,
    argument: &PackagePolicyConformanceConstArgument,
) -> Result<(), PackageReviewEncodingError> {
    match argument {
        PackagePolicyConformanceConstArgument::Evaluated {
            parameter_carrier,
            declared_carrier,
            canonical_value_encoding,
        } => {
            encoder.tag("evaluated", 0);
            encoder.field("parameter_carrier", |encoder| {
                type_identity(encoder, parameter_carrier)
            })?;
            encoder.field("declared_carrier", |encoder| {
                type_identity(encoder, declared_carrier)
            })?;
            encoder.field("canonical_value_encoding", |encoder| {
                encoder.string(canonical_value_encoding)
            })
        }
        PackagePolicyConformanceConstArgument::CallerExpression {
            parameter_carrier,
            expression,
        } => {
            encoder.tag("caller_expression", 2);
            encoder.field("parameter_carrier", |encoder| {
                type_identity(encoder, parameter_carrier)
            })?;
            encoder.field("expression", |encoder| type_identity(encoder, expression))
        }
        PackagePolicyConformanceConstArgument::CallerBinder {
            parameter_carrier,
            binder,
            binder_carrier,
        } => {
            encoder.tag("caller_binder", 1);
            encoder.field("parameter_carrier", |encoder| {
                type_identity(encoder, parameter_carrier)
            })?;
            encoder.field("binder", |encoder| encode_nominal(encoder, binder))?;
            encoder.field("binder_carrier", |encoder| {
                type_identity(encoder, binder_carrier)
            })
        }
    }
}
