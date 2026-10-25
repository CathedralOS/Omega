//! Source-free provisions retained before native realization consumes them.
//!
//! A runtime driver needs the actual admitted entry and installation, not a
//! second source compilation or a guess from names in an emitted image. These
//! records identify those provisions; they do not instantiate storage, grant
//! process-domain authority, or license a host callback. The consuming driver
//! supplies those premises at its own explicit runtime boundary.

use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::TerminalPsiIdentity;

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
    calling_application: Option<calling_conventions::BoundaryCallingApplication>,
    /// Produced once from this admitted row. An unavailable physical route
    /// stays an explicit runtime limitation; it does not invalidate an otherwise
    /// valid native program or cause the interpreter to compile a replacement.
    native_adapter:
        Option<Result<std::sync::Arc<image_emission::NativeForeignAdapterImage>, String>>,
}

/// Exact callable host and selected-provider companions. These are shared
/// by application entries and separately emitted callable entries; no
/// ProgramEntry receipt or field establishment is inferred for a component.
#[derive(Debug, Clone)]
pub struct TerminalRuntimeHostProvisions {
    selection_identities: terminal_psi::TerminalIdentityTable,
    installation: Option<terminal_installation::AdmittedProviderSelections>,
    host: Vec<TerminalRuntimeHostProvision>,
    host_requirements: Vec<RuntimeHostRequirement>,
    borrowed_storage_layouts: Vec<runtime_abi::NativeBorrowedStorageLayout>,
}

impl TerminalRuntimeHostProvisions {
    pub fn capture_native_admission(
        installation: Option<&terminal_installation::AdmittedProviderSelections>,
        host: Vec<TerminalRuntimeHostProvision>,
        boundaries: &[terminal_psi::BoundaryMachineDeclaration],
        selected: &effects::SelectedProviderPlanFacts,
        identities: &terminal_psi::TerminalIdentityTable,
        calling_applications: &[calling_conventions::BoundaryCallingApplication],
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
            let requirement = binding
                .requirement_identity(selected.identities())
                .ok_or("runtime foreign calling application lost its selected requirement")?;
            // The same producer has already settled these applications. Retain
            // its exact shape and strong selected commitment, not a reconstructed
            // shape inferred from a placement's byte extent or report coordinate.
            let mut applications = calling_applications.iter().filter(|application| {
                selected.selected_row_matches_requirement(
                    &application.requirement_identity,
                    requirement,
                    binding.provider_plan_report_identity(),
                ) && Some(application.target) == binding.native_target()
                    && application.plan.plan() == &binding.boundary_entry_plan
            });
            let calling_application = applications.next().cloned();
            if applications.next().is_some() {
                return Err("runtime foreign calling application is ambiguous");
            }
            let native_adapter = binding.import().map(|(locator, _)| {
                let application = calling_application.as_ref().ok_or_else(|| {
                    "foreign runtime adapter has no retained complete calling application"
                        .to_owned()
                })?;
                let bridge = machine_emission::native_foreign_bridge::emit_native_foreign_bridge(
                    machine_emission::native_foreign_bridge::NativeForeignBridgeRequest {
                        target: application.target,
                        foreign: application.plan.clone(),
                    },
                )
                .map_err(|reason| {
                    format!("foreign runtime adapter physical transport: {reason:?}")
                })?;
                image_emission::emit_native_foreign_adapter(
                    bridge,
                    locator.clone(),
                    application.commitment,
                )
                .map(std::sync::Arc::new)
                .map_err(|error| error.diagnostic.to_string())
            });
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
                calling_application,
                native_adapter,
            });
        }
        Ok(Self {
            selection_identities: selected.identities().clone(),
            installation: installation.cloned(),
            host,
            host_requirements,
            borrowed_storage_layouts: Vec::new(),
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

    /// The admitted foreign row's complete source-free calling application.
    /// Runtime adapters must inspect this geometry before interpreting opaque
    /// owned bytes; equal-sized native placements do not establish a C shape.
    pub fn host_calling_application(
        &self,
        boundary: BoundaryMachineId,
    ) -> Option<&calling_conventions::BoundaryCallingApplication> {
        self.host_requirements
            .iter()
            .find(|row| row.boundary == boundary)?
            .calling_application
            .as_ref()
    }

    /// Physical fixed-frame product of the same admitted provider/calling row.
    /// A receiving runtime must still establish the actual provider instance,
    /// native homes, leases and exact semantic completion before using it.
    pub fn host_native_foreign_adapter(
        &self,
        boundary: BoundaryMachineId,
    ) -> Option<Result<&image_emission::NativeForeignAdapterImage, &str>> {
        self.host_requirements
            .iter()
            .find(|row| row.boundary == boundary)?
            .native_adapter
            .as_ref()
            .map(|adapter| {
                adapter
                    .as_ref()
                    .map(std::sync::Arc::as_ref)
                    .map_err(String::as_str)
            })
    }

    pub const fn installation(&self) -> Option<&terminal_installation::AdmittedProviderSelections> {
        self.installation.as_ref()
    }
    pub const fn selection_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.selection_identities
    }
    pub fn host(&self) -> &[TerminalRuntimeHostProvision] {
        &self.host
    }
    pub fn borrowed_storage_layouts(&self) -> &[runtime_abi::NativeBorrowedStorageLayout] {
        &self.borrowed_storage_layouts
    }
    pub fn with_borrowed_storage_layouts(
        mut self,
        layouts: Vec<runtime_abi::NativeBorrowedStorageLayout>,
    ) -> Self {
        self.borrowed_storage_layouts = layouts;
        self
    }
}

