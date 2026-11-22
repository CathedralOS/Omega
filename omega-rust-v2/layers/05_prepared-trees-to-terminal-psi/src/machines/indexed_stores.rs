//! Capture indexed destinations before evaluating their replacements.
use super::branches::BlockFlow;
use super::expressions::Expressions;
use crate::{
    BoundsObligationId, LoweringError, MachineBody, OperationKind, OperationResult, PlaceProjection, ReferenceAccess, RuntimeIndex, TypeShape, site_at,
};
use omega_program_contracts::CollectionPrimitive;
use omega_resolved::IndexedSource;
use omega_syntax::NodeKind;
use omega_typed::{Access, NodeSemantics, ObligationKind, Projection, ValueType};

pub(super) fn has_index(
    file: &IndexedSource<'_>,
    mut node: usize,
) -> bool {
    loop {
        node = file.ungroup(node);
        match (file.nodes[node].kind, file.nodes[node].children.as_slice()) {
            (NodeKind::Index, [_, _]) => return true,
            (NodeKind::FieldAccess, [base]) => node = *base,
            _ => return false,
        }
    }
}

pub(super) fn store(
    expressions: &Expressions<'_>,
    destination: usize,
    rhs: usize,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
) -> Result<(), LoweringError> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let refuse = |node, message| LoweringError::new(&site_at(tree, node), message);
    // Collect syntax in reverse, then retain any ordinary field chain below the
    // first index as an original place. No intermediate array is loaded or copied.
    let mut chain = Vec::new();
    let mut base = destination;
    loop {
        let node = file.ungroup(base);
        match (file.nodes[node].kind, file.nodes[node].children.as_slice()) {
            (NodeKind::Index, [receiver, selector]) => {
                chain.push((node, *receiver, Some(*selector)));
                base = *receiver;
            }
            (NodeKind::FieldAccess, [receiver]) => {
                chain.push((node, *receiver, None));
                base = *receiver;
            }
            _ => break,
        }
    }
    let prefix = chain
        .iter()
        .rposition(|entry| entry.2.is_some())
        .expect("indexed destination");
    let root_node = chain[prefix].1;
    let (mut place, mut carrier) = super::references::original_place(expressions, root_node, body, flow)?;
    let root = &body.places[place.root.index()];
    let root_reference = matches!(
        expressions.types.get(root.ty),
        Some(TypeShape::NominalReference {
            access: ReferenceAccess::Mutable,
            ..
        })
    );
    let mut referents = place
        .path
        .iter()
        .enumerate()
        .filter_map(|(position, part)| (*part == PlaceProjection::Referent).then_some(position));
    if root_reference {
        if referents.next() != Some(0) || referents.next().is_some() {
            return Err(refuse(root_node, "indexed store cannot capture a replaceable stored reference"));
        }
    } else if referents.count() != 0 || !body.mutable_homes.contains(&place.root) {
        return Err(refuse(
            root_node,
            "indexed store requires a mutable owning home or a captured exclusive reference",
        ));
    }

    let final_carrier = carrier_at(expressions, destination)?;
    let final_ty = expressions
        .types
        .id_for(final_carrier)
        .map_err(|error| LoweringError::new(&site_at(tree, destination), error.to_string()))?;
    let fixed_claim = super::ownership::owned(final_ty, expressions.types, expressions.data);
    let index_type = expressions
        .types
        .id_for(&super::access::INDEX_TYPE)
        .map_err(|error| LoweringError::new(&site_at(tree, destination), error.to_string()))?;

    let mut unproved_capture = None;
    for &(node, receiver, selector) in chain[..=prefix].iter().rev() {
        let (_, steps) = tree.forest().place_path(file.node_id(node));
        let step = steps
            .last()
            .filter(|step| step.base == file.node_id(file.ungroup(receiver)) && step.through.is_none())
            .ok_or_else(|| refuse(node, "indexed store disagrees with its typed storage projection"))?;
        if let Some(selector) = selector {
            let ValueType::Array(element, extent) = carrier.carrier() else {
                return Err(refuse(receiver, "indexed store requires a fixed-array receiver"));
            };
            if step.projection != Some(Projection::Index(file.node_id(selector))) {
                return Err(refuse(node, "indexed store lost its typed selector"));
            }
            check_index(expressions, node, receiver, element)?;
            let ordinal = super::fixed_arrays::frozen_selector(expressions.prepared, tree, selector)
                .ok()
                .and_then(|value| u64::try_from(value).ok())
                .filter(|index| index < extent);
            if fixed_claim {
                let ordinal = ordinal.ok_or_else(|| refuse(selector, "owned structural replacement needs a literal or frozen selector in bounds"))?;
                place.path.push(PlaceProjection::FixedIndex(ordinal));
            } else {
                let OperationResult::Scalar(value) = expressions.lower(selector, &super::access::INDEX_TYPE, body, flow)? else {
                    return Err(refuse(selector, "indexed store selector must produce a scalar"));
                };
                if body.values[value.index()].ty != index_type {
                    return Err(refuse(selector, "indexed store selector must have type u64"));
                }
                if ordinal.is_none() && unproved_capture.is_none() {
                    let position = body.operations.len();
                    unproved_capture = Some((selector, position, flow.active_range(position).0));
                }
                let obligation = BoundsObligationId(super::body::next_id(body.diagnostics.bounds.len()));
                body.diagnostics.bounds.push(site_at(tree, selector));
                place
                    .path
                    .push(PlaceProjection::RuntimeIndex(RuntimeIndex { selector: value, obligation }));
            }
            carrier = element;
        } else {
            let Some(Projection::Field(field)) = &step.projection else {
                return Err(refuse(node, "indexed store lacks its selected field"));
            };
            let field = crate::identity::declaration(field, &site_at(tree, node))?;
            let owner_type = expressions
                .types
                .id_for(carrier)
                .map_err(|error| LoweringError::new(&site_at(tree, node), error.to_string()))?;
            let Some(TypeShape::Nominal(owner)) = expressions.types.get(owner_type) else {
                return Err(refuse(node, "indexed field store requires a nominal record place"));
            };
            if let Some(case) = expressions.data[owner.get() - 1]
                .cases
                .iter()
                .find(|case| case.fields.iter().any(|member| member.declaration == field))
            {
                place.path.push(PlaceProjection::Case(case.declaration.clone()));
            }
            place.path.push(PlaceProjection::Field(field));
            carrier = carrier_at(expressions, node)?;
        }
    }
    if carrier != final_carrier {
        return Err(refuse(destination, "indexed store changed the destination carrier"));
    }
    if place
        .path
        .iter()
        .any(|part| matches!(part, PlaceProjection::RuntimeIndex(_)))
        && super::references::has_loan_route(expressions.types, expressions.data, final_ty, super::references::LoanRoute::Descriptor)
    {
        return Err(refuse(
            destination,
            "runtime indexed store requires exact carried descriptor disposal and origin evidence",
        ));
    }
    super::references::store_captured(expressions, place, destination, rhs, final_carrier, body, flow)?;
    if let Some((selector, capture_position, capture_block)) = unproved_capture {
        // The store owns the bounds obligation. Its canonical path has no
        // separate checker program point. Keep all intervening RHS work within
        // the same facts-and-control frontier unless the bound was static.
        // Later selectors are evaluated inside that interval too.
        let (store_block, _) = flow.active_range(body.operations.len());
        // The enclosing store is the last emitted operation. It is the
        // commitment, not intervening replacement work.
        let commitment = body.operations.len() - 1;
        if capture_block != store_block
            || !body.operations[capture_position..commitment]
                .iter()
                .all(|operation| capture_transparent(expressions, body, operation))
        {
            return Err(refuse(
                selector,
                "dynamic indexed store needs a pre-RHS bounds proof at the destination capture",
            ));
        }
    }
    Ok(())
}

