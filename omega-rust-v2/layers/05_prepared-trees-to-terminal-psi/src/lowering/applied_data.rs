//! A closed application has its own data identity. Member identities remain those
//! of the checked declaration, while L3 supplies the substituted storage types.
//!
//! A lifetime argument selects no storage. `Link<'r, const N: u64>` applied as
//! `Link<'a, 3>` in one machine and `Link<'b, 3>` in another is one schema,
//! chosen by `3` alone, so the two share one `DataId` and one type row. The
//! schema still says where its fields keep the declaration's lifetime binders,
//! exactly as a lifetime-only record does: `lifetime_count`, and each field's
//! `lifetime` and `aggregate_lifetimes`. A machine keeps which of its regions
//! it applies there in its `MachineRegion` slots.
use crate::declarations::data::FieldRegions;
use crate::declarations::regions::{self, RegionData, Stored};
use crate::{DataId, DataTemplate, LoweredCase, LoweredData, LoweredField, LoweringError, LoweringTypes, Site, TemplateField};
use omega_resolved::DeclarationId;
use omega_typed::{StaticDataShape, TypedTrees, ValueType};
use std::collections::HashMap;

pub(super) struct Application<'a> {
    /// One application with these value arguments. Another may differ from it
    /// in its lifetime arguments only, and has the same row.
    pub carrier: &'a ValueType,
    /// The type and const arguments, which select the schema. The lifetime
    /// arguments the declaration takes before them are not among these.
    arguments: &'a [ValueType],
    /// How many lifetime binders the declaration has before `arguments`.
    pub(super) lifetime_count: usize,
    shape: &'a StaticDataShape,
    template: &'a DataTemplate,
    /// Where each record field keeps the declaration's lifetime binders.
    field_regions: Vec<FieldRegions>,
    /// The same for each case of `shape.cases`, in that order.
    case_regions: Vec<Vec<FieldRegions>>,
}

/// Where each of `count` declared fields of `shape_owner` keeps the first
/// `lifetimes` binders of `owner`. A declaration with no lifetime stores
/// none. `None` when a field names a lifetime where a schema records nothing,
/// such as behind an exclusive reference to another record at that lifetime.
fn stored_regions(
    trees: &TypedTrees<'_>,
    owner: &DeclarationId,
    shape_owner: &DeclarationId,
    count: usize,
    lifetimes: usize,
    data: &RegionData<'_>,
) -> Option<Vec<FieldRegions>> {
    if lifetimes == 0 {
        return Some((0..count).map(|_| FieldRegions::default()).collect());
    }
    let declared = trees
        .record_fields
        .get(shape_owner)
        .map_or(&[][..], |fields| fields.as_slice());
    if declared.len() != count {
        return None;
    }
    declared
        .iter()
        .map(|field| {
            Some(match regions::stored(&field.ty, owner, lifetimes, data)? {
                Stored::Plain => FieldRegions::default(),
                Stored::Reference { binder, referent, .. } => FieldRegions {
                    lifetime: Some(binder),
                    aggregate_lifetimes: referent,
                },
                Stored::Record(aggregate_lifetimes) => FieldRegions {
                    lifetime: None,
                    aggregate_lifetimes,
                },
            })
        })
        .collect()
}

pub(super) fn collect<'a>(
    trees: &'a TypedTrees<'_>,
    templates: &'a [DataTemplate],
    region_data: &RegionData<'_>,
) -> Vec<Application<'a>> {
    let by_declaration: HashMap<_, _> = templates.iter().map(|t| (&t.declaration, t)).collect();
    let mut applications = Vec::new();
    for (carrier, shape) in &trees.static_data_shapes {
        let ValueType::Applied(owner, arguments) = carrier else { continue };
        // The declaration says how many of its leading arguments are
        // lifetimes. Those select nothing here; the rest are the key.
        let lifetime_count = region_data
            .indexed_lifetimes(owner)
            .map_or(0, |(lifetimes, _)| lifetimes);
        let Some(values) = arguments.get(lifetime_count..) else { continue };
        let Some(key) = values
            .iter()
            .map(|arg| match arg {
                ValueType::StaticInteger { carrier, value } => Some((super::types::scalar_carrier(carrier).ok()?, Some(value))),
                _ => Some((super::types::scalar_carrier(arg).ok()?, None)),
            })
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let Some(id) = crate::identity::portable(owner) else { continue };
        let Some(&template) = by_declaration.get(&id) else { continue };
        let indexed = key.iter().any(|(_, n)| n.is_some());
        if indexed && key.iter().any(|(_, n)| n.is_none()) {
            // Mixed type and const telescopes need one joint instantiation plan.
            continue;
        }
        let mut scopes = template
            .enclosing_parameters
            .iter()
            .map(|outer| by_declaration.get(outer).copied())
            .chain(std::iter::once(Some(template)));
        let mut lifetimes_left = lifetime_count;
        let type_arity = scopes.try_fold(0usize, |mut n, scope| {
            let scope = scope?;
            if scope.witness_count != 0 || !scope.equations.is_empty() || indexed && !scope.requires.is_empty() {
                return None;
            }
            for binder in &scope.parameters {
                if matches!(binder, crate::TemplateBinder::Lifetime) {
                    // Only the declaration's own leading lifetimes are
                    // skipped. A lifetime anywhere else, or in a declaration
                    // that takes nothing but lifetimes, is not a closed
                    // application.
                    if lifetimes_left == 0 || n != 0 {
                        return None;
                    }
                    lifetimes_left -= 1;
                    continue;
                }
                let (carrier, value) = key.get(n)?;
                match binder {
                    crate::TemplateBinder::Type { .. } if value.is_none() => {},
                    crate::TemplateBinder::Const(bound) if value.is_some() && bound.0 == [crate::TypeExpressionStep::Scalar(*carrier)] => {},
                    _ => return None,
                }
                n += 1;
            }
            Some(n)
        });
        if type_arity != Some(values.len()) || lifetimes_left != 0 {
            continue;
        }
        // A concrete schema with a default predicate needs that predicate instantiated at these arguments.
        if shape.requires_establishment || !template.requires.is_empty() || template.witness_count != 0 {
            continue;
        }
        let Some(field_regions) = stored_regions(trees, owner, owner, shape.fields.len(), lifetime_count, region_data) else {
            continue;
        };
        let Some(case_regions) = shape
            .cases
            .iter()
            .map(|(case, payload)| stored_regions(trees, owner, case, payload.len(), lifetime_count, region_data))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        applications.push((
            key,
            Application {
                carrier,
                arguments: values,
                lifetime_count,
                shape,
                template,
                field_regions,
                case_regions,
            },
        ));
    }
    applications.sort_unstable_by(|(ak, a), (bk, b)| (&a.template.declaration, ak).cmp(&(&b.template.declaration, bk)));
    // Two applications that differ only in their lifetime arguments are one
    // schema. Their shapes differ only in the regions the row erases and the
    // declaration supplies every region position, so either stands for both.
    applications.dedup_by(|(bk, b), (ak, a)| a.template.declaration == b.template.declaration && ak == bk);
    applications.into_iter().map(|(_, a)| a).collect()
}

