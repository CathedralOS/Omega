//! Immutable optimization-unit aggregate and its exact carrier-family map.

mod attachment;
mod cycles;
mod graph;
mod manifest;
mod ownership;
mod proof;
mod range;

use super::*;

pub use attachment::*;
pub use cycles::*;
pub use graph::*;
pub use manifest::*;
pub use ownership::*;
pub use proof::*;
pub use range::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationUnit {
    pub identity: OptimizationUnitIdentity,
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub entry: MachineId,
    /// Target-neutral module declarations needed by layout, ABI, and checked
    /// provider installation after the full Terminal module is discarded.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    /// Exact verifier-owned qualification-domain catalog. Bare lowering seeds
    /// leave this empty; optimizer admission attaches it before rewrites run.
    pub structural_domains: Arc<[StructuralDomainDeclaration]>,
    /// Exact verifier-owned boundary-service hierarchy. Bare lowering seeds
    /// leave this empty; optimizer admission attaches the complete catalog so
    /// call reach and concrete service effects remain independently replayable.
    pub services: Arc<[ServiceDeclaration]>,
    /// Exact current-revision closure of services executable from `entry`.
    /// Unlike declaration custody, this derived row may narrow when a checked
    /// rewrite removes an unreachable call or concrete service effect.
    pub root_service_reach: TerminalRootServiceReach,
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    pub accepted_obligation_facts: Vec<AcceptedObligationFact>,
    /// Complete immutable verifier proof-question roster in reconstruction
    /// order. This is source-site authority, not a function-wide range index.
    pub proof_questions: Vec<ProofQuestion>,
    /// Immutable verifier projection, absent only on low-level bare seeds that
    /// are not authorized optimizer inputs.
    pub ownership_frontier_facts: Vec<OwnershipFrontierFact>,
    /// Canonical custody for source functions removed by independently proven
    /// whole-program reachability rewrites. Source ordinals bind each removed
    /// machine to the immutable verified Terminal-Psi function roster.
    pub pruned_machines: Vec<PrunedMachineCustody>,
    pub functions: Vec<PsiOptimizationFunction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrunedMachineCustody {
    pub machine: MachineId,
    pub source_ordinal: u32,
}
