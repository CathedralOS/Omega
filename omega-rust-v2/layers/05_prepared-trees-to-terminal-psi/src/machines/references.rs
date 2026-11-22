use super::expressions::Expressions;
use crate::{CallArgument, LoweringError, MachineBody, OperationResult, PlaceProjection, ProjectedPlace, site_at};
use omega_resolved::Symbol;
use omega_syntax::NodeKind;
use omega_typed::{Access, NodeSemantics, ObligationKind, Projection, ValueType};

pub(super) fn call_argument(
    expressions: &Expressions<'_>,
    node: usize,
    formal: &ValueType,
    body: &MachineBody,
) -> Result<Option<CallArgument>, LoweringError> {
    // A call names the regions it applies in its `CallRegions` row, and a
    // borrow operand has no descriptor place to list in
    // `MachineBody::lifetime_bindings`. So an operand that typing bound to a
    // region binder of the calling machine has nowhere to publish that
    // binding, and the call is refused.
    let tree = expressions.tree;
    let operand = tree.resolved().file.ungroup(node);
    let declared = crate::declarations::regions::direct(formal, &expressions.signature.machine).is_some();
    if declared && super::loans::binding_at(tree, operand, formal).is_some() {
        return Err(LoweringError::new(
            &site_at(tree, operand),
            "a call operand bound to a declared region needs its binding",
        ));
    }
    reference_operand(expressions, node, formal, body)
}

/// The place a reference value at `node` lends, when the node names one: an
/// explicit borrow, or an existing descriptor whose referent is lent.
///
/// `destination` is the type the value lands at: a callee's formal at the
/// call's regions, or the type of the descriptor being formed. It may hold the
/// reference at a lifetime, as in `first::<'u>(cell)`; the lent place is the
/// same, and the region is the call's `CallRegions` argument or the
/// descriptor's binding. Where typing bound the value to that lifetime, the
/// node's own rows state the value's own type and this consumes that one
/// binding (`loans::binding_at`). The caller publishes it.
pub(super) fn reference_operand(
    expressions: &Expressions<'_>,
    node: usize,
    destination: &ValueType,
    body: &MachineBody,
) -> Result<Option<CallArgument>, LoweringError> {
    let Some((access, wanted)) = crate::lowering::types::place_reference(destination) else {
        return Ok(None);
    };
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let node = super::expressions::ungroup_place_value(tree, node, destination)?;
    // Expressions that compute a descriptor must run in operand order. Only
    // an existing place or explicit borrow can name the referent immediately.
    if !matches!(
        file.nodes[node].kind,
        NodeKind::Borrow | NodeKind::Dereference | NodeKind::FieldAccess | NodeKind::PathExpression | NodeKind::Index
    ) {
        return Ok(None);
    }
    let binding = super::loans::binding_at(tree, node, destination);
    let formal = binding.map_or(destination, |bound| bound.reference);
    let landing = binding.map(|bound| bound.destination);
    let expected_borrow = match access {
        crate::ReferenceAccess::Shared => Access::Shared,
        crate::ReferenceAccess::Mutable => Access::Mut,
        crate::ReferenceAccess::WriteOnly => Access::Write,
    };
    let place = if file.nodes[node].kind == NodeKind::Borrow {
        let [operand] = *file.nodes[node].children.as_slice() else {
            return Err(LoweringError::new(&site_at(tree, node), "borrow requires one original place"));
        };
        let occurrence = site_at(tree, operand);
        let mut typed = false;
        let mut borrowed = false;
        let mut consumed = false;
        for record in tree.semantics_at(node) {
            match record {
                NodeSemantics::Expression(e) if &e.ty == formal && e.static_argument_slot.is_none() => typed = true,
                NodeSemantics::Obligation(o)
                    if !borrowed && matches!(&o.kind, ObligationKind::Borrow{access,place} if access==&expected_borrow && place==&occurrence) =>
                {
                    borrowed = true
                }
                // The binding is consumed at the borrow itself, not merely
                // through a lifetime mentioned somewhere in the signature.
                NodeSemantics::Obligation(o) if !consumed && super::loans::is_binding(&o.kind, formal, landing) => consumed = true,
                NodeSemantics::Obligation(o) if matches!(&o.kind,ObligationKind::ReturnBorrow{machine} if machine==&expressions.signature.machine) => {}
                _ => return Err(LoweringError::new(&site_at(tree, node), "unsupported reference formation record")),
            }
        }
        if !typed || !borrowed {
            return Err(LoweringError::new(&site_at(tree, node), "shared borrow lacks its type or source obligation"));
        }
        let (place, ty) = original_place(expressions, operand, body)?;
        if ty != wanted {
            return Err(LoweringError::new(&occurrence, "borrowed field has the wrong referent type"));
        }
        place
    } else {
        let (mut place, ty) = project_place(expressions, node, body, PlaceUse::Original, landing)?;
        if ty != formal {
            return Err(LoweringError::new(&site_at(tree, node), "borrowed argument has the wrong reference type"));
        }
        place.path.push(PlaceProjection::Referent);
        place
    };
    // L3 decided whether this place may be lent with this access: an explicit
    // borrow carries its `Borrow` obligation, and a forwarded descriptor has
    // the formal's exact type, so the child keeps its parent's access. The
    // descriptor publishes the access and every loan the path crosses, from
    // which the checker judges the lineage
    // (wiki/spec/terminal-psi/loans.md#reborrow-lineage-and-access).
    Ok(Some(CallArgument::Borrow { place, access }))
}

