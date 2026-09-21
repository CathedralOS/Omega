//! Provisional provider-claim scaffolding for device/DMA ordering roles.
//!
//! This module closes only the non-authorizing requirement/discharge shape.
//! A structurally closed row retains one provider assertion for the exact
//! emitted demand. It does not prove provider selection/admission, that the
//! operation ran, mint publication or completion evidence, establish
//! synchronization, or authorize lowering.
//!
//! No checked source operation emits these rows. Current constructions are
//! structural tests, not evidence for a source contract, and the provisional
//! role-coordinate rows are not public ABI. A complete admitted DMA boundary
//! may keep these roles provider-private; a future checked-driver surface
//! must derive role-specific payloads from its actual typed operations.

use crate::{AccessPlanDiagnostic, SchemaDeviceCorrespondenceReceiptContext};
use extents::MappedRangeReceiptContext;
use std::collections::{BTreeMap, BTreeSet};

macro_rules! normalized_identity {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn from_normalized_identity(identity: u64) -> Result<Self, AccessPlanDiagnostic> {
                if identity == 0 {
                    return Err(AccessPlanDiagnostic(
                        concat!($label, " cannot be zero").into(),
                    ));
                }
                Ok(Self(identity))
            }

            pub const fn normalized_identity(self) -> u64 {
                self.0
            }
        }
    };
}

normalized_identity!(
    DeviceOperationRequirementId,
    "device-operation requirement identity"
);
normalized_identity!(DeviceOrderingScopeId, "device-ordering scope identity");
normalized_identity!(
    DeviceOrderingScopeOccurrenceId,
    "device ordering-scope occurrence identity"
);

/// Opaque provider-issued occurrence of one admitted ordering-scope
/// capability.
///
/// Per [device ordering](wiki/spec/resources/device_access.md), build
/// selection admits a provider and its scope-capability schema — the
/// `DeviceOrderingScopeId` coordinate on the requirement — but runtime scope
/// occurrences are issued only by the installed provider. Source code can
/// carry this token; it cannot construct one except through provider
/// assertion, inspect its identity, or compare it to another occurrence, so
/// the token intentionally has no `Clone`, `PartialEq`, or identity accessor.
#[derive(Debug)]
#[must_use = "ordering-scope occurrence is provider-issued runtime scope evidence"]
pub struct DeviceOrderingScopeOccurrence {
    scope: DeviceOrderingScopeId,
    occurrence: DeviceOrderingScopeOccurrenceId,
}

impl DeviceOrderingScopeOccurrence {
    /// The installed provider issues a runtime occurrence of an admitted
    /// scope capability. Construction is provider assertion: it records the
    /// occurrence, it does not establish ordering or admission itself.
    pub const fn from_provider_assertion(
        scope: DeviceOrderingScopeId,
        occurrence: DeviceOrderingScopeOccurrenceId,
    ) -> Self {
        Self { scope, occurrence }
    }

    /// The admitted scope-capability coordinate this occurrence was issued
    /// under — not the occurrence identity, which stays opaque to source.
    pub const fn scope_capability(&self) -> DeviceOrderingScopeId {
        self.scope
    }

    const fn occurrence_identity(&self) -> DeviceOrderingScopeOccurrenceId {
        self.occurrence
    }
}
normalized_identity!(
    DeviceOperationProviderPlanId,
    "device-operation provider-plan identity"
);

/// Provisional closed device-operation roles. Portable atomic and checked-ISA
/// fences are intentionally absent: they have different participants and
/// contracts. The role discriminant must enter any future canonical identity;
/// it is never merely a payload-decoding selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviceOperation {
    DmaPublication,
    DeviceAcquisition,
    CacheMaintenance,
    MmioNotification,
    PostedWriteCompletion,
}

