use crate::lowering::applied_tree::LoweringTree;
use omega_program_contracts::CollectionPrimitive;
use omega_syntax::NodeKind;
use omega_typed::{Access, NodeSemantics, ObligationKind, Projection, TypedTrees, ValueType};

use crate::{LoweringError, site_at};

pub(crate) struct IndexedRead<'a> {
    pub root: usize,
    pub root_carrier: &'a ValueType,
    pub selectors: Vec<(usize, &'a ValueType)>,
}

#[derive(Clone, Copy)]
pub(crate) enum CollectionLength {
    Fixed(u64),
    Bytes,
}

pub(crate) const INDEX_TYPE: ValueType = ValueType::Integer {
    signed: false,
    width: Some(64),
};

pub(crate) fn collection_length(operation: &omega_typed::TypedOperation) -> Option<(&ValueType, CollectionLength)> {
    let [receiver] = operation.operands.as_slice() else { return None };
    if operation.token != ".length" || operation.result != INDEX_TYPE {
        return None;
    }
    let length = match receiver {
        ValueType::Array(_, n) => CollectionLength::Fixed(*n),
        ty if crate::lowering::types::byte_view_access(ty).is_some() => CollectionLength::Bytes,
        _ => return None,
    };
    Some((receiver, length))
}

/// A scalar or nominal-element descriptor stores its live element count.
pub(crate) fn element_view_length(operation: &omega_typed::TypedOperation) -> Option<&ValueType> {
    let [receiver] = operation.operands.as_slice() else { return None };
    let is_element_view = crate::lowering::types::element_view_access(receiver).is_some();
    (operation.token == ".length" && operation.result == INDEX_TYPE && is_element_view).then_some(receiver)
}

pub(crate) fn select_indexed_read<'a>(
    types: &crate::LoweringTypes,
    data: &[crate::LoweredData],
    trees: &'a TypedTrees<'_>,
    tree: &'a LoweringTree<'_>,
    node: usize,
    result: &'a ValueType,
) -> Result<IndexedRead<'a>, LoweringError> {
    let file = tree.resolved().file;
    let refuse = |node, message| LoweringError::new(&site_at(tree, node), message);
    let copy_record = crate::lowering::types::contains_nominal_value(result)
        && types
            .id_for(result)
            .is_ok_and(|id| !super::ownership::owned(id, types, data));
    if !types.can_forward_value(result) && !copy_record {
        return Err(refuse(node, "indexed read requires a copyable scalar or fixed-array element"));
    }
    let (_, path) = trees.place_path(file.node_id(node));
    let mut projections = path.into_iter().rev();
    let mut selectors = Vec::new();
    let mut selected = node;
    let mut element = result;
    loop {
        let [base, selector] = *file.nodes[selected].children.as_slice() else {
            return Err(refuse(selected, "indexed read has no exact receiver and selector"));
        };
        let carrier = tree
            .semantics_at(base)
            .find_map(|record| match record {
                NodeSemantics::Expression(expression) if expression.static_argument_slot.is_none() => Some(&expression.ty),
                _ => None,
            })
            .ok_or_else(|| refuse(base, "indexed read has no typed receiver"))?;
        let byte_view = crate::lowering::types::is_shared_byte_view(carrier);
        let element_view = crate::lowering::types::scalar_element_view(carrier);
        if byte_view {
            if !selectors.is_empty() || !matches!(element, ValueType::Integer { signed: false, width: Some(8) }) {
                return Err(refuse(selected, "shared byte view read requires a single byte selector"));
            }
        } else if element_view.is_some() {
            // The result carrier is exactly the declared element; byte views never supply it.
            let exact = matches!(carrier, ValueType::Reference(_, slice) if matches!(slice.as_ref(), ValueType::Slice(inner) if inner.as_ref() == element));
            if !selectors.is_empty() || !exact {
                return Err(refuse(selected, "element view read requires one selector of its exact scalar element"));
            }
        } else if !matches!(carrier, ValueType::Array(inner, _) if inner.as_ref() == element) {
            return Err(refuse(base, "indexed read requires an owned fixed-array receiver"));
        }
        // The path crosses the declared loan. The core Read itself needs only
        // the shared borrow checked by check_read, even for an exclusive root.
        let through = element_view.map(|(access, _)| access).or(byte_view.then_some(&Access::Shared));
        let expected_base = file.node_id(file.ungroup(base));
        let expected_selector = file.node_id(selector);
        if projections.next().is_none_or(|step| {
            step.base != expected_base || step.through.as_ref() != through || step.projection != Some(Projection::Index(expected_selector))
        }) {
            return Err(refuse(selected, "indexed read disagrees with its typed place projection"));
        }
        let selector_carrier = check_read(tree, selected, base, element)?;
        selectors.push((selector, selector_carrier));
        let root = file.ungroup(base);
        if through.is_some() || file.nodes[root].kind != NodeKind::Index {
            selectors.reverse();
            return Ok(IndexedRead {
                root: base,
                root_carrier: carrier,
                selectors,
            });
        }
        selected = root;
        element = carrier;
    }
}

fn check_read<'a>(
    tree: &'a LoweringTree<'_>,
    node: usize,
    base: usize,
    element: &ValueType,
) -> Result<&'a ValueType, LoweringError> {
    let base_site = site_at(tree, base);
    let mut selection = None;
    let mut precondition = None;
    let mut borrow = false;
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::CollectionAccess(access)
                if selection.is_none() && access.primitive == CollectionPrimitive::Read && &access.element == element && &access.result == element =>
            {
                let [selector] = access.selector_carriers.as_slice() else {
                    return Err(LoweringError::new(&access.site, "indexed read requires one typed selector"));
                };
                if selector != &INDEX_TYPE {
                    return Err(LoweringError::new(&access.site, "indexed read requires a u64 selector"));
                }
                selection = Some((&access.machine, selector));
            }
            NodeSemantics::Obligation(obligation) => match &obligation.kind {
                ObligationKind::CallPrecondition(machine) if precondition.is_none() => precondition = Some(machine),
                ObligationKind::Borrow { access: Access::Shared, place } if !borrow && place == &base_site => borrow = true,
                _ => return Err(LoweringError::new(&obligation.site, "indexed read has an unsupported obligation")),
            },
            NodeSemantics::Expression(expression) if expression.static_argument_slot.is_none() && &expression.ty == element => {}
            _ => return Err(LoweringError::new(&site_at(tree, node), "indexed read has unsupported typed records")),
        }
    }
    if let Some((machine, selector)) = selection
        && precondition == Some(machine)
        && borrow
    {
        Ok(selector)
    } else {
        Err(LoweringError::new(
            &site_at(tree, node),
            "indexed read has no complete core operation and access obligation",
        ))
    }
}
