//! Source-free provisions retained before native realization consumes them.
//!
//! A runtime driver needs the actual admitted entry and installation, not a
//! second source compilation or a guess from names in an emitted image. These
//! records identify those provisions; they do not instantiate storage, grant
//! process-domain authority, or license a host callback. The consuming driver
//! supplies those premises at its own explicit runtime boundary.

use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::{CheckedProgramEntryReceiverEligibility, TerminalPsiIdentity};

/// An exact host operation admitted for the native product. Reference execution
/// may implement its semantic contract, but cannot substitute another slot or
/// manufacture authority merely because a realization was selected.
#[derive(Debug, Clone)]
pub struct TerminalRuntimeHostProvision {
    pub boundary: BoundaryMachineId,
    pub realization: target_operations::BoundarySettlementRealization,
}

/// The original call contract served by one exact admitted foreign row.
/// This relation is captured from selected closure custody, not reconstructed
/// from either callable's spelling by a receiving host adapter.
#[derive(Debug, Clone)]
struct RuntimeHostRequirement {
    boundary: BoundaryMachineId,
    boundary_identity: String,
    selected_requirement_identity: String,
    provider_plan_report_identity: u64,
}

/// Entry and provider facts for the exact produced Terminal program. Source
/// arena handles and physical wrapper storage do not cross this record.
#[derive(Debug, Clone)]
pub struct TerminalRuntimeProvisions {
    identity: TerminalPsiIdentity,
    entry: MachineId,
    source_signature_identity: [u8; 32],
    target_slot: target::ProgramEntrySlotDeclaration,
    receiver: Option<CheckedProgramEntryReceiverEligibility>,
    fused_services: Vec<program_entry_plan::ProgramEntryFusedServiceEstablishment>,
    /// The ProgramEntry signature's plan-local identity table — establishment
    /// rows index it, never the module's table.
    plan_identities: terminal_psi::TerminalIdentityTable,
    /// The selection facts table the admitted same-stack requirement indices
    /// resolve against — provider-side texts never enter the module table.
    selection_identities: terminal_psi::TerminalIdentityTable,
    /// Parallel to `fused_services`: exact retained requirement slots of the
    /// same admitted schema/plan, not a provider-name or trait-prefix lookup.
    fused_service_boundary_slots: Vec<Vec<BoundaryMachineId>>,
    installation: Option<terminal_installation::AdmittedProviderSelections>,
    target_provider_proposals:
        Vec<terminal_psi_to_abstract_operations::TerminalTargetProviderProposal>,
    host: Vec<TerminalRuntimeHostProvision>,
    host_requirements: Vec<RuntimeHostRequirement>,
    borrowed_storage_layouts: Vec<runtime_abi::NativeBorrowedStorageLayout>,
    private_entry_ram: Vec<terminal_installation::CheckedPrivateEntryRamRecipe>,
}

impl TerminalRuntimeProvisions {
    pub fn private_entry_ram(&self) -> &[terminal_installation::CheckedPrivateEntryRamRecipe] {
        &self.private_entry_ram
    }
    pub fn with_private_entry_ram(
        mut self,
        recipes: Vec<terminal_installation::CheckedPrivateEntryRamRecipe>,
    ) -> Self {
        self.private_entry_ram = recipes;
        self
    }

