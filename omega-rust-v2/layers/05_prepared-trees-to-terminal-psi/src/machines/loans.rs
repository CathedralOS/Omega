use super::{
    branches::BlockFlow,
    expressions::{EmitOperation, Expressions, OperationOutput},
    references,
};
use crate::{LoweringError, MachineBody, OperationKind, OperationResult, ProjectedPlace, Site, TypeShape, site_at};
use omega_resolved::Symbol;
use omega_syntax::NodeKind;
use omega_typed::{NodeSemantics, ObligationKind, ValueType};

/// A descriptor forwarded from an input keeps the original parameter place.
/// An explicit borrow instead produces a descriptor whose defining operation
/// records the evaluated source path and any parent reference.
pub(super) fn descriptor(
    expressions: &Expressions<'_>,
    node: usize,
    carrier: &ValueType,
    body: &mut MachineBody,
) -> Result<Option<OperationResult>, LoweringError> {
    let tree = expressions.tree;
    let resolved = tree.resolved();
    let file = resolved.file;
    let site = site_at(tree, node);
    let region = crate::declarations::regions::direct(carrier, &expressions.signature.machine);
    let reference = region.map_or(carrier, |(_, reference)| reference);
    let id = expressions
        .types
        .id_for(reference)
        .map_err(|e| LoweringError::new(&site, e.to_string()))?;
    match file.nodes[node].kind {
        NodeKind::PathExpression => {
            check_descriptor(tree, node, carrier, &expressions.signature.machine)?;
            let Some(Symbol::Binding { binder_node, .. }) = resolved.symbol(node) else {
                return Err(LoweringError::new(&site, "reference value has no bound source"));
            };
            let value = expressions
                .bindings
                .iter()
                .rev()
                .find(|(n, _)| n == binder_node)
                .map(|(_, v)| *v);
            let Some(OperationResult::Structural(source)) = value else {
                return Err(LoweringError::new(&site, "reference binding has no lowered descriptor"));
            };
            if body.places[source.index()].ty != id {
                return Err(LoweringError::new(&site, "bound reference differs from its declared type"));
            }
            Ok(value)
        }
        NodeKind::Borrow => {
            let destination = region.map(|_| carrier);
            let Some(crate::CallArgument::Borrow { place: source, access }) = references::reference_operand(expressions, node, reference, body, destination)? else {
                return Err(LoweringError::new(&site, "borrow has no original place"));
            };
            let result = body.push_operation(OperationOutput::Structural(id), OperationKind::BorrowPlace { source, access }, site);
            if let (Some((ordinal, _)), OperationResult::Structural(descriptor)) = (region, result) {
                body.lifetime_bindings.push((descriptor, ordinal));
            }
            Ok(Some(result))
        }
        // A by-value use of an exclusive reference, `@mutable` or `@write`,
        // transfers its loan carrier (language/ownership.md#borrows-and-aliases).
        // From owned storage that is an ordinary field move; through a `@mutable`
        // parent it opens the restoration window L3 recorded. An explicit
        // reborrow is a `Borrow` node, and a call operand reborrows in
        // `references::call_argument`.
        NodeKind::FieldAccess | NodeKind::Move if region.is_none() && crate::lowering::types::is_exclusive_reference(reference) => {
            super::ownership::move_value(expressions, node, carrier, body).map(Some)
        }
        // Reading a stored shared reference copies no record storage. The value
        // reborrows the referent through the stored descriptor, so that stored
        // loan is the parent and the result points into the original backing,
        // not into the record that holds it. A call operand lends the same place.
        NodeKind::FieldAccess => {
            match crate::lowering::types::place_reference(reference) {
                Some((crate::ReferenceAccess::Shared, _)) => {}
                Some((crate::ReferenceAccess::Mutable | crate::ReferenceAccess::WriteOnly, _)) => {
                    return Err(LoweringError::new(
                        &site,
                        "a stored exclusive reference returned at a declared region needs its binding",
                    ));
                }
                None => return Err(LoweringError::new(&site, "a stored reference read needs its shared field place")),
            }
            if region.is_some() {
                return Err(LoweringError::new(&site, "a stored reference returned at a declared region needs its binding"));
            }
            let Some(crate::CallArgument::Borrow { place: source, access: crate::ReferenceAccess::Shared }) =
                references::reference_operand(expressions, node, reference, body, None)?
            else {
                return Err(LoweringError::new(&site, "a stored reference read needs its shared field place"));
            };
            Ok(Some(body.push_operation(OperationOutput::Structural(id), OperationKind::BorrowPlace { source, access: crate::ReferenceAccess::Shared }, site)))
        }
        _ => Ok(None),
    }
}

