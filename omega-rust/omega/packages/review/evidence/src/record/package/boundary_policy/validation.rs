use super::*;
use crate::record::{
    PackagePolicyProviderBinding, PackagePolicySelectedProviders,
    PackageReviewBoundaryApplicationArgument, PackageReviewNominalOwner,
};
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;

impl PackagePolicyBoundaryApplicationRealization {
    pub(crate) fn application_key(
        &self,
    ) -> (
        &PackageReviewOperatorCoordinate,
        &str,
        &PackageReviewBoundaryApplication,
    ) {
        (
            &self.operator_coordinate,
            &self.requirement_identity,
            &self.application,
        )
    }
}

impl PackagePolicyBoundaryApplications {
    pub(crate) fn validate_canonical_structure(
        &self,
        package: PackageKeyIdentity,
        target: TargetProfile,
        providers: &PackagePolicySelectedProviders,
    ) -> Result<(), &'static str> {
        if providers.package() != package || providers.target() != target {
            return Err("D29 provider policy has a different package or target");
        }
        if providers.plans().iter().any(|plan| {
            plan.rows()
                .windows(2)
                .any(|pair| pair[0].requirement() >= pair[1].requirement())
        }) {
            return Err("D29 selected rows are not uniquely ordered");
        }
        if self.demands.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("symbolic demands repeat or are out of canonical order");
        }
        for demand in &self.demands {
            nominal(demand.operator_coordinate.identity())?;
            nominal(&demand.producer_callable)?;
            if demand.producer_callable.owner != PackageReviewNominalOwner::Package(package)
                || demand.arguments.is_empty()
            {
                return Err("symbolic demand has no exact producer, requirement, or application");
            }
            for (ordinal, argument) in demand.arguments.iter().enumerate() {
                let PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
                    requirement_binder_ordinal,
                    ..
                } = argument;
                if usize::try_from(*requirement_binder_ordinal).ok() != Some(ordinal) {
                    return Err("symbolic demand does not retain declaration-ordered binders");
                }
            }
        }
        if self
            .realizations
            .windows(2)
            .any(|pair| pair[0].application_key() >= pair[1].application_key())
        {
            return Err("closed applications repeat or are out of canonical order");
        }
        for realization in &self.realizations {
            nominal(realization.operator_coordinate.identity())?;
            if realization.requirement_identity.is_empty() {
                return Err("closed application has no requirement identity");
            }
            if !realization
                .operator_coordinate
                .matches_policy_requirement(&realization.requirement_identity)
            {
                return Err(
                    "closed application requirement differs from its exact operator coordinate",
                );
            }
            validate_application(&realization.application)?;
            let plan = usize::try_from(realization.selected_plan_index)
                .ok()
                .and_then(|index| providers.plans().get(index))
                .ok_or("closed application escapes the canonical selected plan collection")?;
            let index = plan
                .rows()
                .binary_search_by(|row| {
                    row.requirement()
                        .owner()
                        .cmp(&realization.operator_coordinate.identity().owner())
                        .then_with(|| {
                            row.requirement()
                                .path()
                                .cmp(&realization.requirement_identity)
                        })
                })
                .map_err(|_| "closed application has no unique exact selected requirement")?;
            let row = &plan.rows()[index];
            if plan.schema_declaration() != realization.operator_coordinate.identity() {
                return Err("closed application selects a different operator declaration");
            }
            match &realization.realization {
                PackagePolicyBoundaryRealization::NongenericCheckedBody {
                    declaration,
                    realization: callable,
                } => {
                    if !matches!(
                        realization.application,
                        PackageReviewBoundaryApplication::Empty
                    ) {
                        return Err("nongeneric body has a nonempty application");
                    }
                    checked_body(row, declaration, callable)?;
                }
                PackagePolicyBoundaryRealization::SpecializedCheckedBody {
                    declaration,
                    template,
                } => {
                    if !matches!(
                        realization.application,
                        PackageReviewBoundaryApplication::Exact(_)
                    ) {
                        return Err("specialized body has no exact application");
                    }
                    checked_body(row, declaration, template)?;
                }
                PackagePolicyBoundaryRealization::ExactCompilerIntrinsic { execution } => {
                    if !matches!(
                        realization.application,
                        PackageReviewBoundaryApplication::Empty
                    ) || !matches!(
                        row.binding(),
                        PackagePolicyProviderBinding::CompilerIntrinsic { .. }
                    ) || row.compiler_intrinsic_execution() != Some(*execution)
                    {
                        return Err(
                            "intrinsic application differs from its closed selected execution",
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

fn nominal(value: &PackageReviewNominalIdentity) -> Result<(), &'static str> {
    if value.path.is_empty() || matches!(value.owner, PackageReviewNominalOwner::Unresolved) {
        Err("application policy has an unresolved or empty declaration")
    } else {
        Ok(())
    }
}

fn checked_body(
    row: &crate::record::PackagePolicyProviderRow,
    declaration: &PackageReviewNominalIdentity,
    callable: &PackageReviewNominalIdentity,
) -> Result<(), &'static str> {
    nominal(declaration)?;
    nominal(callable)?;
    if declaration != row.realization()
        || declaration.owner != callable.owner
        || !matches!(
            row.binding(),
            PackagePolicyProviderBinding::CheckedAdapter { .. }
        )
        || row.compiler_intrinsic_execution().is_some()
    {
        return Err("checked application differs from its selected authored realization");
    }
    Ok(())
}

fn validate_application(
    application: &PackageReviewBoundaryApplication,
) -> Result<(), &'static str> {
    let PackageReviewBoundaryApplication::Exact(arguments) = application else {
        return Ok(());
    };
    if arguments.is_empty() {
        return Err("empty application uses the wrong variant");
    }
    for (ordinal, argument) in arguments.iter().enumerate() {
        let binder = match argument {
            PackageReviewBoundaryApplicationArgument::Type {
                binder_ordinal,
                type_identity,
            } => {
                if type_identity.canonical.is_empty() {
                    return Err("application has an empty type identity");
                }
                binder_ordinal
            }
            PackageReviewBoundaryApplicationArgument::Const {
                binder_ordinal,
                declared_carrier,
                value_type,
                value_encoding,
            } => {
                if declared_carrier.canonical.is_empty()
                    || value_type.is_empty()
                    || value_encoding.is_empty()
                {
                    return Err("const application has incomplete evaluated meaning");
                }
                binder_ordinal
            }
        };
        if usize::try_from(*binder).ok() != Some(ordinal) {
            return Err("closed application is not declaration ordered");
        }
    }
    Ok(())
}
