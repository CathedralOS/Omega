//! Connect an explicitly selected semantic-entry transport to Build custody.
//!
//! An ordinary provider conformance cannot choose a Build-bound declaration.
//! This separate edge keeps that choice beside canonical Terminal, so scope
//! selection and native call lowering consume the same exact continuation.
//! It forwards existing operands; declaration settlement introduces no roots.

use super::ValidatedNativeProgramEntrySettlement;
use effects::CompilerIntrinsicExecutionIdentity;
use effects::provider_plan::ProviderBinding;
use semantic_vocabulary::{BoundaryMachineId, MachineId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgramEntryCall {
    boundary: BoundaryMachineId,
    entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    callee: MachineId,
    provider_plan: effects::provider_plan::ProviderPlanDigest,
}

impl SelectedProgramEntryCall {
    pub const fn boundary(&self) -> BoundaryMachineId {
        self.boundary
    }
    pub const fn entry(&self) -> MachineId {
        self.callee
    }
    pub const fn source_signature(&self) -> [u8; 32] {
        self.entry.source_signature_identity()
    }
    pub const fn provider_plan(&self) -> effects::provider_plan::ProviderPlanDigest {
        self.provider_plan
    }

    pub(crate) fn bind(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        entry: &ValidatedNativeProgramEntrySettlement,
        selection: &effects::SelectedProviderPlanFacts,
        intrinsic_executions: &[Vec<Option<CompilerIntrinsicExecutionIdentity>>],
    ) -> Result<Option<Self>, String> {
        // Entry selection precedes executable-demand discovery. Borrow the
        // retained row identities here; native builtin proposals are derived
        // once, after that discovery, and admitted independently.
        let plans = selection.plans();
        if plans.len() != intrinsic_executions.len() {
            return Err("semantic-entry transport has misaligned provider plan custody".into());
        }
        let module = artifact.semantic_module();
        let mut entry_rows = Vec::new();
        for (plan, executions) in plans.iter().zip(intrinsic_executions) {
            if plan.rows.len() != executions.len() {
                return Err(format!(
                    "semantic-entry transport has misaligned intrinsic rows for `{}`",
                    plan.name,
                ));
            }
            for (row, execution) in plan.rows.iter().zip(executions) {
                if *execution != Some(CompilerIntrinsicExecutionIdentity::ProgramStorageEntry) {
                    continue;
                }
                if !matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }) {
                    return Err("semantic-entry transport has no selected intrinsic binding".into());
                }
                let requirement = plan.identities.text(row.requirement_identity);
                entry_rows.push((plan, requirement));
            }
        }
        if entry_rows.is_empty() {
            return Ok(None);
        }
        let mut selected = None;
        // Retained declarations are not calls. Follow actual Terminal
        // occurrences and preserve their boundary ID through the selected
        // family/specialization-to-provider-row relationship.
        for boundary_id in module.blocks().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|operation| match &operation.kind {
                    terminal_psi::OperationKind::BoundaryCall { boundary, .. } => Some(*boundary),
                    _ => None,
                })
                .chain(match &block.terminator {
                    terminal_psi::Terminator::ExternalCompletion { boundary, .. } => {
                        Some(*boundary)
                    }
                    _ => None,
                })
        }) {
            let boundary = module
                .boundary_machine(boundary_id)
                .ok_or("semantic-entry occurrence names an absent Terminal requirement")?;
            let requirement = module
                .identities
                .text(boundary.identity)
                .filter(|identity| !identity.is_empty())
                .ok_or("semantic-entry occurrence has no exact requirement identity")?;
            let mut matching_rows = entry_rows
                .iter()
                .copied()
                .filter(|(plan, row_requirement)| {
                    selection.selected_requirement_identity(requirement) == *row_requirement
                        && selection.selected_row_matches_requirement(
                            requirement,
                            row_requirement,
                            plan.report_fingerprint(),
                        )
                });
            let Some((plan, row_requirement)) = matching_rows.next() else {
                continue;
            };
            if matching_rows.next().is_some() {
                return Err("semantic-entry transport has competing selected provider rows".into());
            }
            if let Some((_, _, previous_boundary)) = selected {
                if previous_boundary != boundary_id {
                    return Err("semantic-entry transport has competing selected bindings".into());
                }
            }
            selected = Some((plan, row_requirement, boundary_id));
        }
        let Some((plan, requirement, boundary_id)) = selected else {
            return Ok(None);
        };
        let boundary = module
            .boundary_machine(boundary_id)
            .ok_or("semantic-entry transport lost its Terminal requirement")?;
        let application = entry
            .semantic_calling_application()
            .ok_or("semantic-entry transport has no selected calling application")?;
        if entry.checked_entry().terminal_psi_identity() != artifact.manifest().semantic()
            || entry.target() != target::NativeTarget::uefi_x64()
            || entry.physical_adapter().is_none()
            || entry.source().target_slot().schema
                != target::ProgramEntrySchema::ProgramStorageApplication
            || application.requirement_identity != requirement
            || !boundary.scalar_parameters.is_empty()
            || boundary.structural_parameters.len() != 2
            || boundary.result != terminal_psi::BoundaryMachineResult::Unit
        {
            return Err("semantic-entry transport does not join the exact selected artifact, requirement and entry shape".into());
        }
        let selections = terminal_installation::admit_target_machine_selections(
            artifact.view(),
            entry.source().target_slot().owner.target_name(),
        );
        let callee = selections
            .entry(
                artifact.view().identity(),
                module,
                terminal_psi::TargetMachineCallable::Machine(
                    entry.checked_entry().terminal_entry(),
                ),
            )
            .map_err(|reason| format!("semantic-entry target selection: {reason}"))?;
        // The selected source declaration receives exactly these two existing
        // roots. Application-local storage is ordinary authored code and its
        // frame demand already belongs to the checked call closure.
        terminal_semantics::validate_empty_erased_invocation_inputs(module, callee)?;
        Ok(Some(Self {
            boundary: boundary.id,
            entry: entry.checked_entry().clone(),
            callee,
            provider_plan: plan.identity_digest(),
        }))
    }
}
