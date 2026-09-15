//! The placement plan joining a layout, an access plan and a boundary reach,
//! and its validated form with the content interpretation it seals.

use crate::plan_policy::normalized_identities::{
    authoritative_placement_interpretation, non_authoritative_placement_compatibility_fingerprint,
};
use crate::validate_access_plan;
use crate::{AccessPlan, AccessPlanDiagnostic, BoundaryReach, ValidatedAccessPlan};
use layout_plans::LayoutPlanReport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementPlan {
    pub layout: LayoutPlanReport,
    pub access: AccessPlan,
    pub reach: BoundaryReach,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlacementPlanId(pub(crate) u64);

impl PlacementPlanId {
    /// Compact compatibility coordinate. Provider-content authority uses
    /// [`ValidatedPlacementPlan::content_interpretation`] instead.
    pub const fn compatibility_fingerprint(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPlacementPlan {
    pub(crate) identity: PlacementPlanId,
    pub(crate) content_interpretation: extents::ExtentContentInterpretation,
    pub(crate) layout: LayoutPlanReport,
    pub(crate) access: ValidatedAccessPlan,
    pub(crate) reach: BoundaryReach,
}

impl ValidatedPlacementPlan {
    pub const fn identity(&self) -> PlacementPlanId {
        self.identity
    }

    /// Exact identity used when provider-existing content is joined to this
    /// placement. The embedded `u64` is compatibility/report data; authority
    /// additionally requires the domain-separated SHA-256 commitment.
    pub const fn content_interpretation(&self) -> extents::ExtentContentInterpretation {
        self.content_interpretation
    }

    pub const fn layout(&self) -> &LayoutPlanReport {
        &self.layout
    }

    pub const fn access(&self) -> &ValidatedAccessPlan {
        &self.access
    }

    pub const fn reach(&self) -> &BoundaryReach {
        &self.reach
    }
}

pub fn validate_placement_plan(
    plan: PlacementPlan,
) -> Result<ValidatedPlacementPlan, AccessPlanDiagnostic> {
    let PlacementPlan {
        layout,
        access,
        reach,
    } = plan;
    let access = validate_access_plan(access, &layout)?;
    let identity = non_authoritative_placement_compatibility_fingerprint(access.identity(), &reach);
    let mut validated = ValidatedPlacementPlan {
        identity,
        content_interpretation: extents::ExtentContentInterpretation::from_sha256_commitment(
            extents::ExtentContentInterpretationId::from_normalized_identity(
                identity.compatibility_fingerprint(),
            )
            .map_err(|diagnostic| AccessPlanDiagnostic(diagnostic.to_string()))?,
            [0; 32],
        ),
        layout,
        access,
        reach,
    };
    validated.content_interpretation = authoritative_placement_interpretation(&validated);
    Ok(validated)
}
