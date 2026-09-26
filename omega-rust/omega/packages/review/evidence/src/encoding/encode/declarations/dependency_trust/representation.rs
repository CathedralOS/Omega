use super::{
    calling_policy_tag, encode_boundary_shape_graph, encode_opaque_occurrence,
    encode_representation_target,
};
use crate::encoding::encode::declarations::encode_type_identity;
use crate::encoding::encode::values::identity::encode_nominal;
use crate::encoding::{PackageReviewEncodingError, encode::encoder::Encoder};
use crate::record::{
    PackageReviewOpaqueRepresentationCopyDisposition, PackageReviewRepresentationTcb,
    PackageReviewRepresentationTcbKind,
};

pub(crate) fn encode_representation_tcb_key(
    encoder: &mut Encoder,
    row: &PackageReviewRepresentationTcb,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &row.declaration)?;
    match &row.kind {
        PackageReviewRepresentationTcbKind::Unbound => encoder.byte(0),
        PackageReviewRepresentationTcbKind::ProducerAvailability { conformance, .. } => {
            encoder.byte(1);
            encode_nominal(encoder, conformance)?;
        }
        PackageReviewRepresentationTcbKind::SelectedCopyReceipt { .. } => encoder.byte(2),
        PackageReviewRepresentationTcbKind::ConsumerDemand {
            boundary_trait,
            boundary_arguments,
            requirement,
            requirement_identity,
            ..
        } => {
            encoder.byte(3);
            encode_nominal(encoder, boundary_trait)?;
            encoder.sequence(boundary_arguments, encode_type_identity)?;
            encode_nominal(encoder, requirement)?;
            encoder.string(requirement_identity)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_representation_tcb(
    encoder: &mut Encoder,
    row: &PackageReviewRepresentationTcb,
) -> Result<(), PackageReviewEncodingError> {
    encode_representation_tcb_key(encoder, row)?;
    match &row.kind {
        PackageReviewRepresentationTcbKind::Unbound => {}
        PackageReviewRepresentationTcbKind::ProducerAvailability { carrier, .. } => {
            encode_nominal(encoder, carrier)?;
        }
        PackageReviewRepresentationTcbKind::SelectedCopyReceipt {
            conformance,
            carrier,
            representation_schema_version,
            origin,
            lifecycle,
            copy_disposition,
            conformance_application_commitment,
            selected_application_commitment,
        } => {
            encode_nominal(encoder, conformance)?;
            encode_nominal(encoder, carrier)?;
            encoder.u16(*representation_schema_version);
            encoder.byte(match origin {
                crate::record::PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance => 1,
            });
            encoder.byte(match lifecycle {
                crate::record::PackageReviewOpaqueRepresentationLifecycleDisposition::Inert => 1,
            });
            encoder.byte(match copy_disposition {
                crate::record::PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly => 1,
                crate::record::PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy => 2,
            });
            encoder.fixed_bytes(conformance_application_commitment);
            encoder.fixed_bytes(selected_application_commitment);
        }
        PackageReviewRepresentationTcbKind::ConsumerDemand {
            target,
            conformance,
            carrier,
            representation_schema_version,
            origin,
            lifecycle,
            copy_disposition,
            shape_graph,
            occurrences,
            calling_policy,
            conformance_application_commitment,
            selected_application_commitment,
            boundary_plan_commitment,
            ..
        } => {
            encode_representation_target(encoder, *target);
            encode_nominal(encoder, conformance)?;
            encode_nominal(encoder, carrier)?;
            encoder.u16(*representation_schema_version);
            encoder.byte(match origin {
                crate::record::PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance => 1,
            });
            encoder.byte(match lifecycle {
                crate::record::PackageReviewOpaqueRepresentationLifecycleDisposition::Inert => 1,
            });
            encoder.byte(copy_disposition_tag(*copy_disposition));
            encode_boundary_shape_graph(encoder, shape_graph)?;
            encoder.sequence(occurrences, encode_opaque_occurrence)?;
            encoder.byte(calling_policy_tag(*calling_policy));
            encoder.fixed_bytes(conformance_application_commitment);
            encoder.fixed_bytes(selected_application_commitment);
            encoder.fixed_bytes(boundary_plan_commitment);
        }
    }
    Ok(())
}

const fn copy_disposition_tag(disposition: PackageReviewOpaqueRepresentationCopyDisposition) -> u8 {
    match disposition {
        PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly => 1,
        PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy => 2,
    }
}
