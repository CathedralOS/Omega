use crate::plan_policy::normalized_identities::{
    authoritative_access_interpretation, non_authoritative_access_compatibility_fingerprint,
};
use crate::validate_access_fields;
use crate::{AccessFields, AccessPlanDiagnostic, BoundaryReach, ValidatedAccessFields};
use layout_plans::LayoutPlanReport;

/// A semantic policy result. Layout and backing are independent inputs of
/// placement validation; these descriptions grant no live caller authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessPlan {
    pub schema: language_semantics::reflection::SemanticSchemaGraph,
    /// None adds no restriction; Some(empty) explicitly admits nobody.
    pub issuance: Option<Vec<language_semantics::reflection::CallableDescription>>,
    /// Rows intersect and their coordinates belong to `schema`. An absent
    /// row adds no restriction; an empty caller list denies that region.
    pub readers: Vec<(
        language_semantics::reflection::SchemaNodeHandle,
        Vec<language_semantics::reflection::CallableDescription>,
    )>,
    pub writers: Vec<(
        language_semantics::reflection::SchemaNodeHandle,
        Vec<language_semantics::reflection::CallableDescription>,
    )>,
    /// Only authored demands appear here. Missing rows do not manufacture
    /// Inaccessible, Stable, External or Atomic supply.
    pub observations: Vec<(
        language_semantics::reflection::SchemaNodeHandle,
        Option<(
            crate::ObservationModel,
            crate::AccessPermissions,
            crate::AccessExposure,
        )>,
    )>,
    pub reach: BoundaryReach,
}

/// Canonicalize descriptive policy data before it is joined to an actual
/// invocation and backing. Keeping independent rows preserves intersection:
/// an empty row denies its region, while an absent row adds no restriction.
pub fn normalize_access_plan(mut plan: AccessPlan) -> Result<AccessPlan, AccessPlanDiagnostic> {
    use language_semantics::reflection::CallableDescription;

    fn callers(values: &mut Vec<CallableDescription>) -> Result<(), AccessPlanDiagnostic> {
        for (_, payload) in values.iter() {
            if payload.iter().any(Vec::is_empty)
                || std::str::from_utf8(&payload[0]).is_err()
                || std::str::from_utf8(&payload[1]).is_err()
            {
                return Err(AccessPlanDiagnostic(
                    "a callable description lost its complete identity or source revision".into(),
                ));
            }
        }
        values.sort_unstable();
        values.dedup();
        Ok(())
    }

    let fields = plan
        .schema
        .governed_fields()
        .map_err(AccessPlanDiagnostic)?;

    if let Some(issuers) = &mut plan.issuance {
        callers(issuers)?;
    }
    for rows in [&mut plan.readers, &mut plan.writers] {
        for (region, allowed) in rows.iter_mut() {
            if *region != plan.schema.root()
                && fields
                    .binary_search_by_key(region, |(handle, _)| *handle)
                    .is_err()
            {
                return Err(AccessPlanDiagnostic(
                    "a caller rule has no exact semantic subject or field".into(),
                ));
            }
            callers(allowed)?;
        }
        rows.sort_unstable();
        rows.dedup();
    }
    for (region, demand) in &plan.observations {
        let index = fields
            .binary_search_by_key(region, |(handle, _)| *handle)
            .map_err(|_| {
                AccessPlanDiagnostic("an observation demand has no exact semantic field".into())
            })?;
        let field = fields[index].1;
        if field.erased && demand.is_some_and(|(_, permissions, _)| permissions.any()) {
            return Err(AccessPlanDiagnostic(
                "an erased field has no physical observation or operation".into(),
            ));
        }
    }
    plan.observations.sort_unstable();
    plan.observations.dedup();
    Ok(plan)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct PlacementId(pub(crate) u64);

impl PlacementId {
    pub const fn compatibility_fingerprint(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ValidatedPlacement {
    pub(crate) identity: PlacementId,
    pub(crate) content_interpretation: extents::ExtentContentInterpretation,
    pub(crate) layout: LayoutPlanReport,
    pub(crate) access: ValidatedAccessFields,
    pub(crate) reach: BoundaryReach,
}

impl ValidatedPlacement {
    pub const fn identity(&self) -> PlacementId {
        self.identity
    }

    pub const fn content_interpretation(&self) -> extents::ExtentContentInterpretation {
        self.content_interpretation
    }

    pub const fn layout(&self) -> &LayoutPlanReport {
        &self.layout
    }

    pub const fn access(&self) -> &ValidatedAccessFields {
        &self.access
    }

    pub const fn reach(&self) -> &BoundaryReach {
        &self.reach
    }
}

/// Join an independently validated layout with complete physical operation
/// decisions. Semantic policy normalization does not call this to invent
/// layout, supply, issuance or permission.
pub fn validate_placement(
    layout: LayoutPlanReport,
    access: AccessFields,
    reach: BoundaryReach,
) -> Result<ValidatedPlacement, AccessPlanDiagnostic> {
    let access = validate_access_fields(access, &layout)?;
    let identity = non_authoritative_access_compatibility_fingerprint(access.identity(), &reach);
    let mut validated = ValidatedPlacement {
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
    validated.content_interpretation = authoritative_access_interpretation(&validated);
    Ok(validated)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementSnapshot {
    pub layout: LayoutPlanReport,
    pub access: Vec<crate::FieldAccess>,
    pub reach: BoundaryReach,
}

impl ValidatedPlacement {
    pub fn snapshot(&self) -> PlacementSnapshot {
        PlacementSnapshot {
            layout: self.layout.clone(),
            access: self
                .access
                .plan()
                .entries()
                .iter()
                .map(|entry| entry.access().clone())
                .collect(),
            reach: self.reach.clone(),
        }
    }
}

impl PlacementSnapshot {
    pub fn validate(self) -> Result<ValidatedPlacement, AccessPlanDiagnostic> {
        let mut access = AccessFields::inaccessible(&self.layout)?;
        if access.entries().len() != self.access.len() {
            return Err(AccessPlanDiagnostic(
                "placement snapshot access cardinality differs from its exact layout".into(),
            ));
        }
        for (slot, field_access) in self.access.into_iter().enumerate() {
            let key = access.key_at(slot).ok_or_else(|| {
                AccessPlanDiagnostic("placement snapshot lost its canonical access slot".into())
            })?;
            access.set(key, field_access)?;
        }
        validate_placement(self.layout, access, self.reach)
    }
}
