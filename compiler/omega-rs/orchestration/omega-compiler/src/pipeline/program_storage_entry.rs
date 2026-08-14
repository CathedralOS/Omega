//! Installation handoff for the core-owned program-storage entry roots.
//!
//! Target entry traits inherit `ProgramStorageEntry::enter`; they do not own a
//! second image/storage convention.  This bridge therefore selects the exact
//! inherited requirement and its semantic parameter ordinals from the checked
//! provider schema, joins them to one validated calling plan and generated
//! prologue, and only then consumes admitted extent grants.

use omega_calling_conventions::{ValidatedBoundaryEntryPlan, ValuePlacement};
use omega_effects::provider_plan::ServiceEntryAuthorityFlow;
use omega_instruction_selection::DerivedBoundaryEntryStorage;
use psi_extents::{
    Extent, ExtentLoan, ExtentRootGrant, OwnedExtentPartition, ValidatedExtentGeometry,
};
use std::path::Path;

const PROGRAM_STORAGE_ENTRY_OWNER: &str = "ProgramStorageEntry";
const PROGRAM_STORAGE_ENTRY_METHOD: &str = "enter";
const GRANTED_DOMAIN: &str = "Extent::Granted";
const IMAGE_PARAMETER_INDEX: usize = 0;
const INITIAL_STORAGE_PARAMETER_INDEX: usize = 1;

/// One exact qualified semantic parameter joined to its generated ABI capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryParameter {
    parameter_index: usize,
    parameter_type_identity: String,
    domain: String,
    effective_carry: psi_language_semantics::CarryPolicy,
    placement: ValuePlacement,
    destination_byte_offset: usize,
    write_range: std::ops::Range<usize>,
}

impl ProgramStorageEntryParameter {
    pub const fn parameter_index(&self) -> usize {
        self.parameter_index
    }

    pub fn parameter_type_identity(&self) -> &str {
        &self.parameter_type_identity
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub const fn effective_carry(&self) -> psi_language_semantics::CarryPolicy {
        self.effective_carry
    }

    pub const fn placement(&self) -> &ValuePlacement {
        &self.placement
    }

    pub const fn destination_byte_offset(&self) -> usize {
        self.destination_byte_offset
    }

    pub const fn write_range(&self) -> &std::ops::Range<usize> {
        &self.write_range
    }
}

/// Exact selected target-entry contract that may introduce program storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryPlanBinding {
    root_slot: omega_external_roots::RootSlotId,
    requirement_identity: String,
    boundary_contract_fingerprint: u64,
    image: ProgramStorageEntryParameter,
    initial_storage: ProgramStorageEntryParameter,
    receiver: Option<ProgramEntryReceiverStoragePlan>,
}

impl ProgramStorageEntryPlanBinding {
    pub const fn root_slot(&self) -> omega_external_roots::RootSlotId {
        self.root_slot
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary_contract_fingerprint
    }

    pub const fn image(&self) -> &ProgramStorageEntryParameter {
        &self.image
    }

    pub const fn initial_storage(&self) -> &ProgramStorageEntryParameter {
        &self.initial_storage
    }

    pub const fn receiver(&self) -> Option<&ProgramEntryReceiverStoragePlan> {
        self.receiver.as_ref()
    }

    /// Attach the concrete layout of a receiver whose exclusive source shape
    /// and ZII validity were already checked for this selected entry.
    pub fn with_checked_receiver_layout(
        mut self,
        type_identity: String,
        layout: omega_layout::TypeLayout,
    ) -> Result<Self, ProgramStorageEntryDiagnostic> {
        if type_identity.is_empty() {
            return Err(ProgramStorageEntryDiagnostic(
                "selected entry receiver type identity cannot be empty".into(),
            ));
        }
        if layout.alignment == 0 || !layout.alignment.is_power_of_two() {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "selected entry receiver has invalid {}-byte alignment",
                layout.alignment
            )));
        }
        if self.receiver.is_some() {
            return Err(ProgramStorageEntryDiagnostic(
                "selected entry already has a checked receiver layout".into(),
            ));
        }
        self.receiver = Some(ProgramEntryReceiverStoragePlan {
            type_identity,
            byte_size: layout.size,
            byte_alignment: layout.alignment,
        });
        Ok(self)
    }
}

/// Compiler-checked storage demand for one occurrence-local entry receiver.
///
/// This is a reservation plan, not evidence that bytes have already been
/// zeroed. The generated physical bridge must materialize the ZII value and
/// lend the resulting occurrence exactly once before source execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntryReceiverStoragePlan {
    type_identity: String,
    byte_size: usize,
    byte_alignment: usize,
}

impl ProgramEntryReceiverStoragePlan {
    pub fn type_identity(&self) -> &str {
        &self.type_identity
    }

    pub const fn byte_size(&self) -> usize {
        self.byte_size
    }

    pub const fn byte_alignment(&self) -> usize {
        self.byte_alignment
    }
}

/// Exact target-owned environment-to-program slot and its normalized source
/// schema. This is deliberately not a provider plan: `ProgramEntry` accepts an
/// environment root and does not model an outbound service conformance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgramStorageEntryPlan {
    root_slot: omega_external_roots::RootSlotId,
    schema: omega_effects::provider_plan::ServiceSchema,
}

impl SelectedProgramStorageEntryPlan {
    pub fn from_target_slot(
        slot: omega_target::ProgramEntrySlotDeclaration,
        schema: omega_effects::provider_plan::ServiceSchema,
    ) -> Result<Self, ProgramStorageEntryDiagnostic> {
        if slot != slot.owner.program_entry_slot()
            || slot.schema != omega_target::ProgramEntrySchema::ProgramStorageApplication
            || slot.visible_parameters
                != omega_target::ProgramEntryVisibleParameters::ImageAndInitialStorage
            || slot.arrival_requirement
                != format!("{PROGRAM_STORAGE_ENTRY_OWNER}::{PROGRAM_STORAGE_ENTRY_METHOD}")
        {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target root slot `{}::{}` does not declare the exact program-storage entry contract",
                slot.owner.root_slot_owner_name(),
                slot.slot_name
            )));
        }
        let Some(boundary_schema) = slot.boundary_schema else {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target root slot `{}::{}` has no source boundary schema",
                slot.owner.root_slot_owner_name(),
                slot.slot_name
            )));
        };
        if schema.trait_name != boundary_schema {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target root slot `{}::{}` requires boundary schema `{boundary_schema}`, not `{}`",
                slot.owner.root_slot_owner_name(),
                slot.slot_name,
                schema.trait_name
            )));
        }

        let canonical = format!(
            "target-root-slot\n{}::{}",
            slot.owner.root_slot_owner_name(),
            slot.slot_name
        );
        let mut identity = 0xcbf29ce484222325u64;
        for byte in canonical.bytes() {
            identity ^= u64::from(byte);
            identity = identity.wrapping_mul(0x100000001b3);
        }
        let root_slot = omega_external_roots::RootSlotId::from_normalized_identity(identity)
            .map_err(|diagnostic| ProgramStorageEntryDiagnostic(diagnostic.to_string()))?;
        Ok(Self { root_slot, schema })
    }

    pub const fn root_slot(&self) -> omega_external_roots::RootSlotId {
        self.root_slot
    }

    pub const fn schema(&self) -> &omega_effects::provider_plan::ServiceSchema {
        &self.schema
    }
}

