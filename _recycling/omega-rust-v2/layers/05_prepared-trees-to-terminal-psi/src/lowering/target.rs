use crate::{LoweringError, identity::portable};
use omega_prepared::{PreparedTrees, PreparedValueOrigin, SelectedLayout, TargetIdentity, TargetObservation, TargetProperty};
use omega_psi::{
    FoldedLayout, FoldedProperty, FoldedRealization, FoldedReferenceAccess, FoldedSubject, FoldedTargetClosure, FoldedTargetInputs, FoldedTargetObservation,
    FoldedValueOrigin,
};
use omega_typed::{Access, Site, StaticValue, StaticValueDemand, ValueType};
use std::sync::Arc;
fn from_typed(ty: &ValueType) -> Option<FoldedSubject> {
    Some(match ty {
        ValueType::Bool => FoldedSubject::Bool,
        ValueType::Integer { signed, width: Some(width) } => FoldedSubject::Integer {
            signed: *signed,
            width: *width,
        },
        ValueType::Integer { signed, width: None } => FoldedSubject::UnboundedInteger { signed: *signed },
        ValueType::Address => FoldedSubject::Address,
        ValueType::Float32 => FoldedSubject::Float32,
        ValueType::Float64 => FoldedSubject::Float64,
        ValueType::Nominal(id) => FoldedSubject::Nominal(portable(id)?),
        ValueType::Applied(id, args) => FoldedSubject::Applied(portable(id)?, arguments(args)?),
        ValueType::Tuple(elements) => FoldedSubject::Tuple(arguments(elements)?),
        ValueType::Array(element, length) => FoldedSubject::Array(Box::new(from_typed(element)?), *length),
        ValueType::Slice(element) => FoldedSubject::Slice(Box::new(from_typed(element)?)),
        ValueType::Reference(access, referent) => FoldedSubject::Reference(
            match access {
                Access::Shared => FoldedReferenceAccess::Shared,
                Access::Mut => FoldedReferenceAccess::Mutable,
                Access::Write => FoldedReferenceAccess::WriteOnly,
            },
            Box::new(from_typed(referent)?),
        ),
        ValueType::Qualified(carrier, domains) => FoldedSubject::Qualified(
            Box::new(from_typed(carrier)?),
            domains
                .iter()
                .map(|domain| Some((portable(&domain.declaration)?, arguments(&domain.arguments)?)))
                .collect::<Option<_>>()?,
        ),
        ValueType::StaticValue {
            carrier,
            value: StaticValue::Bool(value),
        } => FoldedSubject::StaticBoolean(Box::new(from_typed(carrier)?), *value),
        ValueType::StaticInteger { carrier, value } => FoldedSubject::StaticInteger(Box::new(from_typed(carrier)?), value.clone()),
        _ => return None,
    })
}

fn arguments(args: &[ValueType]) -> Option<Vec<FoldedSubject>> {
    args.iter().map(from_typed).collect()
}

fn fold_layout(layout: &SelectedLayout) -> Option<FoldedLayout> {
    Some(FoldedLayout {
        target: layout.target.clone(),
        subject: from_typed(&layout.subject)?,
        schema: layout.schema.clone(),
        plan: layout.plan.clone(),
        geometry: layout.geometry,
    })
}
fn fold_observation(
    observation: &TargetObservation,
    expected: &TargetIdentity,
) -> Option<FoldedTargetObservation> {
    Some(match observation {
        TargetObservation::Layout { target, projection, layout } if target == expected && &layout.target == expected => FoldedTargetObservation::Layout {
            target: target.clone(),
            projection: *projection,
            layout: Arc::new(fold_layout(layout)?),
        },
        TargetObservation::FloatRealization { target, realization } if target == expected => FoldedTargetObservation::FloatRealization {
            target: target.clone(),
            realization: *realization,
        },
        TargetObservation::SelectedRealization { target, realization } if target == expected => FoldedTargetObservation::SelectedRealization {
            target: target.clone(),
            realization: FoldedRealization {
                requirement: omega_target_contract::RequirementSlot {
                    declaration: portable(&realization.requirement.declaration)?,
                    trait_arguments: arguments(&realization.requirement.trait_arguments)?,
                },
                arguments: arguments(&realization.arguments)?,
                implementation: portable(&realization.implementation)?,
                conformance: match &realization.conformance {
                    Some(id) => Some(portable(id)?),
                    None => None,
                },
                binding: realization.binding.clone(),
                checked_contract: realization.checked_contract.clone(),
                selected_plan: realization.selected_plan.clone(),
            },
        },
        TargetObservation::Property { target, property } if target == expected => FoldedTargetObservation::Property {
            target: target.clone(),
            property: omega_target_contract::SelectedTargetProperty {
                projection_version: property.projection_version,
                value: match &property.value {
                    TargetProperty::ExclusiveAddressBound {
                        address_space,
                        exclusive_bound,
                    } => FoldedProperty::ExclusiveAddressBound {
                        address_space: address_space.clone(),
                        exclusive_bound: exclusive_bound.clone(),
                    },
                    TargetProperty::LayoutEndianness { layout, endianness } if &layout.target == expected => FoldedProperty::LayoutEndianness {
                        layout: Arc::new(fold_layout(layout)?),
                        endianness: *endianness,
                    },
                    TargetProperty::EntryStack { profile, guarantee } => FoldedProperty::EntryStack {
                        profile: profile.clone(),
                        guarantee: *guarantee,
                    },
                    _ => return None,
                },
            },
        },
        _ => return None,
    })
}