/// The role-specific coordinate set carried by one emitted device-operation
/// demand.
///
/// Per [device ordering](wiki/spec/resources/device_access.md), roles may
/// relate different data, descriptor, doorbell, read-back, request, or
/// completion coordinates — a uniform one-range carrier is not a public ABI.
/// Carrying the role on the variant makes a role/coordinate mismatch
/// unrepresentable, and the discriminant still participates in canonical
/// identity through enum equality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceOperationCoordinates {
    /// DMA publication names the data range it publishes and the descriptor
    /// place carrying its current write state.
    DmaPublication {
        data: MappedRangeReceiptContext,
        descriptor: MappedRangeReceiptContext,
    },
    /// Acquisition consumes a completion tied to the same request.
    DeviceAcquisition {
        request: MappedRangeReceiptContext,
        completion: MappedRangeReceiptContext,
    },
    /// Cache maintenance names only the maintained range.
    CacheMaintenance {
        maintained: MappedRangeReceiptContext,
    },
    /// An MMIO notification relates the doorbell place to its request.
    MmioNotification {
        doorbell: MappedRangeReceiptContext,
        request: MappedRangeReceiptContext,
    },
    /// Posted-write completion ties the posted request to the completion that
    /// proves the device observed it; a bare write proves nothing.
    PostedWriteCompletion {
        request: MappedRangeReceiptContext,
        completion: MappedRangeReceiptContext,
    },
}

impl DeviceOperationCoordinates {
    /// The ordering role this coordinate set belongs to.
    pub const fn operation(&self) -> DeviceOperation {
        match self {
            Self::DmaPublication { .. } => DeviceOperation::DmaPublication,
            Self::DeviceAcquisition { .. } => DeviceOperation::DeviceAcquisition,
            Self::CacheMaintenance { .. } => DeviceOperation::CacheMaintenance,
            Self::MmioNotification { .. } => DeviceOperation::MmioNotification,
            Self::PostedWriteCompletion { .. } => DeviceOperation::PostedWriteCompletion,
        }
    }
}

/// One provisional candidate demand for structural provider claim.
///
/// Coordinate and schema/device fields are full opaque structural contexts,
/// not compact IDs. The ordering-scope ID remains an inert nominal
/// coordinate; no ordering relation or executable event is inferred from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceOperationRequirement {
    identity: DeviceOperationRequirementId,
    coordinates: DeviceOperationCoordinates,
    correspondence: SchemaDeviceCorrespondenceReceiptContext,
    ordering_scope: DeviceOrderingScopeId,
}

impl DeviceOperationRequirement {
    pub fn new(
        identity: DeviceOperationRequirementId,
        coordinates: DeviceOperationCoordinates,
        correspondence: SchemaDeviceCorrespondenceReceiptContext,
        ordering_scope: DeviceOrderingScopeId,
    ) -> Self {
        Self {
            identity,
            coordinates,
            correspondence,
            ordering_scope,
        }
    }

    pub const fn identity(&self) -> DeviceOperationRequirementId {
        self.identity
    }

    pub const fn operation(&self) -> DeviceOperation {
        self.coordinates.operation()
    }

    pub const fn coordinates(&self) -> &DeviceOperationCoordinates {
        &self.coordinates
    }

    pub const fn correspondence(&self) -> &SchemaDeviceCorrespondenceReceiptContext {
        &self.correspondence
    }

    pub const fn ordering_scope(&self) -> DeviceOrderingScopeId {
        self.ordering_scope
    }
}

/// Non-clonable provider assertion for one exact emitted requirement,
/// carrying the runtime ordering-scope occurrence the provider issued for it.
///
/// Construction snapshots the complete demand instead of asking a provider to
/// restate public IDs or geometry, and binds the provider-issued scope
/// occurrence covering the requirement's scope capability so a closed row
/// cannot name an ordering scope the provider never issued. The provider-plan
/// ID is provenance for the asserted claim row; it is not admission,
/// operation, or mapping authority.
#[derive(Debug)]
#[must_use = "device-operation claim retains one exact provider assertion"]
pub struct ProviderAssertedDeviceOperationClaim {
    provider_plan: DeviceOperationProviderPlanId,
    requirement: DeviceOperationRequirement,
    scope_occurrence: DeviceOrderingScopeOccurrence,
}