/// Provider-admitted authority and the runtime geometry presented at one
/// program-entry parameter. Construction imports no fact; installation first
/// validates both inputs and returns both values intact on failure.
#[derive(Debug, PartialEq, Eq)]
pub struct ProgramStorageRootInput {
    grant: ExtentRootGrant,
    base: u64,
    length: u64,
}

/// Exact selected physical-provider occurrence authorized to supply both
/// roots for one generated program-entry bridge invocation.
///
/// Image and initial-storage roots may carry different route, capacity, and
/// qualification evidence, but they must originate from this same selected
/// provider plan and concrete invocation. Construction retains those typed
/// identities from provider evidence rather than accepting untyped integers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramStorageEntryProviderInvocation {
    provider: psi_extents::ExtentProviderId,
    provider_plan: psi_extents::ExtentProviderPlanId,
    invocation: psi_extents::ExtentProviderInvocationId,
}

/// Pending native handoff emitted for one exact program-storage entry.
///
/// Compilation can bind the selected source continuation to the final object
/// entry symbol and provider selection, but it cannot manufacture runtime
/// geometry, admitted issuances, or mapped receiver bytes. A platform
/// installer consumes this plan with those environment-supplied values through
/// [`install_and_activate_program_storage_entry_receiver`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryNativeBridgePlan {
    binding: ProgramStorageEntryPlanBinding,
    selected_provider: Option<super::provider_plans::SelectedExternalRootProviderPlan>,
    target_profile: String,
    entry_symbol: String,
    entry_text_offset: usize,
    entry_text_size: usize,
    continuation_machine: String,
    continuation_state: String,
}

impl ProgramStorageEntryNativeBridgePlan {
    pub const fn binding(&self) -> &ProgramStorageEntryPlanBinding {
        &self.binding
    }

    pub const fn selected_provider(
        &self,
    ) -> Option<&super::provider_plans::SelectedExternalRootProviderPlan> {
        self.selected_provider.as_ref()
    }

    pub fn target_profile(&self) -> &str {
        &self.target_profile
    }

    pub fn entry_symbol(&self) -> &str {
        &self.entry_symbol
    }

    pub const fn entry_text_offset(&self) -> usize {
        self.entry_text_offset
    }

    pub const fn entry_text_size(&self) -> usize {
        self.entry_text_size
    }

    pub fn continuation_machine(&self) -> &str {
        &self.continuation_machine
    }

    pub fn continuation_state(&self) -> &str {
        &self.continuation_state
    }
}

/// Bind the pending physical bridge to the actual emitted entry function.
/// This records installable coordinates only; successful compilation remains
/// distinct from runtime provider installation.
pub fn bind_emitted_program_storage_entry_native_bridge(
    binding: ProgramStorageEntryPlanBinding,
    selected_provider: Option<super::provider_plans::SelectedExternalRootProviderPlan>,
    target_profile: String,
    object: &omega_object_file::ObjectPlan,
    boundary_contract_fingerprint: Option<u64>,
    continuation_machine: String,
    continuation_state: String,
) -> Result<ProgramStorageEntryNativeBridgePlan, ProgramStorageEntryDiagnostic> {
    if target_profile.is_empty() {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage native bridge has no selected target profile".into(),
        ));
    }
    let entry_handle = object.layout.entry_symbol;
    if !object.layout.symbols.is_valid(entry_handle) {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage native bridge has no emitted object entry symbol".into(),
        ));
    }
    let entry = object.layout.symbols.get(entry_handle);
    if entry.kind != omega_object_file::SymbolKind::Function
        || entry.section
            != omega_object_file::SymbolSection::Section(omega_object_file::SectionKind::Text)
        || entry.size == 0
    {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage native bridge entry symbol is not a nonempty text function".into(),
        ));
    }
    if boundary_contract_fingerprint != Some(binding.boundary_contract_fingerprint) {
        return Err(ProgramStorageEntryDiagnostic(
            "emitted entry footprint does not retain the program-storage boundary fingerprint"
                .into(),
        ));
    }
    if let Some(selected_provider) = &selected_provider {
        validate_selected_provider_binding(&binding, selected_provider)?;
    }
    if continuation_machine.is_empty() || continuation_state.is_empty() {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage native bridge lost its selected source continuation".into(),
        ));
    }
    Ok(ProgramStorageEntryNativeBridgePlan {
        binding,
        selected_provider,
        target_profile,
        entry_symbol: entry.name.clone(),
        entry_text_offset: entry.offset,
        entry_text_size: entry.size,
        continuation_machine,
        continuation_state,
    })
}

impl ProgramStorageEntryProviderInvocation {
    fn bind_selected_provider(
        binding: &ProgramStorageEntryPlanBinding,
        selected: &super::provider_plans::SelectedExternalRootProviderPlan,
        issuance: psi_extents::ExtentProviderIssuance,
    ) -> Result<Self, ProgramStorageEntryDiagnostic> {
        let invocation = issuance.invocation();
        if selected.identity.normalized_identity()
            != invocation.provider_plan().normalized_identity()
        {
            return Err(ProgramStorageEntryDiagnostic(
                "physical entry issuance does not belong to the compiler-selected provider plan"
                    .into(),
            ));
        }
        validate_selected_provider_binding(binding, selected)?;
        Ok(Self {
            provider: issuance.provider(),
            provider_plan: invocation.provider_plan(),
            invocation: invocation.invocation(),
        })
    }

    pub const fn provider(&self) -> psi_extents::ExtentProviderId {
        self.provider
    }

    pub const fn provider_plan(&self) -> psi_extents::ExtentProviderPlanId {
        self.provider_plan
    }

    pub const fn invocation(&self) -> psi_extents::ExtentProviderInvocationId {
        self.invocation
    }