/// A direct borrow has no value computation. Walk the typed field selections
/// down to a bound place and keep each crossing back into original storage.
pub(super) fn original_place<'a>(
    expressions: &'a Expressions<'_>,
    node: usize,
    body: &MachineBody,
) -> Result<(ProjectedPlace, &'a ValueType), LoweringError> {
    project_place(expressions, node, body, PlaceUse::Original, None)
}

/// `landing` is the destination the moved value's lifetime binding names,
/// when the caller consumes that binding at `node` (`loans::binding_at`).
pub(super) fn move_place<'a>(
    expressions: &'a Expressions<'_>,
    node: usize,
    body: &MachineBody,
    landing: Option<&ValueType>,
) -> Result<(ProjectedPlace, &'a ValueType), LoweringError> {
    project_place(expressions, node, body, PlaceUse::Move, landing)
}

fn project_place<'a>(
    expressions: &'a Expressions<'_>,
    node: usize,
    body: &MachineBody,
    usage: PlaceUse,
    landing: Option<&ValueType>,
) -> Result<(ProjectedPlace, &'a ValueType), LoweringError> {
    let (root, path, ty) = select_place(expressions, node, body, usage, landing)?;
    let PlaceRoot::Bound(root) = root else {
        unreachable!("original places require a bound root")
    };
    Ok((ProjectedPlace { root, path }, ty))
}

pub(super) enum PlaceRoot<'a> {
    Bound(crate::PlaceId),
    Expression(usize, &'a ValueType),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlaceUse {
    Original,
    Move,
    ScalarRead,
}

