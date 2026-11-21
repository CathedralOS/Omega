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
        // Reading a stored shared reference copies no record storage. The value
        // reborrows the referent through the stored descriptor, so that stored
        // loan is the parent and the result points into the original backing,
        // not into the record that holds it. A call operand lends the same place.
        NodeKind::FieldAccess => {
            // Reading a stored exclusive reference either moves it out of the
            // field or reborrows it while suspending that field; neither is
            // represented here yet.
            if !matches!(crate::lowering::types::place_reference(reference), Some((crate::ReferenceAccess::Shared, _))) {
                return Err(LoweringError::new(&site, "a stored reference read needs its shared field place"));
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
    let occurrence = file.node_id(operand);
    let restoration=tree.semantics_at(operand).filter(|row| matches!(row,NodeSemantics::Obligation(o) if matches!(&o.kind,ObligationKind::RestoreBorrowed{place,ty} if *place==occurrence && ty==carrier))).count();
    let site = site_at(tree, node);
    if restoration != 1 || !references::mutable_place(expressions, &source, body) {
        return Err(LoweringError::new(
            &site,
            "move through a loan needs its exact exclusive place and restoration obligation",
        ));
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
    let disposition = if vacancy_at(body, premises, flow, &destination, &site)? {
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
) -> Result<bool, LoweringError> {
    use super::view_loans::{Relation, relate};
    let target = captured_place(body, destination);
    let mut checked = Vec::new();
    for operation in &body.operations {
        let OperationKind::BorrowedMove { source } = &operation.kind else { continue };
        let source = captured_place(body, source);
        let reason = match relate(body, premises, &source, &target) {
            Relation::Unproved => "cannot prove the store selector equal to or distinct from the selector of an open vacancy",
            Relation::Overlap => "nested restoration needs the overlapping structural window",
            Relation::Same | Relation::Disjoint => continue,
        };
        if checked
            .iter()
            .any(|prior| matches!(relate(body, premises, prior, &source), Relation::Same))
        {
            continue;
        }
        // An old extraction matters only until its matching store. Later
        // assignments through another selector may reuse the intact backing.
        if exact_vacancy(body, premises, flow, &source, site)? {
            return Err(LoweringError::new(site, reason));
        }
        checked.push(source);
    }
    exact_vacancy(body, premises, flow, &target, site)
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
