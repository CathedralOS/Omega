//! Receipt-free representation policy with one aggregate writer budget.

use super::{
    PackageReviewEncodingError, calling,
    declarations::encode_representation_target,
    encoder::Encoder,
    public_api::conformance_shape as encode_conformance_shape,
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
        policy(&mut encoder, self)?;
        encoder.finish()
    }
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
    encoder.field("opaque", |encoder| {
        encode_nominal(encoder, &selection.opaque)
    })?;
    encoder.field("carrier", |encoder| {
        encode_nominal(encoder, &selection.carrier)
    })?;
    encoder.field("selection_owner", |encoder| {
        match selection.selection_owner {
            PackageReviewNominalOwner::Package(package) => {
                encoder.tag("package", 0);
                encoder.field("package", |encoder| {
                    encoder.package_identity(package);
                    Ok(())
                })?;
            }
            PackageReviewNominalOwner::ToolchainSource(source) => {
                encoder.tag("toolchain_source", 1);
                encoder.field("digest", |encoder| {
                    encoder.fixed_bytes(&source.digest());
                    Ok(())
                })?;
            }
            PackageReviewNominalOwner::Unresolved => {
                return Err(PackageReviewEncodingError::new(
                    "representation policy has unresolved selection ownership",
                ));
            }
        };
        Ok(())
    })?;
    encoder.field("application", |encoder| {
        conformance_policy::encode_application(encoder, &selection.application)
    })?;
    encoder.field("origin", |encoder| {
        match selection.origin {
            PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance => {
                encoder.tag("named_conformance", 0)
            }
        };
        Ok(())
    })?;
    encoder.field("lifecycle", |encoder| {
        match selection.lifecycle {
            PackageReviewOpaqueRepresentationLifecycleDisposition::Inert => encoder.tag("inert", 0),
        };
        Ok(())
    })?;
    encoder.field("copy_disposition", |encoder| {
        match selection.copy_disposition {
            PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly => {
                encoder.tag("placement_only", 0)
            }
            PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy => {
                encoder.tag("checked_semantic_copy", 1)
            }
        };
        Ok(())
    })?;
    Ok(())
}
