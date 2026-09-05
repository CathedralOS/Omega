//! Non-authorizing final substitution of reviewed D29 symbolic demands.
//!
//! Package review exports open applications under an exact producer package,
//! callable, operator, and direct type-binder mapping. This module closes that
//! deliberately narrow form for explicitly supplied composition requests
//! against compiler-reviewed concrete type identities and an independently
//! reviewed selected-application coordinate. The result is demand only: it is
//! not a reachability-complete set and has no route to realization, coverage,
//! Terminal/native authority, or installation issuance. This compiler artifact
//! composition helper is not an install/update prerequisite.
//!
//! Public request/result vocabulary and canonical set deduplication live here;
//! `closure` validates and substitutes one exact reviewed request.

mod closure;

use closure::close_one;

use boundary_applications::{
    BoundaryApplication, BoundaryNominalIdentity, BoundaryOperatorRequirement,
};
use package_evidence::record::{
    CheckedPackageBoundaryApplicationDemandReview,
    CheckedPackageBoundaryApplicationRealizationReview, CheckedPackageReviewProjection,
    PackageReviewNominalIdentity, PackageReviewSymbolicBoundaryApplicationArgument,
    PackageReviewTypeIdentity,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::fmt;

/// One compiler-reviewed concrete type assigned to an exact producer binder.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConcreteProducerTypeSubstitution {
    binder_ordinal: u32,
    category: ConcreteProducerBinderCategory,
    type_identity: PackageReviewTypeIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConcreteProducerBinderCategory {
    Type,
    Const,
    Machine,
    Proposition,
}

impl ConcreteProducerTypeSubstitution {
    pub fn new(
        binder_ordinal: u32,
        category: ConcreteProducerBinderCategory,
        type_identity: &PackageReviewTypeIdentity,
    ) -> Self {
        Self {
            binder_ordinal,
            category,
            type_identity: type_identity.clone(),
        }
    }

    pub fn type_argument(binder_ordinal: u32, type_identity: &PackageReviewTypeIdentity) -> Self {
        Self::new(
            binder_ordinal,
            ConcreteProducerBinderCategory::Type,
            type_identity,
        )
    }

    pub const fn binder_ordinal(&self) -> u32 {
        self.binder_ordinal
    }

    pub const fn category(&self) -> ConcreteProducerBinderCategory {
        self.category
    }

    pub const fn type_identity(&self) -> &PackageReviewTypeIdentity {
        &self.type_identity
    }
}

/// Exact concrete specialization of one reviewed public producer callable.
///
/// Construction is intentionally inert. Closure replays package ownership,
/// callable identity, telescope category and arity, and complete use of every
/// row before accepting it as composition input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteProducerTypeSpecialization {
    package: PackageKeyIdentity,
    producer_callable: PackageReviewNominalIdentity,
    substitutions: Vec<ConcreteProducerTypeSubstitution>,
}

impl ConcreteProducerTypeSpecialization {
    pub fn new(
        package: PackageKeyIdentity,
        producer_callable: &PackageReviewNominalIdentity,
        substitutions: Vec<ConcreteProducerTypeSubstitution>,
    ) -> Self {
        Self {
            package,
            producer_callable: producer_callable.clone(),
            substitutions,
        }
    }

    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn producer_callable(&self) -> &PackageReviewNominalIdentity {
        &self.producer_callable
    }

    pub fn substitutions(&self) -> &[ConcreteProducerTypeSubstitution] {
        &self.substitutions
    }
}

/// One open reviewed row, its exact producer specialization, and the reviewed
/// closed selected-plan coordinate against which final substitution rejoins.
#[derive(Debug, Clone)]
pub struct SymbolicBoundaryApplicationClosureRequest<'a> {
    producer_review: &'a CheckedPackageReviewProjection,
    operator_review: &'a CheckedPackageReviewProjection,
    selected_application_review: &'a CheckedPackageReviewProjection,
    demand: &'a CheckedPackageBoundaryApplicationDemandReview,
    producer_specialization: ConcreteProducerTypeSpecialization,
    selected_application: &'a CheckedPackageBoundaryApplicationRealizationReview,
}

impl<'a> SymbolicBoundaryApplicationClosureRequest<'a> {
    pub fn new(
        producer_review: &'a CheckedPackageReviewProjection,
        operator_review: &'a CheckedPackageReviewProjection,
        selected_application_review: &'a CheckedPackageReviewProjection,
        demand: &'a CheckedPackageBoundaryApplicationDemandReview,
        producer_specialization: ConcreteProducerTypeSpecialization,
        selected_application: &'a CheckedPackageBoundaryApplicationRealizationReview,
    ) -> Self {
        Self {
            producer_review,
            operator_review,
            selected_application_review,
            demand,
            producer_specialization,
            selected_application,
        }
    }
}

/// Artifact-qualified provenance retained when equal semantic demands merge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClosedSuppliedBoundaryApplicationSource {
    package: PackageKeyIdentity,
    producer_callable: BoundaryNominalIdentity,
    symbolic_arguments: Vec<PackageReviewSymbolicBoundaryApplicationArgument>,
    substitutions: Vec<ConcreteProducerTypeSubstitution>,
}

