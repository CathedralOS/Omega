//! Bounded recovery of inert assumed-relationship review meaning. Descriptors
//! reconstructed here are historical obligations, never accepted receipts.

use effects::provider_plan::{BoundaryCallingPlanCommitment, ProviderPlanDigest};
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;
use trust_model::{
    AssumedBindingRelationship, AssumedBindingRelationshipCommitments,
    AssumedBindingRequirementCommitments,
};

use super::super::{Error, reader::Reader};
use crate::record::{PackagePolicyAssumedBindingContracts, PackageReviewAssumedBindingContract};

pub(super) fn contracts(
    reader: &mut Reader<'_>,
    package: PackageKeyIdentity,
    target: TargetProfile,
) -> Result<PackagePolicyAssumedBindingContracts, Error> {
    // Risk string, runtime tag, aggregate digest, 15 fixed commitments, seven
    // length-framed descriptions and the contradiction count precede payload.
    const MINIMUM_RELATIONSHIP_BYTES: usize = 8 + 1 + 32 + 15 * 32 + 7 * 8 + 8;
    let relationships = reader.sequence(MINIMUM_RELATIONSHIP_BYTES, relationship)?;
    Ok(PackagePolicyAssumedBindingContracts::new(
        package,
        target,
        relationships,
    ))
}

fn relationship(reader: &mut Reader<'_>) -> Result<PackageReviewAssumedBindingContract, Error> {
    if reader.string()? != "Extremely high risk - assumed binding contract" || !reader.boolean()? {
        return Err(Error::InvalidValue);
    }
    let expected_digest = reader.digest()?;
    let required = requirement(reader)?;
    let supplied = requirement(reader)?;
    let commitments = AssumedBindingRelationshipCommitments {
        required,
        supplied,
        root_scope: reader.digest()?,
        subject_mapping: reader.digest()?,
        selected_provider: ProviderPlanDigest::from_digest(reader.digest()?),
        selected_implementation: reader.digest()?,
        semantic_dependencies: reader.digest()?,
        calling_plan: BoundaryCallingPlanCommitment::from_digest(reader.digest()?),
        structural_compatibility: reader.digest()?,
        domain_authority: reader.digest()?,
        semantic_obligations: reader.digest()?,
    };
    let relationship =
        AssumedBindingRelationship::new(commitments).map_err(|_| Error::InvalidValue)?;
    if relationship.digest().as_bytes() != expected_digest {
        return Err(Error::InvalidValue);
    }
    Ok(PackageReviewAssumedBindingContract {
        relationship,
        required_contract: description(reader)?,
        supplied_contract: description(reader)?,
        selected_implementation: description(reader)?,
        assumption_difference: description(reader)?,
        authority_introduced: description(reader)?,
        root_scope: description(reader)?,
        dependency_path: description(reader)?,
        demonstrated_contradictions: reader.sequence(9, description)?,
    })
}

fn requirement(reader: &mut Reader<'_>) -> Result<AssumedBindingRequirementCommitments, Error> {
    Ok(AssumedBindingRequirementCommitments {
        declaration_revision: reader.digest()?,
        application: reader.digest()?,
        public_contract: reader.digest()?,
    })
}

fn description(reader: &mut Reader<'_>) -> Result<String, Error> {
    let value = reader.string()?;
    if value.is_empty() || value.contains('\0') {
        return Err(Error::InvalidValue);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::PackagePolicyRecoveryLimits;
    use crate::encoding::encode::{encode_relationship, encoder::Encoder};

    fn review() -> PackageReviewAssumedBindingContract {
        let requirement = AssumedBindingRequirementCommitments {
            declaration_revision: [1; 32],
            application: [2; 32],
            public_contract: [3; 32],
        };
        let relationship = AssumedBindingRelationship::new(AssumedBindingRelationshipCommitments {
            required: requirement,
            supplied: AssumedBindingRequirementCommitments {
                declaration_revision: [4; 32],
                ..requirement
            },
            root_scope: [5; 32],
            subject_mapping: [6; 32],
            selected_provider: ProviderPlanDigest::from_digest([7; 32]),
            selected_implementation: [8; 32],
            semantic_dependencies: [9; 32],
            calling_plan: BoundaryCallingPlanCommitment::from_digest([10; 32]),
            structural_compatibility: [11; 32],
            domain_authority: [12; 32],
            semantic_obligations: [13; 32],
        })
        .unwrap();
        PackageReviewAssumedBindingContract {
            relationship,
            required_contract: "Required::read; Ensures result == 1".to_owned(),
            supplied_contract: "Supplied::read; Ensures result == 0".to_owned(),
            selected_implementation: "exact checked Supplier::read".to_owned(),
            assumption_difference: "assumed caller result == 1".to_owned(),
            authority_introduced: "no new domain claim".to_owned(),
            root_scope: "consuming runtime root".to_owned(),
            dependency_path: "root -> Supplier".to_owned(),
            demonstrated_contradictions: vec![
                "exact result subject == 0 and == 1 contradict".to_owned(),
            ],
        }
    }

    fn encoded(review: &PackageReviewAssumedBindingContract) -> Vec<u8> {
        let mut encoder =
            Encoder::policy_bounded(PackagePolicyRecoveryLimits::default().maximum_bytes);
        encode_relationship(&mut encoder, review).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn assumed_relationship_review_recovery_retains_exact_scope_and_refutation() {
        let review = review();
        let bytes = encoded(&review);
        let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
        assert_eq!(relationship(&mut reader).unwrap(), review);
        reader.finish().unwrap();
    }

    #[test]
    fn assumed_relationship_recovery_refuses_changed_scope_and_build_classification() {
        let review = review();
        let bytes = encoded(&review);
        let runtime_offset = 8 + "Extremely high risk - assumed binding contract".len();
        let digest_offset = runtime_offset + 1;
        let required_application_offset = digest_offset + 32 + 32;
        let mut changed = bytes.clone();
        changed[runtime_offset] = 0;
        let mut reader = Reader::new(&changed, PackagePolicyRecoveryLimits::default()).unwrap();
        assert!(relationship(&mut reader).is_err());
        changed = bytes.clone();
        changed[digest_offset] ^= 1;
        let mut reader = Reader::new(&changed, PackagePolicyRecoveryLimits::default()).unwrap();
        assert!(relationship(&mut reader).is_err());
        changed = bytes.clone();
        changed[required_application_offset..required_application_offset + 32].fill(0);
        let mut reader = Reader::new(&changed, PackagePolicyRecoveryLimits::default()).unwrap();
        assert!(relationship(&mut reader).is_err());
        let limits = PackagePolicyRecoveryLimits {
            maximum_owned_bytes: 0,
            ..PackagePolicyRecoveryLimits::default()
        };
        let mut reader = Reader::new(&bytes, limits).unwrap();
        assert!(relationship(&mut reader).is_err());
        let excessive_count = u64::MAX.to_le_bytes();
        let mut reader =
            Reader::new(&excessive_count, PackagePolicyRecoveryLimits::default()).unwrap();
        assert!(
            contracts(
                &mut reader,
                PackageKeyIdentity::from_digest([7; 32]).unwrap(),
                TargetProfile::WindowsX64
            )
            .is_err()
        );
    }
}
