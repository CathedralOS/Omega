//! Structural associations for inert provider policy, never compiler replay.

use super::*;
use crate::record::{PackageReviewNominalIdentity, PackageReviewNominalOwner};
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
        if self
            .plans
            .windows(2)
            .any(|pair| pair[0].compare_canonical(&pair[1]) != Ordering::Less)
        {
            return Err("provider plans repeat or reorder their canonical coordinates");
        }
        for plan in &self.plans {
            nominal(&plan.schema_declaration)?;
            if plan.plan_name.is_empty()
                || plan.provider_type.is_empty() != plan.provider_type_declaration.is_none()
                || (!plan.target.is_empty() && plan.target != self.target.target_name())
                || plan.methods.is_empty()
                || plan.methods.len() != plan.rows.len()
            {
                return Err("provider plan has an invalid name, target, or method coverage");
            }
            if let Some(provider) = &plan.provider_type_declaration {
                nominal(provider)?;
            }
            if plan.grants.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err("provider grants repeat or reorder selector kinds");
            }
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
            if plan
                .rows
                .windows(2)
                .any(|pair| pair[0].requirement >= pair[1].requirement)
            {
                return Err("provider rows repeat or reorder exact requirement identities");
            }
            for row in &plan.rows {
                nominal(&row.requirement)?;
                nominal(&row.realization)?;
                if !super::binding_validation::matches_owner(
                    row.realization.owner,
                    plan.realizing_package,
                ) {
                    return Err("provider realization disagrees with its plan's realizing owner");
                }
                let mut next = 0;
                for ordinal in &row.requirement_lifetime_partition {
                    if *ordinal > next {
                        return Err("provider lifetime partition is not canonical");
                    }
                    if *ordinal == next {
                        next += 1;
                    }
                }
                if let Some(reach) = &row.installation_reach {
                    for declarations in [&reach.upper_bound, &reach.resolved] {
                        if declarations.windows(2).any(|pair| pair[0] >= pair[1]) {
                            return Err("provider installation reach is unordered or repeated");
                        }
                        for declaration in declarations {
                            nominal(declaration)?;
                        }
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
        if self.families.windows(2).any(|pair| {
            (
                &pair[0].family_identity,
                &pair[0].provider_type_declaration,
                pair[0].target.target_name(),
                pair[0].authority,
                pair[0].coverage,
            ) >= (
                &pair[1].family_identity,
                &pair[1].provider_type_declaration,
                pair[1].target.target_name(),
                pair[1].authority,
                pair[1].coverage,
            )
        }) {
            return Err("provider families repeat or reorder canonical coordinates");
        }
        for family in &self.families {
            nominal(&family.family_identity)?;
            nominal(&family.provider_type_declaration)?;
            if family.target != self.target
                || family.coordinates.is_empty()
                || family.coordinates.windows(2).any(|pair| {
                    (&pair[0].requirement_identity, &pair[0].operator_declaration)
                        >= (&pair[1].requirement_identity, &pair[1].operator_declaration)
                })
            {
                return Err("provider family has an invalid target or coordinate order");
            }
            for coordinate in &family.coordinates {
                nominal(&coordinate.operator_declaration)?;
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

pub(super) fn same_target(
    profile: crate::record::PackageReviewRepresentationTargetProfile,
    target: target::TargetProfile,
) -> bool {
    use crate::record::PackageReviewRepresentationTargetProfile as Profile;
    use target::TargetProfile as Target;
    matches!(
        (profile, target),
        (Profile::LinuxArm64, Target::LinuxArm64)
            | (Profile::LinuxX64, Target::LinuxX64)
            | (Profile::MacosArm64, Target::MacosArm64)
            | (Profile::WindowsX64, Target::WindowsX64)
            | (Profile::UefiX64, Target::UefiX64)
            | (Profile::CrossPlatformCli, Target::CrossPlatformCli)
            | (Profile::LocalUnchecked, Target::LocalUnchecked)
    )
}

pub(super) fn nominal(identity: &PackageReviewNominalIdentity) -> Result<(), &'static str> {
    if identity.path.is_empty() || identity.owner == PackageReviewNominalOwner::Unresolved {
        Err("provider policy contains an unresolved nominal identity")
    } else {
        Ok(())
    }
}
