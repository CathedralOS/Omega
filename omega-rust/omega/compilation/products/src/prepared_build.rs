//! Completed Build continuations and their exact-input reuse within one invocation.
//!
//! The coordinator admits each target's custody before lookup and executes every
//! missing frontend/Build step itself. Only successful continuations are retained;
//! this owner neither executes stages nor shares target-specific checking.

use crate::{BuildAdmission, ComputedBuildConfig};
use artifacts::compile_timings::CompileTimings;
use package_compilation::{BuildExecutionInputs, BuildSourceCustody};
use std::sync::Arc;

/// Evaluated Build and the retained frontend that an actual Product demand
/// completes. Pending authored bodies and generated sources grant no execution.
#[derive(Clone)]
pub struct BuiltTypedProgram {
    pub typing: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    pub pending_bodies: syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
    pub base_sources: Arc<source::SourceMap>,
    pub selected_target_machine_declarations:
        provider_planning::target_machines::SelectedTargetMachineDeclarations,
    pub pending_pre_checks: Vec<build_time_evaluation::PreCheckEvaluation>,
    pub computed_build_config: Arc<ComputedBuildConfig>,
    pub admission: BuildAdmission,
}

/// One completed continuation with its source custody and inclusive timing ladder.
#[derive(Clone)]
pub struct PreparedBuild {
    pub built: BuiltTypedProgram,
    pub build_sources: BuildSourceCustody,
    pub timings: CompileTimings,
}

/// Invocation-local retention keyed by the package owner's complete Build inputs.
/// Equal captured inputs share a result; live output directories remain distinct.
pub struct PreparedBuilds {
    builds: Vec<(BuildExecutionInputs, PreparedBuild)>,
    remaining_targets: usize,
}

impl PreparedBuilds {
    pub fn for_targets(target_count: usize) -> Self {
        Self {
            builds: Vec::new(),
            remaining_targets: target_count,
        }
    }

    /// Start one admitted target, returning an independent continuation on a hit.
    /// Call once before its frontend/Build work, including targets that later fail.
    pub fn begin_target(&mut self, inputs: &BuildExecutionInputs) -> Option<PreparedBuild> {
        self.remaining_targets = self.remaining_targets.saturating_sub(1);
        self.builds
            .iter()
            .find(|(existing, _)| existing == inputs)
            .map(|(_, prepared)| prepared.clone())
    }

    /// Retain a successful miss for later siblings. The final target does not
    /// clone a new continuation when no future target can consume it.
    pub fn retain_completed(&mut self, inputs: BuildExecutionInputs, prepared: &PreparedBuild) {
        if self.remaining_targets > 0 {
            self.builds.push((inputs, prepared.clone()));
        }
    }
}