/// Callable-specific reference provisions captured from its own native
/// admission, without impersonating an application ProgramEntry.
#[derive(Debug, Clone)]
pub struct TerminalCallableRuntimeProvisions {
    identity: TerminalPsiIdentity,
    entry: MachineId,
    target: target::TargetProfile,
    host: TerminalRuntimeHostProvisions,
}

impl TerminalCallableRuntimeProvisions {
    pub fn capture_native_admission(
        artifact: terminal_codec::TerminalArtifactView<'_>,
        entry: MachineId,
        target: target::TargetProfile,
        host: TerminalRuntimeHostProvisions,
    ) -> Result<Self, &'static str> {
        if artifact.module().machine(entry).is_none()
            || host
                .installation()
                .is_some_and(|selection| selection.terminal_psi() != artifact.identity())
        {
            return Err("callable runtime provisions name another artifact or absent entry");
        }
        Ok(Self {
            identity: artifact.identity(),
            entry,
            target,
            host,
        })
    }
    pub const fn identity(&self) -> TerminalPsiIdentity {
        self.identity
    }
    pub const fn entry(&self) -> MachineId {
        self.entry
    }
    pub const fn target(&self) -> target::TargetProfile {
        self.target
    }
    pub const fn host_provisions(&self) -> &TerminalRuntimeHostProvisions {
        &self.host
    }
}

/// Entry and provider facts for the exact produced Terminal program. Source
/// arena handles and physical wrapper storage do not cross this record.
#[derive(Debug, Clone)]
pub struct TerminalRuntimeProvisions {
    identity: TerminalPsiIdentity,
    entry: MachineId,
    source_signature_identity: [u8; 32],
    target_slot: target::ProgramEntrySlotDeclaration,
    fused_services: Vec<program_entry_plan::ProgramEntryFusedServiceEstablishment>,
    /// The ProgramEntry signature's plan-local identity table — establishment
    /// rows index it, never the module's table.
    plan_identities: terminal_psi::TerminalIdentityTable,
    /// Parallel to `fused_services`: exact retained requirement slots of the
    /// same admitted schema/plan, not a provider-name or trait-prefix lookup.
    fused_service_boundary_slots: Vec<Vec<BoundaryMachineId>>,
    target_provider_proposals:
        Vec<terminal_psi_to_abstract_operations::TerminalTargetProviderProposal>,
    host_provisions: TerminalRuntimeHostProvisions,
    private_entry_ram: Vec<terminal_installation::CheckedPrivateEntryRamRecipe>,
    components: Vec<std::sync::Arc<crate::NativeComponentProduct>>,
    component_publications: Vec<crate::NativeComponentPublicationReceipt>,
}

