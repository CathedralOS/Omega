//! Connect an explicitly selected semantic-entry transport to Build custody.
//!
//! An ordinary provider conformance cannot choose a Build-bound declaration.
//! This separate edge keeps that choice beside canonical Terminal, so scope
//! selection and native call lowering consume the same exact continuation.
//! It forwards existing operands; declaration settlement introduces no roots.

use super::ValidatedNativeProgramEntrySettlement;
use crate::provider_admission::{
    TerminalCompilerBuiltinProposal, TerminalCompilerBuiltinSettlement,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use target_operations::CompilerBuiltinExecution;

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
        proposals: &[TerminalCompilerBuiltinProposal],
        plans: &[effects::provider_plan::ProviderPlan],
    ) -> Result<Option<Self>, String> {
        let mut selected = proposals.iter().filter(|proposal| {
            proposal.settlement()
                == TerminalCompilerBuiltinSettlement::Execution(
                    CompilerBuiltinExecution::ProgramStorageEntry,
                )
        });
        let Some(proposal) = selected.next() else {
            return Ok(None);
        };
        if selected.next().is_some() {
            return Err("semantic-entry transport has competing selected bindings".into());
        }
        let application = entry
            .semantic_calling_application()
            .ok_or("semantic-entry transport has no selected calling application")?;
        let plan = plans
            .get(proposal.provider_plan_index())
            .ok_or("semantic-entry transport lost its selected provider plan")?;
        let module = artifact.semantic_module();
        let boundary = module
            .boundary_machine(proposal.boundary())
            .ok_or("semantic-entry transport names an absent Terminal requirement")?;
        if entry.checked_entry().terminal_psi_identity() != artifact.manifest().semantic()
            || entry.target() != target::NativeTarget::uefi_x64()
            || entry.physical_adapter().is_none()
            || entry.source().target_slot().schema
                != target::ProgramEntrySchema::ProgramStorageApplication
            || application.requirement_identity != proposal.requirement_identity()
            || module.identities.text(boundary.identity) != Some(proposal.requirement_identity())
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
        // The generated ordinary caller owns a fresh local and its cleanup.
        // Its complete call closure therefore supplies frame demand before the
        // physical shell partitions the retained private stack. Never prepend
        // a fabricated pointer to the caller's two already-established roots.
        let callee = match entry.source().receiver() {
            program_entry_plan::ProgramEntryReceiverSignature::Free => callee,
            _ => {
                if entry.checked_entry().receiver_eligibility().is_none() {
                    return Err("semantic-entry context requires a checked ZII-valid value and exact retained parameter projection".into());
                }
                if callee != entry.checked_entry().terminal_entry() {
                    return Err(
                        "semantic-entry context bridge does not name the selected source member"
                            .into(),
                    );
                }
                let bridge = entry.checked_entry().context_bridge().ok_or(
                    "selected semantic-entry context has no checked activation-local bridge",
                )?;
                let machine = module.machine(bridge).ok_or(
                    "selected semantic-entry context has no checked activation-local bridge",
                )?;
                if !machine.parameters.is_empty()
                    || machine.structural_parameters.len() != 2
                    || machine.result != terminal_psi::TerminalMachineResult::Unit
                {
                    return Err("selected semantic-entry context bridge has incompatible visible parameters".into());
                }
                bridge
            }
        };
        Ok(Some(Self {
            boundary: boundary.id,
            entry: entry.checked_entry().clone(),
            callee,
            provider_plan: plan.identity_digest(),
        }))
    }
}
