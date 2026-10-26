//! Receipt-free representation policy with one aggregate writer budget.

use super::{
    PackageReviewEncodingError, calling,
    declarations::encode_representation_target,
    encoder::{Encoder, encode_canonical_component},
    public_api::conformance_shape as encode_conformance_shape,
    values::{
        conformance_policy,
        identity::{encode_nominal, encode_selection_owner},
    },
};
use crate::encoding::{PACKAGE_REPRESENTATION_POLICY_VERSION, REPRESENTATION_POLICY_MAGIC};
use crate::record::{
    PackagePolicyRepresentation, PackagePolicyRepresentationSelection,
    PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition,
};

// The opaque-selection join every representation selection and calling opaque
// use carries: carrier, selected owner, closed application, origin, lifecycle,
// and copy disposition in one canonical order. The carrier record differs, the
// encoding does not; `$unresolved_context` names the containing component in
// the refusal for unresolved selection ownership. The expansion is a value, so
// a caller may continue with its own trailing fields.
macro_rules! encode_opaque_representation_fields {
    ($encoder:expr, $record:ident, $unresolved_context:expr) => {{
        ($encoder).field("opaque", |encoder| encode_nominal(encoder, &$record.opaque))?;
        ($encoder).field("carrier", |encoder| {
            encode_nominal(encoder, &$record.carrier)
        })?;
        ($encoder).field("selection_owner", |encoder| {
            encode_selection_owner(encoder, &$record.selection_owner, $unresolved_context)
        })?;
        ($encoder).field("application", |encoder| {
            conformance_policy::encode_application(encoder, &$record.application)
        })?;
        ($encoder).field("origin", |encoder| {
            match $record.origin {
                PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance => {
                    encoder.tag("named_conformance", 0)
                }
            };
            Ok(())
        })?;
        ($encoder).field("lifecycle", |encoder| {
            match $record.lifecycle {
                PackageReviewOpaqueRepresentationLifecycleDisposition::Inert => {
                    encoder.tag("inert", 0)
                }
            };
            Ok(())
        })?;
        ($encoder).field("copy_disposition", |encoder| {
            match $record.copy_disposition {
                PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly => {
                    encoder.tag("placement_only", 0)
                }
                PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy => {
                    encoder.tag("checked_semantic_copy", 1)
                }
            };
            Ok(())
        })
    }};
}
pub(super) use encode_opaque_representation_fields;

impl PackagePolicyRepresentation {
    encode_canonical_component!(
        REPRESENTATION_POLICY_MAGIC,
        PACKAGE_REPRESENTATION_POLICY_VERSION,
        policy,
    );
}

pub(super) fn policy(
    encoder: &mut Encoder,
    policy: &PackagePolicyRepresentation,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("package", |encoder| {
        encoder.package_identity(policy.package);
        Ok(())
    })?;
    encoder.field("target", |encoder| {
        encode_representation_target(encoder, policy.target);
        Ok(())
    })?;
    encoder.field("declarations", |encoder| {
        encoder.sequence(&policy.declarations, encode_nominal)
    })?;
    encoder.field("producer_availability", |encoder| {
        encoder.sequence(&policy.producer_availability, |encoder, availability| {
            encoder.field("opaque", |encoder| {
                encode_nominal(encoder, &availability.opaque)
            })?;
            encoder.field("conformance", |encoder| {
                encode_conformance_shape(encoder, &availability.conformance)
            })?;
            encoder.field("carrier", |encoder| {
                encode_nominal(encoder, &availability.carrier)
            })
        })
    })?;
    encoder.field("selected_availability", |encoder| {
        encoder.sequence(&policy.selected_availability, selection)
    })?;
    encoder.field("demands", |encoder| {
        encoder.sequence(&policy.demands, |encoder, demand| {
            encoder.field("opaque", |encoder| encode_nominal(encoder, &demand.opaque))?;
            encoder.field("calling", |encoder| {
                calling::encode_application(encoder, &demand.calling)
            })
        })
    })
}

pub(super) fn selection(
    encoder: &mut Encoder,
    selection: &PackagePolicyRepresentationSelection,
) -> Result<(), PackageReviewEncodingError> {
    encode_opaque_representation_fields!(
        encoder,
        selection,
        "representation policy has unresolved selection ownership"
    )
}
