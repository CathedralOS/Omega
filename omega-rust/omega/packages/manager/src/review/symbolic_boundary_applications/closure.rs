//! Exact reviewed-request substitution, plan rejoin, and identity lowering.

use super::{
    ClosedSuppliedBoundaryApplicationDemand, ClosedSuppliedBoundaryApplicationSource,
    ConcreteProducerBinderCategory, SymbolicBoundaryApplicationClosureError,
    SymbolicBoundaryApplicationClosureRequest,
};
use boundary_applications::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryNominalIdentity,
    BoundaryOperatorRequirement, BoundaryTypeIdentity,
};
use package_evidence::record::{
    PackageReviewBoundaryApplication, PackageReviewBoundaryApplicationArgument,
    PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewSymbolicBoundaryApplicationArgument, PackageReviewTypeParameterKind,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn close_one(
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

fn has_nontrivial_bounds(parameter: &package_evidence::record::PackageReviewTypeParameter) -> bool {
    parameter.bounds().multiplicity() != language_semantics::Multiplicity::Affine
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
