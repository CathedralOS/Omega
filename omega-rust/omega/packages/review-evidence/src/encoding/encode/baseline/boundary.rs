use super::values;
use crate::encoding::PackageReviewEncodingError;
use crate::encoding::encode::encoder::Encoder;
use crate::record::PackagePolicyBoundaryApplicationDemand;
use crate::record::PackagePolicyBoundaryApplicationRealization;
use crate::record::PackagePolicyBoundaryApplications;
use crate::record::PackagePolicyBoundaryRealization;
use crate::record::PackageReviewSymbolicBoundaryApplicationArgument;
use values::{
    declarations::encode_operator_coordinate,
    identity::encode_nominal,
    providers::{encode_boundary_application, encode_compiler_intrinsic_execution},
};

pub(in crate::encoding::encode) fn applications(
    encoder: &mut Encoder,
    applications: &PackagePolicyBoundaryApplications,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("demands", |encoder| {
        encoder.sequence(&applications.demands, demand)
    })?;
    encoder.field("realizations", |encoder| {
        encoder.sequence(&applications.realizations, realization)
    })
}

pub(in crate::encoding::encode) fn demand(
    encoder: &mut Encoder,
    demand: &PackagePolicyBoundaryApplicationDemand,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by(
        "operator_coordinate",
        &demand.operator_coordinate,
        encode_operator_coordinate,
    )?;
    encoder.field_by(
        "producer_callable",
        &demand.producer_callable,
        encode_nominal,
    )?;
    encoder.field("arguments", |encoder| {
        encoder.sequence(&demand.arguments, |encoder, argument| {
            match argument {
                PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
                    requirement_binder_ordinal,
                    producer_binder_ordinal,
                } => {
                    encoder.tag("type_binder", 0);
                    encoder.u32_field("requirement_binder_ordinal", *requirement_binder_ordinal)?;
                    encoder.u32_field("producer_binder_ordinal", *producer_binder_ordinal)?;
                }
            }
            Ok(())
        })
    })
}

pub(in crate::encoding::encode) fn realization(
    encoder: &mut Encoder,
    realization: &PackagePolicyBoundaryApplicationRealization,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by(
        "operator_coordinate",
        &realization.operator_coordinate,
        encode_operator_coordinate,
    )?;
    encoder.field("requirement_identity", |encoder| {
        encoder.string(&realization.requirement_identity)
    })?;
    encoder.field_by(
        "application",
        &realization.application,
        encode_boundary_application,
    )?;
    encoder.u32_field("selected_plan_index", realization.selected_plan_index)?;
    encoder.field("realization", |encoder| {
        match &realization.realization {
            PackagePolicyBoundaryRealization::NongenericCheckedBody {
                declaration,
                realization,
            } => {
                encoder.tag("nongeneric_checked_body", 0);
                encoder.field_by("declaration", declaration, encode_nominal)?;
                encoder.field_by("realization", realization, encode_nominal)?;
            }
            PackagePolicyBoundaryRealization::SpecializedCheckedBody {
                declaration,
                template,
            } => {
                encoder.tag("specialized_checked_body", 1);
                encoder.field_by("declaration", declaration, encode_nominal)?;
                encoder.field("template", |encoder| encode_nominal(encoder, template))?;
            }
            PackagePolicyBoundaryRealization::ExactCompilerIntrinsic { execution } => {
                encoder.tag("exact_compiler_intrinsic", 2);
                encoder.field_by("execution", execution, encode_compiler_intrinsic_execution)?;
            }
        }
        Ok(())
    })
}
