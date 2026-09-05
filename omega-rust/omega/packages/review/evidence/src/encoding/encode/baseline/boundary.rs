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
    encoder.sequence(&applications.demands, |encoder, demand| {
        encode_operator_coordinate(encoder, &demand.operator_coordinate)?;
        encode_nominal(encoder, &demand.producer_callable)?;
        encoder.sequence(&demand.arguments, |encoder, argument| {
            match argument {
                PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
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
    })?;
    encoder.sequence(&applications.realizations, |encoder, realization| {
        encode_operator_coordinate(encoder, &realization.operator_coordinate)?;
        encoder.string(&realization.requirement_identity)?;
        encode_boundary_application(encoder, &realization.application)?;
        encoder.u32(realization.selected_plan_index);
        match &realization.realization {
            PackagePolicyBoundaryRealization::NongenericCheckedBody {
                declaration,
                realization,
            } => {
                encoder.byte(0);
                encode_nominal(encoder, declaration)?;
                encode_nominal(encoder, realization)?;
            }
            PackagePolicyBoundaryRealization::SpecializedCheckedBody {
                declaration,
                template,
            } => {
                encoder.byte(1);
                encode_nominal(encoder, declaration)?;
                encode_nominal(encoder, template)?;
            }
            PackagePolicyBoundaryRealization::ExactCompilerIntrinsic { execution } => {
                encoder.byte(2);
                encode_compiler_intrinsic_execution(encoder, execution)?;
            }
        }
        Ok(())
    })
}
