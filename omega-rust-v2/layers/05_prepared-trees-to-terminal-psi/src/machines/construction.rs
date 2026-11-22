use super::expressions::Expressions;
use crate::{FieldInitializer, LoweredData, LoweringError, MachineBody, OperationKind, OperationResult, TypeId, TypeShape, site_at};
use omega_resolved::DeclarationId;
use omega_typed::{NodeSemantics, ObligationKind, Site, TypedConstruction, TypingObligation, ValueType};

mod zero;

pub(super) struct Constructor {
    case: Option<crate::DeclarationId>,
    written: Vec<(usize, crate::DeclarationId, TypeId)>,
    /// Each omitted slot, its carrier, what its zero appends to the body, and
    /// the site of the `ZeroField` obligation that requested it.
    zeros: Vec<(usize, crate::DeclarationId, TypeId, zero::Roster, Site)>,
}
impl Constructor {
    pub(super) fn finish(
        self,
        expressions: &Expressions<'_>,
        operands: impl Iterator<Item = OperationResult>,
        body: &mut MachineBody,
        site: &crate::Site,
    ) -> Result<OperationKind, LoweringError> {
        let mut slots = vec![None; self.written.len() + self.zeros.len()];
        for ((position, field, carrier), value) in self.written.into_iter().zip(operands) {
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
        // All authored computations have completed. Supply a dominating value for
        // each omission; the establishment still consumes a complete, ordered roster.
        for (position, field, carrier, roster, origin) in self.zeros {
            let value = zero::establish(expressions, body, &field, carrier, roster, &origin)?;
            slots[position] = Some(FieldInitializer {
                field,
                operand: expressions.value_argument(value, body, &origin)?,
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
    let mut establish_data = false;
    let mut omitted = Vec::new();
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Expression(e) if e.static_argument_slot.is_none() && &e.ty == carrier => typed = true,
            NodeSemantics::Obligation(TypingObligation {
                kind: ObligationKind::ZeroField { field, ty },
                site: origin,
                ..
            }) if origin == &site => omitted.push(Input::Zero(field, ty, origin)),
            NodeSemantics::Obligation(TypingObligation {
                kind: ObligationKind::EstablishData(owner),
                node: occurrence,
                site: origin,
            }) if !establish_data && owner == &construction.owner && occurrence == &tree.resolved().file.node_id(node) && origin == &site => {
                establish_data = true;
            }
            // A returned record that holds a borrow keeps it in the operands
            // that supply its fields. The return edge carries the constructed
            // place, as for any returned binding.
            NodeSemantics::Obligation(o) if matches!(&o.kind, ObligationKind::ReturnBorrow { machine } if machine == &expressions.signature.machine) => {}
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
    plan_fields(
        expressions,
        carrier,
        schema(expressions, ty, &construction.owner, establish_data, &site)?,
        &construction.owner,
        construction.case.as_ref(),
        construction
            .initializers
            .iter()
            .map(|i| Input::Written(i.position, i.expression))
            .chain(omitted),
        // Typing gives the initializer the field's type at the applied
        // lifetimes; the visit expects exactly that row. A plain reference
        // that typing binds to a lifetime of this construction has a
        // `BindLifetime` obligation there instead, as `bytes` does in
        // `Schema { storage: bytes }`. The visit then takes that destination
        // and `loans::view_at_region` consumes the obligation.
        |&initializer, at_lifetimes| {
            tree.semantics_at(initializer).find_map(|record| match record {
                NodeSemantics::Expression(e) if e.static_argument_slot.is_none() && &e.ty == at_lifetimes => Some(&e.ty),
                NodeSemantics::Obligation(TypingObligation {
                    kind: ObligationKind::BindLifetime { destination, .. },
                    ..
                }) if destination == at_lifetimes => Some(destination),
                _ => None,
            })
        },
        &site,
    )
}

pub(super) fn schema<'a>(
    expressions: &'a Expressions<'_>,
    ty: TypeId,
    owner: &DeclarationId,
    establish_data: bool,
    site: &Site,
) -> Result<&'a LoweredData, LoweringError> {
    let error = |message| LoweringError::new(site, message);
    let Some(TypeShape::Nominal(id)) = expressions.types.get(ty) else {
        return Err(error("construction requires a concrete nominal carrier"));
    };
    let schema = expressions
        .data
        .get(id.get() - 1)
        .ok_or_else(|| error("construction carrier has no data schema"))?;
    if schema.carrier != ty || schema.declaration != crate::identity::declaration(owner, site)? {
        return Err(error("construction owner disagrees with the selected data schema"));
    }
    // The operation's structural result selects this schema. All its clauses
    // are goals over the selected case and ordered field operands, not facts.
    if schema.requires.is_empty() == establish_data {
        return Err(error("construction obligation disagrees with its data predicates"));
    }
    if schema.carry.is_some() {
        return Err(error("constructor local requires predicate or cleanup evidence"));
    }
    Ok(schema)
}

/// Frozen values already supply every slot, including any zero established in preparation.
pub(super) fn plan<'a, T>(
    expressions: &'a Expressions<'_>,
    carrier: &ValueType,
    schema: &LoweredData,
    owner: &DeclarationId,
    case: Option<&DeclarationId>,
    inputs: impl Iterator<Item = (usize, T)>,
    site: &Site,
) -> Result<(Constructor, Vec<(T, &'a ValueType)>), LoweringError> {
    plan_fields(
        expressions,
        carrier,
        schema,
        owner,
        case,
        inputs.map(|(i, value)| Input::Written(i, value)),
        |_, _| None,
        site,
    )
}

enum Input<'a, T> {
    Written(usize, T),
    /// The omitted field, its declared type, and the requesting obligation's site.
    Zero(&'a DeclarationId, &'a ValueType, &'a Site),
}

fn plan_fields<'a, 'z, T>(
    expressions: &'a Expressions<'_>,
    carrier: &ValueType,
    schema: &LoweredData,
    owner: &DeclarationId,
    case: Option<&DeclarationId>,
    inputs: impl Iterator<Item = Input<'z, T>>,
    // The carrier a written input must have when its field's declared type
    // names a lifetime binder: that type at the construction's lifetimes.
    at_lifetimes: impl Fn(&T, &ValueType) -> Option<&'a ValueType>,
    site: &Site,
) -> Result<(Constructor, Vec<(T, &'a ValueType)>), LoweringError> {
    let error = |message| LoweringError::new(site, message);
    let stage = expressions.tree.forest();
    // A record applied at lifetimes only has no shape of its own. Its fields
    // are its declaration's, each at the lifetimes this construction applies.
    let lifetimes = match carrier {
        ValueType::Applied(_, arguments) if schema.lifetime_count != 0 && schema.lifetime_count == arguments.len() => Some(arguments.as_slice()),
        _ => None,
    };
    let shape = match carrier {
        ValueType::Applied(..) if lifetimes.is_some() => None,
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
    let selected = case
        .map(|id| crate::identity::declaration(id, site))
        .transpose()?;
    let (schema_fields, carriers) = if let Some(selected_case) = &selected {
        let fields = &schema
            .cases
            .iter()
            .find(|row| &row.declaration == selected_case)
            .ok_or_else(|| error("construction selects a case outside its data schema"))?
            .fields;
        let carriers = shape
            .map(|shape| {
                shape
                    .cases
                    .iter()
                    .find(|(id, _)| Some(id) == case)
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
    if carriers.is_some_and(|types| types.len() != schema_fields.len()) {
        return Err(error("construction carrier roster disagrees with its schema"));
    }
    let declarations = stage
        .record_fields
        .get(case.unwrap_or(owner))
        .map_or(&[][..], |rows| rows.as_slice());
    if declarations.len() != schema_fields.len() {
        return Err(error("construction field roster disagrees with its declaration"));
    }
    for (source, lowered) in declarations.iter().zip(schema_fields) {
        if crate::identity::declaration(&source.declaration, site)? != lowered.declaration {
            return Err(error("construction field identity disagrees with its declaration slot"));
        }
    }
    let mut written = Vec::with_capacity(schema_fields.len());
    let mut visits = Vec::with_capacity(schema_fields.len());
    let mut initialized = vec![false; schema_fields.len()];
    let mut zeros = Vec::new();
    for input in inputs {
        let position = match &input {
            Input::Written(position, _) => *position,
            Input::Zero(id, ..) => declarations
                .iter()
                .position(|row| &row.declaration == *id)
                .ok_or_else(|| error("omitted field does not belong to the selected construction"))?,
        };
        let Some(slot) = initialized.get_mut(position) else {
            return Err(error("invalid initializer slot"));
        };
        if std::mem::replace(slot, true) {
            return Err(error("field initialized more than once"));
        }
        let field = &schema_fields[position];
        let carrier = carriers.map_or(&declarations[position].ty, |types| &types[position]);
        // A field declared `at: @'t u8` keeps the reference as its row and
        // names the binder in `lifetime`, so compare the row with the reference.
        let row = match carrier {
            ValueType::AtLifetime(reference, _) if field.lifetime.is_some() => reference.as_ref(),
            declared => declared,
        };
        if expressions.types.id_for(row).ok() != Some(field.carrier) {
            return Err(error("field construction disagrees with the prepared carrier"));
        }
        let applied = lifetimes
            .filter(|_| carrier.components().any(|part| matches!(part, ValueType::Parameter { owner: binder, .. } if binder.as_ref() == owner)))
            .map(|arguments| carrier.substitute(owner, arguments));
        let input = match input {
            Input::Written(_, input) => input,
            Input::Zero(id, ty, origin) => {
                if applied.as_ref().unwrap_or(carrier) != ty {
                    return Err(error("omitted field obligation disagrees with its declared carrier"));
                }
                let unsupplied = |needed: &str, below: Option<&crate::DeclarationId>| {
                    let holder = below.map_or_else(String::new, |field| format!(" for `{}`", field.path));
                    LoweringError::new(
                        origin,
                        format!("omitted field `{}` of type `{ty}` needs {needed}{holder}, which `..` never supplies", id.path),
                    )
                };
                // `..` supplies no borrowed reference (data_and_literals.md,
                // explicit field omission), and a reference to one referent has
                // no zero that borrows nothing. A view's zero is the empty view,
                // so only a slice referent stays an open lowering form.
                if ty
                    .reference_parts()
                    .is_some_and(|(_, referent)| !matches!(referent, ValueType::Slice(_)))
                {
                    return Err(unsupplied("a borrowed reference", None));
                }
                // The same rule holds below the field: its zero is refused when any
                // field of an active payload would need a borrow or a callable.
                let roster = zero::admit(expressions.types, expressions.data, field.carrier).map_err(|refusal| match refusal.stop {
                    zero::Stop::Reference => unsupplied("a borrowed reference", refusal.below),
                    zero::Stop::Callable => unsupplied("a callable", refusal.below),
                    zero::Stop::Host => LoweringError::new(
                        origin,
                        format!("the zero of omitted field `{}` needs more operations than this host can address", id.path),
                    ),
                    zero::Stop::Blocked(reason) => {
                        let holder = refusal
                            .below
                            .map_or_else(String::new, |field| format!(" at `{}`", field.path));
                        LoweringError::new(
                            origin,
                            format!("cannot lower implicit zero of type `{ty}` for omitted field `{}`{holder}: {reason}", id.path),
                        )
                    }
                })?;
                zeros.push((position, field.declaration.clone(), field.carrier, roster, origin.clone()));
                continue;
            }
        };
        match expressions.types.get(field.carrier) {
            Some(TypeShape::PrimitiveScalar(_) | TypeShape::Nominal(_) | TypeShape::FixedArray { .. }) => {}
            // A by-value sequence is complete and immutable; the field copies it whole.
            Some(TypeShape::ElementSequence { .. }) => {}
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
            // A view stored at a lifetime binder of its record. The operand
            // copies the descriptor, as above, and this establishment is what
            // puts it at that binder: the checker reads the operand's defining
            // operation for the backing. `at_lifetimes` below requires typing's
            // row or binding for the field at the construction's lifetimes.
            Some(TypeShape::SharedByteView | TypeShape::SharedElementView { .. }) if field.lifetime.is_some() && applied.is_some() => {}
            _ => return Err(error("field construction needs its qualification or loan evidence")),
        }
        let carrier = match &applied {
            Some(applied) => at_lifetimes(&input, applied).ok_or_else(|| error("field construction disagrees with its type at the applied lifetimes"))?,
            None => carrier,
        };
        written.push((position, field.declaration.clone(), field.carrier));
        visits.push((input, carrier));
    }
    if initialized.contains(&false) {
        return Err(error("construction lacks an initializer or a zero obligation"));
    }
    Ok((
        Constructor {
            case: selected,
            written,
            zeros,
        },
        visits,
    ))
}
