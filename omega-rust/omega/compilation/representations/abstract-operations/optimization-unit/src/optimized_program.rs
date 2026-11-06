//! The optimized abstract program target lowering reads.
//!
//! Abstract optimization (Omega 01) publishes it beside its replay evidence:
//! the optimized plan and unit, the governing selections and budget, the
//! identity bundle, the validated projection's identities and the
//! pre-physical manifest. Target lowering (Omega 02) and the backends read
//! these rows without naming the optimization product.

use std::sync::Arc;

use abstract_operations::AbstractOperationPlan;
use optimization_core::{
    OptimizationIdentityBundle, OptimizationSelections, OptimizationUnitIdentity,
    OptimizationWorkBudget, OptimizedAbstractPlanProjectionIdentity,
};
use terminal_psi::TerminalPsiIdentity;

use crate::{PrePhysicalOptimizationManifest, PsiOptimizationUnit};

#[derive(Debug)]
pub struct OptimizedAbstractProgram {
    pub plan: Arc<AbstractOperationPlan>,
    pub unit: PsiOptimizationUnit,
    /// These machines retain the control-cycle analysis already validated
    /// against this unit. Body ownership replay uses that same context; the
    /// roster grants no native execution or new cycle admission.
    pub admitted_control_cycle_machines: Vec<semantic_vocabulary::MachineId>,
    pub selections: OptimizationSelections,
    pub budget_per_pass: OptimizationWorkBudget,
    pub identity_bundle: OptimizationIdentityBundle,
    /// The validated projection's identity, the Terminal program it projects
    /// and the final unit it ends at.
    pub projection: OptimizedAbstractPlanProjectionIdentity,
    pub projection_psi: TerminalPsiIdentity,
    pub final_unit: OptimizationUnitIdentity,
    pub pre_physical_manifest: PrePhysicalOptimizationManifest,
}

impl OptimizedAbstractProgram {
    pub fn plan(&self) -> &AbstractOperationPlan {
        &self.plan
    }

    pub fn shared_program(&self) -> Arc<AbstractOperationPlan> {
        Arc::clone(&self.plan)
    }

    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.unit
    }

    pub fn admitted_control_cycle_machines(&self) -> &[semantic_vocabulary::MachineId] {
        &self.admitted_control_cycle_machines
    }

    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }

    pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.budget_per_pass
    }

    pub const fn identity_bundle(&self) -> OptimizationIdentityBundle {
        self.identity_bundle
    }

    pub const fn projection(&self) -> OptimizedAbstractPlanProjectionIdentity {
        self.projection
    }

    pub const fn projection_psi(&self) -> TerminalPsiIdentity {
        self.projection_psi
    }

    pub const fn final_unit(&self) -> OptimizationUnitIdentity {
        self.final_unit
    }

    pub const fn pre_physical_manifest(&self) -> &PrePhysicalOptimizationManifest {
        &self.pre_physical_manifest
    }
}