impl TerminalRuntimeProvisions {
    pub fn record_component_publications(
        &mut self,
        receipts: Vec<crate::NativeComponentPublicationReceipt>,
    ) -> Result<(), &'static str> {
        if !self.component_publications.is_empty() || receipts.len() != self.components.len() {
            return Err(
                "component publication must cover the current immutable product roster exactly once",
            );
        }
        for (receipt, component) in receipts.iter().zip(&self.components) {
            if receipt.product().locator() != component.locator()
                || receipt.product().selection() != component.selection()
                || receipt.product().application() != component.application()
                || receipt.product().artifact().manifest().identity()
                    != component.artifact().manifest().identity()
                || receipt.product().authority().review().identity()
                    != component.authority().review().identity()
            {
                return Err(
                    "component publication receipt belongs to another exact producing image or historical input graph",
                );
            }
        }
        self.component_publications = receipts;
        Ok(())
    }
    /// Explicit preflight for the isolated process runtime, not native image
    /// installation. All dependency edges use these exact adjacent files and
    /// version-qualified exports for this immutable process lifetime (protected
    /// ELF symbols or exact two-level Mach-O dylib bindings).
    pub fn validate_component_process_inputs(&self, root: &std::path::Path) -> Result<(), String> {
        self.validate_component_product_roster()
            .map_err(str::to_owned)?;
        if self.components.is_empty() {
            return Ok(());
        }
        if self.component_publications.len() != self.components.len() {
            return Err(
                "component process has no complete exact publication receipt roster".into(),
            );
        }
        let directory = root
            .parent()
            .ok_or("component process root has no publication directory")?;
        for receipt in &self.component_publications {
            if receipt.path().parent() != Some(directory) {
                return Err("component process dependency is outside the selected root's adjacent immutable publication graph".into());
            }
            receipt.revalidate()?;
        }
        Ok(())
    }
    pub fn component_products(&self) -> &[std::sync::Arc<crate::NativeComponentProduct>] {
        &self.components
    }
    pub fn with_component_products(
        mut self,
        components: Vec<std::sync::Arc<crate::NativeComponentProduct>>,
    ) -> Result<Self, &'static str> {
        self.validate_component_products(&components)?;
        self.components = components;
        Ok(self)
    }
    /// Validate the optional runtime companion against a retained root. This
    /// boundary also handles callers that never attached runtime provisions.
    pub fn validate_root_object(
        object: &image_emission::ObjectArtifact,
        runtime: Option<&Self>,
    ) -> Result<(), &'static str> {
        match runtime {
            Some(runtime) => runtime.validate_component_root_object(object),
            None if object
                .object()
                .layout
                .normalized_imports
                .iter()
                .any(|import| {
                    matches!(
                        import.locator,
                        target::NormalizedImportLocator::Component(_)
                    )
                }) =>
            {
                Err(
                    "component native product has no complete retained runtime image/resource roster",
                )
            }
            None => Ok(()),
        }
    }

    /// Rejoin the actual byte-validated root's physical imports and complete
    /// call bindings. Another runtime with an empty self-consistent roster
    /// cannot answer this object's independent calls or change their WCSU.
    pub fn validate_component_root_object(
        &self,
        object: &image_emission::ObjectArtifact,
    ) -> Result<(), &'static str> {
        if installation_evidence::ObjectEvidence::psi(object) != self.identity
            || object.target() != self.target_slot.owner.native_target()
        {
            return Err("component runtime provisions name another actual root artifact or target");
        }
        self.validate_component_product_roster()?;
        for import in &object.object().layout.normalized_imports {
            let target::NormalizedImportLocator::Component(locator) = &import.locator else {
                continue;
            };
            let mut products = self
                .components
                .iter()
                .filter(|product| product.locator() == locator);
            if products.next().is_none() || products.next().is_some() {
                return Err(
                    "actual component root import has no unique exact retained image/resource product",
                );
            }
        }
        for call in object.foreign_calls() {
            if !matches!(
                &call.binding.mechanism,
                target_operations::NormalizedForeignCallMechanism::Component { .. }
            ) {
                continue;
            }
            if !self.host().iter().any(|provision| {
                matches!(&provision.realization,
                    target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding)
                        if binding == &call.binding)
            }) {
                return Err(
                    "actual component root call differs from its complete runtime ABI, provider or same-stack settlement",
                );
            }
        }
        Ok(())
    }

    /// Every selected component needs its exact physical product, even when
    /// an alternate caller omitted the product-attachment operation entirely.
    pub fn validate_component_product_roster(&self) -> Result<(), &'static str> {
        self.validate_component_products(&self.components)
    }
    fn validate_component_products(
        &self,
        components: &[std::sync::Arc<crate::NativeComponentProduct>],
    ) -> Result<(), &'static str> {
        let mut expected = 0;
        for provision in self.host() {
            let target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding) =
                &provision.realization
            else {
                continue;
            };
            let target_operations::NormalizedForeignCallMechanism::Component {
                locator,
                provider,
                candidate,
                application_commitment,
                same_stack_contribution,
            } = &binding.mechanism
            else {
                continue;
            };
            expected += 1;
            let mut matches = components.iter().filter(|component| {
                component.selection().receiver() == self.identity
                    && component.selection().receiving_boundary() == provision.boundary
                    && component.selection().provider() == *provider
                    && component.selection().candidate() == *candidate
                    && component.locator() == locator
                    && component.application().commitment == *application_commitment
                    && component.application().plan.plan() == &binding.boundary_entry_plan
            });
            let component = matches.next().ok_or(
                "component runtime roster does not exactly cover an admitted independent native call",
            )?;
            if matches.next().is_some() {
                return Err(
                    "component runtime roster ambiguously duplicates an admitted independent native call",
                );
            }
            component.validate_same_stack(same_stack_contribution)?;
        }
        if expected != components.len() {
            return Err("component runtime roster contains an unconsumed or duplicated image");
        }
        Ok(())
    }
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
        calling_applications: &[calling_conventions::BoundaryCallingApplication],
    ) -> Result<Self, &'static str> {
        let host_provisions = TerminalRuntimeHostProvisions::capture_native_admission(
            installation,
            host,
            boundaries,
            selected,
            identities,
            calling_applications,
        )?;
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
            private_entry_ram: Vec::new(),
            components: Vec::new(),
            component_publications: Vec::new(),
            identity: checked.terminal_psi_identity(),
            entry: checked.terminal_entry(),
            source_signature_identity: checked.source_signature_identity(),
            target_slot,
            fused_services: fused_services.to_vec(),
            plan_identities: plan_identities.clone(),
            fused_service_boundary_slots,
            target_provider_proposals: Vec::new(),
            host_provisions,
        })
    }

    pub const fn host_provisions(&self) -> &TerminalRuntimeHostProvisions {
        &self.host_provisions
    }

    pub fn matches_host_requirement(
        &self,
        declaration: &terminal_psi::BoundaryMachineDeclaration,
        selected_requirement: &str,
        provider_plan_report_identity: u64,
        identities: &terminal_psi::TerminalIdentityTable,
    ) -> bool {
        self.host_provisions.matches_host_requirement(
            declaration,
            selected_requirement,
            provider_plan_report_identity,
            identities,
        )
    }

    pub fn host_calling_application(
        &self,
        boundary: BoundaryMachineId,
    ) -> Option<&calling_conventions::BoundaryCallingApplication> {
        self.host_provisions.host_calling_application(boundary)
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
        self.host_provisions.borrowed_storage_layouts = layouts;
        self
    }

    pub fn borrowed_storage_layouts(&self) -> &[runtime_abi::NativeBorrowedStorageLayout] {
        self.host_provisions.borrowed_storage_layouts()
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

    /// The signature table the establishment rows' identity indices resolve
    /// against — rows were minted before the module's table existed.
    pub const fn plan_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.plan_identities
    }

    /// The selection facts table admitted same-stack requirement indices
    /// resolve against.
    pub const fn selection_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        self.host_provisions.selection_identities()
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
        self.host_provisions.installation()
    }

    pub fn host(&self) -> &[TerminalRuntimeHostProvision] {
        self.host_provisions.host()
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
