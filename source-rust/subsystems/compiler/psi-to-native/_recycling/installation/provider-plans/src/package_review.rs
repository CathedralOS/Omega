//! Source-free package inventory for one target. Review-only bindings can
//! describe a reusable package even when no executable entry reaches them.

use diagnostics::Diagnostic;
use evaluation_receipts::NativeBindingMaterialization;
use semantic_vocabulary::BoundaryMachineId;

pub use ::provider_demand::PackageReviewRequirements;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageTargetReview {
    /// Includes declarations reached through selected provider implementations.
    pub numeric: Result<Vec<ReviewedNumericBinding>, Vec<Diagnostic>>,
    pub bindings: Result<ReviewedNativeBindings, Vec<Diagnostic>>,
    /// Checked adapter candidates, in retained plan and binding order.
    pub provider_candidates: Result<Vec<ReviewedProviderRow>, Vec<Diagnostic>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedProviderRow {
    pub plan: String,
    pub requirement: String,
    pub candidates: Vec<(BoundaryMachineId, semantic_vocabulary::MachineId)>,
}

/// A numeric target leaf checked against the exact canonical declaration it implements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedNumericBinding {
    boundary: BoundaryMachineId,
    leaf: super::DirectBoundaryLeaf,
}

impl ReviewedNumericBinding {
    pub fn from_leaf(
        module: &terminal_psi::TerminalModule,
        target: target::TargetProfile,
        leaf: super::DirectBoundaryLeaf,
    ) -> Result<Self, String> {
        let declaration = leaf.bound_calling_declaration(module)?;
        if leaf.plan.target != target.target_name()
            || declaration.declaration.as_ref().is_none_or(|source| {
                source.execution != terminal_psi::CheckedBoundaryExecution::NumericPrimitive
            })
        {
            return Err("the numeric declaration's target realization".into());
        }
        Ok(Self {
            boundary: declaration.id,
            leaf,
        })
    }

    pub const fn boundary(&self) -> BoundaryMachineId {
        self.boundary
    }

    pub const fn leaf(&self) -> &super::DirectBoundaryLeaf {
        &self.leaf
    }
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