impl ClosedSuppliedBoundaryApplicationSource {
    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn producer_callable(&self) -> &BoundaryNominalIdentity {
        &self.producer_callable
    }

    pub fn symbolic_arguments(&self) -> &[PackageReviewSymbolicBoundaryApplicationArgument] {
        &self.symbolic_arguments
    }

    pub fn substitutions(&self) -> &[ConcreteProducerTypeSubstitution] {
        &self.substitutions
    }
}

/// One canonical closed demand after symbolic substitution and exact
/// requirement/selected-plan rejoin.
///
/// The selected-plan digest is retained as a coordinate that prevented an
/// unsafe deduplication. This row does not expose a realization or coverage
/// constructor and grants no authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedSuppliedBoundaryApplicationDemand {
    requirement: BoundaryOperatorRequirement,
    application: BoundaryApplication,
    selected_plan_digest: [u8; 32],
    selected_application_package: PackageKeyIdentity,
    sources: Vec<ClosedSuppliedBoundaryApplicationSource>,
}

impl ClosedSuppliedBoundaryApplicationDemand {
    pub const fn requirement(&self) -> &BoundaryOperatorRequirement {
        &self.requirement
    }

    pub const fn application(&self) -> &BoundaryApplication {
        &self.application
    }

    pub const fn selected_plan_digest(&self) -> &[u8; 32] {
        &self.selected_plan_digest
    }

    pub const fn selected_application_package(&self) -> PackageKeyIdentity {
        self.selected_application_package
    }

    pub fn sources(&self) -> &[ClosedSuppliedBoundaryApplicationSource] {
        &self.sources
    }
}

/// Canonically ordered, non-authorizing closed symbolic-demand set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedSuppliedBoundaryApplicationDemands {
    rows: Vec<ClosedSuppliedBoundaryApplicationDemand>,
}

impl ClosedSuppliedBoundaryApplicationDemands {
    pub fn rows(&self) -> &[ClosedSuppliedBoundaryApplicationDemand] {
        &self.rows
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolicBoundaryApplicationClosureError {
    DemandNotInReviewedPackage,
    SelectedApplicationNotInReviewedPackage,
    ProducerPackageMismatch,
    ProducerCallableMismatch,
    ProducerCallableNotReviewed,
    OperatorPackageMismatch,
    OperatorNotReviewed,
    OperatorIsNotBoundary,
    UnsupportedOperatorLifetimeTelescope,
    UnsupportedOperatorBinderCategory(u32),
    UnsupportedOperatorBinderBounds(u32),
    UnsupportedProducerLifetimeTelescope,
    UnsupportedProducerBinderCategory(u32),
    UnsupportedProducerBinderBounds(u32),
    UnsupportedProducerConformanceBounds,
    ProducerSubstitutionCategoryMismatch(u32),
    OperatorArityMismatch,
    MissingProducerSubstitution(u32),
    ExtraProducerSubstitution(u32),
    NonCanonicalRequirementBinder(u32),
    NonCanonicalProducerSubstitution(u32),
    UnusedProducerSubstitution(u32),
    SelectedRequirementMismatch,
    SelectedOperatorMismatch,
    SelectedApplicationMismatch,
    EmptySelectedPlanDigest,
    InvalidNominalIdentity,
    InvalidTypeIdentity,
    ConflictingSelectedPlan,
}

impl fmt::Display for SymbolicBoundaryApplicationClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "symbolic boundary application closure failed: {self:?}"
        )
    }
}

impl std::error::Error for SymbolicBoundaryApplicationClosureError {}

/// Close supplied reviewed direct type-binder demands without proving that
/// the supplied request list exhausts reachable specializations or producing
/// coverage.
pub fn close_supplied_reviewed_symbolic_boundary_applications(
    requests: Vec<SymbolicBoundaryApplicationClosureRequest<'_>>,
) -> Result<ClosedSuppliedBoundaryApplicationDemands, SymbolicBoundaryApplicationClosureError> {
    let mut rows: Vec<ClosedSuppliedBoundaryApplicationDemand> = Vec::new();
    for request in requests {
        let closed = close_one(request)?;
        if let Some(existing) = rows.iter_mut().find(|existing| {
            existing.requirement == closed.requirement && existing.application == closed.application
        }) {
            if existing.selected_application_package != closed.selected_application_package
                || existing.selected_plan_digest != closed.selected_plan_digest
            {
                return Err(SymbolicBoundaryApplicationClosureError::ConflictingSelectedPlan);
            }
            existing.sources.extend(closed.sources);
            existing.sources.sort();
            existing.sources.dedup();
        } else {
            rows.push(closed);
        }
    }
    rows.sort_by(|left, right| {
        left.requirement
            .cmp(&right.requirement)
            .then(left.application.cmp(&right.application))
            .then(
                left.selected_application_package
                    .cmp(&right.selected_application_package),
            )
            .then(left.selected_plan_digest.cmp(&right.selected_plan_digest))
    });
    Ok(ClosedSuppliedBoundaryApplicationDemands { rows })
}