pub(super) fn check_descriptor(
    tree: &crate::lowering::applied_tree::LoweringTree<'_>,
    node: usize,
    carrier: &ValueType,
    machine: &omega_resolved::DeclarationId,
) -> Result<(), LoweringError> {
    let mut typed = false;
    for row in tree.semantics_at(node) {
        match row {
            NodeSemantics::Expression(e) if !typed && &e.ty == carrier && e.static_argument_slot.is_none() => typed = true,
            NodeSemantics::Obligation(o) if matches!(&o.kind,ObligationKind::ReturnBorrow{machine:owner} if owner==machine) => {}
            _ => return Err(LoweringError::new(&site_at(tree, node), "reference value needs additional lowering evidence")),
        }
    }
    if !typed {
        return Err(LoweringError::new(&site_at(tree, node), "reference descriptor has no checked carrier"));
    }
    Ok(())
}

pub(super) fn move_borrowed(
    expressions: &Expressions<'_>,
    node: usize,
    source: ProjectedPlace,
    carrier: &ValueType,
    body: &mut MachineBody,
) -> Result<OperationResult, LoweringError> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let operand = match (file.nodes[node].kind, file.nodes[node].children.as_slice()) {
        (NodeKind::Move, [value]) => file.ungroup(*value),
        (NodeKind::Move, _) => return Err(LoweringError::new(&site_at(tree, node), "move needs one borrowed place")),
        _ => file.ungroup(node),
    };
    // L3 records `RestoreBorrowed` only for a take out of storage it reaches
    // with exclusive access, so this obligation is the move's authority; the
    // published path names every loan the restoration window suspends.
    let occurrence = file.node_id(operand);
    let restoration=tree.semantics_at(operand).filter(|row| matches!(row,NodeSemantics::Obligation(o) if matches!(&o.kind,ObligationKind::RestoreBorrowed{place,ty} if *place==occurrence && ty==carrier))).count();
    let site = site_at(tree, node);
    if restoration != 1 {
        return Err(LoweringError::new(&site, "move through a loan needs its exact restoration obligation"));
    }
    let ty = expressions
        .types
        .id_for(carrier)
        .map_err(|e| LoweringError::new(&site, e.to_string()))?;
    Ok(body.push_operation(OperationOutput::Structural(ty), OperationKind::BorrowedMove { source }, site))
}

pub(super) fn store_borrowed(
    expressions: &Expressions<'_>,
    destination: ProjectedPlace,
    rhs: usize,
    carrier: &ValueType,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
    site: Site,
    before_return: bool,
) -> Result<(), LoweringError> {
    let id = expressions
        .types
        .id_for(carrier)
        .map_err(|e| LoweringError::new(&site, e.to_string()))?;
    if !matches!(expressions.types.get(id), Some(TypeShape::Nominal(_))) {
        return Err(LoweringError::new(&site, "structural replacement needs an exact nominal type"));
    }
    let OperationResult::Structural(value) = expressions.lower(rhs, carrier, body, flow)? else {
        return Err(LoweringError::new(&site, "structural store needs an owned value"));
    };
    if body.places[value.index()].ty != id {
        return Err(LoweringError::new(&site, "stored value disagrees with its destination type"));
    }
    let premises = &expressions.selector_premises;
    let disposition = if vacancy_at(body, premises, flow, &destination, &site, before_return)? {
        crate::StructuralStoreDisposition::FillVacancy
    } else if matches!(expressions.types.get(id),Some(TypeShape::Nominal(data)) if expressions.data[data.get()-1].multiplicity == crate::Multiplicity::Copy) {
        crate::StructuralStoreDisposition::ReplaceUnrestricted
    } else {
        // The old value has its own custody. Dispose it on the continuation
        // after evaluating the replacement and before filling the vacancy.
        let OperationResult::Structural(old) = body.push_operation(
            OperationOutput::Structural(id),
            OperationKind::BorrowedMove { source: destination.clone() },
            site.clone(),
        ) else {
            unreachable!("an extracted field produces an owned place")
        };
        super::ownership::dispose_temporary(expressions, body, flow, old, site.clone())?;
        crate::StructuralStoreDisposition::FillVacancy
    };
    body.push_operation(
        OperationOutput::Unit,
        OperationKind::StructuralStore {
            destination,
            value,
            disposition,
        },
        site,
    );
    Ok(())
}

