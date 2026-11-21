//! A closed application has its own data identity. Member identities remain those
//! of the checked declaration, while L3 supplies the substituted storage types.
use crate::{DataId, DataTemplate, LoweredCase, LoweredData, LoweredField, LoweringError, LoweringTypes, Site, TemplateField};
use omega_typed::{StaticDataShape, TypedTrees, ValueType};
use std::collections::HashMap;

pub(super) struct Application<'a> {
    pub carrier: &'a ValueType,
    arguments: &'a [ValueType],
    shape: &'a StaticDataShape,
    template: &'a DataTemplate,
}

pub(super) fn collect<'a>(
    trees: &'a TypedTrees<'_>,
    templates: &'a [DataTemplate],
) -> Vec<Application<'a>> {
    let by_declaration: HashMap<_, _> = templates.iter().map(|t| (&t.declaration, t)).collect();
    let mut applications = Vec::new();
    for (carrier, shape) in &trees.static_data_shapes {
        let ValueType::Applied(owner, arguments) = carrier else { continue };
        let Some(key) = arguments
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
        let type_arity = scopes.try_fold(0usize, |mut n, scope| {
            let scope = scope?;
            if scope.witness_count != 0 || !scope.equations.is_empty() || indexed && !scope.requires.is_empty() {
                return None;
            }
            for binder in &scope.parameters {
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
        if type_arity != Some(arguments.len()) {
            continue;
        }
        // A concrete schema with a default predicate needs that predicate instantiated at these arguments.
        if shape.requires_establishment || !template.requires.is_empty() || template.witness_count != 0 {
            continue;
        }
        applications.push((
            key,
            Application {
                carrier,
                arguments,
                shape,
                template,
            },
        ));
    }
    applications.sort_unstable_by(|(ak, a), (bk, b)| (&a.template.declaration, ak).cmp(&(&b.template.declaration, bk)));
    applications.into_iter().map(|(_, a)| a).collect()
}

impl<'a> Application<'a> {
    pub fn collect_carriers(
        &self,
        carriers: &mut Vec<(&'a ValueType, Site)>,
    ) {
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
        let fields = |declared: &[TemplateField], concrete: &[ValueType]| {
            if declared.len() != concrete.len() {
                return Err(LoweringError::new(&template.site, "applied data field roster disagrees with its template"));
            }
            declared
                .iter()
                .zip(concrete)
                .map(|(field, ty)| {
                    Ok(LoweredField {
                        declaration: field.declaration.clone(),
                        site: field.site.clone(),
                        lifetime: None,
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
                let (_, payload) = self
                    .shape
                    .cases
                    .iter()
                    .find(|(id, _)| crate::identity::portable(id).as_ref() == Some(&case.declaration))
                    .ok_or_else(|| LoweringError::new(&case.site, "applied shape is missing its declared case"))?;
                Ok(LoweredCase {
                    declaration: case.declaration.clone(),
                    fields: fields(&case.fields, payload)?,
                    numbering: case.numbering.clone(),
                    site: case.site.clone(),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        Ok(LoweredData {
            id,
            cleanup_target: None,
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
            lifetime_count: 0,
            value_indices: vec![],
            carry: template.carry.clone(),
            requires: vec![],
            fields: fields(&template.fields, &self.shape.fields)?,
            cases,
            numbering: template.numbering.clone(),
            site: template.site.clone(),
        })
    }
}
