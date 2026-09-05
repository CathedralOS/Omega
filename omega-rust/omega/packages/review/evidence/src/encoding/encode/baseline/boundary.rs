use super::*;
use values::{
    declarations::encode_operator_coordinate,
    identity::encode_nominal,
    providers::{encode_boundary_application, encode_compiler_intrinsic_execution},
};

pub(super) fn applications(
    encoder: &mut Encoder,
    applications: &PackagePolicyBoundaryApplications,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("demands", |encoder| {
        encoder.sequence(&applications.demands, |encoder, demand| {
            encoder.field("operator_coordinate", |encoder| {
                encode_operator_coordinate(encoder, &demand.operator_coordinate)
            })?;
            encoder.field("producer_callable", |encoder| {
                encode_nominal(encoder, &demand.producer_callable)
            })?;
            encoder.field("arguments", |encoder| {
                encoder.sequence(&demand.arguments, |encoder, argument| {
                    match argument {
                        PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
                            requirement_binder_ordinal,
                            producer_binder_ordinal,
                        } => {
                            encoder.tag("type_binder", 0);
                            encoder.field("requirement_binder_ordinal", |encoder| {
                                encoder.u32(*requirement_binder_ordinal);
                                Ok(())
                            })?;
                            encoder.field("producer_binder_ordinal", |encoder| {
                                encoder.u32(*producer_binder_ordinal);
                                Ok(())
                            })?;
                        }
                    }
                    Ok(())
                })
            })
        })
    })?;
    encoder.field("realizations", |encoder| {
        encoder.sequence(&applications.realizations, |encoder, realization| {
            encoder.field("operator_coordinate", |encoder| {
                encode_operator_coordinate(encoder, &realization.operator_coordinate)
            })?;
            encoder.field("requirement_identity", |encoder| {
                encoder.string(&realization.requirement_identity)
            })?;
            encoder.field("application", |encoder| {
                encode_boundary_application(encoder, &realization.application)
            })?;
            encoder.field("selected_plan_index", |encoder| {
                encoder.u32(realization.selected_plan_index);
                Ok(())
            })?;
            encoder.field("realization", |encoder| {
                match &realization.realization {
                    PackagePolicyBoundaryRealization::NongenericCheckedBody {
                        declaration,
                        realization,
                    } => {
                        encoder.tag("nongeneric_checked_body", 0);
                        encoder.field("declaration", |encoder| {
                            encode_nominal(encoder, declaration)
                        })?;
                        encoder.field("realization", |encoder| {
                            encode_nominal(encoder, realization)
                        })?;
                    }
                    PackagePolicyBoundaryRealization::SpecializedCheckedBody {
                        declaration,
                        template,
                    } => {
                        encoder.tag("specialized_checked_body", 1);
                        encoder.field("declaration", |encoder| {
                            encode_nominal(encoder, declaration)
                        })?;
                        encoder.field("template", |encoder| encode_nominal(encoder, template))?;
                    }
                    PackagePolicyBoundaryRealization::ExactCompilerIntrinsic { execution } => {
                        encoder.tag("exact_compiler_intrinsic", 2);
                        encoder.field("execution", |encoder| {
                            encode_compiler_intrinsic_execution(encoder, execution)
                        })?;
                    }
                }
                Ok(())
            })
        })
    })
}