/// The selected place has the type its own rows state. `landing` is the
/// destination named by the lifetime binding recorded on the selection
/// itself, when the caller consumes that binding (`loans::binding_at`); with
/// `None`, a lifetime binding there is refused like any other unread row.
fn select_place<'a>(
    expressions: &'a Expressions<'_>,
    node: usize,
    body: &MachineBody,
    usage: PlaceUse,
    landing: Option<&ValueType>,
) -> Result<(PlaceRoot<'a>, Vec<PlaceProjection>, &'a ValueType), LoweringError> {
    let tree = expressions.tree;
    let resolved = tree.resolved();
    let file = resolved.file;
    let mut current = node;
    let mut fields = Vec::new();
    let mut selected_type = None;
    loop {
        let site = site_at(tree, current);
        // A field of a computed record reads the result of that
        // expression. Its calls, selections and bounds checks execute once.
        if usage == PlaceUse::ScalarRead
            && !fields.is_empty()
            && !matches!(
                file.nodes[current].kind,
                NodeKind::PathExpression | NodeKind::FieldAccess | NodeKind::Dereference | NodeKind::Paren
            )
        {
            let ty = tree
                .semantics_at(current)
                .find_map(|record| match record {
                    NodeSemantics::Expression(e) if e.static_argument_slot.is_none() => Some(&e.ty),
                    _ => None,
                })
                .ok_or_else(|| LoweringError::new(&site, "field receiver has no result type"))?;
            if ty.nominal_parts().is_none() {
                return Err(LoweringError::new(&site, "computed field receiver requires a record"));
            }
            fields.reverse();
            return Ok((PlaceRoot::Expression(current, ty), fields, selected_type.expect("field has a selected type")));
        }
        let mut field = None;
        let mut ty = None;
        let mut copied = None;
        let mut bound = None;
        for record in tree.semantics_at(current) {
            match record {
                NodeSemantics::Obligation(o)
                    if bound.is_none()
                        && landing.is_some()
                        && current == file.ungroup(node)
                        && matches!(&o.kind, ObligationKind::BindLifetime { destination, .. } if Some(destination) == landing) =>
                {
                    if let ObligationKind::BindLifetime { reference, .. } = &o.kind {
                        bound = Some(reference);
                    }
                }
                NodeSemantics::Expression(e) if e.static_argument_slot.is_none() && ty.is_none_or(|t| t == &e.ty) => ty = Some(&e.ty),
                NodeSemantics::Field(id) if field.is_none() => field = Some(id),
                NodeSemantics::Obligation(o)
                    if usage != PlaceUse::Original && current == file.ungroup(node) && copied.is_none() && matches!(o.kind, ObligationKind::CopyRead(_)) =>
                {
                    if let ObligationKind::CopyRead(carrier) = &o.kind {
                        copied = Some(carrier);
                    }
                }
                NodeSemantics::Obligation(o)
                    if usage == PlaceUse::Move
                        && current == file.ungroup(node)
                        && matches!(&o.kind,ObligationKind::RestoreBorrowed{place,..} if *place==file.node_id(current)) => {}
                // A returned stored reference reborrows this place, or for an
                // exclusive one moves it out; the return edge carries that
                // descriptor, as for a returned binding.
                NodeSemantics::Obligation(o)
                    if matches!(usage, PlaceUse::Original | PlaceUse::Move)
                        && current == file.ungroup(node)
                        && matches!(&o.kind,ObligationKind::ReturnBorrow{machine} if machine==&expressions.signature.machine) => {}
                _ => return Err(LoweringError::new(&site, "reference projection requires a typed original place")),
            }
        }
        let ty = ty.ok_or_else(|| LoweringError::new(&site, "reference projection lacks a receiver type"))?;
        if bound.is_some_and(|reference| reference != ty) {
            return Err(LoweringError::new(&site, "lifetime binding disagrees with the place it binds"));
        }
        selected_type.get_or_insert(ty);
        match (file.nodes[current].kind, file.nodes[current].children.as_slice(), field) {
            (NodeKind::Paren, [inner], None) => current = *inner,
            (NodeKind::FieldAccess, [base], Some(id)) => {
                let (_, steps) = expressions.tree.forest().place_path(file.node_id(current));
                let Some(step) = steps
                    .last()
                    .filter(|step| step.base == file.node_id(file.ungroup(*base)) && matches!(&step.projection,Some(Projection::Field(f)) if f.as_ref()==id))
                else {
                    return Err(LoweringError::new(&site, "field disagrees with its typed place projection"));
                };
                if copied.is_some_and(|carrier| {
                    carrier != ty
                        || expressions
                            .types
                            .id_for(carrier)
                            .map_or(true, |ty| super::ownership::owned(ty, expressions.types, expressions.data))
                }) {
                    return Err(LoweringError::new(&site, "field copy requires its exact copyable borrowed carrier"));
                }
                let declaration = crate::identity::declaration(id, &site)?;
                fields.push(PlaceProjection::Field(declaration.clone()));
                let receiver = tree.semantics_at(*base).find_map(|row| match row {
                    NodeSemantics::Expression(e) => Some(&e.ty),
                    _ => None,
                });
                if let Some(crate::TypeShape::Nominal(owner)) = receiver
                    .map(|r| r.reference_parts().map_or(r, |(_, inner)| inner))
                    .and_then(|r| expressions.types.id_for(r).ok())
                    .and_then(|id| expressions.types.get(id))
                    && let Some(case) = expressions.data[owner.get() - 1]
                        .cases
                        .iter()
                        .find(|c| c.fields.iter().any(|f| f.declaration == declaration))
                {
                    fields.push(PlaceProjection::Case(case.declaration.clone()));
                }
                // L3 published the loan this selection crosses and already
                // refused a read through `@write`; the path keeps the crossing.
                if step.through.is_some() {
                    fields.push(PlaceProjection::Referent);
                }
                current = *base;
            }
            (NodeKind::Dereference, [base], None) => {
                let (_, steps) = expressions.tree.forest().place_path(file.node_id(current));
                let receiver = tree.semantics_at(*base).find_map(|row| match row {
                    NodeSemantics::Expression(expression) => expression.ty.reference_parts(),
                    _ => None,
                });
                let valid = receiver.is_some_and(|(access, referent)| {
                    referent == ty
                        && steps.last().is_some_and(|step| {
                            step.base == file.node_id(file.ungroup(*base)) && step.through.as_ref() == Some(access) && step.projection.is_none()
                        })
                });
                if !valid || copied.is_some_and(|carrier| carrier != ty) {
                    return Err(LoweringError::new(&site, "dereference disagrees with its typed place projection"));
                }
                fields.push(PlaceProjection::Referent);
                current = *base;
            }
            // Every fixed ordinal on the route to the moved operand belongs to its claim.
            (NodeKind::Index, [base, _], None) if usage == PlaceUse::Move && copied.is_none() => {
                fields.push(super::fixed_arrays::element_step(expressions, current, ty)?);
                current = *base;
            }
            (NodeKind::PathExpression, [], None) if copied.is_none() => {
                let Some(Symbol::Binding { binder_node, .. }) = resolved.symbol(current) else {
                    return Err(LoweringError::new(&site, "reference root requires a bound place"));
                };
                let source = expressions
                    .bindings
                    .iter()
                    .rev()
                    .find(|(n, _)| n == binder_node)
                    .map(|row| row.1);
                let Some(OperationResult::Structural(root)) = source else {
                    return Err(LoweringError::new(&site, "reference root has no structural place"));
                };
                let carrier = if file.nodes[*binder_node].kind == NodeKind::Param {
                    crate::declarations::regions::slot_carrier(ty, &expressions.signature.machine)
                } else {
                    ty
                };
                if expressions.types.id_for(carrier).ok() != Some(body.places[root.index()].ty) {
                    return Err(LoweringError::new(&site, "reference root disagrees with its place type"));
                }
                fields.reverse();
                return Ok((PlaceRoot::Bound(root), fields, selected_type.expect("each selected place has a type")));
            }
            _ => return Err(LoweringError::new(&site, "unsupported original storage projection")),
        }
    }
}

