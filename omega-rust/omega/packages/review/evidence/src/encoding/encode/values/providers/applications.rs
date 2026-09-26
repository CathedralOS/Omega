use super::encode_compiler_intrinsic_execution;
use crate::encoding::encode::values::identity::encode_nominal;
use crate::encoding::encode::{
    declarations::encode_type_identity, values::declarations::encode_operator_coordinate,
};
use crate::encoding::{PackageReviewEncodingError, encode::encoder::Encoder};
use crate::record::{
    CheckedPackageBoundaryApplicationDemandReview,
    CheckedPackageBoundaryApplicationRealizationReview, PackageReviewBoundaryApplication,
    PackageReviewBoundaryApplicationRealization,
};

pub(crate) fn encode_boundary_application_realization_key(
    encoder: &mut Encoder,
    realization: &CheckedPackageBoundaryApplicationRealizationReview,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &realization.operator_declaration)?;
    encoder.string(&realization.requirement_identity)?;
    encode_boundary_application(encoder, &realization.application)
}

pub(crate) fn encode_boundary_application_demand_key(
    encoder: &mut Encoder,
    demand: &CheckedPackageBoundaryApplicationDemandReview,
) -> Result<(), PackageReviewEncodingError> {
    encode_operator_coordinate(encoder, &demand.operator_coordinate)?;
    encoder.string(&demand.requirement_identity)?;
    encode_nominal(encoder, &demand.producer_callable)?;
    encoder.sequence(&demand.arguments, |encoder, argument| {
        match argument {
            crate::record::PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
                requirement_binder_ordinal,
                producer_binder_ordinal,
            } => {
                encoder.byte(0);
                encoder.u32(*requirement_binder_ordinal);
                encoder.u32(*producer_binder_ordinal);
            }
        }
        Ok(())
    })
}

pub(crate) fn encode_boundary_application_demand(
    encoder: &mut Encoder,
    demand: &CheckedPackageBoundaryApplicationDemandReview,
) -> Result<(), PackageReviewEncodingError> {
    encode_boundary_application_demand_key(encoder, demand)
}

pub(crate) fn encode_boundary_application_realization(
    encoder: &mut Encoder,
    realization: &CheckedPackageBoundaryApplicationRealizationReview,
) -> Result<(), PackageReviewEncodingError> {
    encode_boundary_application_realization_key(encoder, realization)?;
    encoder.fixed_bytes(&realization.selected_plan_digest);
    match &realization.realization {
        PackageReviewBoundaryApplicationRealization::NongenericCheckedBody {
            realization_machine,
            realization_state,
            realization_contract_commitment,
        } => {
            encoder.byte(0);
            encode_nominal(encoder, realization_machine)?;
            encode_nominal(encoder, realization_state)?;
            encoder.fixed_bytes(realization_contract_commitment);
        }
        PackageReviewBoundaryApplicationRealization::SpecializedCheckedBody {
            realization_template,
            realization_machine,
            realization_state,
            specialization_commitment,
            realization_contract_commitment,
        } => {
            encoder.byte(2);
            encode_nominal(encoder, realization_template)?;
            encode_nominal(encoder, realization_machine)?;
            encode_nominal(encoder, realization_state)?;
            encoder.fixed_bytes(specialization_commitment);
            encoder.fixed_bytes(realization_contract_commitment);
        }
        PackageReviewBoundaryApplicationRealization::ExactCompilerIntrinsic { execution } => {
            encoder.byte(1);
            encode_compiler_intrinsic_execution(encoder, execution)?;
        }
    }
    Ok(())
}

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
