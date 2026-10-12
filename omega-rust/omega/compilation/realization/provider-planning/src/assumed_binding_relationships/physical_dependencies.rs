//! Reached physical supplier rows after ordinary execution settlement.
//!
//! Preparation retains selected source rows before compiler intrinsic identity
//! production. This join takes those original coordinates and the actual
//! settled row, preserving the sealed import receipt or closed execution
//! identity. It does not replay provider selection, verify native artifacts,
//! or grant installation or receiving consent.

use arena::Arena;
use diagnostics::Diagnostic;
use effects::CompilerIntrinsicExecutionIdentity;
use effects::provider_plan::{ProviderBinding, ProviderPlanDigest};
use symbols::SymbolHandle;
use target::TargetProfile;

use super::{AssumedBindingSupplierSelection, PreparedAssumedBindingRelationship};
use crate::SelectedProviderReviewProvenance;

/// Original physical binding evidence under one exact selected target.
/// Construction is private to the ordinary post-settlement retention join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumedBindingPhysicalProviderEvidence {
    selection: AssumedBindingSupplierSelection,
    selected_plan: ProviderPlanDigest,
    target: TargetProfile,
    requirement: SymbolHandle,
    realization: SymbolHandle,
    binding: ProviderBinding,
    compiler_intrinsic_execution: Option<CompilerIntrinsicExecutionIdentity>,
}

impl AssumedBindingPhysicalProviderEvidence {
    pub fn selection(&self) -> AssumedBindingSupplierSelection {
        self.selection
    }
    pub fn selected_plan(&self) -> ProviderPlanDigest {
        self.selected_plan
    }
    pub fn target(&self) -> TargetProfile {
        self.target
    }
    pub fn requirement(&self) -> SymbolHandle {
        self.requirement
    }
    /// Zero is retained for a genuine toolchain-settled row; it never names
    /// an inferred source implementation.
    pub fn realization(&self) -> SymbolHandle {
        self.realization
    }
    /// Imports keep their original evaluated foreign locator and receipt.
    /// Syscall number and dynamic table coordinates keep their existing
    /// binding vocabulary; this record supplies the actual target scope.
    pub fn binding(&self) -> &ProviderBinding {
        &self.binding
    }
    pub fn compiler_intrinsic_execution(&self) -> Option<CompilerIntrinsicExecutionIdentity> {
        self.compiler_intrinsic_execution
    }
}

/// Arena slot with an explicit absence state. The arena's zero/dummy slot
/// cannot fabricate a target, provider binding or physical evidence claim.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssumedBindingPhysicalProviderDependency {
    evidence: Option<AssumedBindingPhysicalProviderEvidence>,
}

impl AssumedBindingPhysicalProviderDependency {
    pub fn evidence(&self) -> Option<&AssumedBindingPhysicalProviderEvidence> {
        self.evidence.as_ref()
    }
}

/// Retain the actual physical evidence for every reached selected row.
/// Invoke once after ordinary compiler-intrinsic identity production. Empty
/// evidence means all reached rows are checked code; absence on a prepared
/// relationship means this producer has not run. Publish the batch only when
/// every original coordinate has its corresponding settled owner.
pub fn retain_assumed_binding_physical_dependencies(
    relationships: &mut [PreparedAssumedBindingRelationship],
    selected: &[SelectedProviderReviewProvenance],
    target: Option<TargetProfile>,
) -> Result<(), Vec<Diagnostic>> {
    let retained = relationships
        .iter()
        .map(|relationship| capture(relationship, selected, target))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|diagnostic| vec![diagnostic])?;
    for (relationship, evidence) in relationships.iter_mut().zip(retained) {
        relationship.physical_dependencies = Some(evidence);
    }
    Ok(())
}

