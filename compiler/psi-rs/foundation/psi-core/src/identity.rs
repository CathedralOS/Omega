use std::fmt;
use std::num::NonZeroU64;

/// Common behavior for nonzero semantic identities carried by terminal Psi.
pub trait PsiSemanticId: Copy + Eq + Ord + std::hash::Hash {
    fn new(raw: u64) -> Option<Self>;
    fn get(self) -> u64;
}

macro_rules! semantic_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(NonZeroU64);

        impl $name {
            pub fn new(raw: u64) -> Option<Self> {
                NonZeroU64::new(raw).map(Self)
            }

            pub const fn get(self) -> u64 {
                self.0.get()
            }
        }

        impl PsiSemanticId for $name {
            fn new(raw: u64) -> Option<Self> {
                $name::new(raw)
            }

            fn get(self) -> u64 {
                $name::get(self)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}", self.0)
            }
        }
    };
}

semantic_id!(ValueId, "Stable identity of one terminal-Psi value.");
semantic_id!(MachineId, "Stable identity of one terminal-Psi machine.");
semantic_id!(BlockId, "Stable identity of one terminal-Psi block.");
semantic_id!(
    PlaceId,
    "Stable identity of one terminal-Psi structural place."
);
semantic_id!(
    OperationId,
    "Stable identity of one terminal-Psi operation."
);
semantic_id!(
    EdgeId,
    "Stable identity of one terminal-Psi control-flow edge."
);
semantic_id!(
    PropositionId,
    "Stable identity of one proposition in a terminal-Psi semantic module."
);
semantic_id!(
    ContractId,
    "Stable identity of one author contract in a terminal-Psi semantic module."
);
semantic_id!(
    ObligationId,
    "Stable identity of one verifier-reconstructed obligation."
);
semantic_id!(
    AdmissionSiteId,
    "Stable identity of one semantic-module-authorized admission site."
);
semantic_id!(
    EvidenceIdentity,
    "Stable identity of proof, provider, or admission evidence outside semantic identity."
);
semantic_id!(
    ProfileDecisionId,
    "Stable identity of an installation profile's acceptance decision."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_ids_reject_the_reserved_zero_value() {
        assert!(ValueId::new(0).is_none());
        assert_eq!(ValueId::new(7).expect("nonzero identity").get(), 7);
    }
}
