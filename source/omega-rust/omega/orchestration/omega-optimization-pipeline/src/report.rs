use omega_optimization_validation::PrePhysicalOptimizationManifest;
use omega_regalloc::PostAllocationOptimizationManifest;

use crate::{
    FunctionRelativeOptimizationRealizationManifest, StagedOptimizedVerifiedPhysicalPipeline,
};

/// An auxiliary human projection request. This is deliberately independent of
/// the exact optimization selections and never participates in a decision.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OptimizationReportRequest {
    #[default]
    Suppressed,
    EmitHumanText,
}

impl OptimizationReportRequest {
    pub const fn emits_human_text(self) -> bool {
        matches!(self, Self::EmitHumanText)
    }
}

/// Compiler-owned cumulative custody for the optimization records available at
/// the current physical boundary. Each nested record remains independently
/// content-identified and replayable; this carrier grants no new publication
/// or emission authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationPipelineReport {
    pre_physical: PrePhysicalOptimizationManifest,
    post_allocation: PostAllocationOptimizationManifest,
    function_relative: Option<FunctionRelativeOptimizationRealizationManifest>,
}

impl OptimizationPipelineReport {
    pub const fn pre_physical(&self) -> &PrePhysicalOptimizationManifest {
        &self.pre_physical
    }

    pub const fn post_allocation(&self) -> &PostAllocationOptimizationManifest {
        &self.post_allocation
    }

    pub const fn function_relative(
        &self,
    ) -> Option<&FunctionRelativeOptimizationRealizationManifest> {
        self.function_relative.as_ref()
    }

    /// Project optional text after all semantic and physical decisions. The
    /// request cannot alter the structured carrier or the staged realization.
    pub fn render_human_text(&self, request: OptimizationReportRequest) -> Option<String> {
        if !request.emits_human_text() {
            return None;
        }
        let mut report = String::from("Omega optimization report\n\n[pre-physical]\n");
        report.push_str(&self.pre_physical.render_text());
        report.push_str("\n[post-allocation]\n");
        report.push_str(&self.post_allocation.render_text());
        if let Some(function_relative) = &self.function_relative {
            report.push_str("\n[function-relative realization]\n");
            report.push_str(&function_relative.render_text());
        }
        Some(report)
    }
}

pub fn optimization_pipeline_report(
    staged: &StagedOptimizedVerifiedPhysicalPipeline,
) -> OptimizationPipelineReport {
    let pre_physical = staged.pre_physical_manifest().record().clone();
    let post_allocation = staged.post_allocation_manifest().record().clone();
    let function_relative = staged
        .function_relative_manifest()
        .map(|manifest| manifest.record().clone());
    OptimizationPipelineReport {
        pre_physical,
        post_allocation,
        function_relative,
    }
}
