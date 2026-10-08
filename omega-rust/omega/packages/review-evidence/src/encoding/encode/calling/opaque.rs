use super::{Encoder, PackageReviewEncodingError};
use crate::encoding::encode::declarations::encode_opaque_occurrence;
use crate::encoding::encode::values::conformance_policy::encode_application;
use crate::encoding::encode::values::identity::encode_nominal;
use crate::record::{
    PackagePolicyCallingOpaqueUse, PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition,
};

pub(super) fn encode(
    encoder: &mut Encoder,
    use_: &PackagePolicyCallingOpaqueUse,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("opaque", |encoder| encode_nominal(encoder, &use_.opaque))?;
    encoder.field("carrier", |encoder| encode_nominal(encoder, &use_.carrier))?;
    encoder.field("selection_owner", |encoder| {
        crate::encoding::encode::values::identity::encode_selection_owner(
            encoder,
            &use_.selection_owner,
            "calling policy has unresolved selection ownership",
        )
    })?;
    encoder.field("application", |encoder| {
        encode_application(encoder, &use_.application)
    })?;
    encoder.field("origin", |encoder| {
        match use_.origin {
            PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance => {
                encoder.tag("named_conformance", 0)
            }
        };
        Ok(())
    })?;
    encoder.field("lifecycle", |encoder| {
        match use_.lifecycle {
            PackageReviewOpaqueRepresentationLifecycleDisposition::Inert => encoder.tag("inert", 0),
        };
        Ok(())
    })?;
    encoder.field("copy_disposition", |encoder| {
        match use_.copy_disposition {
            PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly => {
                encoder.tag("placement_only", 0)
            }
            PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy => {
                encoder.tag("checked_semantic_copy", 1)
            }
        };
        Ok(())
    })?;
    encoder.field("occurrences", |encoder| {
        encoder.sequence(&use_.occurrences, encode_opaque_occurrence)
    })
}
