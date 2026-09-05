//! Receipt-free representation policy with one aggregate writer budget.

use super::{
    PackageReviewEncodingError, calling,
    declarations::{encode_conformance_shape, encode_representation_target},
    encoder::Encoder,
    values::{conformance_policy, identity::encode_nominal},
};
use crate::encoding::{PACKAGE_REPRESENTATION_POLICY_VERSION, REPRESENTATION_POLICY_MAGIC};
use crate::record::{
    PackagePolicyRepresentation, PackagePolicyRepresentationSelection, PackageReviewNominalOwner,
    PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition,
};

impl PackagePolicyRepresentation {
    /// Complete representation-policy meaning, without compiler/native
    /// receipts or any assertion of project acceptance.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        self.validate_canonical_structure()
            .map_err(PackageReviewEncodingError::new)?;
        let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
        encoder.fixed_bytes(REPRESENTATION_POLICY_MAGIC);
        encoder.u16(PACKAGE_REPRESENTATION_POLICY_VERSION);
        encoder.package_identity(self.package);
        encode_representation_target(&mut encoder, self.target);
        encoder.sequence(&self.declarations, encode_nominal)?;
        encoder.sequence(&self.producer_availability, |encoder, availability| {
            encode_nominal(encoder, &availability.opaque)?;
            encode_conformance_shape(encoder, &availability.conformance)?;
            encode_nominal(encoder, &availability.carrier)
        })?;
        encoder.sequence(&self.selected_availability, selection)?;
        encoder.sequence(&self.demands, |encoder, demand| {
            encode_nominal(encoder, &demand.opaque)?;
            calling::encode_application(encoder, &demand.calling)
        })?;
        encoder.finish()
    }
}

fn selection(
    encoder: &mut Encoder,
    selection: &PackagePolicyRepresentationSelection,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &selection.opaque)?;
    encode_nominal(encoder, &selection.carrier)?;
    match selection.selection_owner {
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
                "representation policy has unresolved selection ownership",
            ));
        }
    }
    conformance_policy::encode_application(encoder, &selection.application)?;
    encoder.byte(match selection.origin {
        PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance => 0,
    });
    encoder.byte(match selection.lifecycle {
        PackageReviewOpaqueRepresentationLifecycleDisposition::Inert => 0,
    });
    encoder.byte(match selection.copy_disposition {
        PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly => 0,
        PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy => 1,
    });
    Ok(())
}