fn capture(
    relationship: &PreparedAssumedBindingRelationship,
    selected: &[SelectedProviderReviewProvenance],
    target: Option<TargetProfile>,
) -> Result<Arena<AssumedBindingPhysicalProviderDependency>, Diagnostic> {
    let mut evidence = Arena::default();
    for selection in relationship.supplier_dependencies().selected_rows() {
        let original = relationship
            .dependency_supplier(*selection)
            .ok_or_else(|| {
                Diagnostic::error("assumed physical dependency lost its original selected owner")
            })?;
        let original_row = original
            .plan
            .rows
            .get(selection.row_ordinal)
            .ok_or_else(|| {
                Diagnostic::error("assumed physical dependency lost its original selected row")
            })?;
        if matches!(original_row.binding, ProviderBinding::CheckedAdapter { .. }) {
            continue;
        }
        let settled = selected.get(selection.provider_ordinal).ok_or_else(|| {
            Diagnostic::error("assumed physical dependency has no settled selected owner")
        })?;
        let row = settled
            .plan
            .rows
            .get(selection.row_ordinal)
            .ok_or_else(|| {
                Diagnostic::error("assumed physical dependency has no settled selected row")
            })?;
        let selected_plan = original.plan.identity_digest();
        if settled.plan.identity_digest() != selected_plan {
            return Err(Diagnostic::error(
                "assumed physical dependency names a different settled provider plan",
            ));
        }
        let requirement = exact_coordinate(
            &original.provider.row_requirements,
            &settled.provider.row_requirements,
            selection.row_ordinal,
        )?;
        let realization = exact_coordinate(
            &original.provider.row_realizations,
            &settled.provider.row_realizations,
            selection.row_ordinal,
        )?;
        // Source target-declaration custody is optional by binding family,
        // but its original row must still exist and survive this join.
        let original_origin = original
            .provider
            .row_target_machine_origins
            .get(selection.row_ordinal)
            .ok_or_else(|| {
                Diagnostic::error("assumed physical dependency lost target declaration custody")
            })?;
        if settled
            .provider
            .row_target_machine_origins
            .get(selection.row_ordinal)
            != Some(original_origin)
        {
            return Err(Diagnostic::error(
                "assumed physical dependency changed target declaration custody",
            ));
        }
        let target = target.ok_or_else(|| {
            Diagnostic::error("assumed physical dependency has no selected realization target")
        })?;
        if (!settled.plan.target.is_empty() && settled.plan.target != target.target_name())
            || matches!(&row.binding, ProviderBinding::Import { evaluated }
                if evaluated.locator().target() != target)
        {
            return Err(Diagnostic::error(
                "assumed physical dependency belongs to a different selected target",
            ));
        }
        let compiler_intrinsic_execution = *settled
            .row_compiler_intrinsic_executions
            .get(selection.row_ordinal)
            .ok_or_else(|| {
                Diagnostic::error("assumed physical dependency has no settled execution row")
            })?;
        if matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. })
            != compiler_intrinsic_execution.is_some()
        {
            return Err(Diagnostic::error(
                "assumed physical dependency has no exact compiler intrinsic execution identity",
            ));
        }
        evidence.append(AssumedBindingPhysicalProviderDependency {
            evidence: Some(AssumedBindingPhysicalProviderEvidence {
                selection: *selection,
                selected_plan,
                target,
                requirement,
                realization,
                binding: row.binding.clone(),
                compiler_intrinsic_execution,
            }),
        });
    }
    Ok(evidence)
}

fn exact_coordinate(
    original: &[SymbolHandle],
    settled: &[SymbolHandle],
    ordinal: usize,
) -> Result<SymbolHandle, Diagnostic> {
    let original = original.get(ordinal).ok_or_else(|| {
        Diagnostic::error("assumed physical dependency lost its original declaration coordinate")
    })?;
    if settled.get(ordinal) != Some(original) {
        return Err(Diagnostic::error(
            "assumed physical dependency changed its settled declaration coordinate",
        ));
    }
    Ok(*original)
}
