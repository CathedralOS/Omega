//! Inert review of an exact runtime assumption, not provider permission.

use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;
use trust_model::AssumedBindingRelationship;

/// Compiler-produced relationship plus the compact readable risk meaning.
/// Text explains the committed structures; it never selects declarations or
/// supplies missing evidence. Unknown producer commitments must not be filled
/// from these descriptions. Refutations retain their exact conflicting facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewAssumedBindingContract {
    pub relationship: AssumedBindingRelationship,
    pub required_contract: String,
    pub supplied_contract: String,
    pub selected_implementation: String,
    pub assumption_difference: String,
    pub authority_introduced: String,
    pub root_scope: String,
    pub dependency_path: String,
    pub demonstrated_contradictions: Vec<String>,
}

/// One receiving Product occurrence's exact assumption obligations. The
/// authenticated review owner supplies this package and target; a dependency's
/// own acceptance does not authorize its consumer. No Build execution grant is
/// represented here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyAssumedBindingContracts {
    package: PackageKeyIdentity,
    target: TargetProfile,
    relationships: Vec<PackageReviewAssumedBindingContract>,
}

impl PackagePolicyAssumedBindingContracts {
    pub fn new(
        package: PackageKeyIdentity,
        target: TargetProfile,
        mut relationships: Vec<PackageReviewAssumedBindingContract>,
    ) -> Self {
        relationships.sort_unstable_by_key(|value| value.relationship.digest());
        Self {
            package,
            target,
            relationships,
        }
    }

    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn target(&self) -> TargetProfile {
        self.target
    }

    pub fn relationships(&self) -> &[PackageReviewAssumedBindingContract] {
        &self.relationships
    }

    pub(crate) fn validate_canonical_structure(&self) -> Result<(), &'static str> {
        if self
            .relationships
            .windows(2)
            .any(|values| values[0].relationship.digest() >= values[1].relationship.digest())
        {
            return Err("assumed binding contracts contain duplicate relationship identities");
        }
        Ok(())
    }
}
