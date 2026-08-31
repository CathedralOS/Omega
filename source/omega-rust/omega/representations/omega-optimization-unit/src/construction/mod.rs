//! Optimizer module role: executable entrance. Deterministically projects one abstract plan into a canonical optimization unit seed.

mod control_flow;
mod facts;
mod function;
mod provenance;
mod scalar_dataflow;
mod structural_custody;

use super::*;
use function::build_function;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationUnitBuildError {
    MissingBlocks(MachineId),
    FirstBlockDoesNotStartAtZero(MachineId),
    InvalidBlockOffset { machine: MachineId, offset: usize },
    DuplicateBlock(MachineId, BlockId),
    NodeIndexOverflow(MachineId),
    ParameterIndexOverflow(MachineId),
}

impl std::fmt::Display for OptimizationUnitBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot construct canonical Psi optimization unit: {self:?}"
        )
    }
}

impl std::error::Error for OptimizationUnitBuildError {}

/// Low-level deterministic projection from the clean lowering seed.
///
/// This is not an optimizer admission boundary: consumers that may transform
/// the unit must use the verified constructor owned by the Terminal-Psi
/// artifact boundary so the plan cannot detach from its verifier context.
pub fn reconstruct_psi_optimization_unit_seed(
    plan: &AbstractOperationPlan,
    fuel_schedule: FuelScheduleIdentity,
) -> Result<PsiOptimizationUnit, OptimizationUnitBuildError> {
    let functions = plan
        .functions
        .iter()
        .map(build_function)
        .collect::<Result<Vec<_>, _>>()?;
    let mut unit = PsiOptimizationUnit {
        identity: OptimizationUnitIdentity::from_canonical_bytes(b"pending canonical content"),
        psi: plan.psi,
        fuel_schedule,
        entry: plan.entry,
        structural_types: plan.structural_types.clone(),
        structural_domains: Arc::new([]),
        services: Arc::new([]),
        root_service_reach: TerminalRootServiceReach::default(),
        boundary_machines: plan.boundary_machines.clone(),
        provider_candidates: plan.provider_candidates.clone(),
        accepted_obligation_facts: Vec::new(),
        proof_questions: Vec::new(),
        ownership_frontier_facts: Vec::new(),
        pruned_machines: Vec::new(),
        functions,
    };
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    Ok(unit)
}
