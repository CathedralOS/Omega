//! Structural associations for inert provider policy, never compiler replay.
use crate::record::PackagePolicyProviderPlan;
use crate::record::PackagePolicySelectedProviders;
use crate::record::package::callable_policy::validation::nominal_set;
use crate::record::validation::{ordered, ordered_by};
use std::cmp::Ordering;

impl PackagePolicyProviderPlan {
    pub(crate) fn compare_canonical(&self, other: &Self) -> Ordering {
        self.plan_name
            .cmp(&other.plan_name)
            .then_with(|| self.realizing_package.cmp(&other.realizing_package))
            .then_with(|| {
                self.provider_type_declaration
                    .cmp(&other.provider_type_declaration)
            })
            .then_with(|| self.schema_declaration.cmp(&other.schema_declaration))
            .then_with(|| self.target.cmp(&other.target))
    }
}

impl PackagePolicySelectedProviders {
    pub(crate) fn validate_canonical_structure(&self) -> Result<(), &'static str> {
        ordered_by(&self.plans, PackagePolicyProviderPlan::compare_canonical)?;
        for plan in &self.plans {
            plan.schema_declaration.validate_resolved()?;
            // A toolchain-owned plan (realizing package `None`, no provider
            // declaration) may still carry its readable toolchain provider
            // label; an authored plan's non-empty provider type always needs
            // its exact declaration.
            if plan.plan_name.is_empty()
                || (plan.provider_type.is_empty() != plan.provider_type_declaration.is_none()
                    && !(plan.provider_type_declaration.is_none()
                        && plan.realizing_package.is_none()))
                || (!plan.target.is_empty() && plan.target != self.target.target_name())
                || plan.methods.is_empty()
                || plan.methods.len() != plan.rows.len()
            {
                return Err("provider plan has an invalid name, target, or method coverage");
            }
            if let Some(provider) = &plan.provider_type_declaration {
                provider.validate_resolved()?;
            }
            ordered(&plan.grants)?;
            super::validate_service_methods(&plan.methods, &plan.schema_declaration, self.target)?;
            for method in &plan.methods {
                let mut rows = plan
                    .rows
                    .iter()
                    .filter(|row| row.requirement == method.requirement);
                let Some(row) = rows.next() else {
                    return Err("provider method does not have one exact realization row");
                };
                if rows.next().is_some() || row.method != method.name {
                    return Err("provider row changes its method spelling");
                }
            }
            ordered_by(&plan.rows, |a, b| a.requirement.cmp(&b.requirement))?;
            for row in &plan.rows {
                row.requirement.validate_resolved()?;
                match &row.realization {
                    Some(realization) => {
                        realization.validate_resolved()?;
                        if !super::binding_validation::matches_owner(
                            realization.owner,
                            plan.realizing_package,
                        ) {
                            return Err(
                                "provider realization disagrees with its plan's realizing owner",
                            );
                        }
                    }
                    // A toolchain-settled row names no authored machine.
                    None if plan.realizing_package.is_none() => {}
                    None => {
                        return Err("provider row omits its realization under a realizing package");
                    }
                }
                if row.requirement_lifetime_partition
                    != language_semantics::type_identity::normalize_requirement_lifetime_partition(
                        &row.requirement_lifetime_partition,
                    )
                {
                    return Err("provider lifetime partition is not canonical");
                }
                if let Some(reach) = &row.installation_reach {
                    for declarations in [&reach.upper_bound, &reach.resolved] {
                        nominal_set(declarations)?;
                    }
                    if reach
                        .resolved
                        .iter()
                        .any(|actual| !reach.upper_bound.contains(actual))
                    {
                        return Err("provider installation reach exceeds its upper bound");
                    }
                }
                row.binding.validate_canonical_structure(self.target, row)?;
            }
        }
        self.validate_families()
    }

    fn validate_families(&self) -> Result<(), &'static str> {
        ordered_by(&self.families, |a, b| {
            (
                &a.family_identity,
                &a.provider_type_declaration,
                a.target.target_name(),
                a.authority,
                a.coverage,
            )
                .cmp(&(
                    &b.family_identity,
                    &b.provider_type_declaration,
                    b.target.target_name(),
                    b.authority,
                    b.coverage,
                ))
        })?;
        for family in &self.families {
            family.family_identity.validate_resolved()?;
            family.provider_type_declaration.validate_resolved()?;
            if family.target != self.target || family.coordinates.is_empty() {
                return Err("provider family has an invalid target or coordinate order");
            }
            ordered_by(&family.coordinates, |a, b| {
                (&a.requirement_identity, &a.operator_declaration)
                    .cmp(&(&b.requirement_identity, &b.operator_declaration))
            })?;
            for coordinate in &family.coordinates {
                coordinate.operator_declaration.validate_resolved()?;
                let Some(plan) = self.plans.get(coordinate.plan_index as usize) else {
                    return Err("provider family references a missing canonical plan");
                };
                if coordinate.requirement_identity.is_empty()
                    || plan.provider_type_declaration.as_ref()
                        != Some(&family.provider_type_declaration)
                    || plan.schema_declaration != coordinate.operator_declaration
                    || !plan.rows.iter().any(|row| {
                        row.requirement.owner == coordinate.operator_declaration.owner
                            && row.requirement.path == coordinate.requirement_identity
                    })
                    || coordinate.operator_declaration != family.family_identity
                {
                    return Err("provider family coordinate is detached from its selected plan");
                }
            }
            for (index, plan) in self.plans.iter().enumerate().filter(|(_, plan)| {
                plan.schema_declaration == family.family_identity
                    && plan.provider_type_declaration.as_ref()
                        == Some(&family.provider_type_declaration)
            }) {
                for row in &plan.rows {
                    if !family.coordinates.iter().any(|coordinate| {
                        coordinate.plan_index as usize == index
                            && coordinate.requirement_identity == row.requirement.path
                    }) {
                        return Err("provider family omits a selected declaration coordinate");
                    }
                }
            }
        }
        Ok(())
    }
}
