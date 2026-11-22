//! Structural custody of owned fixed-array elements.
//!
//! An element transfer names its array by a static literal ordinal. Its claim
//! path keeps the root and every ordinal; cleanup derives the remaining
//! maximal subtrees from the declared extent in decreasing index order. There
//! is no runtime liveness record: a selector that cannot be frozen here, an
//! ordinal outside the declared extent, or an extent too large to enumerate
//! statically rejects instead of producing uncertain partial occupancy.
use super::expressions::Expressions;
use crate::lowering::applied_tree::LoweringTree;
use crate::{LoweringError, PlaceProjection, site_at};
use num_bigint::BigInt;
use omega_prepared::{PreparedTrees, PreparedValue};
use omega_resolved::Symbol;
use omega_syntax::NodeKind;
use omega_typed::{NodeSemantics, Projection, ValueType};

/// The `FixedIndex` segment of one moved element `base[selector]`. The typed
/// place step must select exactly this receiver without a loan, and the
/// receiver must be an owned fixed array of the selected element type.
pub(super) fn element_step(
    expressions: &Expressions<'_>,
    node: usize,
    element: &ValueType,
) -> Result<PlaceProjection, LoweringError> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let site = site_at(tree, node);
    let [base, selector] = *file.nodes[node].children.as_slice() else {
        return Err(LoweringError::new(&site, "owned element transfer has no exact receiver and selector"));
    };
    let receiver = tree
        .semantics_at(base)
        .find_map(|record| match record {
            NodeSemantics::Expression(e) if e.static_argument_slot.is_none() => Some(&e.ty),
            _ => None,
        })
        .ok_or_else(|| LoweringError::new(&site, "owned element transfer has no typed receiver"))?;
    let ValueType::Array(inner, length) = receiver else {
        return Err(LoweringError::new(&site, "owned element transfer requires an owned fixed-array receiver"));
    };
    if inner.as_ref() != element || *length == 0 {
        return Err(LoweringError::new(&site, "owned element transfer disagrees with its nonempty receiver"));
    }
    let (_, steps) = expressions.tree.forest().place_path(file.node_id(node));
    let expected = Projection::Index(file.node_id(selector));
    if steps
        .last()
        .is_none_or(|step| step.base != file.node_id(file.ungroup(base)) || step.through.is_some() || step.projection.as_ref() != Some(&expected))
    {
        return Err(LoweringError::new(&site, "owned element transfer disagrees with its typed place projection"));
    }
    let value = frozen_selector(expressions.prepared, tree, selector)?;
    let ordinal = u64::try_from(value)
        .ok()
        .filter(|ordinal| ordinal < length)
        .ok_or_else(|| LoweringError::new(&site_at(tree, selector), "owned element transfer selects outside its fixed array"))?;
    Ok(PlaceProjection::FixedIndex(ordinal))
}

/// Whether an array literal assembles completed owned nominal elements. Each
/// operand is a fresh owned place that the constructor consumes exactly once;
/// copyable and scalar elements keep their existing construction gate.
pub(super) fn owned_elements(
    types: &crate::LoweringTypes,
    data: &[crate::LoweredData],
    element: &ValueType,
    extent: u64,
    count: usize,
    site: &crate::Site,
) -> Result<bool, LoweringError> {
    if !crate::lowering::types::contains_nominal_value(element) {
        return Ok(false);
    }
    let id = types
        .id_for(element)
        .map_err(|e| LoweringError::new(site, e.to_string()))?;
    if !super::ownership::owned(id, types, data) {
        return Ok(false);
    }
    if extent == 0 || u64::try_from(count) != Ok(extent) {
        return Err(LoweringError::new(
            site,
            "owned array construction requires one completed element per declared ordinal",
        ));
    }
    Ok(true)
}

/// A literal or frozen constant selector. Every value record must agree; a
/// computed, runtime or parameter selector supplies no static ordinal.
pub(super) fn frozen_selector<'a>(
    prepared: &'a PreparedTrees<'_>,
    tree: &'a LoweringTree<'_>,
    selector: usize,
) -> Result<&'a BigInt, LoweringError> {
    let file = tree.resolved().file;
    let site = site_at(tree, selector);
    let refuse = || LoweringError::new(&site, "owned element transfer requires a literal or frozen constant selector");
    let kind = file.nodes[selector].kind;
    if !matches!(kind, NodeKind::Literal | NodeKind::PathExpression) {
        return Err(refuse());
    }
    let mut typed = false;
    let mut values: Vec<&BigInt> = Vec::new();
    for record in tree.semantics_at(selector) {
        match record {
            NodeSemantics::Expression(expression) if expression.static_argument_slot.is_none() => match &expression.ty {
                ty if ty == &super::access::INDEX_TYPE => typed = true,
                ValueType::Exact {
                    value,
                    fractional_origin: None,
                } if kind == NodeKind::Literal && value.is_integer() => {
                    typed = true;
                    values.push(value.numer());
                }
                _ => return Err(refuse()),
            },
            NodeSemantics::Landing(landing) if landing.ty == super::access::INDEX_TYPE && landing.value.is_integer() => values.push(landing.value.numer()),
            NodeSemantics::Constant(constant)
                if kind == NodeKind::PathExpression && matches!(tree.resolved().symbol(selector), Some(Symbol::Declaration(id)) if id == constant) =>
            {
                match prepared.constant(constant) {
                    Some(PreparedValue::Integer(value)) => values.push(value),
                    _ => return Err(refuse()),
                }
            }
            _ => return Err(refuse()),
        }
    }
    match values.split_first() {
        Some((first, rest)) if typed && rest.iter().all(|value| value == first) => Ok(first),
        _ => Err(refuse()),
    }
}
