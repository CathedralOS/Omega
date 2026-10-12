//! Exact runtime assumption obligations, independent of provider authority.
//!
//! Settlement supplies commitments after checking both declarations, the actual
//! selected implementation and the independent structural obligations. These
//! values describe the relationship to accept; constructing them accepts
//! nothing and does not install a provider. Receiving policy must join current
//! obligations to its own exact retained consent before producing a receipt.

use effects::provider_plan::{BoundaryCallingPlanCommitment, ProviderPlanDigest};

use crate::{TrustAdmission, digest_encoder::DomainDigestEncoder};

/// Content identities supplied by the checked relationship producer. An empty
/// application has its own nonzero canonical commitment; zero means absent
/// evidence, never an empty telescope. Declaration identity includes its source
/// owner and revision, rather than a compilation-local symbol handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingRequirementCommitments {
    pub declaration_revision: [u8; 32],
    pub application: [u8; 32],
    /// Canonical public requirement signature and contract content. A trait
    /// signature uses its canonical declaration bytes; a top-level requirement
    /// uses its original checked public contract plan, including machine
    /// clauses. Neither is a fabricated implementation contract commitment.
    pub public_contract: [u8; 32],
}

/// Complete exact scope of one runtime relationship. The named commitments
/// bind canonical producer-owned structures, not display strings. In
/// particular, `semantic_obligations` retains entry/exit differences, their
/// proved/assumed classification and any demonstrated contradictions; it does
/// not assert that the assumption is true. `subject_mapping` includes static
/// arguments, every parameter/result and their lifetime correspondence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingRelationshipCommitments {
    pub required: AssumedBindingRequirementCommitments,
    pub supplied: AssumedBindingRequirementCommitments,
    pub root_scope: [u8; 32],
    pub subject_mapping: [u8; 32],
    pub selected_provider: ProviderPlanDigest,
    pub selected_implementation: [u8; 32],
    pub semantic_dependencies: [u8; 32],
    pub calling_plan: BoundaryCallingPlanCommitment,
    pub structural_compatibility: [u8; 32],
    pub domain_authority: [u8; 32],
    pub semantic_obligations: [u8; 32],
}

/// Domain-separated content identity, distinct from a provider-plan admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssumedBindingRelationshipDigest([u8; 32]);

impl AssumedBindingRelationshipDigest {
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl std::fmt::Display for AssumedBindingRelationshipDigest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// An assumption awaiting exact root and, where applicable, receiving consent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumedBindingRelationship {
    commitments: AssumedBindingRelationshipCommitments,
    digest: AssumedBindingRelationshipDigest,
}

impl AssumedBindingRelationship {
    pub fn new(commitments: AssumedBindingRelationshipCommitments) -> Result<Self, &'static str> {
        let parts = [
            commitments.required.declaration_revision,
            commitments.required.application,
            commitments.required.public_contract,
            commitments.supplied.declaration_revision,
            commitments.supplied.application,
            commitments.supplied.public_contract,
            commitments.root_scope,
            commitments.subject_mapping,
            *commitments.selected_provider.as_bytes(),
            commitments.selected_implementation,
            commitments.semantic_dependencies,
            commitments.calling_plan.as_bytes(),
            commitments.structural_compatibility,
            commitments.domain_authority,
            commitments.semantic_obligations,
        ];
        if parts.contains(&[0; 32]) {
            return Err("assumed binding relationship requires every exact content commitment");
        }
        let mut encoder = DomainDigestEncoder::new(b"omega.assumed-binding-relationship.v1\0");
        encoder.bytes(b"runtime-only");
        for part in parts {
            encoder.raw(&part);
        }
        Ok(Self {
            commitments,
            digest: AssumedBindingRelationshipDigest(encoder.finish()),
        })
    }

    pub const fn commitments(&self) -> &AssumedBindingRelationshipCommitments {
        &self.commitments
    }

    pub const fn digest(&self) -> AssumedBindingRelationshipDigest {
        self.digest
    }

    /// Exact portable admission coordinate. The package review row retains the
    /// readable relationship meaning separately; no user spelling selects it.
    pub fn trust_obligation(&self) -> TrustAdmission {
        TrustAdmission::for_assumed_binding_relationship(
            format!("assumed binding contract: {}", self.digest),
            self.digest,
        )
    }
}

/// Exact consent for this relationship only. This receipt supplies neither
/// provider authority nor a checked conformance, and cannot authorize Build
/// computation. Installation still needs the checked descriptor and ordinary
/// provider/domain/custody authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedAssumedBindingRelationship {
    relationship: AssumedBindingRelationshipDigest,
    admission: TrustAdmission,
}

impl AcceptedAssumedBindingRelationship {
    pub const fn relationship(&self) -> AssumedBindingRelationshipDigest {
        self.relationship
    }

    pub const fn admission(&self) -> &TrustAdmission {
        &self.admission
    }

    pub fn authorizes_relationship(&self, relationship: &AssumedBindingRelationship) -> bool {
        self.relationship == relationship.digest()
    }
}

/// Join the current relationship to the receiving owner's exact accepted
/// admission. The caller supplies consent from its authenticated policy/lock
/// boundary, never from Build intent, dependency self-acceptance or a provider
/// grant. Missing, stale and wrong-subject admissions reject without checking
/// or assuming the truth of the relationship again.
pub fn accept_assumed_binding_relationship(
    relationship: &AssumedBindingRelationship,
    accepted: &[TrustAdmission],
) -> Result<AcceptedAssumedBindingRelationship, &'static str> {
    let expected = relationship.trust_obligation();
    let mut matches = accepted.iter().filter(|admission| **admission == expected);
    let admission = matches
        .next()
        .ok_or("assumed binding relationship lacks exact receiving acceptance")?;
    if matches.next().is_some() {
        return Err("assumed binding relationship has duplicate receiving acceptance");
    }
    Ok(AcceptedAssumedBindingRelationship {
        relationship: relationship.digest(),
        admission: admission.clone(),
    })
}