    fn matches(&self, issuance: psi_extents::ExtentProviderIssuance) -> bool {
        let invocation = issuance.invocation();
        self.provider == issuance.provider()
            && self.provider_plan == invocation.provider_plan()
            && self.invocation == invocation.invocation()
    }
}

fn validate_selected_provider_binding(
    binding: &ProgramStorageEntryPlanBinding,
    selected: &super::provider_plans::SelectedExternalRootProviderPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let matching_methods = selected
        .schema
        .methods
        .iter()
        .filter(|method| method.requirement_identity == binding.requirement_identity)
        .collect::<Vec<_>>();
    let [method] = matching_methods.as_slice() else {
        return Err(ProgramStorageEntryDiagnostic(
            "selected physical entry provider does not implement the bound arrival requirement exactly once"
                .into(),
        ));
    };
    if method.calling_plan_fingerprint != Some(binding.boundary_contract_fingerprint) {
        return Err(ProgramStorageEntryDiagnostic(
            "selected physical entry provider calling plan does not match the generated bridge binding"
                .into(),
        ));
    }
    Ok(())
}

impl ProgramStorageRootInput {
    pub const fn new(grant: ExtentRootGrant, base: u64, length: u64) -> Self {
        Self {
            grant,
            base,
            length,
        }
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub fn with_geometry(self, base: u64, length: u64) -> Self {
        Self {
            grant: self.grant,
            base,
            length,
        }
    }

    pub fn into_grant(self) -> ExtentRootGrant {
        self.grant
    }
}

/// The two non-duplicable `Extent::Granted` roots introduced by one installed
/// target entry. Their roles remain attached to the exact inherited semantic
/// positions used to capture them.
#[derive(Debug)]
pub struct InstalledProgramStorageRoots {
    binding: ProgramStorageEntryPlanBinding,
    provider_invocation: Option<ProgramStorageEntryProviderInvocation>,
    image: Extent,
    initial_storage: Option<Extent>,
    receiver_storage: Option<ReservedProgramEntryReceiverStorage>,
    initial_storage_record: ProgramStorageInstalledExtentRecord,
}

/// Conserved storage reserved for one future ZII entry-receiver occurrence.
/// The selected extent and every nonempty remainder retain the installed
/// initial-storage root's exact lineage.
#[derive(Debug)]
pub struct ReservedProgramEntryReceiverStorage {
    plan: ProgramEntryReceiverStoragePlan,
    placement: ProgramEntryReceiverPlacementRecord,
    partition: Option<OwnedExtentPartition>,
}

impl ReservedProgramEntryReceiverStorage {
    pub const fn plan(&self) -> &ProgramEntryReceiverStoragePlan {
        &self.plan
    }

    pub const fn placement(&self) -> &ProgramEntryReceiverPlacementRecord {
        &self.placement
    }

    pub fn storage(&self) -> Option<&Extent> {
        self.partition.as_ref().map(OwnedExtentPartition::selected)
    }

    pub fn before(&self) -> Option<&Extent> {
        self.partition
            .as_ref()
            .and_then(OwnedExtentPartition::before)
    }

    pub fn after(&self) -> Option<&Extent> {
        self.partition
            .as_ref()
            .and_then(OwnedExtentPartition::after)
    }
}

/// Program-storage roots whose successful installation has also been recorded
/// in the installing bridge's artifact directory.
///
/// Keeping this wrapper distinct from [`InstalledProgramStorageRoots`] prevents
/// an ordinary successful handoff from silently skipping the required
/// non-authoritative completion record.
#[derive(Debug)]
pub struct RecordedProgramStorageInstallation {
    roots: InstalledProgramStorageRoots,
}

impl RecordedProgramStorageInstallation {
    pub const fn roots(&self) -> &InstalledProgramStorageRoots {
        &self.roots
    }

    pub fn installation_record(&self) -> ProgramStorageInstallationRecord {
        self.roots.installation_record()
    }

    pub const fn provider_invocation(&self) -> Option<ProgramStorageEntryProviderInvocation> {
        self.roots.provider_invocation
    }

    /// Release an installation that does not reserve an attached entry
    /// receiver. Receiver-bound installations must instead pass through
    /// [`RecordedProgramStorageInstallation::activate_receiver`], so callers
    /// cannot bypass the bridge's required ZII construction and exclusive
    /// activation loan.
    pub fn into_roots(
        self,
    ) -> Result<InstalledProgramStorageRoots, ProgramEntryReceiverActivationError> {
        if self.roots.receiver_storage.is_some() {
            return Err(ProgramEntryReceiverActivationError {
                installation: self,
                diagnostic: ProgramStorageEntryDiagnostic(
                    "receiver-bound program storage must be zeroed and activated before its roots are released"
                        .into(),
                ),
            });
        }
        Ok(self.roots)
    }

    /// Bind the exact mapped reservation, construct its checked ZII value, and
    /// lend it for this one source-entry activation.
    ///
    /// The backing slice is deliberately supplied by the installed physical
    /// bridge: an admitted numeric address alone is not a Rust memory mapping
    /// and cannot be dereferenced by the compiler. Exact base and length checks
    /// bind that physical mapping to the conserved reservation before any byte
    /// is written. Consuming `self` makes this the installation's only route to
    /// an activation; the returned token keeps the exclusive borrow live until
    /// the source activation finishes.
    pub fn activate_receiver(
        self,
        mapped_base: u64,
        mapped_storage: &mut [u8],
    ) -> Result<ProgramEntryReceiverActivation<'_>, ProgramEntryReceiverActivationError> {
        let Some(placement) = self
            .roots
            .receiver_storage
            .as_ref()
            .map(|receiver| receiver.placement.clone())
        else {
            return Err(ProgramEntryReceiverActivationError {
                installation: self,
                diagnostic: ProgramStorageEntryDiagnostic(
                    "program-storage installation has no entry receiver to activate".into(),
                ),
            });
        };
        let expected_length = match usize::try_from(placement.length) {
            Ok(length) => length,
            Err(_) => {
                return Err(ProgramEntryReceiverActivationError {
                    installation: self,
                    diagnostic: ProgramStorageEntryDiagnostic(
                        "entry receiver length does not fit the bridge address model".into(),
                    ),
                });
            }
        };
        if mapped_base != placement.base || mapped_storage.len() != expected_length {
            return Err(ProgramEntryReceiverActivationError {
                installation: self,
                diagnostic: ProgramStorageEntryDiagnostic(format!(
                    "entry receiver mapping must exactly cover 0x{:016x}..0x{:016x}, got base 0x{mapped_base:016x} and {} bytes",
                    placement.base,
                    placement.end(),
                    mapped_storage.len()
                )),
            });
        }