/// A positive whitelist: total scalar evaluation or a simple scalar snapshot
/// cannot establish new premises about earlier values or change live storage.
fn capture_transparent(
    expressions: &Expressions<'_>,
    body: &MachineBody,
    operation: &crate::PsiOperation,
) -> bool {
    let OperationResult::Scalar(result) = operation.result else {
        return false;
    };
    if !matches!(expressions.types.get(body.values[result.index()].ty), Some(TypeShape::PrimitiveScalar(_))) {
        return false;
    }
    match &operation.kind {
        OperationKind::Constant(_) | OperationKind::Boolean { .. } | OperationKind::Float { .. } => true,
        OperationKind::Integer { operation, .. } => operation.obligations.is_empty(),
        OperationKind::ProjectedScalarRead { source } => source.path.iter().all(|part| {
            matches!(
                part,
                PlaceProjection::Referent | PlaceProjection::Case(_) | PlaceProjection::Field(_) | PlaceProjection::FixedIndex(_)
            )
        }),
        OperationKind::PrimitiveScalarRead { path, .. } => path.is_empty(),
        _ => false,
    }
}

fn carrier_at<'a>(
    expressions: &'a Expressions<'_>,
    node: usize,
) -> Result<&'a ValueType, LoweringError> {
    expressions
        .tree
        .semantics_at(node)
        .find_map(|row| match row {
            NodeSemantics::CollectionAccess(access) if access.primitive == CollectionPrimitive::ElementMut => Some(&access.element),
            NodeSemantics::Expression(expression) if expression.static_argument_slot.is_none() => Some(&expression.ty),
            _ => None,
        })
        .ok_or_else(|| LoweringError::new(&site_at(expressions.tree, node), "indexed store lacks its typed destination carrier"))
}