    /// Capture already admitted facts while they still accompany the image
    /// request. This is not another admission or a runtime authority grant.
    pub fn capture_native_admission(
        checked: &terminal_psi::CheckedProgramEntryTerminalReceipt,
        fused_services: &[program_entry_plan::ProgramEntryFusedServiceEstablishment],
        installation: Option<&terminal_installation::AdmittedProviderSelections>,
        host: Vec<TerminalRuntimeHostProvision>,
        boundaries: &[terminal_psi::BoundaryMachineDeclaration],
        candidates: &[terminal_psi::ProviderCandidateConformance],
        selected: &effects::SelectedProviderPlanFacts,
        identities: &terminal_psi::TerminalIdentityTable,
        plan_identities: &terminal_psi::TerminalIdentityTable,
        target_slot: target::ProgramEntrySlotDeclaration,
    ) -> Result<Self, &'static str> {
        let mut host_requirements = Vec::new();
        for provision in &host {
            let target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding) =
                &provision.realization
            else {
                continue;
            };
            let mut matching = boundaries
                .iter()
                .filter(|boundary| boundary.id == provision.boundary);
            let boundary = matching
                .next()
                .ok_or("runtime host provision lost its exact boundary declaration")?;
            if matching.next().is_some()
                || !selected.selected_row_matches_requirement(
                    identities.text(boundary.identity).unwrap_or_default(),
                    binding
                        .requirement_identity(selected.identities())
                        .unwrap_or_default(),
                    binding.provider_plan_report_identity(),
                )
            {
                return Err("runtime host provision lost its exact selected requirement row");
            }
            host_requirements.push(RuntimeHostRequirement {
                boundary: boundary.id,
                boundary_identity: identities
                    .text(boundary.identity)
                    .unwrap_or_default()
                    .to_owned(),
                selected_requirement_identity: binding
                    .requirement_identity(selected.identities())
                    .unwrap_or_default()
                    .to_owned(),
                provider_plan_report_identity: binding.provider_plan_report_identity(),
            });
        }
        let mut fused_service_boundary_slots = Vec::with_capacity(fused_services.len());
        for establishment in fused_services {
            let mut matching_plans = selected.plans().iter().filter(|plan| {
                plan.identity_digest() == establishment.selected_provider_plan_digest()
            });
            let plan = matching_plans
                .next()
                .ok_or("runtime Fused establishment lost its exact admitted plan/schema")?;
            if matching_plans.next().is_some() {
                return Err("runtime Fused establishment repeats its exact admitted plan/schema");
            }
            let receiver = plan
                .schema
                .receiver_applications
                .get(establishment.receiver_application_index())
                .ok_or("runtime Fused establishment lost its admitted receiver view")?;
            if !establishment.receiving_requirements().is_empty() {
                let installation = installation
                    .filter(|installation| {
                        installation.terminal_psi() == checked.terminal_psi_identity()
                    })
                    .ok_or("runtime Fused receiving roster has no same-artifact installation")?;
                fused_service_boundary_slots.push(capture_receiving_slots(
                    establishment,
                    plan,
                    receiver,
                    candidates,
                    selected,
                    identities,
                    plan_identities,
                    installation,
                )?);
                continue;
            }
            // The native admission already authenticated these identities and
            // contracts. Capture the existing coordinate relation before that
            // admission is consumed; do not reconstruct semantics or authority.
            let mut slots = Vec::new();
            for boundary in boundaries {
                if receiver.requirement_identities.iter().any(|requirement| {
                    selected.selected_row_matches_requirement(
                        identities.text(boundary.identity).unwrap_or_default(),
                        plan.schema.identity_text(*requirement),
                        plan.report_fingerprint(),
                    )
                }) {
                    slots.push(boundary.id);
                }
            }
            slots.sort_unstable();
            slots.dedup();
            fused_service_boundary_slots.push(slots);
        }
        Ok(Self {
            borrowed_storage_layouts: Vec::new(),
            private_entry_ram: Vec::new(),
            identity: checked.terminal_psi_identity(),
            entry: checked.terminal_entry(),
            source_signature_identity: checked.source_signature_identity(),
            target_slot,
            receiver: checked.receiver_eligibility().cloned(),
            fused_services: fused_services.to_vec(),
            plan_identities: plan_identities.clone(),
            selection_identities: selected.identities().clone(),
            fused_service_boundary_slots,
            installation: installation.cloned(),
            target_provider_proposals: Vec::new(),
            host,
            host_requirements,
        })
    }

    /// Rejoin one original boundary contract to its captured foreign provider
    /// row before offering a host implementation. This establishes identity
    /// correspondence only; the adapter must still admit the complete contract,
    /// ABI and actual host capability independently.
    pub fn matches_host_requirement(
        &self,
        declaration: &terminal_psi::BoundaryMachineDeclaration,
        selected_requirement: &str,
        provider_plan_report_identity: u64,
        identities: &terminal_psi::TerminalIdentityTable,
    ) -> bool {
        let mut matching = self
            .host_requirements
            .iter()
            .filter(|row| row.boundary == declaration.id);
        matching.next().is_some_and(|row| {
            identities.text(declaration.identity) == Some(row.boundary_identity.as_str())
                && row.selected_requirement_identity == selected_requirement
                && row.provider_plan_report_identity == provider_plan_report_identity
        }) && matching.next().is_none()
    }

    pub const fn identity(&self) -> TerminalPsiIdentity {
        self.identity
    }

    /// Retain the same producer's portable selections separately from this
    /// native target's admitted entry, backing and external host provisions.
    pub fn with_target_provider_proposals(
        mut self,
        proposals: Vec<terminal_psi_to_abstract_operations::TerminalTargetProviderProposal>,
    ) -> Self {
        self.target_provider_proposals = proposals;
        self
    }

    pub fn target_provider_proposals(
        &self,
    ) -> &[terminal_psi_to_abstract_operations::TerminalTargetProviderProposal] {
        &self.target_provider_proposals
    }

    /// Retain the same native producer's layout companion. These coordinates
    /// confer neither host authority nor permission to replace guest fields.
    pub fn with_borrowed_storage_layouts(
        mut self,
        layouts: Vec<runtime_abi::NativeBorrowedStorageLayout>,
    ) -> Self {
        self.borrowed_storage_layouts = layouts;
        self
    }

    pub fn borrowed_storage_layouts(&self) -> &[runtime_abi::NativeBorrowedStorageLayout] {
        &self.borrowed_storage_layouts
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub const fn source_signature_identity(&self) -> [u8; 32] {
        self.source_signature_identity
    }

    /// The actual admitted root's slot/arrival contract, not recovered from
    /// the source-signature digest or inferred from selected host providers.
    pub const fn target_slot(&self) -> target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub const fn receiver(&self) -> Option<&CheckedProgramEntryReceiverEligibility> {
        self.receiver.as_ref()
    }

    /// The signature table the establishment rows' identity indices resolve
    /// against — rows were minted before the module's table existed.
    pub const fn plan_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.plan_identities
    }

    /// The selection facts table admitted same-stack requirement indices
    /// resolve against.
    pub const fn selection_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.selection_identities
    }

    pub fn fused_services(&self) -> &[program_entry_plan::ProgramEntryFusedServiceEstablishment] {
        &self.fused_services
    }

    /// Each actual root establishment's exact retained requirement slots.
    /// This association identifies what the host must establish; it supplies
    /// neither runtime Binding custody nor containing-process-domain authority.
    pub fn fused_service_slots(
        &self,
    ) -> impl Iterator<
        Item = (
            &program_entry_plan::ProgramEntryFusedServiceEstablishment,
            &[BoundaryMachineId],
        ),
    > {
        self.fused_services
            .iter()
            .zip(&self.fused_service_boundary_slots)
            .map(|(establishment, slots)| (establishment, slots.as_slice()))
    }

    pub const fn installation(&self) -> Option<&terminal_installation::AdmittedProviderSelections> {
        self.installation.as_ref()
    }

    pub fn host(&self) -> &[TerminalRuntimeHostProvision] {
        &self.host
    }
}