        mapped_storage.fill(0);
        Ok(ProgramEntryReceiverActivation {
            roots: self.roots,
            receiver: mapped_storage,
        })
    }
}

/// The one exclusive source-entry activation of a provisioned receiver.
///
/// This token is intentionally non-cloneable. Its lifetime holds the physical
/// bridge's exact mapped bytes exclusively, while its owned roots keep the
/// reservation and every conserved remainder alive.
#[derive(Debug)]
pub struct ProgramEntryReceiverActivation<'a> {
    roots: InstalledProgramStorageRoots,
    receiver: &'a mut [u8],
}

impl ProgramEntryReceiverActivation<'_> {
    pub const fn provider_invocation(&self) -> Option<ProgramStorageEntryProviderInvocation> {
        self.roots.provider_invocation
    }

    pub const fn placement(&self) -> &ProgramEntryReceiverPlacementRecord {
        &self
            .roots
            .receiver_storage
            .as_ref()
            .expect("receiver activation retains its reserved storage")
            .placement
    }

    /// The ZII-initialized occurrence lent as the selected machine's one
    /// `&mut self` activation.
    pub fn receiver(&mut self) -> &mut [u8] {
        self.receiver
    }

    /// End the source activation and return the installed authority with its
    /// receiver reservation and all remainders still conserved.
    pub fn finish(self) -> InstalledProgramStorageRoots {
        self.roots
    }
}

/// Failed receiver activation retains the recorded installation, allowing the
/// bridge to correct its mapping without replaying root admission.
#[derive(Debug)]
pub struct ProgramEntryReceiverActivationError {
    installation: RecordedProgramStorageInstallation,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl ProgramEntryReceiverActivationError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_installation(self) -> RecordedProgramStorageInstallation {
        self.installation
    }
}

impl std::fmt::Display for ProgramEntryReceiverActivationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramEntryReceiverActivationError {}

/// A generated physical bridge failed either while installing its selected
/// provider invocation or while binding the installed receiver reservation to
/// the exact mapped bytes. Each variant retains the still-live authority
/// needed to retry its own transition.
#[derive(Debug)]
pub enum ProgramStorageEntryBridgeError {
    Installation(ProgramStorageInstallationHandoffError),
    Activation(ProgramEntryReceiverActivationError),
}

impl std::fmt::Display for ProgramStorageEntryBridgeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Installation(error) => std::fmt::Display::fmt(error, formatter),
            Self::Activation(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for ProgramStorageEntryBridgeError {}

/// Report-only identity and geometry of one installed program-storage root.
/// This value carries no grant and cannot recreate an [`Extent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageInstalledExtentRecord {
    base: u64,
    length: u64,
    address_space: psi_extents::AddressSpaceId,
    rights: Vec<psi_extents::ExtentRightId>,
    provenance: psi_extents::ExtentProvenanceId,
    mapping_era: psi_extents::MappingEraId,
    origin: psi_extents::ExtentRootOrigin,
    lineage_root: psi_extents::ExtentLineageId,
}

/// Non-authoritative placement of the storage reserved beneath the installed
/// initial-storage root for one entry-receiver occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntryReceiverPlacementRecord {
    type_identity: String,
    base: u64,
    length: u64,
    alignment: u64,
    initial_storage_offset: u64,
    lineage_root: psi_extents::ExtentLineageId,
}

impl ProgramEntryReceiverPlacementRecord {
    pub fn type_identity(&self) -> &str {
        &self.type_identity
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn end(&self) -> u64 {
        self.base + self.length
    }

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    pub const fn initial_storage_offset(&self) -> u64 {
        self.initial_storage_offset
    }

    pub const fn lineage_root(&self) -> psi_extents::ExtentLineageId {
        self.lineage_root
    }
}

impl ProgramStorageInstalledExtentRecord {
    fn from_extent(extent: &Extent) -> Self {
        Self {
            base: extent.base(),
            length: extent.length(),
            address_space: extent.address_space(),
            rights: extent.rights().identities().collect(),
            provenance: extent.provenance(),
            mapping_era: extent.era(),
            origin: extent.origin(),
            lineage_root: extent.lineage_root(),
        }
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn end(&self) -> u64 {
        self.base + self.length
    }

    pub const fn address_space(&self) -> psi_extents::AddressSpaceId {
        self.address_space
    }

    pub fn rights(&self) -> &[psi_extents::ExtentRightId] {
        &self.rights
    }

    pub const fn provenance(&self) -> psi_extents::ExtentProvenanceId {
        self.provenance
    }

    pub const fn mapping_era(&self) -> psi_extents::MappingEraId {
        self.mapping_era
    }

    pub const fn origin(&self) -> psi_extents::ExtentRootOrigin {
        self.origin
    }

    pub const fn provider_issuance(&self) -> Option<psi_extents::ExtentProviderIssuance> {
        self.origin.provider_issuance()
    }

    pub const fn lineage_root(&self) -> psi_extents::ExtentLineageId {
        self.lineage_root
    }
}

/// Stable audit record produced only after both program-storage geometries
/// validate and both admitted grants are consumed. Cloning this record clones
/// observations, never authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageInstallationRecord {
    binding: ProgramStorageEntryPlanBinding,
    provider_invocation: Option<ProgramStorageEntryProviderInvocation>,
    image: ProgramStorageInstalledExtentRecord,
    initial_storage: ProgramStorageInstalledExtentRecord,
    receiver: Option<ProgramEntryReceiverPlacementRecord>,
}

impl ProgramStorageInstallationRecord {
    pub const fn binding(&self) -> &ProgramStorageEntryPlanBinding {
        &self.binding
    }

    pub const fn provider_invocation(&self) -> Option<ProgramStorageEntryProviderInvocation> {
        self.provider_invocation
    }

    pub const fn image(&self) -> &ProgramStorageInstalledExtentRecord {
        &self.image
    }

    pub const fn initial_storage(&self) -> &ProgramStorageInstalledExtentRecord {
        &self.initial_storage
    }

    pub const fn receiver(&self) -> Option<&ProgramEntryReceiverPlacementRecord> {
        self.receiver.as_ref()
    }
}

impl InstalledProgramStorageRoots {
    pub const fn binding(&self) -> &ProgramStorageEntryPlanBinding {
        &self.binding
    }

    pub const fn provider_invocation(&self) -> Option<ProgramStorageEntryProviderInvocation> {
        self.provider_invocation
    }

    pub const fn image(&self) -> &Extent {
        &self.image
    }

