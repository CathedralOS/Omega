use crate::{NominalDeclarationIdentity, TerminalIdentityIndex, TerminalModule};
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
    pub candidate: Option<FoldedConstantCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FoldedConstantCandidate {
    pub boundary: BoundaryMachineId,
    pub machine: MachineId,
    pub declaration: NominalDeclarationIdentity,
}

impl FoldedConstantClosure {
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
            .map(|preimage| crate::digest_encoder::content_identity(preimage))
            .collect();
        if supplied != expected || supplied.len() != inputs.len() {
            return Err(
                "folded constant normalized plan input differs from its retained commitment",
            );
        }
        Ok(())
    }

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
                .interfaces.provider_candidates
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
        }
        Ok(())
    }
}
