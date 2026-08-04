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
use psi_extents::{Extent, ExtentRootGrant, ValidatedExtentGeometry};

use super::SelectedExternalRootProviderPlan;

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
    provider_plan: omega_external_roots::ProviderPlanId,
    requirement_identity: String,
    boundary_contract_fingerprint: u64,
    image: ProgramStorageEntryParameter,
    initial_storage: ProgramStorageEntryParameter,
}

impl ProgramStorageEntryPlanBinding {
    pub const fn provider_plan(&self) -> omega_external_roots::ProviderPlanId {
        self.provider_plan
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

    pub fn into_parts(self) -> (ProgramStorageEntryPlanBinding, Extent, Extent) {
        (self.binding, self.image, self.initial_storage)
    }
}

/// Bind a selected target entry to the stable core storage requirement.
///
/// Only the core requirement owner and semantic domain are recognized. The
/// selected target trait's name, source parameter names, registers, and stack
/// offsets play no role in identifying image versus initial storage.
pub fn bind_program_storage_entry_plan(
    selected: &SelectedExternalRootProviderPlan,
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
        provider_plan: selected.identity,
        requirement_identity: method.requirement_identity.clone(),
        boundary_contract_fingerprint: boundary_fingerprint,
        image,
        initial_storage,
    })
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
