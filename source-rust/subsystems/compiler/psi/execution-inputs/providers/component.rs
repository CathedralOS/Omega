//! Match a checked receiving boundary to a selected provider in a separately
//! published artifact. Keep artifact-local membership mappings with the selection.

mod memberships;

use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_codec::{TerminalArtifactView, TerminalExecutionIdentity, TerminalExecutionView};
use terminal_psi::TerminalPsiIdentity;
use terminal_semantics::synchronization::{
    NormalEffects, PrivateWaitFailure, ProgressScope, SelectedCall, SpinWait, component_normal_effects,
    component_spin_wait, private_wait_failure,
};

use crate::{
    AdmittedProviderSelections, ProviderInstallationSelection, admit_provider_installation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedComponentProvider {
    receiver: TerminalExecutionIdentity,
    receiving_boundary: BoundaryMachineId,
    provider: TerminalPsiIdentity,
    provider_boundary: BoundaryMachineId,
    candidate: MachineId,
    selection: AdmittedProviderSelections,
    progress: ImportProgress,
    input_memberships: memberships::InputMemberships,
    result_memberships: memberships::Memberships,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ImportProgress {
    Closed,
    Waiting(SpinWait),
    Effects(std::sync::Arc<NormalEffects>),
    Unresolved,
}

impl AdmittedComponentProvider {
    pub const fn receiver(&self) -> &TerminalExecutionIdentity {
        &self.receiver
    }
    pub const fn receiving_boundary(&self) -> BoundaryMachineId {
        self.receiving_boundary
    }
    pub const fn provider(&self) -> TerminalPsiIdentity {
        self.provider
    }
    pub const fn provider_boundary(&self) -> BoundaryMachineId {
        self.provider_boundary
    }
    pub const fn candidate(&self) -> MachineId {
        self.candidate
    }
    pub const fn selection(&self) -> &AdmittedProviderSelections {
        &self.selection
    }

    /// A nested invocation may rely on its caller only when the receiving
    /// judgment checked the exact projected input obligation.
    pub fn invocation_scope(&self) -> ProgressScope {
        match self.progress {
            ImportProgress::Waiting(_) => ProgressScope::OpenInterface,
            ImportProgress::Effects(ref effects) if effects.requires_progress() => {
                ProgressScope::OpenInterface
            }
            _ => ProgressScope::Executable,
        }
    }

    pub fn input_membership(
        &self,
        input: usize,
        domain: semantic_vocabulary::StructuralDomainId,
    ) -> Option<semantic_vocabulary::StructuralDomainId> {
        self.input_memberships
            .get(input)?
            .iter()
            .find(|(source, _)| *source == domain)
            .map(|(_, target)| *target)
    }
    pub fn result_membership(
        &self,
        domain: semantic_vocabulary::StructuralDomainId,
    ) -> Option<semantic_vocabulary::StructuralDomainId> {
        self.result_memberships
            .iter()
            .find(|(source, _)| *source == domain)
            .map(|(_, target)| *target)
    }
}

pub fn admit_component_provider<'receiver>(
    receiver: impl Into<TerminalExecutionView<'receiver>>,
    receiving_boundary: BoundaryMachineId,
    provider: TerminalArtifactView<'_>,
    selected: &ProviderInstallationSelection,
) -> Result<AdmittedComponentProvider, String> {
    let receiver = receiver.into();
    let receiving = receiver
        .module()
        .interfaces
        .boundary_machines
        .iter()
        .find(|row| row.id == receiving_boundary)
        .ok_or("component import names no receiving boundary")?;
    let supplying = provider
        .module()
        .interfaces
        .boundary_machines
        .iter()
        .find(|row| row.id == selected.boundary)
        .ok_or("component export names no producing boundary")?;
    let left = &receiver.module().identities;
    let right = &provider.module().identities;
    if !left
        .text(receiving.identity)
        .zip(right.text(supplying.identity))
        .is_some_and(|(left, right)| !left.is_empty() && left == right)
        || !receiving
            .owner
            .same_identity_as(left, &supplying.owner, right)
    {
        return Err("component import/export name different requirement declarations".into());
    }
    let (Some(receiving_contract), Some(supplying_contract)) =
        (&receiving.declaration, &supplying.declaration)
    else {
        return Err("component import/export lacks a checked callable contract".into());
    };
    if !receiving_contract.same_identity_as(
        receiver.module(),
        supplying_contract,
        provider.module(),
    ) || receiving.completion != supplying.completion
        || receiving.unconditional_termination != supplying.unconditional_termination
        || receiving.parameter_order != supplying.parameter_order
        || receiving.scalar_parameters != supplying.scalar_parameters
    {
        return Err("component import/export checked callable contracts differ".into());
    }
    if !terminal_semantics::boundary_result_lifetimes_match(
        receiver.module(),
        receiving,
        provider.module(),
        supplying,
    )
    .map_err(str::to_owned)?
    {
        return Err("component import/export declared result lifetimes differ".into());
    }
    let selection = admit_provider_installation(provider, std::slice::from_ref(selected))
        .map_err(|error| format!("component export provider admission: {error:?}"))?;
    let input_memberships = memberships::admit_input_memberships(
        receiver.module(),
        receiving,
        provider.module(),
        supplying,
    )?;
    let result_memberships = memberships::admit_result_memberships(
        receiver.module(),
        receiving,
        provider.module(),
        supplying,
    )?;
    Ok(AdmittedComponentProvider {
        receiver: receiver.custody_identity(),
        receiving_boundary,
        provider: provider.identity(),
        provider_boundary: selected.boundary,
        candidate: selected.candidate,
        selection,
        progress: if let Some(wait) = component_spin_wait(
            receiver.module(),
            receiving,
            provider.module(),
            selected.candidate,
        ) {
            ImportProgress::Waiting(wait)
        } else if let Some(effects) = component_normal_effects(
            receiver.module(),
            receiving,
            provider.module(),
            selected.candidate,
        ) {
            ImportProgress::Effects(std::sync::Arc::new(effects))
        } else if private_wait_failure(
            provider.module(),
            selected.candidate,
            ProgressScope::Executable,
            |_| None,
        )
        .is_some()
        {
            ImportProgress::Unresolved
        } else {
            ImportProgress::Closed
        },
        input_memberships,
        result_memberships,
    })
}

/// Check the entry with its receiving scope and the actual installed provider bodies.
/// Only a selection admitted for this receiver can describe one of its calls.
/// Missing, opaque and unrecognized suppliers leave that call unknown.
pub fn invocation_progress_failure<'a, 'receiver>(
    receiver: impl Into<TerminalExecutionView<'receiver>>,
    entry: MachineId,
    scope: ProgressScope,
    installed: impl IntoIterator<Item = &'a AdmittedComponentProvider>,
    local: impl Fn(BoundaryMachineId) -> Option<MachineId>,
) -> Option<PrivateWaitFailure> {
    let receiver = receiver.into();
    let identity = receiver.custody_identity();
    let mut imports: Vec<_> = installed
        .into_iter()
        .filter(|supplier| supplier.receiver() == &identity)
        .collect();
    imports.sort_unstable_by_key(|supplier| supplier.receiving_boundary);
    private_wait_failure(receiver.module(), entry, scope, |slot| {
        let first = imports.partition_point(|supplier| supplier.receiving_boundary < slot);
        let Some(supplier) = imports
            .get(first)
            .filter(|supplier| supplier.receiving_boundary == slot)
        else {
            return local(slot).map(SelectedCall::Local);
        };
        if imports
            .get(first + 1)
            .is_some_and(|other| other.receiving_boundary == slot)
        {
            return None;
        }
        match &supplier.progress {
            ImportProgress::Closed => None,
            ImportProgress::Waiting(wait) => Some(SelectedCall::ImportedWait(wait.clone())),
            ImportProgress::Effects(effects) => Some(SelectedCall::ImportedEffects(effects.clone())),
            ImportProgress::Unresolved => Some(SelectedCall::UnresolvedImport),
        }
    })
}