fn check_index(
    expressions: &Expressions<'_>,
    node: usize,
    receiver: usize,
    element: &ValueType,
) -> Result<(), LoweringError> {
    let tree = expressions.tree;
    let site = site_at(tree, node);
    let receiver_site = site_at(tree, receiver);
    let refuse = || LoweringError::new(&site, "indexed store lacks its exact core mutable element selection, loan or bound");

    // Match the admitted toolchain declaration, including its complete
    // resolved identity. A same-spelled helper or paired forged rows cannot
    // turn a value observation into a writable place.
    let forest = tree.forest();
    let primitive = CollectionPrimitive::ElementMut;
    let catalog = forest.library.unwrap_or(forest);
    let core = catalog
        .signatures
        .iter()
        .find(|signature| {
            signature.catalog_supplied
                && signature.machine.kind == NodeKind::Machine
                && signature.machine.package == "@omega/toolchain"
                && signature.machine.path == primitive.path()
        })
        .map(|signature| &signature.machine)
        .ok_or_else(refuse)?;

    let mut selected = false;
    let mut bounded = false;
    let mut borrowed = false;
    for row in tree.semantics_at(node) {
        match row {
            NodeSemantics::CollectionAccess(access)
                if !selected
                    && access.primitive == primitive
                    && &access.machine == core
                    && &access.element == element
                    && access.selector_carriers == [super::access::INDEX_TYPE]
                    && matches!(&access.result, ValueType::Reference(Access::Mut, inner) if inner.as_ref() == element) =>
            {
                selected = true;
            }
            NodeSemantics::Obligation(o) => match &o.kind {
                ObligationKind::CallPrecondition(machine) if !bounded && machine == core => bounded = true,
                ObligationKind::Borrow { access: Access::Mut, place } if !borrowed && place == &receiver_site => borrowed = true,
                _ => return Err(refuse()),
            },
            NodeSemantics::Expression(e)
                if e.static_argument_slot.is_none()
                    && (&e.ty == element || matches!(&e.ty, ValueType::Reference(Access::Mut, child) if child.as_ref() == element)) => {}
            _ => return Err(refuse()),
        }
    }
    if selected && bounded && borrowed { Ok(()) } else { Err(refuse()) }
}
