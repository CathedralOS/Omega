//! Target-local checking and Build inputs, admitted before source reuse or effects.

use super::ValidatedTargetCompilation;
use crate::RetainedCompilationChecks;
use diagnostics::Diagnostic;
use optimization_core::OptimizationRollback;
use package_compilation::build_execution::RestrictedBuildGrants;
use package_compilation::{BuildSnapshotRequest, PackageCompilationInputs};
use std::path::Path;
use target::TargetProfile;

/// Inputs to checked compilation, without native publication authority. Source
/// preparation belongs to the compiler session: every activation independently
/// admits its package custody, Build sponsors and occurrence-bound consent.
pub struct CheckedCompileRequest {
    pub root_path: std::path::PathBuf,
    /// `None` preserves targetless semantic checking.
    pub target_name: Option<String>,
    /// Build runs against this profile, separately from the product target.
    /// `None` selects the host, or stays unprofiled on an uncatalogued host.
    pub build_execution_profile: Option<TargetProfile>,
    pub package_inputs: Option<PackageCompilationInputs>,
    /// Acquired physical source before Build declares its package identity.
    /// Mutually exclusive with a supplied package graph. No later live read may
    /// replace these bytes, including Product imports after Build returns.
    pub source_capture: Option<package_compilation::CapturedLocalSource>,
    /// Writable staging, separate from immutable package snapshots.
    pub build_dir: Option<std::path::PathBuf>,
    pub filesystem_sponsor: Option<build_time_evaluation::BuildMachineFilesystemSponsor>,
    pub evaluation_sponsor: Option<build_time_evaluation::BuildEvaluationSponsor>,
    /// Captured immutable source and required sealed outputs for this activation.
    pub build_snapshot: Option<BuildSnapshotRequest>,
    pub optimization_rollback: OptimizationRollback,
    /// `None` reports restricted requests without executing them. Private
    /// issued snapshot/staging work projects none and continues normally.
    pub restricted_build_grants: Option<Box<dyn RestrictedBuildGrants>>,
    /// A reused preparation keeps the timing policy under which it was created.
    pub collect_timings: bool,
    /// Discovery leaves unsettled fields unestablished; the bound recompile must
    /// derive or diagnose them under its own inputs.
    pub permit_unsettled_fused_service_fields: bool,
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
            build_execution_profile: None,
            package_inputs: None,
            source_capture: None,
            build_dir: None,
            filesystem_sponsor: None,
            evaluation_sponsor: None,
            build_snapshot: None,
            optimization_rollback: OptimizationRollback::default(),
            restricted_build_grants: None,
            collect_timings: false,
            permit_unsettled_fused_service_fields: false,
        }
    }
}

/// One checked activation's policy and custody inputs, not authority produced by
/// checking. Product and Build profiles remain separate. A supplied consent
/// guard is consumed by Build execution, never by input admission or reuse.
pub struct CheckedExecutionRequest<'request> {
    pub selected_target_profile: Option<TargetProfile>,
    /// `None` admits the compiler host without a catalogued profile.
    pub build_execution_profile: Option<TargetProfile>,
    pub package_inputs: Option<&'request PackageCompilationInputs>,
    pub build_dir: Option<&'request Path>,
    pub filesystem_sponsor: Option<build_time_evaluation::BuildMachineFilesystemSponsor>,
    pub evaluation_sponsor: Option<build_time_evaluation::BuildEvaluationSponsor>,
    pub build_snapshot: Option<BuildSnapshotRequest>,
    pub optimization_rollback: OptimizationRollback,
    pub restricted_build_grants: Option<Box<dyn RestrictedBuildGrants>>,
    /// Discovery leaves unsettled Service fields unestablished. The bound
    /// recompile must derive or diagnose every field under its own inputs.
    pub permit_unsettled_fused_service_fields: bool,
    pub settled_check_reuse: SettledCheckReuse,
}

impl ValidatedTargetCompilation {
    /// Derive ordinary checking inputs without sponsors or discovery authority.
    /// An explicit snapshot narrows authority. Otherwise canonical package source
    /// metadata selects a fresh private capture with no invocation roster, matching
    /// package review's source custody; a binding without that index stays live.
    pub fn checked_execution<'request>(
        &'request self,
        build_dir: &'request Path,
    ) -> CheckedExecutionRequest<'request> {
        let package_inputs = self.package_inputs();
        CheckedExecutionRequest {
            selected_target_profile: self.profile(),
            build_execution_profile: TargetProfile::host_if_supported(),
            package_inputs,
            build_dir: Some(build_dir),
            filesystem_sponsor: None,
            evaluation_sponsor: None,
            // Explicit custody always wins; failed admission never falls back.
            build_snapshot: self
                .build_snapshot()
                .cloned()
                .or_else(|| BuildSnapshotRequest::automatic_package_inventory(package_inputs)),
            optimization_rollback: self.optimization_rollback().clone(),
            restricted_build_grants: None,
            permit_unsettled_fused_service_fields: false,
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
        inputs
            .validate_execution_context(self.selected_target_profile, self.build_execution_profile)
    }
}
