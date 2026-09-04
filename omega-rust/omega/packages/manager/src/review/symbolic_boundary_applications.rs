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

use omega_boundary_applications::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryNominalIdentity,
    BoundaryOperatorRequirement, BoundaryTypeIdentity,
};
use omega_package_evidence::record::{
    CheckedPackageBoundaryApplicationDemandReview,
    CheckedPackageBoundaryApplicationRealizationReview, CheckedPackageReviewProjection,
    PackageReviewBoundaryApplication, PackageReviewBoundaryApplicationArgument,
    PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewSymbolicBoundaryApplicationArgument, PackageReviewTypeIdentity,
    PackageReviewTypeParameterKind,
};
use psi_core::PackageKeyIdentity;
use std::collections::{BTreeMap, BTreeSet};
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

fn close_one(
    request: SymbolicBoundaryApplicationClosureRequest<'_>,
) -> Result<ClosedSuppliedBoundaryApplicationDemand, SymbolicBoundaryApplicationClosureError> {
    let demand = request.demand;
    if request
        .producer_review
        .boundary_application_demands()
        .iter()
        .filter(|candidate| *candidate == demand)
        .count()
        != 1
    {
        return Err(SymbolicBoundaryApplicationClosureError::DemandNotInReviewedPackage);
    }
    if request
        .selected_application_review
        .boundary_application_realizations()
        .iter()
        .filter(|candidate| *candidate == request.selected_application)
        .count()
        != 1
    {
        return Err(
            SymbolicBoundaryApplicationClosureError::SelectedApplicationNotInReviewedPackage,
        );
    }

    let package = request.producer_review.package();
    let specialization = &request.producer_specialization;
    if specialization.package != package
        || demand.producer_callable().owner() != PackageReviewNominalOwner::Package(package)
    {
        return Err(SymbolicBoundaryApplicationClosureError::ProducerPackageMismatch);
    }
    if specialization.producer_callable != *demand.producer_callable() {
        return Err(SymbolicBoundaryApplicationClosureError::ProducerCallableMismatch);
    }
    let producers = request
        .producer_review
        .callables()
        .iter()
        .filter(|callable| callable.identity() == demand.producer_callable())
        .collect::<Vec<_>>();
    let [producer] = producers.as_slice() else {
        return Err(SymbolicBoundaryApplicationClosureError::ProducerCallableNotReviewed);
    };

    let operator_package = request.operator_review.package();
    if demand.operator_declaration().owner() != PackageReviewNominalOwner::Package(operator_package)
    {
        return Err(SymbolicBoundaryApplicationClosureError::OperatorPackageMismatch);
    }
    let operators = request
        .operator_review
        .public_operators()
        .iter()
        .filter(|operator| operator.coordinate() == demand.operator_coordinate())
        .collect::<Vec<_>>();
    let [operator] = operators.as_slice() else {
        return Err(SymbolicBoundaryApplicationClosureError::OperatorNotReviewed);
    };
    if !operator.is_boundary() {
        return Err(SymbolicBoundaryApplicationClosureError::OperatorIsNotBoundary);
    }
    if operator.lifetime_parameter_count() != 0 {
        return Err(SymbolicBoundaryApplicationClosureError::UnsupportedOperatorLifetimeTelescope);
    }
    for (ordinal, parameter) in operator.type_parameters().iter().enumerate() {
        if !matches!(parameter.kind(), PackageReviewTypeParameterKind::Type) {
            return Err(
                SymbolicBoundaryApplicationClosureError::UnsupportedOperatorBinderCategory(
                    u32::try_from(ordinal).unwrap_or(u32::MAX),
                ),
            );
        }
        if has_nontrivial_bounds(parameter) {
            return Err(
                SymbolicBoundaryApplicationClosureError::UnsupportedOperatorBinderBounds(
                    u32::try_from(ordinal).unwrap_or(u32::MAX),
                ),
            );
        }
    }
    if operator.type_parameters().len() != demand.arguments().len() {
        return Err(SymbolicBoundaryApplicationClosureError::OperatorArityMismatch);
    }

    if producer.lifetime_parameter_count() != 0 {
        return Err(SymbolicBoundaryApplicationClosureError::UnsupportedProducerLifetimeTelescope);
    }
    if !producer.conformance_bounds().is_empty() {
        return Err(SymbolicBoundaryApplicationClosureError::UnsupportedProducerConformanceBounds);
    }
    for (ordinal, parameter) in producer.type_parameters().iter().enumerate() {
        if !matches!(parameter.kind(), PackageReviewTypeParameterKind::Type) {
            return Err(
                SymbolicBoundaryApplicationClosureError::UnsupportedProducerBinderCategory(
                    u32::try_from(ordinal).unwrap_or(u32::MAX),
                ),
            );
        }
        if has_nontrivial_bounds(parameter) {
            return Err(
                SymbolicBoundaryApplicationClosureError::UnsupportedProducerBinderBounds(
                    u32::try_from(ordinal).unwrap_or(u32::MAX),
                ),
            );
        }
    }
    if specialization.substitutions.len() < producer.type_parameters().len() {
        return Err(
            SymbolicBoundaryApplicationClosureError::MissingProducerSubstitution(
                u32::try_from(specialization.substitutions.len()).unwrap_or(u32::MAX),
            ),
        );
    }
    if specialization.substitutions.len() > producer.type_parameters().len() {
        return Err(
            SymbolicBoundaryApplicationClosureError::ExtraProducerSubstitution(
                u32::try_from(producer.type_parameters().len()).unwrap_or(u32::MAX),
            ),
        );
    }
    let mut substitutions = BTreeMap::new();
    for (position, substitution) in specialization.substitutions.iter().enumerate() {
        let expected = u32::try_from(position).map_err(|_| {
            SymbolicBoundaryApplicationClosureError::NonCanonicalProducerSubstitution(u32::MAX)
        })?;
        if substitution.binder_ordinal != expected
            || substitutions
                .insert(substitution.binder_ordinal, &substitution.type_identity)
                .is_some()
        {
            return Err(
                SymbolicBoundaryApplicationClosureError::NonCanonicalProducerSubstitution(
                    substitution.binder_ordinal,
                ),
            );
        }
        if substitution.category != ConcreteProducerBinderCategory::Type {
            return Err(
                SymbolicBoundaryApplicationClosureError::ProducerSubstitutionCategoryMismatch(
                    substitution.binder_ordinal,
                ),
            );
        }
    }

    let mut used = BTreeSet::new();
    let mut arguments = Vec::with_capacity(demand.arguments().len());
    for (position, argument) in demand.arguments().iter().enumerate() {
        let expected = u32::try_from(position).map_err(|_| {
            SymbolicBoundaryApplicationClosureError::NonCanonicalRequirementBinder(u32::MAX)
        })?;
        let PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
            requirement_binder_ordinal,
            producer_binder_ordinal,
        } = argument;
        if *requirement_binder_ordinal != expected {
            return Err(
                SymbolicBoundaryApplicationClosureError::NonCanonicalRequirementBinder(
                    *requirement_binder_ordinal,
                ),
            );
        }
        let concrete = substitutions.get(producer_binder_ordinal).ok_or(
            SymbolicBoundaryApplicationClosureError::MissingProducerSubstitution(
                *producer_binder_ordinal,
            ),
        )?;
        used.insert(*producer_binder_ordinal);
        arguments.push(BoundaryApplicationArgument::type_argument(
            *requirement_binder_ordinal,
            BoundaryTypeIdentity::new(concrete.canonical().to_owned())
                .map_err(|_| SymbolicBoundaryApplicationClosureError::InvalidTypeIdentity)?,
        ));
    }
    if let Some(unused) = substitutions.keys().find(|ordinal| !used.contains(ordinal)) {
        return Err(SymbolicBoundaryApplicationClosureError::UnusedProducerSubstitution(*unused));
    }
    let application = BoundaryApplication::exact(arguments)
        .map_err(|_| SymbolicBoundaryApplicationClosureError::OperatorArityMismatch)?;

    let selected = request.selected_application;
    if selected.requirement_identity() != demand.requirement_identity() {
        return Err(SymbolicBoundaryApplicationClosureError::SelectedRequirementMismatch);
    }
    if selected.operator_declaration() != demand.operator_declaration() {
        return Err(SymbolicBoundaryApplicationClosureError::SelectedOperatorMismatch);
    }
    if selected.selected_plan_digest() == &[0; 32] {
        return Err(SymbolicBoundaryApplicationClosureError::EmptySelectedPlanDigest);
    }
    if lower_review_application(selected.application())? != application {
        return Err(SymbolicBoundaryApplicationClosureError::SelectedApplicationMismatch);
    }

    let requirement = BoundaryOperatorRequirement::new(
        lower_nominal(demand.operator_declaration())?,
        demand.requirement_identity().to_owned(),
    )
    .map_err(|_| SymbolicBoundaryApplicationClosureError::InvalidNominalIdentity)?;
    let source = ClosedSuppliedBoundaryApplicationSource {
        package,
        producer_callable: lower_nominal(demand.producer_callable())?,
        symbolic_arguments: demand.arguments().to_vec(),
        substitutions: specialization.substitutions.clone(),
    };
    Ok(ClosedSuppliedBoundaryApplicationDemand {
        requirement,
        application,
        selected_plan_digest: *selected.selected_plan_digest(),
        selected_application_package: request.selected_application_review.package(),
        sources: vec![source],
    })
}

