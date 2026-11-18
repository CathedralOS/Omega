//! Source-free package inventory for one target. Review-only bindings can
//! describe a reusable package even when no executable entry reaches them.

use diagnostics::Diagnostic;
use evaluation_receipts::NativeBindingMaterialization;
use semantic_vocabulary::BoundaryMachineId;

pub struct PackageReviewRequirements {
    pub numeric: Result<Vec<BoundaryMachineId>, Vec<Diagnostic>>,
    pub bindings: Result<Vec<(BoundaryMachineId, NativeBindingMaterialization)>, Vec<Diagnostic>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageTargetReview {
    /// Includes declarations reached through selected provider implementations.
    pub numeric: Result<Vec<super::DirectBoundaryLeaf>, Vec<Diagnostic>>,
    pub bindings: Result<ReviewedNativeBindings, Vec<Diagnostic>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedNativeBindings {
    pub identities: terminal_psi::TerminalIdentityTable,
    /// In the same order as the Build supplied the bindings for this target.
    pub rows: Vec<ReviewedNativeBinding>,
    pub calling: Result<Vec<calling_conventions::BoundaryCallingApplication>, Vec<Diagnostic>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedNativeBinding {
    pub boundary: BoundaryMachineId,
    pub materialization: NativeBindingMaterialization,
    pub binding: effects::provider_plan::EvaluatedNativeBinding,
}
