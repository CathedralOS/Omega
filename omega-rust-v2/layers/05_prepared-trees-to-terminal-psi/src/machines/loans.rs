use super::{
    branches::BlockFlow,
    expressions::{EmitOperation, Expressions, OperationOutput},
    references,
};
use crate::{LoweringError, MachineBody, OperationKind, OperationResult, ProjectedPlace, Site, TypeShape, site_at};
use omega_resolved::Symbol;
use omega_syntax::NodeKind;
use omega_typed::{NodeSemantics, ObligationKind, ValueType};

/// The lifetime binding typing recorded on one place-reference value.
#[derive(Clone, Copy)]
pub(super) struct Binding<'t> {
    /// The type the value has by itself.
    pub reference: &'t ValueType,
    /// The type the binding row names as its destination.
    pub destination: &'t ValueType,
}

/// The binding typing recorded at `node` for a value that lands at `landing`.
///
/// Typing records `BindLifetime` where a place reference lands at a type that
/// differs from its own only in the lifetime it is held at. Both types are
/// one type row (`lowering::types::held_reference`), so the value lowers as
/// itself, and the operation that consumes it is where the binding happens.
/// A plain reference stored into a field can acquire the field's lifetime;
/// a stored reference returned through an elided result keeps the relation
/// fixed by that signature. Typing supplies the final destination for both.
pub(super) fn binding_at<'t>(
    tree: &'t crate::lowering::applied_tree::LoweringTree<'_>,
    node: usize,
    landing: &ValueType,
) -> Option<Binding<'t>> {
    use crate::lowering::types::{held_reference, place_reference};
    tree.semantics_at(node).find_map(|row| match row {
        NodeSemantics::Obligation(o) => match &o.kind {
            ObligationKind::BindLifetime { reference, destination }
                if place_reference(reference).is_some()
                    && held_reference(reference) == held_reference(destination)
                    && destination == landing =>
            {
                Some(Binding { reference, destination })
            }
            _ => None,
        },
        _ => None,
    })
}

/// Whether `kind` is the binding of a value of type `own` whose row names
/// `destination`, which the caller found with `binding_at` and consumes.
pub(super) fn is_binding(
    kind: &ObligationKind,
    own: &ValueType,
    destination: Option<&ValueType>,
) -> bool {
    matches!(kind, ObligationKind::BindLifetime { reference, destination: named } if reference == own && Some(named) == destination)
}

/// A descriptor forwarded from an input keeps the original parameter place.
/// An explicit borrow instead produces a descriptor whose defining operation
/// records the evaluated source path and any parent reference.
///
/// `carrier` may hold the reference at a lifetime, as `@'r u8` does. The
/// descriptor's type row is the reference's either way. Where typing bound
/// the value to that lifetime (`binding_at`) and the lifetime is a
/// region binder of this machine, the descriptor is listed in
/// `MachineBody::lifetime_bindings` under that binder, and the checker judges
/// whether its backing lives that long. A lifetime typing inferred inside the
/// body has no binder and gets no row: the operation that takes the
/// descriptor, such as the establishment of the record that stores it, is
/// the binding. A value that already has `carrier`'s type was placed in its
/// region by what produced it, a formal's slot or the field it was read
/// from, and gets no row either.
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
    let machine = &expressions.signature.machine;
    let binding = binding_at(tree, node, carrier);
    let own = binding.map_or(carrier, |bound| bound.reference);
    let landing = binding.map(|bound| bound.destination);
    let id = expressions
        .types
        .id_for(carrier)
        .map_err(|e| LoweringError::new(&site, e.to_string()))?;
    let value = match file.nodes[node].kind {
        NodeKind::PathExpression => {
            check_descriptor(tree, node, own, machine, landing)?;
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
            OperationResult::Structural(source)
        }
        NodeKind::Borrow => {
            let Some(crate::CallArgument::Borrow { place: source, access }) = references::reference_operand(expressions, node, carrier, body)? else {
                return Err(LoweringError::new(&site, "borrow has no original place"));
            };
            body.push_operation(OperationOutput::Structural(id), OperationKind::BorrowPlace { source, access }, site)
        }
        // A by-value use of an exclusive reference, `@mutable` or `@write`,
        // transfers its loan carrier (language/ownership.md#borrows-and-aliases).
        // From owned storage that is an ordinary field move; through a `@mutable`
        // parent it opens the restoration window L3 recorded. An explicit
        // reborrow is a `Borrow` node, and a call operand reborrows in
        // `references::call_argument`. The moved descriptor names the field
        // it left, and that field's row says which lifetime binder of its
        // record it is stored at.
        NodeKind::FieldAccess | NodeKind::Move if crate::lowering::types::is_exclusive_reference(own) => {
            super::ownership::move_value(expressions, node, own, landing, body)?
        }
        // Reading a stored shared reference copies no record storage. The value
        // reborrows the referent through the stored descriptor, so that stored
        // loan is the parent and the result points into the original backing,
        // not into the record that holds it. A call operand lends the same place.
        NodeKind::FieldAccess => {
            if !matches!(crate::lowering::types::place_reference(own), Some((crate::ReferenceAccess::Shared, _))) {
                return Err(LoweringError::new(&site, "a stored reference read needs its shared field place"));
            }
            let Some(crate::CallArgument::Borrow { place: source, access: crate::ReferenceAccess::Shared }) =
                references::reference_operand(expressions, node, carrier, body)?
            else {
                return Err(LoweringError::new(&site, "a stored reference read needs its shared field place"));
            };
            body.push_operation(OperationOutput::Structural(id), OperationKind::BorrowPlace { source, access: crate::ReferenceAccess::Shared }, site)
        }
        _ => return Ok(None),
    };
    // Only a value that lands at another lifetime is bound here. One that
    // already has `carrier`'s type keeps the region its own type states.
    if binding.is_some_and(|bound| bound.destination == carrier)
        && let Some((ordinal, _)) = crate::declarations::regions::direct(carrier, machine)
        && let OperationResult::Structural(descriptor) = value
    {
        body.lifetime_bindings.push((descriptor, ordinal));
    }
    Ok(Some(value))
}

