//! Lower owned transfers from the typed place. Schedule normal cleanup against
//! each predecessor's actual transfers after all blocks have been emitted.
use super::expressions::{EmitOperation, Expressions, OperationOutput};
use crate::{LoweringError, MachineBody, OperationKind, OperationResult, PlaceId, PlaceProjection, TypeShape, site_at};
use omega_syntax::NodeKind;
use omega_typed::{NodeSemantics, ObligationKind, ValueType};
mod cleanup;
pub(crate) use cleanup::{OwnedCrossings, admit_sequence_elements};
pub(super) use cleanup::{ReturnBackings, dispose_temporary, owned, schedule};

/// An owned transfer needs the entire selected subtree. A parked borrow that
/// overlaps a hole stays in Psi for the checker to judge.
fn overlaps_moved_path(
    moved: &[Vec<PlaceProjection>],
    selected: &[PlaceProjection],
) -> bool {
    moved.iter().any(|hole| hole.starts_with(selected) || selected.starts_with(hole))
}

/// `carrier` is the type the moved value has by itself. `landing` is the
/// destination its lifetime binding names, when the caller consumes that
/// binding at `node` (`loans::binding_at`).
pub(super) fn move_value(
    expressions: &Expressions<'_>,
    node: usize,
    carrier: &ValueType,
    landing: Option<&ValueType>,
    body: &mut MachineBody,
) -> Result<OperationResult, LoweringError> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let site = site_at(tree, node);
    let explicit = file.nodes[node].kind == NodeKind::Move;
    let operand = if explicit {
        let [operand] = *file.nodes[node].children.as_slice() else {
            return Err(LoweringError::new(&site, "move requires one place"));
        };
        let mut typed = false;
        let mut moved = false;
        for row in tree.semantics_at(node) {
            match row {
                NodeSemantics::Expression(e) if !typed && e.ty == *carrier && e.static_argument_slot.is_none() => typed = true,
                NodeSemantics::Obligation(o) if !moved && matches!(&o.kind,ObligationKind::Move{place,ty} if *place==site_at(tree,operand) && ty==carrier) => {
                    moved = true
                }
                // The return edge names the moved place. Its loan can be the
                // value itself or be stored inside the returned aggregate.
                NodeSemantics::Obligation(o)
                    if matches!(&o.kind,ObligationKind::ReturnBorrow{machine} if machine==&expressions.signature.machine) => {}
                // An explicit move lands as a whole, so its binding is on the
                // `move`, not on the place it moves.
                NodeSemantics::Obligation(o) if super::loans::is_binding(&o.kind, carrier, landing) => {}
                _ => return Err(LoweringError::new(&site, "move requires additional typed evidence")),
            }
        }
        if !typed || !moved {
            return Err(LoweringError::new(&site, "move lacks its typed operand obligation"));
        }
        operand
    } else {
        node
    };
    let (source, ty) = super::references::move_place(expressions, operand, body, landing.filter(|_| !explicit))?;
    if ty != carrier {
        return Err(LoweringError::new(&site, "move source and result differ in type"));
    }
    let ty = expressions
        .types
        .id_for(carrier)
        .map_err(|e| LoweringError::new(&site, e.to_string()))?;
    let typed_endpoint = || tree.semantics_at(file.ungroup(operand));
    let crosses_loan = source.path.contains(&PlaceProjection::Referent);
    if !crosses_loan
        && typed_endpoint()
            .any(|row| matches!(row,NodeSemantics::Obligation(o) if matches!(o.kind,ObligationKind::RestoreBorrowed{..}|ObligationKind::CopyRead(_))))
    {
        return Err(LoweringError::new(&site, "owned field access carries an unrelated loan obligation"));
    }
    // An exclusive reference, `@mutable` or `@write`, owes no cleanup, but it is
    // affine: a by-value use transfers the loan carrier and never copies it
    // (language/ownership.md, "A by-value reference transfers or copies its
    // loan carrier").
    let exclusive = crate::lowering::types::is_exclusive_reference(carrier);
    if !owned(ty, expressions.types, expressions.data) && !exclusive {
        if crosses_loan
            && typed_endpoint()
                .filter(|row| matches!(row,NodeSemantics::Obligation(o) if matches!(&o.kind,ObligationKind::CopyRead(read) if read==carrier)))
                .count()
                != 1
        {
            return Err(LoweringError::new(&site, "borrowed field copy lacks its exact copy-read obligation"));
        }
        if !explicit && source.path.is_empty() {
            return Ok(OperationResult::Structural(source.root));
        }
        return Ok(body.push_operation(OperationOutput::Structural(ty), OperationKind::CopyStructural { source }, site));
    }
    let mut owner_ty = body.places[source.root.index()].ty;
    let mut active_case = None;
    for step in &source.path {
        owner_ty = match (step, expressions.types.get(owner_ty)) {
            (PlaceProjection::Referent, Some(TypeShape::NominalReference { referent, .. })) => expressions.data[referent.get() - 1].carrier,
            (PlaceProjection::Case(case), Some(TypeShape::Nominal(id))) => {
                active_case = Some(
                    expressions.data[id.get() - 1]
                        .cases
                        .iter()
                        .find(|c| &c.declaration == case)
                        .ok_or_else(|| LoweringError::new(&site, "selected payload disagrees with its sum"))?,
                );
                owner_ty
            }
            (PlaceProjection::Field(field), Some(TypeShape::Nominal(id))) => {
                // A selected cleanup forbids partial moves, but L6 owns that
                // verdict. Publish the exact path; the owner's declaration
                // keeps its cleanup target for the checker to compare.
                let owner = &expressions.data[id.get() - 1];
                active_case
                    .take()
                    .map_or(&owner.fields, |case| &case.fields)
                    .iter()
                    .find(|f| &f.declaration == field)
                    .ok_or_else(|| LoweringError::new(&site, "owned field move requires its selected field"))?
                    .carrier
            }
            (PlaceProjection::FixedIndex(ordinal), Some(TypeShape::FixedArray { element, length })) if active_case.is_none() && ordinal < length => *element,
            _ => return Err(LoweringError::new(&site, "owned field move requires a concrete original place")),
        };
    }
    if source.path.contains(&PlaceProjection::Referent) {
        return super::loans::move_borrowed(expressions, node, source, carrier, body);
    }
    Ok(body.push_operation(OperationOutput::Structural(ty), OperationKind::MoveStructural { source }, site))
}

