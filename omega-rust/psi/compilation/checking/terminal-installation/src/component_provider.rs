//! Cross-artifact checked provider selection, without execution state.
//!
//! A runtime root chooses the receiving slot and the producing candidate. This
//! admission preserves both artifact owners and the checked declaration contract.
//! Native ABI/representation admission and actual installed entry/era custody
//! remain separate. In particular, this cannot make a host adapter a provider.

use semantic_vocabulary::{BoundaryMachineId, MachineId, StructuralDomainId};
use terminal_codec::TerminalArtifactView;
use terminal_psi::TerminalPsiIdentity;

use crate::{
    AdmittedProviderSelections, ProviderInstallationSelection, admit_provider_installation,
};

/// The exact symbolic import/export relationship a component instance may bind.
/// This immutable selection grants no native address, installed code or storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedComponentProvider {
    receiver: TerminalPsiIdentity,
    receiving_boundary: BoundaryMachineId,
    provider: TerminalPsiIdentity,
    provider_boundary: BoundaryMachineId,
    candidate: MachineId,
    selection: AdmittedProviderSelections,
    memberships: crate::component_memberships::ComponentMemberships,
}

impl AdmittedComponentProvider {
    pub const fn receiver(&self) -> TerminalPsiIdentity {
        self.receiver
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

    /// Rebind only memberships already present on the actual argument. Missing
    /// receiving theories fail; the formal parameter is never an erasure grant.
    pub fn present_input_memberships(
        &self,
        memberships: &mut [StructuralDomainId],
    ) -> Result<(), &'static str> {
        self.memberships.present(memberships, true)
    }

    /// Preserve actual completed evidence under the authenticated receiver's
    /// local coordinates. This does not establish any membership.
    pub fn present_result_memberships(
        &self,
        memberships: &mut [StructuralDomainId],
    ) -> Result<(), &'static str> {
        self.memberships.present(memberships, false)
    }
}

/// Match an explicitly selected checked export to one receiving requirement.
/// The normal provider-catalog admission authenticates the producing candidate;
/// the source-free declaration join checks exact nominal and contract identity
/// across the two artifact-local namespaces. It never synthesizes conformance.
pub fn admit_component_provider(
    receiver: TerminalArtifactView<'_>,
    receiving_boundary: BoundaryMachineId,
    provider: TerminalArtifactView<'_>,
    selected: &ProviderInstallationSelection,
) -> Result<AdmittedComponentProvider, String> {
    let receiving = receiver
        .module()
        .boundary_machines
        .iter()
        .find(|row| row.id == receiving_boundary)
        .ok_or("component import names no receiving boundary")?;
    let supplying = provider
        .module()
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
    if !receiving_contract.same_identity_as(left, supplying_contract, right)
        || receiving.completion != supplying.completion
        || receiving.unconditional_termination != supplying.unconditional_termination
        || receiving.parameter_order != supplying.parameter_order
        || receiving.scalar_parameters != supplying.scalar_parameters
    {
        return Err("component import/export checked callable contracts differ".into());
    }
    let selection = admit_provider_installation(provider, std::slice::from_ref(selected), &[])
        .map_err(|error| format!("component export provider admission: {error:?}"))?;
    let memberships = crate::component_memberships::ComponentMemberships::join(
        receiver.module(),
        provider.module(),
    )?;
    Ok(AdmittedComponentProvider {
        receiver: receiver.identity(),
        receiving_boundary,
        provider: provider.identity(),
        provider_boundary: selected.boundary,
        candidate: selected.candidate,
        selection,
        memberships,
    })
}
