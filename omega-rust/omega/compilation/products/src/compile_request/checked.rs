//! Target-local checking and Build inputs, admitted before source reuse or effects.

use super::ValidatedTargetCompilation;
use crate::RetainedCompilationChecks;
use diagnostics::Diagnostic;
use optimization_core::OptimizationRollback;
use package_compilation::{BuildExecutionInputs, PackageCompilationInputs};
use std::path::Path;
use target::TargetProfile;

/// Inputs to checked compilation, without native publication authority. Source
/// preparation belongs to the compiler session: every activation independently
/// admits its package custody, Build sponsors and occurrence-bound consent.
pub struct CheckedCompileRequest {
    pub root_path: std::path::PathBuf,
    /// `None` preserves targetless semantic checking.
    pub target_name: Option<String>,
    pub package_inputs: Option<PackageCompilationInputs>,
    /// Acquired physical source before Build declares its package identity.
    /// Mutually exclusive with a supplied package graph. No later live read may
    /// replace these bytes, including Product imports after Build returns.
    pub source_capture: Option<package_compilation::CapturedLocalSource>,
    /// Execution obligations move together and must precede all Build effects.
    pub build: BuildExecutionInputs,
    pub optimization_rollback: OptimizationRollback,
    /// A reused preparation keeps the timing policy under which it was created.
    pub collect_timings: bool,
}

/// Compiler-owned prior language judgments. Psi matches complete inputs before
/// reuse; these records cannot carry executor resources or package consent.
#[derive(Debug, Clone, Default)]
pub struct SettledCheckReuse {
    pub prior: RetainedCompilationChecks,
    pub retain: bool,
}

impl CheckedCompileRequest {
    pub fn new(root_path: &Path, target_name: Option<&str>) -> Self {
        Self {
            root_path: root_path.to_owned(),
            target_name: target_name.map(str::to_owned),
            package_inputs: None,
            source_capture: None,
            build: BuildExecutionInputs::default(),
            optimization_rollback: OptimizationRollback::default(),
            collect_timings: false,
        }
    }
}

/// One checked activation's policy and custody inputs, not authority produced by
/// checking. Product and Build profiles remain separate. A supplied consent
/// guard is consumed by Build execution, never by input admission or reuse.
pub struct CheckedExecutionRequest<'request> {
    pub selected_target_profile: Option<TargetProfile>,
    pub package_inputs: Option<&'request PackageCompilationInputs>,
    pub build: BuildExecutionInputs,
    pub optimization_rollback: OptimizationRollback,
    pub settled_check_reuse: SettledCheckReuse,
}

impl ValidatedTargetCompilation {
    /// Derive ordinary checking inputs without sponsors or discovery authority.
    /// An explicit snapshot narrows authority. Otherwise canonical package source
    /// metadata selects a fresh private capture with no invocation roster, matching
    /// package review's source custody; a binding without that index stays live.
    pub fn checked_execution<'request>(
        &'request self,
        build_directory: &Path,
    ) -> CheckedExecutionRequest<'request> {
        let package_inputs = self.package_inputs();
        CheckedExecutionRequest {
            selected_target_profile: self.profile(),
            package_inputs,
            build: BuildExecutionInputs {
                directory: Some(build_directory.to_path_buf()),
                snapshot: self.build_snapshot().cloned(),
                ..BuildExecutionInputs::default()
            }
            .resolve_compilation_defaults(package_inputs),
            optimization_rollback: self.optimization_rollback().clone(),
            settled_check_reuse: SettledCheckReuse::default(),
        }
    }
}

impl CheckedExecutionRequest<'_> {
    /// Admit generated package sources for this activation before assembling or
    /// reusing its frontend. Build-purpose packages must match the execution
    /// profile; product bundles must match the product target. Neither check
    /// grants Build execution or replaces the source checkpoint's custody check.
    pub fn admit_package_inputs(&self) -> Result<(), Vec<Diagnostic>> {
        let Some(inputs) = self.package_inputs else {
            return Ok(());
        };
        inputs.validate_execution_context(self.selected_target_profile, self.build.profile)
    }
}
