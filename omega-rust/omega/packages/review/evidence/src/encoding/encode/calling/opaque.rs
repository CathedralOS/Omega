use super::{Encoder, PackageReviewEncodingError};
use crate::encoding::encode::declarations::encode_opaque_occurrence;
use crate::encoding::encode::values::conformance_policy::encode_application;
use crate::encoding::encode::values::identity::encode_nominal;
use crate::record::{
    PackagePolicyCallingOpaqueUse, PackageReviewNominalOwner,
    PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition,
};

pub(super) fn encode(
    encoder: &mut Encoder,
    use_: &PackagePolicyCallingOpaqueUse,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &use_.opaque)?;
    encode_nominal(encoder, &use_.carrier)?;
    match use_.selection_owner {
        PackageReviewNominalOwner::Package(package) => {
            encoder.byte(0);
            encoder.package_identity(package);
        }
        PackageReviewNominalOwner::ToolchainSource(source) => {
            encoder.byte(1);
            encoder.fixed_bytes(&source.digest());
        }
        PackageReviewNominalOwner::Unresolved => {
            return Err(PackageReviewEncodingError::new(
                "calling policy has unresolved selection ownership",
            ));
        }
    }
    encode_application(encoder, &use_.application)?;
    encoder.byte(match use_.origin {
        PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance => 0,
    });
    encoder.byte(match use_.lifecycle {
        PackageReviewOpaqueRepresentationLifecycleDisposition::Inert => 0,
    });
    encoder.byte(match use_.copy_disposition {
        PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly => 0,
        PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy => 1,
    });
    encoder.sequence(&use_.occurrences, encode_opaque_occurrence)
}