/// A shared view held at a region, as `@'t [u8]`, in the two forms whose
/// backing an existing place already names. Any other expression returns
/// `None` and keeps the refusal it had.
///
/// `schema.storage`, at a region binder of this machine, copies the
/// descriptor the record stores. The copy names the field place, and the
/// record's own region slot says which region that field is in, so the copy
/// carries no region of its own.
///
/// `bytes` in `Schema { storage: bytes }` is a plain `@[u8]` that typing binds
/// to the region it inferred for the record (`BindLifetime` at the operand).
/// The operand forwards the binding's own place. Psi names no region for it:
/// the `EstablishRecord` that takes this operand for a field stored at a
/// lifetime binder is the binding, and the place is the backing. The same
/// operand bound to a region the machine declares is refused.
pub(super) fn view_at_region(
    expressions: &Expressions<'_>,
    node: usize,
    carrier: &ValueType,
    body: &mut MachineBody,
) -> Result<Option<OperationResult>, LoweringError> {
    let ValueType::AtLifetime(view, region) = carrier else { return Ok(None) };
    if !(crate::lowering::types::is_shared_byte_view(view) || crate::lowering::types::is_shared_element_view(view)) {
        return Ok(None);
    }
    let tree = expressions.tree;
    let resolved = tree.resolved();
    let file = resolved.file;
    let site = site_at(tree, node);
    let ty = |site: &Site| {
        expressions
            .types
            .id_for(view)
            .map_err(|e| LoweringError::new(site, e.to_string()))
    };
    match (file.nodes[node].kind, region.as_ref()) {
        (NodeKind::FieldAccess, _)
            if crate::declarations::regions::stored_view(carrier, &expressions.signature.machine).is_some()
                && tree
                    .semantics_at(node)
                    .any(|row| matches!(row, NodeSemantics::Field(_))) =>
        {
            let (source, selected) = references::move_place(expressions, node, body, None)?;
            if selected != carrier {
                return Err(LoweringError::new(&site, "stored view read disagrees with its checked type"));
            }
            // A read through a reference copies out of borrowed storage and
            // owes typing's copy-read evidence, which this form does not carry.
            if source.path.contains(&crate::PlaceProjection::Referent) {
                return Err(LoweringError::new(&site, "a stored view read through a reference needs its copy-read evidence"));
            }
            let id = ty(&site)?;
            Ok(Some(body.push_operation(
                OperationOutput::Structural(id),
                OperationKind::CopyStructural { source },
                site,
            )))
        }
        (NodeKind::PathExpression, _) => {
            let mut typed = false;
            let mut bound = false;
            for row in tree.semantics_at(node) {
                match row {
                    NodeSemantics::Expression(e) if !typed && &e.ty == view.as_ref() && e.static_argument_slot.is_none() => typed = true,
                    NodeSemantics::Obligation(o)
                        if !bound
                            && matches!(&o.kind, ObligationKind::BindLifetime { reference, destination }
                                if reference == view.as_ref() && destination == carrier) =>
                    {
                        bound = true
                    }
                    _ => return Ok(None),
                }
            }
            if !typed || !bound {
                return Ok(None);
            }
            // A binding to a region the machine declares is a relation the
            // checker must judge against that binder. An establishment alone
            // does not publish it, so only an inferred region lowers here.
            if !matches!(region.as_ref(), ValueType::InferredLifetime { .. }) {
                return Err(LoweringError::new(
                    &site,
                    "a reference bound into a stored field at a declared region needs its binding",
                ));
            }
            let Some(Symbol::Binding { binder_node, .. }) = resolved.symbol(node) else {
                return Ok(None);
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
            if body.places[source.index()].ty != ty(&site)? {
                return Err(LoweringError::new(&site, "bound reference differs from its declared type"));
            }
            Ok(value)
        }
        _ => Ok(None),
    }
}

/// `carrier` is the type the value at `node` has by itself. `landing` is the
/// destination its binding row names, when the caller consumes that binding
/// (`binding_at`); any other lifetime binding at `node` is refused.
pub(super) fn check_descriptor(
    tree: &crate::lowering::applied_tree::LoweringTree<'_>,
    node: usize,
    carrier: &ValueType,
    machine: &omega_resolved::DeclarationId,
    landing: Option<&ValueType>,
) -> Result<(), LoweringError> {
    let mut typed = false;
    let mut bound = false;
    for row in tree.semantics_at(node) {
        match row {
            NodeSemantics::Expression(e) if !typed && &e.ty == carrier && e.static_argument_slot.is_none() => typed = true,
            NodeSemantics::Obligation(o) if !bound && is_binding(&o.kind, carrier, landing) => bound = true,
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
    let store = Replacement::capture(expressions, destination, rhs, carrier, body, flow, site)?;
    let Vacancy::Known(vacant) = store.vacancy(&expressions.selector_premises, body, flow, before_return)? else {
        return Err(store.ambiguous());
    };
    store.emit(expressions, body, flow, vacant)
}

/// Selector and replacement are evaluated before choosing the occupied or vacant schedule.
pub(super) struct Replacement {
    pub destination: ProjectedPlace,
    pub site: Site,
    value: crate::PlaceId,
    id: crate::TypeId,
}

impl Replacement {
    pub fn capture(
        expressions: &Expressions<'_>, destination: ProjectedPlace, rhs: usize, carrier: &ValueType,
        body: &mut MachineBody, flow: &mut BlockFlow, site: Site,
    ) -> Result<Self, LoweringError> {
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
        Ok(Self { destination, site, value, id })
    }

    pub fn ambiguous(&self) -> LoweringError {
        LoweringError::new(&self.site, "cannot prove the store selector equal to or distinct from the selector of an open vacancy")
    }

    pub fn vacancy(
        &self, premises: &[super::view_loans::SelectorPremise], body: &MachineBody, flow: &BlockFlow, before_return: bool,
    ) -> Result<Vacancy, LoweringError> {
        vacancy_at(body, premises, flow, &self.destination, &self.site, before_return)
    }

    pub fn emit(&self, expressions: &Expressions<'_>, body: &mut MachineBody, flow: &mut BlockFlow, vacant: bool) -> Result<(), LoweringError> {
        let Self { destination, value, id, site } = self;
        let disposition = if vacant {
            crate::StructuralStoreDisposition::FillVacancy
        } else if matches!(expressions.types.get(*id),Some(TypeShape::Nominal(data)) if expressions.data[data.get()-1].multiplicity == crate::Multiplicity::Copy) {
            crate::StructuralStoreDisposition::ReplaceUnrestricted
        } else {
            // The old value has its own custody. Dispose it on the continuation
            // after evaluating the replacement and before filling the vacancy.
            let OperationResult::Structural(old) = body.push_operation(
                OperationOutput::Structural(*id),
                OperationKind::BorrowedMove { source: destination.clone() },
                site.clone(),
            ) else {
                unreachable!("an extracted field produces an owned place")
            };
            super::ownership::dispose_temporary(body, flow, old, site.clone());
            crate::StructuralStoreDisposition::FillVacancy
        };
        body.push_operation(
            OperationOutput::Unit,
            OperationKind::StructuralStore {
                destination: destination.clone(),
                value: *value,
                disposition,
            },
            site.clone(),
        );
        Ok(())
    }
}

pub(super) enum Vacancy {
    Known(bool),
    DependsOn(ProjectedPlace),
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
) -> Result<Vacancy, LoweringError> {
    use super::view_loans::{Relation, relate};
    let target = captured_place(body, destination);
    let known_vacancy = exact_vacancy(body, premises, flow, &target, site)?;
    let mut unresolved = None;
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
                Relation::Unproved if unresolved.is_none() => unresolved = Some(source.clone()),
                _ => return Err(LoweringError::new(site, "cannot prove the store selector equal to or distinct from the selector of an open vacancy")),
            }
        }
        checked.push(source);
    }
    Ok(match unresolved {
        Some(_) if before_return => Vacancy::Known(true),
        Some(open) => Vacancy::DependsOn(open),
        None => Vacancy::Known(known_vacancy),
    })
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