/// Partition a complete value in the declaring package or its selected cleanup.
/// Bound fields get independent identities. Ignored children keep their own disposition.
pub(super) fn decompose(
    expressions: &mut Expressions<'_>,
    pattern: usize,
    initializer: usize,
    body: &mut MachineBody,
    flow: &mut super::branches::BlockFlow,
) -> Result<(), LoweringError> {
    use super::body::{MachineBodyValues, next_id};
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let site = site_at(tree, pattern);
    let (source, ty) = super::references::original_place(expressions, initializer, body)?;
    let tyid = expressions
        .types
        .id_for(ty)
        .map_err(|e| LoweringError::new(&site, e.to_string()))?;
    let Some(TypeShape::Nominal(id)) = expressions.types.get(tyid) else {
        return Err(LoweringError::new(&site, "decomposition requires a nominal record"));
    };
    let schema = &expressions.data[id.get() - 1];
    let [head, members @ ..] = file.nodes[pattern].children.as_slice() else {
        return Err(LoweringError::new(&site, "record pattern lacks its declaration"));
    };
    if !source.path.is_empty()
        || !matches!(tree.resolved().symbol(*head),Some(omega_resolved::Symbol::Declaration(d)) if Some(d)==ty.nominal_parts().map(|parts|parts.0))
        || !schema.cases.is_empty()
    {
        return Err(LoweringError::new(&site, "decomposition requires the complete record and its field partition"));
    }
    if schema.cleanup_target.is_some()
        && schema.declaration.package != expressions.signature.machine.package
        && schema.cleanup_target != Some(expressions.machine)
    {
        return Err(LoweringError::new(&site, "consuming pattern requires its carrier's owner or selected cleanup"));
    }
    let mut binders = vec![None; schema.fields.len()];
    let mut seen = vec![false; schema.fields.len()];
    let mut rest = false;
    for &member in members {
        let node = &file.nodes[member];
        if node.kind == NodeKind::RestPattern && node.children.is_empty() && !rest {
            rest = true;
            continue;
        }
        if node.kind != NodeKind::FieldPattern || !node.children.is_empty() {
            return Err(LoweringError::new(&site, "decomposition needs an irrefutable field partition"));
        }
        let (_, steps) = expressions.tree.forest().place_path(file.node_id(member));
        let Some(step) = steps.last() else {
            return Err(LoweringError::new(&site, "decomposition lacks its selected field"));
        };
        let Some(omega_typed::Projection::Field(id)) = &step.projection else {
            return Err(LoweringError::new(&site, "decomposition requires a typed field selection"));
        };
        if step.through.is_some() || step.base != file.node_id(file.ungroup(initializer)) {
            return Err(LoweringError::new(&site, "decomposition must consume its original subject"));
        }
        let id = crate::identity::declaration(id, &site)?;
        let slot = schema
            .fields
            .iter()
            .position(|f| f.declaration == id)
            .ok_or_else(|| LoweringError::new(&site, "pattern field does not belong to its subject"))?;
        if std::mem::replace(&mut seen[slot], true) {
            return Err(LoweringError::new(&site, "pattern selects a field more than once"));
        }
        if tree.resolved().pattern_binding(member).is_some() {
            binders[slot] = Some(member);
        }
    }
    if !rest && seen.contains(&false) {
        return Err(LoweringError::new(&site, "decomposition requires every field"));
    }
    let operation = crate::OperationId(next_id(body.operations.len()));
    let mut fields = Vec::with_capacity(schema.fields.len());
    let mut ignored = Vec::new();
    for (position, (f, binder)) in schema.fields.iter().zip(binders).enumerate() {
        let result = match expressions.types.get(f.carrier) {
            Some(TypeShape::PrimitiveScalar(_)) => OperationResult::Scalar(body.push_value(f.carrier, crate::ValueOrigin::Component { operation, position })),
            Some(TypeShape::Nominal(_) | TypeShape::FixedArray { .. }) => {
                let p = PlaceId(next_id(body.places.len()));
                body.places.push(crate::Place {
                    ty: f.carrier,
                    origin: crate::PlaceOrigin::Component { operation, position },
                });
                if binder.is_none() && owned(f.carrier, expressions.types, expressions.data) {
                    ignored.push(p);
                }
                OperationResult::Structural(p)
            }
            _ => {
                return Err(LoweringError::new(
                    &site,
                    "decomposition requires the component's qualification or loan handoff",
                ));
            }
        };
        fields.push(crate::DecomposedField {
            field: f.declaration.clone(),
            result,
        });
        if let Some(binder) = binder {
            expressions.bindings.push((binder, result));
        }
    }
    body.push_operation(
        OperationOutput::Unit,
        OperationKind::DecomposeRecord {
            source,
            declaration: schema.declaration.clone(),
            fields,
        },
        site.clone(),
    );
    for p in ignored.into_iter().rev() {
        dispose_temporary(body, flow, p, site.clone());
    }
    Ok(())
}
