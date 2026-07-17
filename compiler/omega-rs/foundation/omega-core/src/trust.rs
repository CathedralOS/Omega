//! The chapter-10 SHARED GRANT/RECEIPT CARRIER (design-ruled 2026-07-17;
//! GR1 of the trust-system ladder). Three consumers admit semantic
//! commitments through ONE pipeline -- sealed semantic-domain introduction
//! (`MintAuthority`), sealed progress-profile qualification, and admitted
//! provider plans -- plus the accepted proof rows (boundary machines) the
//! chapter is named for. The carrier is COMMITMENT-KEYED: each consumer
//! supplies its own identity into the same grant -> admission -> receipt ->
//! trust-report slots, which is why the ruling says build it once.
//!
//! Grant locality (ch10): OWN-PACKAGE claims are dev-active with a standing
//! warning until granted; claims arriving from packages are INERT until the
//! root grants them; a package can never self-grant. Grants flow from the
//! final build's build.omg (`b.accept_boundary<symbol>();` -- GR3). The
//! lockfile records each admitted commitment's statement hash automatically
//! (GR4); a statement that drifts under a grant fails the build until
//! re-approved.

use crate::semantics::{ProgressProfileId, SemanticDomainId};

/// What a grant admits: one semantic commitment, identified by the
/// consumer's own identity type. The carrier never interprets the
/// commitment -- admission is opaque (never flow-inferred, never an
/// entailment relation); the consumer's checker does its own validation and
/// only asks the carrier "is this commitment granted here?".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TrustCommitment {
    /// Sealed semantic-domain INTRODUCTION: minting values of a declared
    /// domain (`x as u32 in Meters`). The identity is the interned domain.
    SemanticDomainIntroduction(SemanticDomainId),
    /// A sealed boundary progress profile's qualification (TPR4).
    ProgressProfile(ProgressProfileId),
    /// An ACCEPTED-tier proof row: a bodyless `boundary machine` axiom,
    /// identified by its rendered symbol path (the honest identity carrier
    /// until artifact serialization lands a stable symbol id).
    AcceptedFact(String),
    /// An admitted provider plan's semantic claims (PRV3).
    ProviderPlan(String),
}

/// Where a grant's authority came from -- the provenance the trust report
/// names and the lockfile pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustProvenance {
    /// The commitment's declaration lives in the OWN (root) package:
    /// dev-active with a standing warning until an explicit root grant
    /// lands (grant locality).
    OwnPackageDev,
    /// An explicit root grant from the final build's build.omg.
    RootGrant,
}

/// One grant: authority for one commitment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustGrant {
    pub commitment: TrustCommitment,
    pub provenance: TrustProvenance,
}

/// One receipt: an ADMITTED commitment plus the hash of the statement the
/// admission covered (the lockfile row's payload -- drift under a grant
/// fails the build until re-approved). The hash is the consumer-rendered
/// canonical statement text's hash; `0` = not yet computed (GR4 wires the
/// real hashing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustReceipt {
    pub commitment: TrustCommitment,
    pub statement_hash: u64,
    pub provenance: TrustProvenance,
}

/// The grant table a compilation carries: consumers query it at their
/// admission points; the build-config path populates it (GR3); receipts
/// accumulate beside it for the lockfile writer (GR4) and the accepted-tier
/// trust-report rows (GR5).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustGrantTable {
    grants: Vec<TrustGrant>,
    receipts: Vec<TrustReceipt>,
}

impl TrustGrantTable {
    pub fn grant(&mut self, grant: TrustGrant) {
        if !self.grants.contains(&grant) {
            self.grants.push(grant);
        }
    }

    /// The authority for a commitment, if any (RootGrant preferred over
    /// OwnPackageDev when both exist -- the explicit grant retires the
    /// standing warning).
    pub fn authority(&self, commitment: &TrustCommitment) -> Option<&TrustProvenance> {
        let mut dev: Option<&TrustProvenance> = None;
        for grant in &self.grants {
            if grant.commitment != *commitment {
                continue;
            }
            match grant.provenance {
                TrustProvenance::RootGrant => return Some(&grant.provenance),
                TrustProvenance::OwnPackageDev => dev = Some(&grant.provenance),
            }
        }
        dev
    }

    /// Record an admission. Deduplicated by commitment (one receipt per
    /// commitment; the statement hash is re-checked by the lockfile layer).
    pub fn admit(&mut self, receipt: TrustReceipt) {
        if !self
            .receipts
            .iter()
            .any(|existing| existing.commitment == receipt.commitment)
        {
            self.receipts.push(receipt);
        }
    }

    pub fn receipts(&self) -> &[TrustReceipt] {
        &self.receipts
    }

    pub fn grants(&self) -> &[TrustGrant] {
        &self.grants
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_grant_outranks_dev_activity() {
        let commitment =
            TrustCommitment::SemanticDomainIntroduction(SemanticDomainId(4));
        let mut table = TrustGrantTable::default();
        table.grant(TrustGrant {
            commitment: commitment.clone(),
            provenance: TrustProvenance::OwnPackageDev,
        });
        assert_eq!(
            table.authority(&commitment),
            Some(&TrustProvenance::OwnPackageDev)
        );
        table.grant(TrustGrant {
            commitment: commitment.clone(),
            provenance: TrustProvenance::RootGrant,
        });
        assert_eq!(
            table.authority(&commitment),
            Some(&TrustProvenance::RootGrant)
        );
        assert_eq!(
            table.authority(&TrustCommitment::AcceptedFact("other".to_owned())),
            None
        );
    }

    #[test]
    fn receipts_deduplicate_by_commitment() {
        let commitment = TrustCommitment::ProgressProfile(ProgressProfileId(2));
        let mut table = TrustGrantTable::default();
        table.admit(TrustReceipt {
            commitment: commitment.clone(),
            statement_hash: 7,
            provenance: TrustProvenance::OwnPackageDev,
        });
        table.admit(TrustReceipt {
            commitment,
            statement_hash: 9,
            provenance: TrustProvenance::OwnPackageDev,
        });
        assert_eq!(table.receipts().len(), 1);
        assert_eq!(table.receipts()[0].statement_hash, 7);
    }
}