/// Choose the store operation that corresponds to the authored assignment.
/// The checker independently authenticates each loan and its restoration,
/// including any selector relation taken from an authored entry clause.
fn vacancy_at(
    body: &MachineBody,
    premises: &[super::view_loans::SelectorPremise],
    flow: &BlockFlow,
    destination: &ProjectedPlace,
    site: &Site,
    before_return: bool,
) -> Result<bool, LoweringError> {
    use super::view_loans::{Relation, relate};
    let target = captured_place(body, destination);
    let known_vacancy = exact_vacancy(body, premises, flow, &target, site)?;
    let mut owed_final_fill = false;
    let mut checked = Vec::new();
    for operation in &body.operations {
        let OperationKind::BorrowedMove { source } = &operation.kind else { continue };
        let source = captured_place(body, source);
        let relation = relate(body, premises, &source, &target);
        match relation {
            Relation::Same | Relation::Disjoint => continue,
            // Valid simultaneous extractions must already be disjoint. If this
            // store fills a known vacancy, another move cannot make it occupied.
            Relation::Unproved if known_vacancy => continue,
            Relation::Unproved | Relation::Overlap => {}
        }
        if checked
            .iter()
            .any(|prior| matches!(relate(body, premises, prior, &source), Relation::Same))
        {
            continue;
        }
        // An old extraction matters only until its matching store. Later
        // assignments through another selector may reuse the intact backing.
        if exact_vacancy(body, premises, flow, &source, site)? {
            match relation {
                Relation::Overlap => return Err(LoweringError::new(site, "nested restoration needs the overlapping structural window")),
                // An occupied displacement here leaves an open window at the
                // implicit return. Emit the only possible normal repair at
                // the actual store selector. L6 still owes its identity with
                // the captured move, its bound and its receiving validity.
                Relation::Unproved if before_return && !owed_final_fill => owed_final_fill = true,
                _ => return Err(LoweringError::new(site, "cannot prove the store selector equal to or distinct from the selector of an open vacancy")),
            }
        }
        checked.push(source);
    }
    Ok(known_vacancy || owed_final_fill)
}

fn exact_vacancy(
    body: &MachineBody,
    premises: &[super::view_loans::SelectorPremise],
    flow: &BlockFlow,
    target: &ProjectedPlace,
    site: &Site,
) -> Result<bool, LoweringError> {
    let (active, range) = flow.active_range(body.operations.len());
    let mut pending = vec![(active, range)];
    let mut visited = vec![
        false;
        body.blocks
            .iter()
            .map(|b| b.id.get())
            .max()
            .unwrap_or(1)
            .max(active.get())
    ];
    let mut predecessors = None;
    let mut answer = None;
    while let Some((block, range)) = pending.pop() {
        if std::mem::replace(&mut visited[block.get() - 1], true) {
            continue;
        }
        let mut state = None;
        for op in body.operations[range].iter().rev() {
            let (place, vacant) = match &op.kind {
                OperationKind::BorrowedMove { source } => (source, true),
                OperationKind::StructuralStore { destination, .. } => (destination, false),
                _ => continue,
            };
            let path = captured_place(body, place);
            // View elements match by their evaluated selector, never by the
            // occurrence-local bound obligation or length observation.
            if matches!(super::view_loans::relate(body, premises, &path, target), super::view_loans::Relation::Same) {
                state = Some(vacant);
                break;
            }
        }
        if state.is_none() && block.get() == 1 {
            state = Some(false);
        }
        if let Some(vacant) = state {
            if answer
                .replace(vacant)
                .is_some_and(|previous| previous != vacant)
            {
                return Err(LoweringError::new(site, "borrowed replacement has inconsistent vacancy at its incoming edges"));
            }
            continue;
        }
        let arrivals = predecessors.get_or_insert_with(|| {
            let mut by_target = vec![Vec::new(); visited.len()];
            for predecessor in &body.blocks {
                let mut record = |target: crate::BlockId| {
                    if let Some(sources) = by_target.get_mut(target.get() - 1) {
                        sources.push((predecessor.id, predecessor.operations.clone()));
                    }
                };
                match &predecessor.terminator {
                    crate::BlockTerminator::Conditional { when_true, when_false, .. } => {
                        record(when_true.target);
                        record(when_false.target);
                    }
                    crate::BlockTerminator::Jump(edge) => record(edge.target),
                    _ => {}
                }
            }
            by_target
        });
        pending.extend(arrivals[block.get() - 1].iter().cloned());
    }
    answer.ok_or_else(|| LoweringError::new(site, "borrowed replacement has no established incoming storage"))
}

pub(super) fn captured_place(
    body: &MachineBody,
    place: &ProjectedPlace,
) -> ProjectedPlace {
    let mut root = place.root;
    let mut path = place.path.clone();
    while path.first() == Some(&crate::PlaceProjection::Referent) {
        let crate::PlaceOrigin::Operation(id) = body.places[root.index()].origin else {
            break;
        };
        let OperationKind::BorrowPlace { source, .. } = &body.operations[id.get() - 1].kind else {
            break;
        };
        path.splice(..1, source.path.iter().cloned());
        root = source.root;
    }
    ProjectedPlace { root, path }
}
