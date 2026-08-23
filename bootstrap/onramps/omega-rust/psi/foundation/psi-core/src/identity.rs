use std::fmt;
use std::num::NonZeroU32;
use std::num::NonZeroU64;

/// Identity of the current logical-cost schedule, independent from terminal
/// Psi semantics. The schedule implementation lives above this dependency-
/// light identity so installation/resource records can name its units without
/// depending on a semantic evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FuelScheduleIdentity(NonZeroU32);

impl FuelScheduleIdentity {
    pub const fn new(marker: u32) -> Option<Self> {
        match NonZeroU32::new(marker) {
            Some(marker) => Some(Self(marker)),
            None => None,
        }
    }

    pub const fn marker(self) -> u32 {
        self.0.get()
    }
}

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
semantic_id!(
    StructuralTypeId,
    "Stable identity of one concrete instantiated structural type in terminal Psi."
);
semantic_id!(
    StructuralFieldId,
    "Stable identity of one ordered field in a terminal-Psi structural type."
);
semantic_id!(
    StructuralCaseId,
    "Stable identity of one ordered case in a terminal-Psi structural sum type."
);
semantic_id!(
    StructuralDomainId,
    "Stable identity of one structural qualification domain in terminal Psi."
);
semantic_id!(
    DomainSemanticId,
    "Source-handle-free identity of one semantic qualification domain."
);
semantic_id!(
    ServiceId,
    "Stable identity of one boundary-service declaration in terminal Psi."
);
semantic_id!(
    BoundaryMachineId,
    "Stable identity of one target-neutral boundary-machine declaration in terminal Psi."
);
semantic_id!(MachineId, "Stable identity of one terminal-Psi machine.");
semantic_id!(BlockId, "Stable identity of one terminal-Psi block.");
semantic_id!(
    PlaceId,
    "Stable identity of one terminal-Psi structural place."
);
semantic_id!(
    ClaimId,
    "Stable machine-local identity of one owned claim in a terminal-Psi claim frontier."
);
semantic_id!(
    ContentDomainId,
    "Stable identity of one exact content-bearing semantic domain."
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
    EvidenceTermId,
    "Stable identity of one erased evidence term in a terminal-Psi semantic module."
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
        assert!(ClaimId::new(0).is_none());
    }

    #[test]
    fn fuel_schedule_identity_is_nonzero_and_separate_from_semantic_ids() {
        assert_eq!(FuelScheduleIdentity::new(0), None);
        assert_eq!(
            FuelScheduleIdentity::new(3)
                .expect("nonzero fuel schedule")
                .marker(),
            3
        );
    }
}
