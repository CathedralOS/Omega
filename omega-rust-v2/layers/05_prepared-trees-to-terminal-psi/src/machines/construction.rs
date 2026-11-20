use super::expressions::Expressions;
use crate::{FieldInitializer, LoweringError, MachineBody, OperationKind, OperationResult, TypeShape, site_at};
use omega_typed::{NodeSemantics, TypedConstruction, ValueType};

pub(super) struct Constructor {
    case: Option<crate::DeclarationId>,
    fields: Vec<(usize, crate::DeclarationId, crate::TypeId)>,
}
impl Constructor {
    pub(super) fn finish(
        self,
        expressions: &Expressions<'_>,
        operands: impl Iterator<Item = OperationResult>,
        body: &MachineBody,
        site: &crate::Site,
    ) -> Result<OperationKind, LoweringError> {
        let mut slots = vec![None; self.fields.len()];
        for ((position, field, carrier), value) in self.fields.into_iter().zip(operands) {
            let actual = match value {
                OperationResult::Scalar(v) => body.values[v.index()].ty,
                OperationResult::Structural(p) => body.places[p.index()].ty,
                OperationResult::Unit => return Err(LoweringError::new(site, "field initializer requires a value")),
            };
            if actual != carrier {
                return Err(LoweringError::new(site, "initializer produced the wrong field type"));
            }
            slots[position] = Some(FieldInitializer {
                field,
                operand: expressions.value_argument(value, body, site)?,
            });
        }
        let fields = slots
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| LoweringError::new(site, "missing field initializer"))?;
        Ok(match self.case {
            Some(case) => OperationKind::EstablishCase { case, fields },
            None => OperationKind::EstablishRecord { fields },
        })
    }
}

/// Initializers execute in authored order. Each carries the declaration slot where its result belongs.
pub(super) fn select<'a>(
    expressions: &'a Expressions<'_>,
    node: usize,
    carrier: &ValueType,
    construction: &TypedConstruction,
) -> Result<(Constructor, Vec<(usize, &'a ValueType)>), LoweringError> {
    let tree = expressions.tree;
    let site = site_at(tree, node);
    let error = |message| LoweringError::new(&site, message);
    if carrier != &construction.carrier {
        return Err(error("construction disagrees with the destination type"));
    }
    let mut typed = false;
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Expression(e) if e.static_argument_slot.is_none() && &e.ty == carrier => typed = true,
            _ => return Err(error("construction requires additional establishment evidence")),
        }
    }
    if !typed {
        return Err(error("construction lacks its exact expression type"));
    }
    let ty = expressions
        .types
        .id_for(carrier)
        .map_err(|e| LoweringError::new(&site, e.to_string()))?;
    let Some(TypeShape::Nominal(id)) = expressions.types.get(ty) else {
        return Err(error("construction requires a concrete nominal carrier"));
    };
    let schema = expressions
        .data
        .get(id.get() - 1)
        .ok_or_else(|| error("construction carrier has no data schema"))?;
    if schema.carrier != ty || schema.declaration != crate::identity::declaration(&construction.owner, &site)? {
        return Err(error("construction owner disagrees with the selected data schema"));
    }
    if !schema.requires.is_empty() || schema.carry.is_some() {
        return Err(error("constructor local requires predicate or cleanup evidence"));
    }
    let stage = expressions.prepared.typed();
    let shape = match carrier {
        ValueType::Applied(..) => Some(
            stage
                .static_data_shapes
                .get(carrier)
                .ok_or_else(|| error("construction lacks its applied field carriers"))?,
        ),
        ValueType::Nominal(_) => None,
        _ => return Err(error("construction requires a nominal data declaration")),
    };
    if shape.is_some_and(|shape| shape.requires_establishment) {
        return Err(error("construction requires its declaration predicate"));
    }
    let selected = construction
        .case
        .as_ref()
        .map(|case| crate::identity::declaration(case, &site))
        .transpose()?;
    let (schema_fields, carriers) = if let Some(case) = &selected {
        let fields = &schema
            .cases
            .iter()
            .find(|row| &row.declaration == case)
            .ok_or_else(|| error("construction selects a case outside its data schema"))?
            .fields;
        let carriers = shape
            .map(|shape| {
                shape
                    .cases
                    .iter()
                    .find(|(id, _)| Some(id) == construction.case.as_ref())
                    .map(|(_, types)| types)
                    .ok_or_else(|| error("construction lacks its selected payload carriers"))
            })
            .transpose()?;
        (fields, carriers)
    } else if schema.cases.is_empty() && shape.is_none_or(|shape| shape.cases.is_empty()) {
        (&schema.fields, shape.map(|s| &s.fields))
    } else {
        return Err(error("sum construction must select its case"));
    };
    if carriers.is_some_and(|types| types.len() != schema_fields.len()) || schema_fields.len() != construction.initializers.len() {
        return Err(error("construction needs a complete roster of explicit fields"));
    }
    let owner = construction.case.as_ref().unwrap_or(&construction.owner);
    let declarations = stage
        .record_fields
        .get(owner)
        .map_or(&[][..], |rows| rows.as_slice());
    if declarations.len() != schema_fields.len() {
        return Err(error("construction field roster disagrees with its declaration"));
    }
    for (source, lowered) in declarations.iter().zip(schema_fields) {
        if crate::identity::declaration(&source.declaration, &site)? != lowered.declaration {
            return Err(error("construction field identity disagrees with its declaration slot"));
        }
    }
    let mut fields = Vec::with_capacity(schema_fields.len());
    let mut visits = Vec::with_capacity(schema_fields.len());
    let mut initialized = vec![false; schema_fields.len()];
    for initializer in &construction.initializers {
        let Some(slot) = initialized.get_mut(initializer.position) else {
            return Err(error("invalid initializer slot"));
        };
        if std::mem::replace(slot, true) {
            return Err(error("field initialized more than once"));
        }
        let field = &schema_fields[initializer.position];
        let carrier = carriers.map_or(&declarations[initializer.position].ty, |types| &types[initializer.position]);
        if expressions.types.id_for(carrier).ok() != Some(field.carrier) {
            return Err(error("field construction disagrees with the prepared carrier"));
        }
        match expressions.types.get(field.carrier) {
            Some(TypeShape::PrimitiveScalar(_) | TypeShape::Nominal(_) | TypeShape::FixedArray { .. }) => {}
            // The operand copies the initialized descriptor. Its defining operation
            // still identifies any borrow and the place where that borrow began.
            Some(
                TypeShape::ScalarReference {
                    access: crate::ReferenceAccess::Shared,
                    ..
                }
                | TypeShape::NominalReference {
                    access: crate::ReferenceAccess::Shared,
                    ..
                },
            ) if field.lifetime.is_none() => {}
            _ => return Err(error("field construction needs its qualification or loan evidence")),
        }
        fields.push((initializer.position, field.declaration.clone(), field.carrier));
        visits.push((initializer.expression, carrier));
    }
    Ok((Constructor { case: selected, fields }, visits))
}
