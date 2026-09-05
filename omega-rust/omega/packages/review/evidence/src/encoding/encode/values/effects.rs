use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::{
    PackageReviewCapabilityFlow, PackageReviewInstallationReach, PackageReviewMutation,
    PackageReviewProgressSubject, PackageReviewSynchronousInvocation, PackageReviewTermination,
    PackageReviewWriteFrameCompleteness,
};

use super::identity::encode_nominal;

pub(crate) fn encode_synchronous_invocation(
    encoder: &mut Encoder,
    invocation: &PackageReviewSynchronousInvocation,
) -> Result<(), PackageReviewEncodingError> {
    match invocation {
        PackageReviewSynchronousInvocation::Parameter(position) => {
            encoder.tag("parameter", 0);
            encoder.field("position", |encoder| {
                encoder.u32(*position);
                Ok(())
            })?;
        }
        PackageReviewSynchronousInvocation::Service(service) => {
            encoder.tag("service", 1);
            encoder.field("service", |encoder| encode_nominal(encoder, service))?;
        }
    }
    Ok(())
}
pub(crate) fn encode_installation_reach(
    encoder: &mut Encoder,
    reach: &PackageReviewInstallationReach,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("requirement", |encoder| {
        encode_nominal(encoder, &reach.requirement)
    })?;
    encoder.field("upper_bound", |encoder| {
        encoder.sequence(&reach.upper_bound, encode_nominal)
    })
}

pub(crate) fn encode_capability_flow(
    encoder: &mut Encoder,
    flow: &PackageReviewCapabilityFlow,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &flow.capability)?;
    encoder.byte(match flow.kind {
        psi_effects::CapabilityFlowKind::Uses => 0,
        psi_effects::CapabilityFlowKind::Returns => 1,
        psi_effects::CapabilityFlowKind::Acquires => 2,
        psi_effects::CapabilityFlowKind::Stores => 3,
        psi_effects::CapabilityFlowKind::Derives => 4,
    });
    encode_nominal(encoder, &flow.state)?;
    encoder.usize(flow.statement_index)?;
    encoder.usize(flow.call_ordinal)?;
    encoder.option(flow.via_state.as_ref(), encode_nominal)
}

pub(crate) fn encode_termination(
    encoder: &mut Encoder,
    termination: &PackageReviewTermination,
) -> Result<(), PackageReviewEncodingError> {
    match termination {
        PackageReviewTermination::NoGuarantee => encoder.byte(0),
        PackageReviewTermination::Terminates { premises } => {
            encoder.byte(1);
            encoder.sequence(premises, |encoder, premise| {
                encode_nominal(encoder, &premise.profile)?;
                match &premise.subject {
                    PackageReviewProgressSubject::Declaration(identity) => {
                        encoder.byte(0);
                        encode_nominal(encoder, identity)?;
                    }
                    PackageReviewProgressSubject::Receiver => encoder.byte(1),
                    PackageReviewProgressSubject::Parameter(position) => {
                        encoder.byte(2);
                        encoder.u32(*position);
                    }
                }
                encoder.sequence(&premise.projections, encode_nominal)
            })?;
        }
    }
    Ok(())
}

pub(crate) fn encode_mutation(
    encoder: &mut Encoder,
    mutation: &PackageReviewMutation,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &mutation.state)?;
    encoder.byte(match mutation.completeness {
        PackageReviewWriteFrameCompleteness::Complete => 0,
        PackageReviewWriteFrameCompleteness::Opaque => 1,
    });
    encoder.sequence(&mutation.paths, |encoder, path| encoder.string(path))
}