pub(crate) fn fold_target_closure(prepared: &PreparedTrees<'_>) -> Result<Option<FoldedTargetClosure>, LoweringError> {
    let all_observations = prepared.target_observations();
    let Some(first) = all_observations.first() else {
        return Ok(None);
    };
    let target = match first {
        TargetObservation::Layout { target, .. }
        | TargetObservation::FloatRealization { target, .. }
        | TargetObservation::SelectedRealization { target, .. }
        | TargetObservation::Property { target, .. } => target.clone(),
    };
    let mut pending = Vec::new();
    let mut record = |origin: PreparedValueOrigin, site: &Site| -> Result<(), LoweringError> {
        let Some(used) = prepared.dependencies().inputs(&origin) else {
            return Ok(());
        };
        let invalid = || LoweringError::new(site, "folded target inputs need a closed portable static identity");
        let origin = match origin {
            PreparedValueOrigin::Constant(declaration) => FoldedValueOrigin::Constant(portable(&declaration).ok_or_else(invalid)?),
            PreparedValueOrigin::StaticIndex(demand) => FoldedValueOrigin::StaticIndex {
                carrier: from_typed(&demand.request().carrier).ok_or_else(invalid)?,
                owner_arguments: match demand {
                    StaticValueDemand::Closed(_) => None,
                    StaticValueDemand::Specialized { arguments: args, .. } => Some(arguments(&args).ok_or_else(invalid)?),
                },
            },
        };
        pending.push((origin, site.clone(), used));
        Ok(())
    };
    for constant in &prepared.typed().constants {
        record(PreparedValueOrigin::Constant(constant.declaration.clone()), &constant.initializer.expression)?;
    }
    for (request, _) in &prepared.typed().evaluated_indices {
        record(
            PreparedValueOrigin::StaticIndex(StaticValueDemand::Closed(request.clone())),
            &request.expression,
        )?;
    }
    for index in prepared.specialized_indices() {
        record(
            PreparedValueOrigin::StaticIndex(StaticValueDemand::Specialized {
                request: index.request.clone(),
                arguments: index.arguments.clone(),
            }),
            &index.request.expression,
        )?;
    }
    pending.sort_unstable_by(|(left, left_site, _), (right, right_site, _)| (left, left_site).cmp(&(right, right_site)));
    let mut target_observations = Vec::new();
    let mut inputs: Vec<FoldedTargetInputs> = Vec::with_capacity(pending.len());
    for (origin, site, used) in pending {
        if inputs
            .last()
            .is_some_and(|previous| previous.origin == origin && previous.site == site)
        {
            return Err(LoweringError::new(&site, "folded target inputs share a portable static identity"));
        }
        let mut constants = used
            .constants
            .iter()
            .map(|id| crate::identity::declaration(id, &site))
            .collect::<Result<Vec<_>, _>>()?;
        constants.sort_unstable();
        let mut observations = Vec::with_capacity(used.target_observations.len());
        for observation in &used.target_observations {
            let closed =
                fold_observation(observation, &target).ok_or_else(|| LoweringError::new(&site, "target observation needs a closed portable subject"))?;
            let index = target_observations
                .iter()
                .position(|previous| previous == &closed)
                .unwrap_or_else(|| {
                    let index = target_observations.len();
                    target_observations.push(closed);
                    index
                });
            observations.push(index);
        }
        inputs.push(FoldedTargetInputs {
            origin,
            site,
            constants,
            observations,
        });
    }
    if target_observations.len() != all_observations.len() {
        let site = inputs
            .first()
            .map(|input| &input.site)
            .or_else(|| {
                prepared
                    .typed()
                    .constants
                    .first()
                    .map(|constant| &constant.initializer.expression)
            })
            .or_else(|| {
                prepared
                    .typed()
                    .signatures
                    .first()
                    .map(|signature| &signature.site)
            })
            .expect("target preparation occurs in a declaration");
        return Err(LoweringError::new(site, "target observation has no portable source origin"));
    }
    Ok(Some(FoldedTargetClosure::new(target, target_observations, inputs)))
}

#[cfg(test)]
mod tests;