fn has_nontrivial_bounds(
    parameter: &omega_package_evidence::record::PackageReviewTypeParameter,
) -> bool {
    parameter.bounds().multiplicity() != psi_language_semantics::Multiplicity::Affine
        || parameter.bounds().carry().is_some()
}

fn lower_review_application(
    application: PackageReviewBoundaryApplication,
) -> Result<BoundaryApplication, SymbolicBoundaryApplicationClosureError> {
    match application {
        PackageReviewBoundaryApplication::Empty => Ok(BoundaryApplication::Empty),
        PackageReviewBoundaryApplication::Exact(arguments) => BoundaryApplication::exact(
            arguments
                .into_iter()
                .map(|argument| match argument {
                    PackageReviewBoundaryApplicationArgument::Type {
                        binder_ordinal,
                        type_identity,
                    } => BoundaryTypeIdentity::new(type_identity.canonical().to_owned())
                        .map(|type_identity| {
                            BoundaryApplicationArgument::type_argument(
                                binder_ordinal,
                                type_identity,
                            )
                        })
                        .map_err(|_| SymbolicBoundaryApplicationClosureError::InvalidTypeIdentity),
                    PackageReviewBoundaryApplicationArgument::Const { .. } => {
                        Err(SymbolicBoundaryApplicationClosureError::SelectedApplicationMismatch)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(|_| SymbolicBoundaryApplicationClosureError::SelectedApplicationMismatch),
    }
}

fn lower_nominal(
    identity: &PackageReviewNominalIdentity,
) -> Result<BoundaryNominalIdentity, SymbolicBoundaryApplicationClosureError> {
    let mut canonical = String::new();
    match identity.owner() {
        PackageReviewNominalOwner::Package(package) => {
            canonical.push_str("package:");
            push_hex(&mut canonical, &package.digest());
        }
        PackageReviewNominalOwner::ToolchainSource(source) => {
            canonical.push_str("toolchain:");
            push_hex(&mut canonical, &source.digest());
        }
        PackageReviewNominalOwner::Unresolved => {
            return Err(SymbolicBoundaryApplicationClosureError::InvalidNominalIdentity);
        }
    }
    canonical.push_str("::");
    canonical.push_str(identity.path());
    BoundaryNominalIdentity::new(canonical)
        .map_err(|_| SymbolicBoundaryApplicationClosureError::InvalidNominalIdentity)
}

fn push_hex(output: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
}