impl ProviderAssertedDeviceOperationClaim {
    pub fn from_provider_assertion(
        provider_plan: DeviceOperationProviderPlanId,
        requirement: &DeviceOperationRequirement,
        scope_occurrence: DeviceOrderingScopeOccurrence,
    ) -> Result<Self, AccessPlanDiagnostic> {
        if scope_occurrence.scope != requirement.ordering_scope {
            return Err(AccessPlanDiagnostic(format!(
                "ordering-scope occurrence {} does not cover the demanded scope capability {}",
                scope_occurrence.occurrence_identity().normalized_identity(),
                requirement.ordering_scope.normalized_identity()
            )));
        }
        Ok(Self {
            provider_plan,
            requirement: requirement.clone(),
            scope_occurrence,
        })
    }

    pub const fn provider_plan(&self) -> DeviceOperationProviderPlanId {
        self.provider_plan
    }

    /// The provider-issued runtime scope occurrence bound into this claim.
    /// Opaque to consumers: carryable, never inspectable or comparable.
    pub const fn scope_occurrence(&self) -> &DeviceOrderingScopeOccurrence {
        &self.scope_occurrence
    }

    pub const fn requirement(&self) -> &DeviceOperationRequirement {
        &self.requirement
    }
}

/// One exact demand joined to one exact provider-asserted claim row.
#[derive(Debug)]
pub struct StructurallyClosedDeviceOperationRequirement {
    requirement: DeviceOperationRequirement,
    claim: ProviderAssertedDeviceOperationClaim,
}

impl StructurallyClosedDeviceOperationRequirement {
    pub const fn requirement(&self) -> &DeviceOperationRequirement {
        &self.requirement
    }

    pub const fn provider_plan(&self) -> DeviceOperationProviderPlanId {
        self.claim.provider_plan
    }

    /// The runtime ordering-scope occurrence the provider issued under this
    /// admitted coverage row. Opaque: a consumer may carry it but cannot
    /// construct, inspect, or compare its identity.
    pub const fn scope_occurrence(&self) -> &DeviceOrderingScopeOccurrence {
        self.claim.scope_occurrence()
    }

    fn validate_structure(&self) -> Result<(), AccessPlanDiagnostic> {
        if self.requirement != self.claim.requirement {
            return Err(AccessPlanDiagnostic(
                "structurally closed device-operation row no longer matches its provider assertion"
                    .into(),
            ));
        }
        Ok(())
    }
}

/// Exact closed set of supplied candidate device-operation demands.
#[derive(Debug)]
#[must_use = "structurally closed device-operation requirements retain exact provider assertions"]
pub struct StructurallyClosedDeviceOperationRequirements {
    rows: Vec<StructurallyClosedDeviceOperationRequirement>,
}

impl StructurallyClosedDeviceOperationRequirements {
    pub fn rows(&self) -> &[StructurallyClosedDeviceOperationRequirement] {
        &self.rows
    }

    /// Independently replay the sealed one-to-one closure without granting any
    /// device event, publication, completion, custody, or lowering authority.
    pub fn validate_structure(&self) -> Result<(), AccessPlanDiagnostic> {
        let mut identities = BTreeSet::new();
        for row in &self.rows {
            if !identities.insert(row.requirement.identity) {
                return Err(AccessPlanDiagnostic(
                    "structurally closed device-operation requirements contain a duplicate emitted identity"
                        .into(),
                ));
            }
            row.validate_structure()?;
        }
        Ok(())
    }
}

