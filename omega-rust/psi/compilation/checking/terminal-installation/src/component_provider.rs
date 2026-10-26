//! Cross-artifact checked provider selection, without execution state.
//!
//! A runtime root chooses the receiving slot and the producing candidate. This
//! admission preserves both artifact owners and the checked declaration contract.
//! Native ABI/representation admission and actual installed entry/era custody
//! remain separate. In particular, this cannot make a host adapter a provider.

use semantic_vocabulary::{BoundaryMachineId, MachineId};
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
    input_memberships: crate::component_memberships::InputMemberships,
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

    /// Translate only an actual input membership through this sealed artifact
    /// pair. A missing mapping cannot be replaced by the provider's annotation.
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
    let input_memberships = crate::component_memberships::admit_input_memberships(
        receiver.module(),
        receiving,
        provider.module(),
        supplying,
    )?;
    Ok(AdmittedComponentProvider {
        receiver: receiver.identity(),
        receiving_boundary,
        provider: provider.identity(),
        provider_boundary: selected.boundary,
        candidate: selected.candidate,
        selection,
        input_memberships,
    })
}
