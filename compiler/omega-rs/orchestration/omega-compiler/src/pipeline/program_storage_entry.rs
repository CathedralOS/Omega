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
    image: Extent,
    initial_storage: Extent,
}

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
    lineage_root: psi_extents::ExtentLineageId,
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
    image: ProgramStorageInstalledExtentRecord,
    initial_storage: ProgramStorageInstalledExtentRecord,
}

impl ProgramStorageInstallationRecord {
    pub const fn binding(&self) -> &ProgramStorageEntryPlanBinding {
        &self.binding
    }

    pub const fn image(&self) -> &ProgramStorageInstalledExtentRecord {
        &self.image
    }

    pub const fn initial_storage(&self) -> &ProgramStorageInstalledExtentRecord {
        &self.initial_storage
    }
}

impl InstalledProgramStorageRoots {
    pub const fn binding(&self) -> &ProgramStorageEntryPlanBinding {
        &self.binding
    }

    pub const fn image(&self) -> &Extent {
        &self.image
    }

    pub const fn initial_storage(&self) -> &Extent {
        &self.initial_storage
    }

    pub fn installation_record(&self) -> ProgramStorageInstallationRecord {
        ProgramStorageInstallationRecord {
            binding: self.binding.clone(),
            image: ProgramStorageInstalledExtentRecord::from_extent(&self.image),
            initial_storage: ProgramStorageInstalledExtentRecord::from_extent(
                &self.initial_storage,
            ),
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
            image,
            initial_storage,
        } = self;
        match initial_storage.partition_owned(offset, length) {
            Ok(initial_storage) => Ok(PartitionedProgramStorageRoots {
                binding,
                image,
                initial_storage,
            }),
            Err(error) => {
                let diagnostic = ProgramStorageEntryDiagnostic(format!(
                    "initial-storage allocation cannot be derived: {}",
                    error.diagnostic()
                ));
                Err(Box::new(ProgramStoragePartitionError {
                    roots: Self {
                        binding,
                        image,
                        initial_storage: error.into_extent(),
                    },
                    diagnostic,
                }))
            }
        }
    }

    pub fn into_parts(self) -> (ProgramStorageEntryPlanBinding, Extent, Extent) {
        (self.binding, self.image, self.initial_storage)
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
    image: Extent,
    initial_storage: OwnedExtentPartition,
}

impl PartitionedProgramStorageRoots {
    pub const fn binding(&self) -> &ProgramStorageEntryPlanBinding {
        &self.binding
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

    pub fn into_parts(self) -> (ProgramStorageEntryPlanBinding, Extent, OwnedExtentPartition) {
        (self.binding, self.image, self.initial_storage)
    }

    pub fn rejoin(self) -> InstalledProgramStorageRoots {
        InstalledProgramStorageRoots {
            binding: self.binding,
            image: self.image,
            initial_storage: self.initial_storage.rejoin(),
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
    })
}

/// Join the selected root slot to the concrete entry-frame captures generated
/// for its source continuation. Parameter order comes from the checked entry
/// state; ABI shapes and placements come only from the retained evaluated plan.
pub fn bind_generated_program_storage_entry_plan(
    selected: &SelectedProgramStorageEntryPlan,
    plan: &omega_calling_conventions::BoundaryEntryPlan,
    runtime_storage: &omega_runtime_storage::RuntimeStoragePlan,
    entry_key: omega_control_flow::StateKey,
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
    bind_program_storage_entry_plan(selected, &boundary, &storage)
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
pub fn install_program_storage_entry_roots(
    binding: ProgramStorageEntryPlanBinding,
    image: ProgramStorageRootInput,
    initial_storage: ProgramStorageRootInput,
) -> Result<InstalledProgramStorageRoots, Box<ProgramStorageRootInstallationError>> {
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

    Ok(InstalledProgramStorageRoots {
        binding,
        image: image.grant.mint_validated(image_geometry),
        initial_storage: initial_storage.grant.mint_validated(storage_geometry),
    })
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
