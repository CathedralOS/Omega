//! Declaration scope of retained proof debt, not executable reachability.
//!
//! These edges survive source disposal and use the same membership walk as
//! checked-source closure. Target admission supplies settled provider edges.
//! Keeping a declaration here never grants an executable body or provider.

use super::{CheckedMachineDeclaration, TerminalIdentityIndex};
use arena::Arena;
use semantic_vocabulary::proof_applications::{
    ProofApplicationPlan, ProofApplications, ProofCallGraph,
};

/// One-based, module-local declaration coordinate. Zero is absence; source
/// arena positions and generations never enter this namespace.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProofDeclarationId(pub u32);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProofScopeDeclaration {
    #[default]
    Missing,
    Nominal(TerminalIdentityIndex),
    Callable(CheckedMachineDeclaration),
    Operator(super::BoundaryOperatorRequirement),
    /// Dormant symbolic metadata may lack a closed portable description.
    /// Keep its refusal without making it an executable or published supplier.
    Unavailable(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProofCallableAssociation {
    #[default]
    Missing,
    Callable {
        declaration: ProofDeclarationId,
        callable: super::TargetMachineCallable,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofApplicationScope {
    pub declarations: Arena<ProofScopeDeclaration>,
    /// Exact producer associations, not identities reconstructed from names.
    /// Declarations without emitted code remain in the proof plan only.
    pub callables: Arena<ProofCallableAssociation>,
    pub plan: ProofApplicationPlan<ProofDeclarationId>,
    pub calls: ProofCallGraph<ProofDeclarationId>,
}

impl ProofApplicationScope {
    pub fn callable_declaration(
        &self,
        callable: super::TargetMachineCallable,
    ) -> Option<ProofDeclarationId> {
        self.callables.iter().find_map(|(_, row)| match row {
            ProofCallableAssociation::Callable {
                declaration,
                callable: emitted,
            } if *emitted == callable => Some(*declaration),
            _ => None,
        })
    }

    pub fn applications(
        &self,
    ) -> ProofApplications<'_, ProofDeclarationId, ProofCallGraph<ProofDeclarationId>> {
        ProofApplications::new(&self.plan, &self.calls)
    }

    pub fn declaration(&self, identity: ProofDeclarationId) -> Option<&ProofScopeDeclaration> {
        let position = usize::try_from(identity.0.checked_sub(1)?).ok()?;
        self.declarations.storage_slice().get(position)
    }
}