    /// Whole initial-storage authority is available only when the selected
    /// entry has no reserved receiver occurrence.
    pub const fn initial_storage(&self) -> Option<&Extent> {
        self.initial_storage.as_ref()
    }

    pub const fn receiver_storage(&self) -> Option<&ReservedProgramEntryReceiverStorage> {
        self.receiver_storage.as_ref()
    }

    pub fn installation_record(&self) -> ProgramStorageInstallationRecord {
        ProgramStorageInstallationRecord {
            binding: self.binding.clone(),
            provider_invocation: self.provider_invocation,
            image: ProgramStorageInstalledExtentRecord::from_extent(&self.image),
            initial_storage: self.initial_storage_record.clone(),
            receiver: self
                .receiver_storage
                .as_ref()
                .map(|receiver| receiver.placement.clone()),
        }
    }

    /// Derive a section/static view without splitting the installed image's
    /// ownership. Several disjoint or overlapping compiler views may coexist
    /// under the same one admitted image root.
    pub fn image_subextent(
        &self,
        offset: u64,
        length: u64,
    ) -> Result<InstalledImageSubextent<'_>, ProgramStorageEntryDiagnostic> {
        let loan = self.image.loan(offset, length).map_err(|diagnostic| {
            ProgramStorageEntryDiagnostic(format!(
                "installed image subextent is outside the admitted image root: {diagnostic}"
            ))
        })?;
        Ok(InstalledImageSubextent {
            binding: &self.binding,
            loan,
        })
    }

    /// Split one independently owned allocation from initial storage while
    /// retaining the installed image and every storage remainder.
    pub fn partition_initial_storage(
        self,
        offset: u64,
        length: u64,
    ) -> Result<PartitionedProgramStorageRoots, Box<ProgramStoragePartitionError>> {
        let Self {
            binding,
            provider_invocation,
            image,
            initial_storage,
            receiver_storage,
            initial_storage_record,
        } = self;
        let Some(initial_storage) = initial_storage else {
            return Err(Box::new(ProgramStoragePartitionError {
                roots: Self {
                    binding,
                    provider_invocation,
                    image,
                    initial_storage,
                    receiver_storage,
                    initial_storage_record,
                },
                diagnostic: ProgramStorageEntryDiagnostic(
                    "initial storage already reserves the selected entry receiver".into(),
                ),
            }));
        };
        match initial_storage.partition_owned(offset, length) {
            Ok(initial_storage) => Ok(PartitionedProgramStorageRoots {
                binding,
                provider_invocation,
                image,
                initial_storage,
                initial_storage_record,
            }),
            Err(error) => {
                let diagnostic = ProgramStorageEntryDiagnostic(format!(
                    "initial-storage allocation cannot be derived: {}",
                    error.diagnostic()
                ));
                Err(Box::new(ProgramStoragePartitionError {
                    roots: Self {
                        binding,
                        provider_invocation,
                        image,
                        initial_storage: Some(error.into_extent()),
                        receiver_storage,
                        initial_storage_record,
                    },
                    diagnostic,
                }))
            }
        }
    }
}

/// Borrowed compiler-derived range within the one admitted program image.
pub struct InstalledImageSubextent<'a> {
    binding: &'a ProgramStorageEntryPlanBinding,
    loan: ExtentLoan<'a>,
}

impl<'a> InstalledImageSubextent<'a> {
    pub const fn binding(&self) -> &'a ProgramStorageEntryPlanBinding {
        self.binding
    }

    pub const fn loan(&self) -> &ExtentLoan<'a> {
        &self.loan
    }
}

impl std::fmt::Debug for InstalledImageSubextent<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstalledImageSubextent")
            .field("root_slot", &self.binding.root_slot)
            .field("requirement_identity", &self.binding.requirement_identity)
            .field("loan", &self.loan)
            .finish()
    }
}

/// One owned allocation plus all authority needed to conserve its installed
/// initial-storage parent.
#[derive(Debug)]
pub struct PartitionedProgramStorageRoots {
    binding: ProgramStorageEntryPlanBinding,
    provider_invocation: Option<ProgramStorageEntryProviderInvocation>,
    image: Extent,
    initial_storage: OwnedExtentPartition,
    initial_storage_record: ProgramStorageInstalledExtentRecord,
}

impl PartitionedProgramStorageRoots {
    pub const fn binding(&self) -> &ProgramStorageEntryPlanBinding {
        &self.binding
    }

    pub const fn provider_invocation(&self) -> Option<ProgramStorageEntryProviderInvocation> {
        self.provider_invocation
    }

    pub const fn image(&self) -> &Extent {
        &self.image
    }

    pub const fn allocation(&self) -> &Extent {
        self.initial_storage.selected()
    }

    pub const fn before(&self) -> Option<&Extent> {
        self.initial_storage.before()
    }

    pub const fn after(&self) -> Option<&Extent> {
        self.initial_storage.after()
    }

    pub fn into_parts(
        self,
    ) -> (
        ProgramStorageEntryPlanBinding,
        Option<ProgramStorageEntryProviderInvocation>,
        Extent,
        OwnedExtentPartition,
    ) {
        (
            self.binding,
            self.provider_invocation,
            self.image,
            self.initial_storage,
        )
    }

    pub fn rejoin(self) -> InstalledProgramStorageRoots {
        InstalledProgramStorageRoots {
            binding: self.binding,
            provider_invocation: self.provider_invocation,
            image: self.image,
            initial_storage: Some(self.initial_storage.rejoin()),
            receiver_storage: None,
            initial_storage_record: self.initial_storage_record,
        }
    }
}