pub(super) fn scalar_read<'a>(
    expressions: &'a Expressions<'_>,
    node: usize,
    carrier: &ValueType,
    body: &MachineBody,
) -> Result<(PlaceRoot<'a>, Vec<PlaceProjection>), LoweringError> {
    let tree = expressions.tree;
    let site = site_at(tree, node);
    let borrowed_read = tree
        .semantics_at(node)
        .any(|row| matches!(row, NodeSemantics::Obligation(o) if matches!(&o.kind, ObligationKind::CopyRead(ty) if ty == carrier)));
    // Typing publishes CopyRead when the scalar comes through a loan. An owned
    // field has the same selected path and no loan to justify copying through.
    let (root, path, selected) = select_place(expressions, node, body, PlaceUse::ScalarRead, None)?;
    if selected != carrier || borrowed_read != path.contains(&PlaceProjection::Referent) {
        return Err(LoweringError::new(
            &site,
            "scalar read disagrees with its type or borrowed copy-read obligation",
        ));
    }
    Ok((root, path))
}

pub(super) fn dispatch_cases(
    expressions: &Expressions<'_>,
    dispatch: usize,
    body: &mut MachineBody,
    flow: &mut super::branches::BlockFlow,
) -> Result<Option<Vec<(crate::BlockId, usize)>>, LoweringError> {
    use super::expressions::{EmitOperation, OperationOutput};
    use crate::{BlockTerminator, OperationKind};
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let Some((&subject, arms)) = file.nodes[dispatch].children.split_first() else {
        return Ok(None);
    };
    let Some(carrier) = tree.semantics_at(subject).find_map(|record| match record {
        NodeSemantics::Expression(e) => Some(&e.ty),
        _ => None,
    }) else {
        return Ok(None);
    };
    let borrowed = crate::lowering::types::nominal_reference(carrier).is_some();
    let underlying = carrier.reference_parts().map_or(carrier, |(_, inner)| inner);
    let Some(crate::TypeShape::Nominal(owner)) = expressions
        .types
        .id_for(underlying)
        .ok()
        .and_then(|ty| expressions.types.get(ty))
    else {
        return Ok(None);
    };
    if expressions.data[owner.get() - 1].cases.is_empty() {
        return Ok(None);
    }
    let (mut source, _) = original_place(expressions, subject, body)?;
    if borrowed {
        source.path.push(PlaceProjection::Referent);
    }
    let mut targets = Vec::with_capacity(arms.len());
    let mut tests = Vec::with_capacity(arms.len());
    for &arm in arms {
        let [pattern, expression] = *file.nodes[arm].children.as_slice() else {
            return Err(LoweringError::new(&site_at(tree, arm), "case arm requires a pattern and a body"));
        };
        super::body::check_container(tree, arm)?;
        let selected = expressions
            .tree
            .forest()
            .complete_case_patterns
            .get(&file.node_id(pattern));
        if selected.is_none() && file.nodes[pattern].kind != NodeKind::WildcardPattern {
            return Err(LoweringError::new(
                &site_at(tree, pattern),
                "pattern needs payload comparison or binding lowering",
            ));
        }
        for (offset, node) in file.nodes[pattern..file.nodes[pattern].end].iter().enumerate() {
            let child = pattern + offset;
            if node.kind == NodeKind::FieldPattern && tree.resolved().pattern_binding(child).is_some() {
                return Err(LoweringError::new(&site_at(tree, child), "case payload binding requires its arm-local value"));
            }
            if tree
                .semantics_at(child)
                .any(|r| !matches!(r, NodeSemantics::Field(_)))
            {
                return Err(LoweringError::new(&site_at(tree, child), "unsupported typed payload pattern record"));
            }
        }
        // The typer verified exhaustiveness. No later test is evaluated after a wildcard.
        if selected.is_none() && arm != *arms.last().expect("an arm exists") {
            return Err(LoweringError::new(
                &site_at(tree, pattern),
                "case arms after a wildcard require unreachable-arm lowering",
            ));
        }
        targets.push((flow.reserve(), expression));
        tests.push((pattern, selected));
    }
    if tests.last().is_some_and(|(_, case)| case.is_some()) {
        let roster = &expressions.data[owner.get() - 1].cases;
        let selected = tests
            .iter()
            .map(|(pattern, case)| {
                case.map(|case| crate::identity::declaration(case, &site_at(tree, *pattern)))
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;
        if selected.len() != roster.len()
            || roster
                .iter()
                .any(|c| !selected.iter().any(|s| s.as_ref() == Some(&c.declaration)))
        {
            return Err(LoweringError::new(
                &site_at(tree, dispatch),
                "final case selection requires the complete discriminator roster",
            ));
        }
    }
    let Some((&(fallback, _), preceding)) = targets.split_last() else {
        return Err(LoweringError::new(&site_at(tree, dispatch), "case dispatch has no arms"));
    };
    let selection = |index: usize| -> Result<Option<crate::CaseSelection>, LoweringError> {
        let (pattern, case) = tests[index];
        case.map(|case| {
            Ok(crate::CaseSelection {
                source: source.clone(),
                case: crate::identity::declaration(case, &site_at(tree, pattern))?,
            })
        })
        .transpose()
    };
    if preceding.is_empty() {
        let mut edge = super::branches::successor(body, fallback, vec![], site_at(tree, tests[0].0));
        edge.selected_case = selection(0)?;
        flow.close(body, BlockTerminator::Jump(edge));
    } else {
        let bool_ty = expressions
            .types
            .id_for(&ValueType::Bool)
            .map_err(|e| LoweringError::new(&site_at(tree, dispatch), e.to_string()))?;
        for (i, &(target, _)) in preceding.iter().enumerate() {
            let (pattern, case) = tests[i];
            let site = site_at(tree, pattern);
            let case = crate::identity::declaration(case.expect("only the last pattern can be a wildcard"), &site)?;
            let OperationResult::Scalar(condition) = body.push_operation(
                OperationOutput::Scalar(bool_ty),
                OperationKind::StructuralCaseMembership { source: source.clone(), case },
                site.clone(),
            ) else {
                unreachable!()
            };
            let last = i + 1 == preceding.len();
            let next = if last { fallback } else { flow.reserve() };
            let mut when_true = super::branches::successor(body, target, vec![], site);
            when_true.selected_case = selection(i)?;
            let mut when_false = super::branches::successor(body, next, vec![], site_at(tree, tests[i + 1].0));
            if last {
                when_false.selected_case = selection(i + 1)?;
            }
            flow.close(
                body,
                BlockTerminator::Conditional {
                    condition,
                    when_true,
                    when_false,
                },
            );
            if !last {
                flow.open(body, next, None);
            }
        }
    }
    Ok(Some(targets))
}

/// Whole-root discard suffices for a plain affine record of scalar leaves.
/// References, sums and nested affine children require their own
/// cleanup evidence. A `requires` clause does not change the cleanup, so it does
/// not stop provisioning: the schema keeps the clause and the checker judges the
/// binding's `ZeroValue` obligation against the all-zero value.
pub(super) fn zero_discardable_record(
    types: &crate::LoweringTypes,
    data: &[crate::LoweredData],
    root: crate::DataId,
) -> bool {
    let mut pending = vec![(data[root.get() - 1].carrier, false)];
    let mut done = std::collections::BTreeSet::new();
    let mut active = std::collections::BTreeSet::new();
    while let Some((ty, leave)) = pending.pop() {
        if leave {
            active.remove(&ty);
            done.insert(ty);
            continue;
        }
        if done.contains(&ty) {
            continue;
        }
        if !active.insert(ty) {
            return false;
        }
        pending.push((ty, true));
        match types.get(ty) {
            Some(crate::TypeShape::PrimitiveScalar(_)) => {}
            Some(crate::TypeShape::FixedArray { element, .. }) => pending.push((*element, false)),
            Some(crate::TypeShape::Nominal(id)) => {
                let schema = &data[id.get() - 1];
                if !schema.cases.is_empty()
                    || !(schema.multiplicity == crate::Multiplicity::Copy || *id == root && schema.multiplicity == crate::Multiplicity::Affine)
                {
                    return false;
                }
                pending.extend(schema.fields.iter().map(|f| (f.carrier, false)));
            }
            _ => return false,
        }
    }
    true
}

pub(super) fn provision_local(
    expressions: &mut Expressions<'_>,
    node: usize,
    binder: usize,
    data: &[crate::LoweredData],
    body: &mut MachineBody,
) -> Result<(), LoweringError> {
    use super::expressions::{EmitOperation, OperationOutput};
    let tree = expressions.tree;
    let resolved = tree.resolved();
    let file = resolved.file;
    let site = site_at(tree, node);
    let mut rows = tree.semantics_at(node);
    let Some(NodeSemantics::Obligation(o)) = rows.next() else {
        return Err(LoweringError::new(&site, "local lacks its zero-value requirement"));
    };
    let ObligationKind::ZeroValue(carrier) = &o.kind else {
        return Err(LoweringError::new(&site, "unexpected local establishment requirement"));
    };
    if rows.next().is_some() || resolved.pattern_binding(binder).is_none() || file.nodes[binder].kind != NodeKind::BindingPattern {
        return Err(LoweringError::new(&site, "unsupported zero-initialized pattern or obligation"));
    }
    let ty = expressions
        .types
        .id_for(carrier)
        .map_err(|e| LoweringError::new(&site, e.to_string()))?;
    let Some(crate::TypeShape::Nominal(id)) = expressions.types.get(ty) else {
        return Err(LoweringError::new(&site, "zero-initialized local requires a supported nominal record"));
    };
    if !zero_discardable_record(expressions.types, data, *id) {
        return Err(LoweringError::new(&site, "zero initialization requires cleanup evidence for this record"));
    }
    let value = body.push_operation(OperationOutput::Structural(ty), crate::OperationKind::EstablishZeroedStructural, site);
    let OperationResult::Structural(home) = value else { unreachable!() };
    if expressions
        .tree
        .forest()
        .mutable_bindings
        .contains(&file.node_id(binder))
    {
        body.mutable_homes.push(home);
    }

    expressions.bindings.push((binder, value));
    Ok(())
}

pub(super) fn store_place(
    expressions: &Expressions<'_>,
    node: usize,
    rhs: usize,
    data: &[crate::LoweredData],
    body: &mut MachineBody,
    flow: &mut super::branches::BlockFlow,
) -> Result<(), LoweringError> {
    use super::expressions::{EmitOperation, OperationOutput};
    let site = site_at(expressions.tree, node);
    // L3 already refused a store the source may not make. Which operation
    // represents an admitted store depends on the access of the deepest loan
    // it goes through, not on the permissions of the loans above it.
    let (mut place, carrier) = original_place(expressions, node, body)?;
    let mut projected = body.places[place.root.index()].ty;
    let mut deepest_loan = None;
    for part in &place.path {
        projected = match (part, expressions.types.get(projected)) {
            (PlaceProjection::Referent, Some(crate::TypeShape::ScalarReference { referent, access })) => {
                deepest_loan = Some(*access);
                expressions
                    .types
                    .id_for(carrier)
                    .ok()
                    .filter(|id| expressions.types.get(*id) == Some(&crate::TypeShape::PrimitiveScalar(*referent)))
                    .ok_or_else(|| LoweringError::new(&site, "scalar store disagrees with its reference type"))?
            }
            (PlaceProjection::Referent, Some(crate::TypeShape::NominalReference { referent, access })) => {
                deepest_loan = Some(*access);
                data[referent.get() - 1].carrier
            }
            (PlaceProjection::Field(field), Some(crate::TypeShape::Nominal(id))) => {
                let owner = &data[id.get() - 1];
                if !owner.requires.is_empty() {
                    return Err(LoweringError::new(
                        &site,
                        "field replacement requires evidence for the enclosing invariant",
                    ));
                }
                owner
                    .fields
                    .iter()
                    .find(|f| &f.declaration == field)
                    .ok_or_else(|| LoweringError::new(&site, "field replacement requires a concrete record field"))?
                    .carrier
            }
            _ => return Err(LoweringError::new(&site, "field replacement requires an original record place")),
        };
    }
    if expressions.types.id_for(carrier).ok() != Some(projected) {
        return Err(LoweringError::new(&site, "field replacement disagrees with its selected carrier"));
    }
    if carrier.nominal_parts().is_some() {
        return match deepest_loan {
            None => store_intact(expressions, place, rhs, carrier, body, flow, site, None),
            // Any loan crossing makes this a store into borrowed storage, even when the
            // path starts at an owned field that holds the `@mutable`: the replaced place
            // may be a vacancy, and the displaced value is the lender's, not the owner's.
            Some(crate::ReferenceAccess::Mutable) => super::loans::store_borrowed(expressions, place, rhs, carrier, body, flow, site, false),
            Some(crate::ReferenceAccess::WriteOnly) => {
                use crate::{Multiplicity, StructuralStoreDisposition, TypeShape, WriteOnlyCleanup};
                let Some(TypeShape::Nominal(id)) = expressions.types.get(projected) else {
                    return Err(LoweringError::new(&site, "write-only structural replacement needs its cleanup authority"));
                };
                let schema = &data[id.get() - 1];
                let authority = schema.write_only_cleanup.ok_or_else(|| LoweringError::new(&site, "write-only structural replacement needs its cleanup authority"))?;
                let disposition = if schema.multiplicity == Multiplicity::Copy && authority == WriteOnlyCleanup::NoCodeAffine {
                    StructuralStoreDisposition::ReplaceUnrestricted
                } else {
                    StructuralStoreDisposition::ReplaceWriteOnly(authority)
                };
                store_intact(expressions, place, rhs, carrier, body, flow, site, Some(disposition))
            }
            Some(crate::ReferenceAccess::Shared) => Err(LoweringError::new(&site, "structural store requires a writable loan")),
        };
    }
    // A shared descriptor in a field of writable owned storage is intact,
    // freely discardable content. Replacing it ends the displaced descriptor's
    // loan and installs the incoming descriptor's exact loan
    // (wiki/spec/language/lifetimes.md#carried-loan-transport); it does not
    // read or dispose either referent. Through a stored reference the same
    // store would need the loan's restoration window, so it still refuses.
    if matches!(
        expressions.types.get(projected),
        Some(
            crate::TypeShape::ScalarReference {
                access: crate::ReferenceAccess::Shared,
                ..
            } | crate::TypeShape::NominalReference {
                access: crate::ReferenceAccess::Shared,
                ..
            }
        )
    ) && !place.path.is_empty()
        && !place.path.contains(&PlaceProjection::Referent)
    {
        let OperationResult::Structural(value) = expressions.lower(rhs, carrier, body, flow)? else {
            return Err(LoweringError::new(&site, "descriptor store needs a descriptor value"));
        };
        if body.places[value.index()].ty != projected {
            return Err(LoweringError::new(&site, "stored descriptor disagrees with its field"));
        }
        body.push_operation(
            OperationOutput::Unit,
            crate::OperationKind::StructuralStore {
                destination: place,
                value,
                disposition: crate::StructuralStoreDisposition::ReplaceUnrestricted,
            },
            site,
        );
        return Ok(());
    }
    if !matches!(expressions.types.get(projected), Some(crate::TypeShape::PrimitiveScalar(_))) {
        return Err(LoweringError::new(
            &site,
            "field replacement requires qualification, ownership, or structural store evidence",
        ));
    }
    let OperationResult::Scalar(value) = expressions.lower(rhs, carrier, body, flow)? else {
        return Err(LoweringError::new(&site, "scalar store needs a scalar value"));
    };
    let operation = if place.path.last() == Some(&PlaceProjection::Referent) {
        crate::OperationKind::ProjectedScalarStore { destination: place, value }
    } else {
        let Some(PlaceProjection::Field(field)) = place.path.pop() else {
            return Err(LoweringError::new(&site, "scalar store destination has no selected place"));
        };
        crate::OperationKind::StructuralScalarFieldStore { destination: place, field, value }
    };
    body.push_operation(OperationOutput::Unit, operation, site);
    Ok(())
}

pub(super) fn store_intact(
    expressions: &Expressions<'_>,
    destination: ProjectedPlace,
    rhs: usize,
    carrier: &ValueType,
    body: &mut MachineBody,
    flow: &mut super::branches::BlockFlow,
    site: crate::Site,
    borrowed_disposition: Option<crate::StructuralStoreDisposition>,
) -> Result<(), LoweringError> {
    use super::expressions::{EmitOperation, OperationOutput};
    let ty = expressions
        .types
        .id_for(carrier)
        .map_err(|e| LoweringError::new(&site, e.to_string()))?;
    let OperationResult::Structural(value) = expressions.lower(rhs, carrier, body, flow)? else {
        return Err(LoweringError::new(&site, "structural store needs a complete value"));
    };
    if body.places[value.index()].ty != ty {
        return Err(LoweringError::new(&site, "stored value disagrees with its destination type"));
    }
    let disposition = if let Some(disposition) = borrowed_disposition {
        disposition
    } else if super::ownership::owned(ty, expressions.types, expressions.data) {
        crate::StructuralStoreDisposition::Dispose(vec![])
    } else {
        crate::StructuralStoreDisposition::ReplaceUnrestricted
    };
    body.push_operation(
        OperationOutput::Unit,
        crate::OperationKind::StructuralStore {
            destination,
            value,
            disposition,
        },
        site,
    );
    Ok(())
}