impl<'a> Application<'a> {
    pub fn collect_carriers(
        &self,
        carriers: &mut Vec<(&'a ValueType, Site)>,
    ) {
        if !self.shape.cases.is_empty() {
            carriers.push((&ValueType::Bool, self.template.site.clone()));
        }
        carriers.extend(
            std::iter::once(self.carrier)
                .chain(self.arguments.iter().filter(|a| !matches!(a, ValueType::StaticInteger { .. })))
                .chain(&self.shape.fields)
                .chain(self.shape.cases.iter().flat_map(|(_, fields)| fields))
                .map(|ty| (ty, self.template.site.clone())),
        );
    }

    pub fn lower(
        &self,
        id: DataId,
        types: &LoweringTypes,
    ) -> Result<LoweredData, LoweringError> {
        let template = self.template;
        let id_for = |ty: &ValueType| {
            types
                .id_for(ty)
                .map_err(|error| LoweringError::new(&template.site, error.to_string()))
        };
        let fields = |declared: &[TemplateField], concrete: &[ValueType], stored: &[FieldRegions]| {
            if declared.len() != concrete.len() || declared.len() != stored.len() {
                return Err(LoweringError::new(&template.site, "applied data field roster disagrees with its template"));
            }
            declared
                .iter()
                .zip(concrete)
                .zip(stored)
                .map(|((field, ty), stored)| {
                    Ok(LoweredField {
                        declaration: field.declaration.clone(),
                        site: field.site.clone(),
                        lifetime: stored.lifetime,
                        aggregate_lifetimes: stored.aggregate_lifetimes.clone(),
                        carrier: types.id_for(ty).map_err(|e| LoweringError::new(&field.site, e.to_string()))?,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()
        };
        if template.cases.len() != self.shape.cases.len() {
            return Err(LoweringError::new(&template.site, "applied data case roster disagrees with its template"));
        }
        let cases = template
            .cases
            .iter()
            .map(|case| {
                let position = self
                    .shape
                    .cases
                    .iter()
                    .position(|(id, _)| crate::identity::portable(id).as_ref() == Some(&case.declaration))
                    .ok_or_else(|| LoweringError::new(&case.site, "applied shape is missing its declared case"))?;
                Ok(LoweredCase {
                    declaration: case.declaration.clone(),
                    fields: fields(&case.fields, &self.shape.cases[position].1, &self.case_regions[position])?,
                    numbering: case.numbering.clone(),
                    site: case.site.clone(),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        Ok(LoweredData {
            id,
            cleanup_target: None,
            write_only_cleanup: None,
            declaration: template.declaration.clone(),
            carrier: id_for(self.carrier)?,
            type_arguments: self
                .arguments
                .iter()
                .map(|arg| match arg {
                    ValueType::StaticInteger { carrier, value } => Ok(omega_psi::DataArgument::Integer {
                        carrier: super::types::scalar_carrier(carrier).map_err(|e| LoweringError::new(&template.site, e.to_string()))?,
                        value: value.clone(),
                    }),
                    ty => id_for(ty).map(omega_psi::DataArgument::Type),
                })
                .collect::<Result<_, LoweringError>>()?,
            public: template.public,
            multiplicity: template.multiplicity,
            lifetime_count: self.lifetime_count,
            value_indices: vec![],
            carry: template.carry.clone(),
            requires: vec![],
            fields: fields(&template.fields, &self.shape.fields, &self.field_regions)?,
            cases,
            numbering: template.numbering.clone(),
            site: template.site.clone(),
        })
    }
}