/// Bind a selected target entry to the stable core storage requirement.
///
/// Only the core requirement owner and semantic domain are recognized. The
/// selected target trait's name, source parameter names, registers, and stack
/// offsets play no role in identifying image versus initial storage.
pub fn bind_program_storage_entry_plan(
    selected: &SelectedProgramStorageEntryPlan,
    boundary: &ValidatedBoundaryEntryPlan,
    storage: &DerivedBoundaryEntryStorage,
) -> Result<ProgramStorageEntryPlanBinding, ProgramStorageEntryDiagnostic> {
    let matches = selected
        .schema
        .methods
        .iter()
        .filter(|method| {
            method.requirement_owner == PROGRAM_STORAGE_ENTRY_OWNER
                && method.name == PROGRAM_STORAGE_ENTRY_METHOD
        })
        .collect::<Vec<_>>();
    let [method] = matches.as_slice() else {
        return Err(ProgramStorageEntryDiagnostic(match matches.len() {
            0 => "selected target entry does not inherit ProgramStorageEntry::enter".into(),
            count => format!(
                "selected target entry carries {count} copies of ProgramStorageEntry::enter"
            ),
        }));
    };
    if method.requirement_identity.is_empty() {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage entry requirement identity cannot be empty".into(),
        ));
    }
    if method.parameter_count != 2 || method.parameter_type_identities.len() != 2 {
        return Err(ProgramStorageEntryDiagnostic(
            "ProgramStorageEntry::enter must retain exactly two semantic parameters".into(),
        ));
    }
    let boundary_fingerprint = boundary.contract_fingerprint();
    if method.calling_plan_fingerprint != Some(boundary_fingerprint) {
        return Err(ProgramStorageEntryDiagnostic(
            "selected target-entry schema is not bound to this validated boundary plan".into(),
        ));
    }
    if boundary.plan().call.parameters.len() != 2 {
        return Err(ProgramStorageEntryDiagnostic(
            "validated program-storage boundary plan must place exactly two parameters".into(),
        ));
    }

    let image = bind_parameter(method, boundary, storage, IMAGE_PARAMETER_INDEX)?;
    let initial_storage =
        bind_parameter(method, boundary, storage, INITIAL_STORAGE_PARAMETER_INDEX)?;
    Ok(ProgramStorageEntryPlanBinding {
        root_slot: selected.root_slot,
        requirement_identity: method.requirement_identity.clone(),
        boundary_contract_fingerprint: boundary_fingerprint,
        image,
        initial_storage,
        receiver: None,
    })
}

/// Join the selected root slot to the concrete entry-frame captures generated
/// for its source continuation. Parameter order comes from the checked entry
/// state; ABI shapes and placements come only from the retained evaluated plan.
pub fn bind_generated_program_storage_entry_plan(
    selected: &SelectedProgramStorageEntryPlan,
    plan: &omega_calling_conventions::BoundaryEntryPlan,
    runtime_storage: &omega_runtime_storage::RuntimeStoragePlan,
    layouts: &omega_layout::LayoutPlan,
    entry_key: omega_control_flow::StateKey,
    receiver_type_identity: Option<&str>,
) -> Result<ProgramStorageEntryPlanBinding, ProgramStorageEntryDiagnostic> {
    let signature = omega_calling_conventions::CallSignature {
        parameters: plan
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: plan.call.result.as_ref().map(|placement| placement.shape),
    };
    let boundary =
        omega_calling_conventions::validate_boundary_entry_plan(plan.clone(), &signature).map_err(
            |diagnostic| {
                ProgramStorageEntryDiagnostic(format!(
                    "retained program-entry calling plan is invalid: {diagnostic}"
                ))
            },
        )?;
    let mut slots = runtime_storage
        .frame_slots
        .iter()
        .filter_map(|(_, slot)| {
            (slot.source_key == entry_key
                && matches!(
                    slot.kind,
                    omega_runtime_storage::RuntimeFrameSlotKind::Parameter
                ))
            .then_some(slot)
        })
        .collect::<Vec<_>>();
    slots.sort_unstable_by_key(|slot| slot.byte_offset);
    if slots.len() != boundary.plan().call.parameters.len() {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "generated program entry retains {} parameter slots for {} calling-plan parameters",
            slots.len(),
            boundary.plan().call.parameters.len()
        )));
    }
    let mut destinations = Vec::with_capacity(slots.len());
    for (index, (slot, placement)) in slots
        .into_iter()
        .zip(boundary.plan().call.parameters.iter())
        .enumerate()
    {
        if slot.byte_size != usize::from(placement.shape.byte_size) {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "generated program-entry parameter {index} reserves {} bytes, but its retained calling plan places {}",
                slot.byte_size, placement.shape.byte_size
            )));
        }
        destinations.push((slot.byte_offset, placement.shape));
    }
    let storage = omega_instruction_selection::derive_boundary_entry_storage(
        boundary.plan(),
        &destinations,
        None,
        None,
    )
    .map_err(|diagnostic| {
        ProgramStorageEntryDiagnostic(format!(
            "cannot derive generated program-entry captures: {diagnostic}"
        ))
    })?;
    let mut binding = bind_program_storage_entry_plan(selected, &boundary, &storage)?;
    if let Some(type_identity) = receiver_type_identity {
        let layout = layouts
            .machine_layouts
            .iter()
            .find_map(|(_, layout)| (layout.symbol == entry_key.machine).then_some(layout.layout))
            .ok_or_else(|| {
                ProgramStorageEntryDiagnostic(
                    "selected entry receiver has no concrete machine layout".into(),
                )
            })?;
        binding = binding.with_checked_receiver_layout(type_identity.to_owned(), layout)?;
    }
    Ok(binding)
}

fn bind_parameter(
    method: &omega_effects::provider_plan::ServiceMethod,
    boundary: &ValidatedBoundaryEntryPlan,
    storage: &DerivedBoundaryEntryStorage,
    parameter_index: usize,
) -> Result<ProgramStorageEntryParameter, ProgramStorageEntryDiagnostic> {
    let claims = method
        .entry_claims
        .iter()
        .filter(|claim| claim.parameter_index == parameter_index)
        .collect::<Vec<_>>();
    let [claim] = claims.as_slice() else {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "ProgramStorageEntry::enter parameter {parameter_index} carries {} routed entry claims instead of exactly one",
            claims.len()
        )));
    };
    if claim.domain != GRANTED_DOMAIN
        || !claim.predicate_body.is_present()
        || claim.effective_carry != psi_language_semantics::CarryPolicy::STRICT
        || claim.authority_flow != ServiceEntryAuthorityFlow::Accepts
    {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "ProgramStorageEntry::enter parameter {parameter_index} does not carry the exact strict accepted Extent::Granted claim"
        )));
    }
    let placement = boundary
        .plan()
        .call
        .parameters
        .get(parameter_index)
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(format!(
                "validated boundary plan has no placement for program-storage parameter {parameter_index}"
            ))
        })?;
    let capture = storage.parameter(parameter_index).ok_or_else(|| {
        ProgramStorageEntryDiagnostic(format!(
            "generated entry prologue has no capture for program-storage parameter {parameter_index}"
        ))
    })?;
    if capture.placement != *placement {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "generated entry capture for program-storage parameter {parameter_index} drifted from the validated boundary plan"
        )));
    }
    Ok(ProgramStorageEntryParameter {
        parameter_index,
        parameter_type_identity: method.parameter_type_identities[parameter_index].clone(),
        domain: claim.domain.clone(),
        effective_carry: claim.effective_carry,
        placement: placement.clone(),
        destination_byte_offset: capture.destination_byte_offset,
        write_range: capture.write_range.clone(),
    })
}

