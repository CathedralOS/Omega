//! Provisional provider-coverage scaffolding for device/DMA ordering roles.
//!
//! This module closes only the non-authorizing requirement/discharge shape.
//! A structurally closed row retains one provider assertion for the exact
//! emitted demand. It does not prove provider selection/admission, that the
//! operation ran, mint publication or completion evidence, establish
//! synchronization, or authorize lowering.
//!
//! No checked source operation emits these rows. Current constructions are
//! structural tests, not evidence for a source contract, and the uniform
//! one-range row is not public ABI. A complete admitted DMA boundary may keep
//! these roles provider-private; a future checked-driver surface must derive
//! role-specific payloads from its actual typed operations.

use super::{AccessPlanDiagnostic, SchemaDeviceCorrespondenceReceiptContext};
use psi_extents::MappedRangeReceiptContext;
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

/// One provisional candidate demand for structural provider coverage.
///
/// Mapping and schema/device fields are full opaque structural contexts, not
/// compact IDs. The ordering-scope ID remains an inert nominal coordinate in
/// this first slice; no ordering relation or executable event is inferred from
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceOperationRequirement {
    identity: DeviceOperationRequirementId,
    operation: DeviceOperation,
    range: MappedRangeReceiptContext,
    correspondence: SchemaDeviceCorrespondenceReceiptContext,
    ordering_scope: DeviceOrderingScopeId,
}

impl DeviceOperationRequirement {
    pub fn new(
        identity: DeviceOperationRequirementId,
        operation: DeviceOperation,
        range: MappedRangeReceiptContext,
        correspondence: SchemaDeviceCorrespondenceReceiptContext,
        ordering_scope: DeviceOrderingScopeId,
    ) -> Self {
        Self {
            identity,
            operation,
            range,
            correspondence,
            ordering_scope,
        }
    }

    pub const fn identity(&self) -> DeviceOperationRequirementId {
        self.identity
    }

    pub const fn operation(&self) -> DeviceOperation {
        self.operation
    }

    pub const fn range(&self) -> &MappedRangeReceiptContext {
        &self.range
    }

    pub const fn correspondence(&self) -> &SchemaDeviceCorrespondenceReceiptContext {
        &self.correspondence
    }

    pub const fn ordering_scope(&self) -> DeviceOrderingScopeId {
        self.ordering_scope
    }
}

/// Non-clonable provider assertion for one exact emitted requirement.
///
/// Construction snapshots the complete demand instead of asking a provider to
/// restate public IDs or geometry. The provider-plan ID is provenance for the
/// admitted coverage row; it is not operation or mapping authority.
#[derive(Debug)]
#[must_use = "device-operation coverage retains one exact provider assertion"]
pub struct ProviderAssertedDeviceOperationCoverage {
    provider_plan: DeviceOperationProviderPlanId,
    requirement: DeviceOperationRequirement,
}

impl ProviderAssertedDeviceOperationCoverage {
    pub fn from_provider_assertion(
        provider_plan: DeviceOperationProviderPlanId,
        requirement: &DeviceOperationRequirement,
    ) -> Self {
        Self {
            provider_plan,
            requirement: requirement.clone(),
        }
    }

    pub const fn provider_plan(&self) -> DeviceOperationProviderPlanId {
        self.provider_plan
    }

    pub const fn requirement(&self) -> &DeviceOperationRequirement {
        &self.requirement
    }
}

/// One exact demand joined to one exact provider-asserted coverage row.
#[derive(Debug)]
pub struct StructurallyClosedDeviceOperationRequirement {
    requirement: DeviceOperationRequirement,
    coverage: ProviderAssertedDeviceOperationCoverage,
}

impl StructurallyClosedDeviceOperationRequirement {
    pub const fn requirement(&self) -> &DeviceOperationRequirement {
        &self.requirement
    }

    pub const fn provider_plan(&self) -> DeviceOperationProviderPlanId {
        self.coverage.provider_plan
    }

    fn validate_structure(&self) -> Result<(), AccessPlanDiagnostic> {
        if self.requirement != self.coverage.requirement {
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
    coverage: Vec<ProviderAssertedDeviceOperationCoverage>,
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
        Vec<ProviderAssertedDeviceOperationCoverage>,
    ) {
        (self.requirements, self.coverage)
    }
}

/// Close every emitted demand against exactly one admitted provider row.
///
/// Validation borrows the complete input first. No evidence is consumed into
/// a success row until duplicate, missing, extra, and structural-drift checks
/// all pass.
pub fn structurally_close_device_operation_requirements(
    requirements: Vec<DeviceOperationRequirement>,
    coverage: Vec<ProviderAssertedDeviceOperationCoverage>,
) -> Result<StructurallyClosedDeviceOperationRequirements, DeviceOperationStructuralClosureError> {
    let coverage_order = match validate_device_operation_requirements(&requirements, &coverage) {
        Ok(coverage_order) => coverage_order,
        Err(diagnostic) => {
            return Err(DeviceOperationStructuralClosureError {
                requirements,
                coverage,
                diagnostic,
            });
        }
    };

    // Validation above borrowed the original vectors. Consumption and
    // normalization into emitted-requirement order begins only after closure.
    let mut coverage = coverage.into_iter().map(Some).collect::<Vec<_>>();
    let rows = requirements
        .into_iter()
        .zip(coverage_order)
        .map(
            |(requirement, index)| StructurallyClosedDeviceOperationRequirement {
                requirement,
                coverage: coverage[index]
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
    coverage: &[ProviderAssertedDeviceOperationCoverage],
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

    let mut coverage_indices = BTreeMap::new();
    for (index, row) in coverage.iter().enumerate() {
        let identity = row.requirement.identity;
        if coverage_indices.insert(identity, index).is_some() {
            return Err(AccessPlanDiagnostic(format!(
                "device-operation requirement identity {} has duplicate provider coverage assertions",
                identity.normalized_identity()
            )));
        }
    }

    let mut coverage_order = Vec::with_capacity(requirements.len());
    for requirement in requirements {
        let Some(&coverage_index) = coverage_indices.get(&requirement.identity) else {
            return Err(AccessPlanDiagnostic(format!(
                "device-operation requirement identity {} has no provider coverage assertion",
                requirement.identity.normalized_identity()
            )));
        };
        if *requirement != coverage[coverage_index].requirement {
            return Err(AccessPlanDiagnostic(format!(
                "device-operation requirement identity {} has structurally drifted provider coverage",
                requirement.identity.normalized_identity()
            )));
        }
        coverage_order.push(coverage_index);
    }

    for row in coverage {
        if !requirement_indices.contains_key(&row.requirement.identity) {
            return Err(AccessPlanDiagnostic(format!(
                "provider coverage names un-emitted device-operation requirement identity {}",
                row.requirement.identity.normalized_identity()
            )));
        }
    }
    Ok(coverage_order)
}
