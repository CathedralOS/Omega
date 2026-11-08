//! Source-free work remaining after source admission and Terminal preparation.
//! Application Build owns native execution and reporting.

use crate::{
    CompileReportSettlement, NativeProductRequest, RetainedTerminalArtifact,
    SelectedIndependentComponent,
};
use optimization_core::OptimizationRollbackSettlement;
use terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy;

/// Owned input to application-sponsored native completion. No checked trees or
/// source continuation survive this boundary. The native driver still admits
/// the actual receiver and requires completed scalar inputs before emission.
pub struct PendingNativeProduct {
    pub terminal: RetainedTerminalArtifact,
    pub components: Result<Vec<SelectedIndependentComponent>, String>,
    pub accepted_package_permissions: TerminalAuthorityPermissionPolicy,
    pub request: NativeProductRequest,
    pub rollback: OptimizationRollbackSettlement,
    pub subsystem: u16,
    pub settlement: CompileReportSettlement,
}
