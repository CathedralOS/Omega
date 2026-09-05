//! Exact D29 declaration joins across inert package baselines.

use super::*;

#[cfg(test)]
mod tests;

impl PackagePolicyBaseline {
    /// Check package-owned symbolic demands against their retained declaration
    /// owners. The lookup must return baselines from the same target section.
    /// This checks record consistency, not source truth or acceptance authority.
    /// Toolchain declarations have no managed-package baseline to look up.
    pub fn validate_boundary_application_owners<'a>(
        &self,
        mut lookup: impl FnMut(PackageKeyIdentity) -> Option<&'a PackagePolicyBaseline>,
    ) -> Result<(), &'static str> {
        for demand in &self.boundary_applications.demands {
            let owner = match demand.operator_coordinate.identity.owner {
                PackageReviewNominalOwner::Package(package) if package == self.package => self,
                PackageReviewNominalOwner::Package(package) => {
                    let owner = lookup(package)
                        .ok_or("symbolic demand owner has no retained package baseline")?;
                    if owner.package != package || owner.target != self.target {
                        return Err("symbolic demand owner baseline has another package or target");
                    }
                    owner
                }
                PackageReviewNominalOwner::ToolchainSource(_) => continue,
                PackageReviewNominalOwner::Unresolved => {
                    return Err("symbolic demand has an unresolved operator owner");
                }
            };
            validate_demand(demand, owner)?;
        }
        Ok(())
    }
}

pub(super) fn validate_demand(
    demand: &PackagePolicyBoundaryApplicationDemand,
    owner: &PackagePolicyBaseline,
) -> Result<(), &'static str> {
    let index = owner
        .public_api
        .operators
        .binary_search_by(|operator| operator.coordinate.cmp(&demand.operator_coordinate))
        .map_err(|_| "symbolic demand has no exact retained owner operator")?;
    let operator = &owner.public_api.operators[index];
    if !operator.is_boundary || operator.type_parameters.len() != demand.arguments.len() {
        return Err("symbolic demand differs from its boundary operator telescope");
    }
    for argument in &demand.arguments {
        let PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
            requirement_binder_ordinal,
            ..
        } = argument;
        if !usize::try_from(*requirement_binder_ordinal)
            .ok()
            .and_then(|ordinal| operator.type_parameters.get(ordinal))
            .is_some_and(|parameter| matches!(parameter.kind, PackagePolicyTypeParameterKind::Type))
        {
            return Err(
                "symbolic demand requirement binder is not an exact retained type parameter",
            );
        }
    }
    Ok(())
}
