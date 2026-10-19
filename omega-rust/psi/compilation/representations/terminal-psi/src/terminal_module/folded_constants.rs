//! Historical selected-realization applications consumed by constant folding.
//! These are immutable semantic compatibility facts, never installation requests.

use super::{
    NominalDeclarationIdentity, TargetMachineCallable, TerminalIdentityIndex, TerminalModule,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FoldedConstantClosure {
    pub target: Option<String>,
    pub dependencies: Vec<FoldedConstantDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FoldedConstantDependency {
    pub target: String,
    pub requirement: TerminalIdentityIndex,
    pub plan: [u8; 32],
    /// Exact application in this artifact's checked candidate catalog. The
    /// artifact identity commits its body, entry, contracts and dependencies.
    /// None denotes the existing sealed primitive intrinsic route only.
    pub candidate: Option<FoldedConstantCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FoldedConstantCandidate {
    pub boundary: BoundaryMachineId,
    pub machine: MachineId,
    pub declaration: NominalDeclarationIdentity,
}

impl FoldedConstantClosure {
    /// Reconstruct primary normalized-plan commitments from their complete
    /// canonical inputs using the shared Psi SHA-256 encoder. Artifact digest
    /// fields alone cannot satisfy this receiving compatibility obligation.
    pub fn validate_plan_inputs(
        &self,
        target: &str,
        inputs: &[Vec<u8>],
    ) -> Result<(), &'static str> {
        if self.target.as_deref() != Some(target) {
            return Err("folded constant closure belongs to another target");
        }
        let expected: std::collections::BTreeSet<_> = self
            .dependencies
            .iter()
            .filter(|dependency| dependency.target == target)
            .map(|dependency| dependency.plan)
            .collect();
        if inputs.len() != expected.len() {
            return Err("folded constant selected realization closure is incomplete or repeated");
        }
        let supplied: std::collections::BTreeSet<_> = inputs
            .iter()
            .map(|preimage| {
                // The existing v4 domain is part of the canonical input, exactly
                // as captured by the normalized plan's authoritative encoder.
                let mut digest = crate::digest_encoder::DomainDigestEncoder::new(b"");
                digest.raw(preimage);
                digest.finish()
            })
            .collect();
        if supplied != expected || supplied.len() != inputs.len() {
            return Err(
                "folded constant normalized plan input differs from its retained commitment",
            );
        }
        Ok(())
    }

    /// Check every historical scope against its own retained exact candidate.
    /// Supplemental proof applications remain provenance, never installations
    /// under the primary execution target.
    pub fn validate(&self, module: &TerminalModule) -> Result<(), &'static str> {
        if self.dependencies.is_empty() {
            return if self.target.is_none() {
                Ok(())
            } else {
                Err("folded constant target has no realization dependencies")
            };
        }
        if self.target.as_deref().is_none_or(str::is_empty) {
            return Err("folded constant dependencies have no primary target closure");
        }
        if self.dependencies.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("folded constant dependencies are repeated or out of canonical order");
        }
        for dependency in &self.dependencies {
            if dependency.target.is_empty()
                || dependency.plan == [0; 32]
                || module
                    .identities
                    .text(dependency.requirement)
                    .is_none_or(str::is_empty)
            {
                return Err("folded constant dependency has incomplete realization identity");
            }
            let Some(association) = &dependency.candidate else {
                continue;
            };
            let count = module
                .provider_candidates
                .iter()
                .filter(|candidate| {
                    candidate.boundary == association.boundary
                        && candidate.candidate == association.machine
                        && candidate.provider_declaration == association.declaration
                        && candidate.source_requirement_identity == dependency.requirement
                })
                .count();
            if count != 1 {
                return Err(
                    "folded constant dependency lost its exact checked candidate association",
                );
            }
            let callable = TargetMachineCallable::Machine(association.machine);
            for family in &module.target_machine_families {
                if family
                    .members
                    .iter()
                    .any(|member| member.callable == callable)
                    && !family.members.iter().any(|member| {
                        member.callable == callable && member.target == dependency.target
                    })
                {
                    return Err("folded constant candidate belongs to another target scope");
                }
            }
        }
        Ok(())
    }
}
