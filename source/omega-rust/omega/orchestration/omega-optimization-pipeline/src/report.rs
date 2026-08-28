use omega_optimization_validation::PrePhysicalOptimizationManifest;
use omega_regalloc::PostAllocationOptimizationManifest;

use crate::{
    FunctionFragmentEmissionManifest, FunctionFragmentObjectContainerManifest,
    FunctionFragmentTextSectionManifest, FunctionRelativeOptimizationRealizationManifest,
    OptimizedTerminalObjectArtifactManifest, StagedOptimizedVerifiedPhysicalPipeline,
    StagedValidatedOptimizedTerminalObjectArtifact,
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
    function_fragment: Option<FunctionFragmentEmissionManifest>,
    text_section: Option<FunctionFragmentTextSectionManifest>,
    object_container: Option<FunctionFragmentObjectContainerManifest>,
    terminal_object_artifact: Option<OptimizedTerminalObjectArtifactManifest>,
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

    pub const fn function_fragment(&self) -> Option<&FunctionFragmentEmissionManifest> {
        self.function_fragment.as_ref()
    }

    pub const fn text_section(&self) -> Option<&FunctionFragmentTextSectionManifest> {
        self.text_section.as_ref()
    }

    pub const fn object_container(&self) -> Option<&FunctionFragmentObjectContainerManifest> {
        self.object_container.as_ref()
    }

    pub const fn terminal_object_artifact(
        &self,
    ) -> Option<&OptimizedTerminalObjectArtifactManifest> {
        self.terminal_object_artifact.as_ref()
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
        if let Some(function_fragment) = &self.function_fragment {
            report.push_str("\n[function-fragment emission]\n");
            report.push_str(&format!("identity: {:?}\n", function_fragment.identity));
        }
        if let Some(text_section) = &self.text_section {
            report.push_str("\n[relocation-free text section]\n");
            report.push_str(&format!("identity: {:?}\n", text_section.identity));
        }
        if let Some(object_container) = &self.object_container {
            report.push_str("\n[relocation-free object container]\n");
            report.push_str(&format!("identity: {:?}\n", object_container.identity));
        }
        if let Some(artifact) = &self.terminal_object_artifact {
            report.push_str("\n[optimized Terminal object artifact]\n");
            report.push_str(&format!(
                "identity: {:?}\nexternal entry bridge: unavailable\nexecutable image: unavailable\ninstallation: unavailable\npublication: unavailable\n",
                artifact.identity
            ));
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
        function_fragment: None,
        text_section: None,
        object_container: None,
        terminal_object_artifact: None,
    }
}

/// Project cumulative records only from the opaque artifact carrier that owns every nested stage.
pub fn optimization_pipeline_report_from_terminal_object_artifact(
    staged: &StagedValidatedOptimizedTerminalObjectArtifact,
) -> OptimizationPipelineReport {
    let object_stage = staged.source();
    let text_stage = object_stage.source();
    let fragment_stage = text_stage.source();
    OptimizationPipelineReport {
        pre_physical: fragment_stage.pre_physical_manifest().record().clone(),
        post_allocation: fragment_stage.post_allocation_manifest().record().clone(),
        function_relative: Some(fragment_stage.function_relative_manifest().record().clone()),
        function_fragment: Some(fragment_stage.manifest().record().clone()),
        text_section: Some(text_stage.manifest().record().clone()),
        object_container: Some(object_stage.manifest().record().clone()),
        terminal_object_artifact: Some(staged.manifest().record().clone()),
    }
}