/// Validate both `no_wrap` obligations before importing either complete root,
/// then consume the two provider-admitted grants in semantic position order.
fn install_program_storage_entry_roots_unrecorded(
    binding: ProgramStorageEntryPlanBinding,
    provider_invocation: Option<ProgramStorageEntryProviderInvocation>,
    image: ProgramStorageRootInput,
    initial_storage: ProgramStorageRootInput,
) -> Result<InstalledProgramStorageRoots, Box<ProgramStorageRootInstallationError>> {
    if let Some(provider_invocation) = provider_invocation {
        let validation = validate_physical_provider_root("image", &image, provider_invocation)
            .and_then(|()| {
                validate_physical_provider_root(
                    "initial-storage",
                    &initial_storage,
                    provider_invocation,
                )
            });
        if let Err(diagnostic) = validation {
            return Err(Box::new(ProgramStorageRootInstallationError {
                binding,
                image,
                initial_storage,
                diagnostic,
            }));
        }
    }

    let image_geometry = match ValidatedExtentGeometry::check(image.base, image.length) {
        Ok(geometry) => geometry,
        Err(diagnostic) => {
            return Err(Box::new(ProgramStorageRootInstallationError {
                binding,
                image,
                initial_storage,
                diagnostic: ProgramStorageEntryDiagnostic(format!(
                    "image root does not satisfy Extent::Granted no_wrap: {diagnostic}"
                )),
            }));
        }
    };
    let storage_geometry = match ValidatedExtentGeometry::check(
        initial_storage.base,
        initial_storage.length,
    ) {
        Ok(geometry) => geometry,
        Err(diagnostic) => {
            return Err(Box::new(ProgramStorageRootInstallationError {
                binding,
                image,
                initial_storage,
                diagnostic: ProgramStorageEntryDiagnostic(format!(
                    "initial-storage root does not satisfy Extent::Granted no_wrap: {diagnostic}"
                )),
            }));
        }
    };

    let receiver_placement = match binding.receiver.as_ref() {
        Some(receiver) => match receiver_placement(
            receiver,
            initial_storage.base,
            initial_storage.length,
            initial_storage.grant.lineage_root(),
        ) {
            Ok(placement) => Some(placement),
            Err(diagnostic) => {
                return Err(Box::new(ProgramStorageRootInstallationError {
                    binding,
                    image,
                    initial_storage,
                    diagnostic,
                }));
            }
        },
        None => None,
    };

    let image = image.grant.mint_validated(image_geometry);
    let initial_storage = initial_storage.grant.mint_validated(storage_geometry);
    let initial_storage_record = ProgramStorageInstalledExtentRecord::from_extent(&initial_storage);
    let (initial_storage, receiver_storage) = match receiver_placement {
        Some(placement) if placement.length != 0 => {
            let partition = initial_storage
                .partition_owned(placement.initial_storage_offset, placement.length)
                .expect("receiver placement was validated before consuming either grant");
            let plan = binding
                .receiver
                .clone()
                .expect("receiver placement requires a checked receiver plan");
            (
                None,
                Some(ReservedProgramEntryReceiverStorage {
                    plan,
                    placement,
                    partition: Some(partition),
                }),
            )
        }
        Some(placement) => {
            let plan = binding
                .receiver
                .clone()
                .expect("receiver placement requires a checked receiver plan");
            (
                Some(initial_storage),
                Some(ReservedProgramEntryReceiverStorage {
                    plan,
                    placement,
                    partition: None,
                }),
            )
        }
        None => (Some(initial_storage), None),
    };

    Ok(InstalledProgramStorageRoots {
        binding,
        provider_invocation,
        image,
        initial_storage,
        receiver_storage,
        initial_storage_record,
    })
}

fn validate_physical_provider_root(
    role: &str,
    input: &ProgramStorageRootInput,
    selected: ProgramStorageEntryProviderInvocation,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let Some(issuance) = input.grant.origin().provider_issuance() else {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "{role} root is not issued by the selected physical entry provider invocation"
        )));
    };
    if !selected.matches(issuance) {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "{role} root does not belong to the selected physical entry provider, plan, and invocation"
        )));
    }
    Ok(())
}

fn receiver_placement(
    plan: &ProgramEntryReceiverStoragePlan,
    storage_base: u64,
    storage_length: u64,
    lineage_root: psi_extents::ExtentLineageId,
) -> Result<ProgramEntryReceiverPlacementRecord, ProgramStorageEntryDiagnostic> {
    let alignment = u64::try_from(plan.byte_alignment).map_err(|_| {
        ProgramStorageEntryDiagnostic(
            "entry receiver alignment does not fit the target address model".into(),
        )
    })?;
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "entry receiver requires invalid {alignment}-byte alignment"
        )));
    }
    let length = u64::try_from(plan.byte_size).map_err(|_| {
        ProgramStorageEntryDiagnostic(
            "entry receiver size does not fit the target address model".into(),
        )
    })?;
    let aligned_base = storage_base
        .checked_add(alignment - 1)
        .map(|address| address & !(alignment - 1))
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(
                "initial-storage base cannot be aligned for the selected entry receiver".into(),
            )
        })?;
    let offset = aligned_base - storage_base;
    let end = offset.checked_add(length).ok_or_else(|| {
        ProgramStorageEntryDiagnostic("entry receiver storage range overflows".into())
    })?;
    if end > storage_length {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "initial storage cannot reserve the selected entry receiver: aligned range {offset}..{end} exceeds {storage_length} bytes"
        )));
    }
    Ok(ProgramEntryReceiverPlacementRecord {
        type_identity: plan.type_identity.clone(),
        base: aligned_base,
        length,
        alignment,
        initial_storage_offset: offset,
        lineage_root,
    })
}

/// Install two compiler-provisioned program-storage roots and emit the
/// completion record before releasing the installed authority.
///
/// This is the sealed local-provisioning seam. Provider-issued physical roots
/// must use [`install_program_storage_entry_provider_invocation`], which joins
/// them to the compiler-selected provider plan and concrete invocation.
/// Predicate rejection returns both unconsumed grants. If record emission
/// fails after installation, the installed roots remain sealed inside the
/// error and can only be recovered by successfully retrying the record write.
pub fn install_program_storage_entry_roots(
    artifact_directory: &Path,
    binding: ProgramStorageEntryPlanBinding,
    image: ProgramStorageRootInput,
    initial_storage: ProgramStorageRootInput,
) -> Result<RecordedProgramStorageInstallation, ProgramStorageInstallationHandoffError> {
    let validation = validate_compiler_provisioned_root("image", &image)
        .and_then(|()| validate_compiler_provisioned_root("initial-storage", &initial_storage));
    if let Err(diagnostic) = validation {
        return Err(ProgramStorageInstallationHandoffError::Rejected(Box::new(
            ProgramStorageRootInstallationError {
                binding,
                image,
                initial_storage,
                diagnostic,
            },
        )));
    }
    let roots =
        install_program_storage_entry_roots_unrecorded(binding, None, image, initial_storage)
            .map_err(ProgramStorageInstallationHandoffError::Rejected)?;
    record_program_storage_installation(artifact_directory, roots)
}