/// Capture the complete canonical roster for the checked field application.
/// Each ancestor owns its own application key; each finite tuple owns a separate
/// closed coordinate. A supplied subset cannot replace the artifact's roster.
fn capture_receiving_slots(
    establishment: &program_entry_plan::ProgramEntryFusedServiceEstablishment,
    plan: &effects::provider_plan::ProviderPlan,
    receiver: &effects::provider_plan::ServiceReceiverApplication,
    candidates: &[terminal_psi::ProviderCandidateConformance],
    selected: &effects::SelectedProviderPlanFacts,
    identities: &terminal_psi::TerminalIdentityTable,
    plan_identities: &terminal_psi::TerminalIdentityTable,
    installation: &terminal_installation::AdmittedProviderSelections,
) -> Result<Vec<BoundaryMachineId>, &'static str> {
    let rows = establishment.receiving_requirements();
    let first = rows.first().ok_or("runtime receiving roster is empty")?;
    let text = |identity| plan_identities.text(identity).unwrap_or_default();
    let regions_match =
        |candidate: &terminal_psi::ProviderCandidateConformance,
         row: &program_entry_plan::ProgramEntryReceivingRequirement| {
            candidate.receiving_region_arguments.len() == row.receiving_regions.len()
                && candidate
                    .receiving_region_arguments
                    .iter()
                    .zip(&row.receiving_regions)
                    .all(|(candidate, row)| identities.text(*candidate) == Some(text(*row)))
        };
    for requirement in &receiver.requirement_identities {
        if !rows
            .iter()
            .any(|row| text(row.source_requirement) == plan.schema.identity_text(*requirement))
        {
            return Err("runtime receiving roster omitted an original requirement");
        }
    }
    if rows.iter().any(|row| {
        row.provider_declaration != first.provider_declaration
            || row.receiving_regions != first.receiving_regions
            || !receiver.requirement_identities.iter().any(|requirement| {
                text(row.source_requirement) == plan.schema.identity_text(*requirement)
            })
    }) {
        return Err("runtime receiving roster mixes selecting owners or provider applications");
    }
    let expected = candidates
        .iter()
        .filter(|candidate| {
            identities.text(candidate.provider_identity) == Some(plan.provider_type.as_str())
                && identities.text(candidate.provider_declaration.identity())
                    == Some(text(first.provider_declaration))
                && regions_match(candidate, first)
                // The original declaration roster and complete map tuple,
                // not supplied application keys, determine the expected set.
                // Type/Const-generic provider maps are not admitted: their
                // independent applications would need additional coordinates.
                && receiver.requirement_identities.iter().any(|requirement| {
                    identities.text(candidate.source_requirement_identity)
                        == Some(plan.schema.identity_text(*requirement))
                })
        })
        .collect::<Vec<_>>();
    if expected.len() != rows.len() {
        return Err("runtime receiving roster differs from complete canonical applications");
    }
    let mut slots = Vec::with_capacity(rows.len());
    for row in rows {
        let mut matching = expected
            .iter()
            .copied()
            .filter(|candidate| row.matches_candidate(candidate, identities, plan_identities));
        let candidate = matching
            .next()
            .ok_or("runtime receiving requirement has no canonical candidate")?;
        if matching.next().is_some()
            || slots.contains(&candidate.boundary)
            || !installation.contains_candidate(candidate)
            || !selected.selected_row_matches_requirement(
                text(row.requirement),
                text(row.source_requirement),
                plan.report_fingerprint(),
            )
        {
            return Err("runtime receiving requirement does not join one exact installed plan row");
        }
        slots.push(candidate.boundary);
    }
    slots.sort_unstable();
    Ok(slots)
}
