//! Retained source judgments, consumed without re-entering source proof.
//!
//! Entry and exit claims remain in their checked issuer's coordinate space.
//! This view supplies no receiving consent, complete dependency closure or
//! executable authority; realization retains its independent compatibility.

use super::declarations::{DeclaredRequirement, PublicRequirement};
pub use checked_trees::{
    AssumedBindingContractSide, AssumedBindingDirectionalObligations, AssumedBindingPublicClaim,
    AssumedBindingResultCase, AssumedBindingSemanticConnection, AssumedBindingSemanticDirection,
    AssumedBindingSemanticJudgment, AssumedBindingSemanticObligation, AssumedBindingSemanticSource,
    AssumedBindingSemanticSubject,
};
use checked_trees::{CheckedPublicContractOwner, ContractProofFactOwner};

pub(super) fn owns_contract(
    declaration: &DeclaredRequirement<'_>,
    owner: ContractProofFactOwner,
) -> bool {
    let declaration = match declaration.declaration {
        PublicRequirement::Trait {
            definition,
            signature,
        } => CheckedPublicContractOwner::Trait {
            owner: definition.symbol,
            signature: signature.symbol,
        },
        PublicRequirement::Machine { declaration, entry } => CheckedPublicContractOwner::Machine {
            machine: declaration.symbol,
            entry: entry.symbol,
        },
    };
    declaration.owns_contract(owner)
}