/// Failed exact closure. Every input is returned in its original order so a
/// caller can repair the row set and retry without losing non-Clone evidence.
#[derive(Debug)]
pub struct DeviceOperationStructuralClosureError {
    requirements: Vec<DeviceOperationRequirement>,
    claims: Vec<ProviderAssertedDeviceOperationClaim>,
    diagnostic: AccessPlanDiagnostic,
}

impl DeviceOperationStructuralClosureError {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        Vec<DeviceOperationRequirement>,
        Vec<ProviderAssertedDeviceOperationClaim>,
    ) {
        (self.requirements, self.claims)
    }
}

/// Close every emitted demand against exactly one supplied provider claim.
///
/// Validation borrows the complete input first. No evidence is consumed into
/// a success row until duplicate, missing, extra, and structural-drift checks
/// all pass.
pub fn structurally_close_device_operation_requirements(
    requirements: Vec<DeviceOperationRequirement>,
    claims: Vec<ProviderAssertedDeviceOperationClaim>,
) -> Result<StructurallyClosedDeviceOperationRequirements, DeviceOperationStructuralClosureError> {
    let claim_order = match validate_device_operation_requirements(&requirements, &claims) {
        Ok(claim_order) => claim_order,
        Err(diagnostic) => {
            return Err(DeviceOperationStructuralClosureError {
                requirements,
                claims,
                diagnostic,
            });
        }
    };

    // Validation above borrowed the original vectors. Consumption and
    // normalization into emitted-requirement order begins only after closure.
    let mut claims = claims.into_iter().map(Some).collect::<Vec<_>>();
    let rows = requirements
        .into_iter()
        .zip(claim_order)
        .map(
            |(requirement, index)| StructurallyClosedDeviceOperationRequirement {
                requirement,
                claim: claims[index]
                    .take()
                    .expect("validated unique evidence index is consumed exactly once"),
            },
        )
        .collect();
    let closed = StructurallyClosedDeviceOperationRequirements { rows };
    closed
        .validate_structure()
        .expect("newly structurally closed device-operation rows replay exactly");
    Ok(closed)
}

fn validate_device_operation_requirements(
    requirements: &[DeviceOperationRequirement],
    claims: &[ProviderAssertedDeviceOperationClaim],
) -> Result<Vec<usize>, AccessPlanDiagnostic> {
    let mut requirement_indices = BTreeMap::new();
    for (index, requirement) in requirements.iter().enumerate() {
        if requirement_indices
            .insert(requirement.identity, index)
            .is_some()
        {
            return Err(AccessPlanDiagnostic(format!(
                "device-operation requirement identity {} is emitted more than once",
                requirement.identity.normalized_identity()
            )));
        }
    }

    let mut claim_indices = BTreeMap::new();
    for (index, row) in claims.iter().enumerate() {
        let identity = row.requirement.identity;
        if claim_indices.insert(identity, index).is_some() {
            return Err(AccessPlanDiagnostic(format!(
                "device-operation requirement identity {} has duplicate provider claims",
                identity.normalized_identity()
            )));
        }
    }

    let mut claim_order = Vec::with_capacity(requirements.len());
    for requirement in requirements {
        let Some(&claim_index) = claim_indices.get(&requirement.identity) else {
            return Err(AccessPlanDiagnostic(format!(
                "device-operation requirement identity {} has no provider claim",
                requirement.identity.normalized_identity()
            )));
        };
        if *requirement != claims[claim_index].requirement {
            return Err(AccessPlanDiagnostic(format!(
                "device-operation requirement identity {} has structurally drifted provider claim",
                requirement.identity.normalized_identity()
            )));
        }
        claim_order.push(claim_index);
    }

    for row in claims {
        if !requirement_indices.contains_key(&row.requirement.identity) {
            return Err(AccessPlanDiagnostic(format!(
                "provider claim names un-emitted device-operation requirement identity {}",
                row.requirement.identity.normalized_identity()
            )));
        }
    }
    Ok(claim_order)
}