/// Install the two roots supplied by one exact selected physical-provider
/// invocation and retain that occurrence identity through audit and entry
/// activation.
///
/// This is the production-facing installation carrier consumed by a generated
/// bridge. It does not itself emit or invoke native code. Unlike the local
/// provisioning seam, it rejects compiler-provisioned grants and roots from
/// another provider plan or invocation before consuming either grant.
pub fn install_program_storage_entry_provider_invocation(
    artifact_directory: &Path,
    binding: ProgramStorageEntryPlanBinding,
    selected_provider: &super::provider_plans::SelectedExternalRootProviderPlan,
    provider_issuance: psi_extents::ExtentProviderIssuance,
    image: ProgramStorageRootInput,
    initial_storage: ProgramStorageRootInput,
) -> Result<RecordedProgramStorageInstallation, ProgramStorageInstallationHandoffError> {
    let provider_invocation = match ProgramStorageEntryProviderInvocation::bind_selected_provider(
        &binding,
        selected_provider,
        provider_issuance,
    ) {
        Ok(invocation) => invocation,
        Err(diagnostic) => {
            return Err(ProgramStorageInstallationHandoffError::Rejected(Box::new(
                ProgramStorageRootInstallationError {
                    binding,
                    image,
                    initial_storage,
                    diagnostic,
                },
            )));
        }
    };
    let roots = install_program_storage_entry_roots_unrecorded(
        binding,
        Some(provider_invocation),
        image,
        initial_storage,
    )
    .map_err(ProgramStorageInstallationHandoffError::Rejected)?;
    record_program_storage_installation(artifact_directory, roots)
}

/// Prepare a receiver-bearing generated bridge in one linear handoff: install
/// exact invocation roots, emit the completion audit, bind the exact mapped
/// reservation, construct its ZII value, and return the exclusive activation
/// loan that a native bridge caller must pass to the source continuation.
pub fn install_and_activate_program_storage_entry_receiver<'mapping>(
    artifact_directory: &Path,
    binding: ProgramStorageEntryPlanBinding,
    selected_provider: &super::provider_plans::SelectedExternalRootProviderPlan,
    provider_issuance: psi_extents::ExtentProviderIssuance,
    image: ProgramStorageRootInput,
    initial_storage: ProgramStorageRootInput,
    mapped_base: u64,
    mapped_storage: &'mapping mut [u8],
) -> Result<ProgramEntryReceiverActivation<'mapping>, ProgramStorageEntryBridgeError> {
    let installation = install_program_storage_entry_provider_invocation(
        artifact_directory,
        binding,
        selected_provider,
        provider_issuance,
        image,
        initial_storage,
    )
    .map_err(ProgramStorageEntryBridgeError::Installation)?;
    installation
        .activate_receiver(mapped_base, mapped_storage)
        .map_err(ProgramStorageEntryBridgeError::Activation)
}

fn validate_compiler_provisioned_root(
    role: &str,
    input: &ProgramStorageRootInput,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    if input.grant.origin().compiler_provisioning().is_none() {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "{role} root is provider-issued; use the selected physical-provider invocation installer"
        )));
    }
    Ok(())
}

fn record_program_storage_installation(
    artifact_directory: &Path,
    roots: InstalledProgramStorageRoots,
) -> Result<RecordedProgramStorageInstallation, ProgramStorageInstallationHandoffError> {
    let record = roots.installation_record();
    match super::artifacts::write_program_storage_installation_record(artifact_directory, &record) {
        Ok(()) => Ok(RecordedProgramStorageInstallation { roots }),
        Err(diagnostic) => Err(ProgramStorageInstallationHandoffError::Record(Box::new(
            ProgramStorageRecordEmissionError { roots, diagnostic },
        ))),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryDiagnostic(pub String);

impl std::fmt::Display for ProgramStorageEntryDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ProgramStorageEntryDiagnostic {}

#[derive(Debug)]
pub struct ProgramStorageRootInstallationError {
    binding: ProgramStorageEntryPlanBinding,
    image: ProgramStorageRootInput,
    initial_storage: ProgramStorageRootInput,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl ProgramStorageRootInstallationError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        ProgramStorageEntryPlanBinding,
        ProgramStorageRootInput,
        ProgramStorageRootInput,
    ) {
        (self.binding, self.image, self.initial_storage)
    }
}

impl std::fmt::Display for ProgramStorageRootInstallationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStorageRootInstallationError {}

/// A program-storage handoff either failed before consuming its grants or
/// installed them but could not yet persist its completion record.
#[derive(Debug)]
pub enum ProgramStorageInstallationHandoffError {
    Rejected(Box<ProgramStorageRootInstallationError>),
    Record(Box<ProgramStorageRecordEmissionError>),
}

impl std::fmt::Display for ProgramStorageInstallationHandoffError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(error) => std::fmt::Display::fmt(error, formatter),
            Self::Record(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for ProgramStorageInstallationHandoffError {}

/// Installed authority retained across an audit-artifact write failure.
/// Retrying the write is the only route back to usable installed roots.
#[derive(Debug)]
pub struct ProgramStorageRecordEmissionError {
    roots: InstalledProgramStorageRoots,
    diagnostic: psi_diagnostics::Diagnostic,
}

impl ProgramStorageRecordEmissionError {
    pub const fn diagnostic(&self) -> &psi_diagnostics::Diagnostic {
        &self.diagnostic
    }

    pub fn retry(
        self,
        artifact_directory: &Path,
    ) -> Result<RecordedProgramStorageInstallation, ProgramStorageInstallationHandoffError> {
        record_program_storage_installation(artifact_directory, self.roots)
    }
}

impl std::fmt::Display for ProgramStorageRecordEmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStorageRecordEmissionError {}

#[derive(Debug)]
pub struct ProgramStoragePartitionError {
    roots: InstalledProgramStorageRoots,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl ProgramStoragePartitionError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_roots(self) -> InstalledProgramStorageRoots {
        self.roots
    }
}

impl std::fmt::Display for ProgramStoragePartitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStoragePartitionError {}
