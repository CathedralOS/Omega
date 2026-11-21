//! Whole-element extraction and restoration through an exclusive element view.
//!
//! An `@mut [T]` machine parameter owns none of its elements and has no static
//! extent, so the restoration window stays rooted at the view. Each authored
//! occurrence evaluates its selector once, observes that same view's element
//! count, and names the element as `Referent` followed by a `ViewElement` owed
//! against that observation. The extraction and the repairing store are the
//! ordinary borrowed-storage window in [`super::loans`]; this module only
//! selects the exact typed element place. Lowering records the relation and
//! its bound obligation; the lowered-form checker judges them.
//!
//! Admitted subset: a nominal element type, a machine-parameter view spelled
//! directly as the receiver. Each selector lowers through the ordinary scalar
//! expression path at its source occurrence.
use super::{
    branches::BlockFlow,
    expressions::{EmitOperation, Expressions, OperationOutput},
};
use crate::{LoweringError, MachineBody, OperationKind, OperationResult, PlaceProjection, ProjectedPlace, RuntimeIndex, TypeShape, ValueId, site_at};
use omega_program_contracts::CollectionPrimitive;
use omega_resolved::Symbol;
use omega_syntax::NodeKind;
use omega_typed::{Access, NodeSemantics, ObligationKind, Projection, ValueType};

/// How two captured places on the same storage relate for restoration.
pub(super) enum Relation {
    /// The identical place: same root, same static segments, same selector values.
    Same,
    /// One place encloses the other.
    Overlap,
    /// Same root and static prefix, but element selectors not proved equal.
    Unproved,
    Disjoint,
}

/// One operand of an authored entry clause that relates two selectors.
#[derive(Clone, Debug, PartialEq)]
enum PremiseOperand {
    /// The entry value of this scalar formal.
    Formal(ValueId),
    Integer(num_bigint::BigInt),
}

/// An entry clause of the form `a == b` or `a != b` over scalar formals and
/// integer literals, taken from the published `contract.requires`.
///
/// Psi fixes a store's disposition before the checker runs, so lowering must
/// pick an encoding: fill the open vacancy or displace a distinct occupied
/// element. Such a clause tells lowering which encoding the program means. It
/// is not a verdict: the clause stays in the published contract, both
/// selectors stay at their occurrences, and the lowered-form checker must
/// establish the same relation from that premise
/// (`wiki/spec/terminal-psi/loans.md#captured-places-and-available-premises`).
/// Lowering does no premise reasoning beyond this exact clause shape; any other
/// premise leaves the relation unproved.
#[derive(Clone, Debug)]
pub(crate) struct SelectorPremise {
    left: PremiseOperand,
    right: PremiseOperand,
    equal: bool,
}

/// Collect the selector relations stated directly by `requires` clauses.
pub(crate) fn selector_premises(
    requires: &[crate::ContractClause],
    parameters: &[crate::Parameter],
) -> Vec<SelectorPremise> {
    use crate::{Parameter, PredicateOperator, PredicateStep};
    let operand = |step: &PredicateStep| match step {
        PredicateStep::Formal(index) => match parameters.get(*index) {
            Some(Parameter::Scalar(value)) => Some(PremiseOperand::Formal(*value)),
            _ => None,
        },
        PredicateStep::Integer(value) => Some(PremiseOperand::Integer(value.clone())),
        _ => None,
    };
    requires
        .iter()
        .filter_map(|clause| {
            let [left, right, PredicateStep::Apply(operator)] = clause.predicate.as_slice() else {
                return None;
            };
            let equal = match operator {
                PredicateOperator::Equal => true,
                PredicateOperator::NotEqual => false,
                _ => return None,
            };
            Some(SelectorPremise {
                left: operand(left)?,
                right: operand(right)?,
                equal,
            })
        })
        .collect()
}

/// Compare two captured places. View elements match by their evaluated
/// selector only; each occurrence keeps its own bound obligation and length
/// observation, so those never decide identity. Distinct literal indices select
/// disjoint elements. Two unrelated SSA values can select the same element
/// unless an authored entry clause relates them.
pub(super) fn relate(
    body: &MachineBody,
    premises: &[SelectorPremise],
    left: &ProjectedPlace,
    right: &ProjectedPlace,
) -> Relation {
    if left.root != right.root {
        return Relation::Disjoint;
    }
    for (a, b) in left.path.iter().zip(&right.path) {
        match (a, b) {
            (PlaceProjection::ViewElement { index: a, .. }, PlaceProjection::ViewElement { index: b, .. }) => {
                match compare_selectors(body, premises, a.selector, b.selector) {
                    Relation::Same => {}
                    result => return result,
                }
            }
            _ if a == b => {}
            _ => return Relation::Disjoint,
        }
    }
    if left.path.len() == right.path.len() {
        Relation::Same
    } else {
        Relation::Overlap
    }
}

/// Classify congruent selectors as the same and unequal literal indices as
/// disjoint. Otherwise an authored entry clause that equates or separates
/// exactly these two selectors decides; every other pair, including selectors
/// that only arithmetic or a less direct premise would relate, stays unproved.
fn compare_selectors(
    body: &MachineBody,
    premises: &[SelectorPremise],
    left: ValueId,
    right: ValueId,
) -> Relation {
    if congruent(body, left, right) {
        return Relation::Same;
    }
    let literal = |value: ValueId| {
        let crate::ValueOrigin::Operation(op) = &body.values[value.index()].origin else {
            return None;
        };
        match &body.operations[op.index()].kind {
            OperationKind::Constant(literal) => Some((body.values[value.index()].ty, literal)),
            _ => None,
        }
    };
    match (literal(left), literal(right)) {
        (Some((ta, a)), Some((tb, b))) if ta == tb => {
            if a == b {
                Relation::Same
            } else {
                Relation::Disjoint
            }
        }
        _ => premised(body, premises, left, right),
    }
}

/// The relation an authored clause states between these exact selectors.
/// A formal matches only its own entry value, never a later assignment; a
/// literal matches only a selector that is that integer constant. Both
/// selectors must share a scalar type.
fn premised(
    body: &MachineBody,
    premises: &[SelectorPremise],
    left: ValueId,
    right: ValueId,
) -> Relation {
    if body.values[left.index()].ty != body.values[right.index()].ty {
        return Relation::Unproved;
    }
    let names = |operand: &PremiseOperand, selector: ValueId| match operand {
        PremiseOperand::Formal(value) => *value == selector,
        PremiseOperand::Integer(expected) => matches!(
            &body.values[selector.index()].origin,
            crate::ValueOrigin::Operation(op)
                if matches!(&body.operations[op.index()].kind, OperationKind::Constant(crate::ScalarLiteral::Integer(v)) if v == expected)
        ),
    };
    let mut stated = None;
    for premise in premises {
        let relates = (names(&premise.left, left) && names(&premise.right, right)) || (names(&premise.left, right) && names(&premise.right, left));
        // Contradictory clauses leave the store with no single meaning.
        if relates && stated.replace(premise.equal).is_some_and(|prior| prior != premise.equal) {
            return Relation::Unproved;
        }
    }
    match stated {
        Some(true) => Relation::Same,
        Some(false) => Relation::Disjoint,
        None => Relation::Unproved,
    }
}

/// Two scalar values are congruent when they are the same SSA value, or when
/// the same pure scalar operation produced each of them from congruent
/// operands. Equal literals are the operand-free case. These operations read
/// no storage, so an unchanged operand value is the whole input; a load, a
/// call or a block parameter never matches another occurrence. The checker
/// re-derives the same equality from these operations
/// (`wiki/spec/terminal-psi/fact_validity.md#equality-relations`).
fn congruent(
    body: &MachineBody,
    left: ValueId,
    right: ValueId,
) -> bool {
    if left == right {
        return true;
    }
    if body.values[left.index()].ty != body.values[right.index()].ty {
        return false;
    }
    let operation = |value: ValueId| match &body.values[value.index()].origin {
        crate::ValueOrigin::Operation(op) => Some(&body.operations[op.index()].kind),
        _ => None,
    };
    let (Some(a), Some(b)) = (operation(left), operation(right)) else {
        return false;
    };
    let all = |a: &[ValueId], b: &[ValueId]| a.len() == b.len() && a.iter().zip(b).all(|(a, b)| congruent(body, *a, *b));
    match (a, b) {
        (OperationKind::Constant(a), OperationKind::Constant(b)) => a == b,
        (OperationKind::Boolean { primitive: p, operands: a }, OperationKind::Boolean { primitive: q, operands: b }) => p == q && all(a, b),
        (OperationKind::ExactNumericCast { operand: a }, OperationKind::ExactNumericCast { operand: b }) => congruent(body, *a, *b),
        (
            OperationKind::TrappingInteger {
                primitive: p,
                left: a,
                right: c,
            },
            OperationKind::TrappingInteger {
                primitive: q,
                left: b,
                right: d,
            },
        ) => p == q && congruent(body, *a, *b) && congruent(body, *c, *d),
        (OperationKind::Integer { operation: p, operands: a }, OperationKind::Integer { operation: q, operands: b }) => p == q && all(a, b),
        _ => false,
    }
}

/// The receiver, selector and view carrier of an index through `@mut [T]`,
/// for a nominal or a scalar element.
fn receiver(
    expressions: &Expressions<'_>,
    node: usize,
) -> Option<(usize, usize, ValueType)> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let index = file.ungroup(node);
    let (NodeKind::Index, [base, selector]) = (file.nodes[index].kind, file.nodes[index].children.as_slice()) else {
        return None;
    };
    let carrier = tree.semantics_at(*base).find_map(|row| match row {
        NodeSemantics::Expression(e) if e.static_argument_slot.is_none() => Some(&e.ty),
        _ => None,
    })?;
    if !crate::lowering::types::is_exclusive_element_view(carrier) {
        return None;
    }
    Some((*base, *selector, carrier.clone()))
}

/// Whether this place indexes an exclusive element view. Selection only
/// routes the node; [`element_place`] checks every typed record.
pub(super) fn selects_view_element(
    expressions: &Expressions<'_>,
    node: usize,
) -> bool {
    receiver(expressions, node).is_some()
}

/// `move view[k]`: extract the selected element through the view's loan,
/// leaving restoration debt at that exact element place.
pub(super) fn move_element(
    expressions: &Expressions<'_>,
    node: usize,
    carrier: &ValueType,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
) -> Result<OperationResult, LoweringError> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let site = site_at(tree, node);
    let [operand] = *file.nodes[node].children.as_slice() else {
        return Err(LoweringError::new(&site, "move requires one element place"));
    };
    let mut typed = false;
    let mut moved = false;
    for row in tree.semantics_at(node) {
        match row {
            NodeSemantics::Expression(e) if !typed && &e.ty == carrier && e.static_argument_slot.is_none() => typed = true,
            NodeSemantics::Obligation(o) if !moved && matches!(&o.kind,ObligationKind::Move{place,ty} if *place==site_at(tree,operand) && ty==carrier) => {
                moved = true
            }
            _ => return Err(LoweringError::new(&site, "element move requires additional typed evidence")),
        }
    }
    if !typed || !moved {
        return Err(LoweringError::new(&site, "element move lacks its typed operand obligation"));
    }
    let source = element_place(expressions, operand, carrier, true, body, flow)?;
    super::loans::move_borrowed(expressions, node, source, carrier, body)
}

/// `view[k] = value`: evaluate this occurrence's selector and length, then
/// fill the vacancy or displace the occupied element through the ordinary
/// borrowed replacement.
pub(super) fn store_element(
    expressions: &Expressions<'_>,
    place: usize,
    rhs: usize,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
    before_return: bool,
) -> Result<(), LoweringError> {
    let site = site_at(expressions.tree, place);
    let Some((_, _, view)) = receiver(expressions, place) else {
        return Err(LoweringError::new(&site, "element store requires an exclusive element view"));
    };
    let element = match &view {
        ValueType::Reference(Access::Mut, slice) => match slice.as_ref() {
            ValueType::Slice(element) => element.as_ref().clone(),
            _ => return Err(LoweringError::new(&site, "element store requires an exclusive element view")),
        },
        _ => return Err(LoweringError::new(&site, "element store requires an exclusive element view")),
    };
    if let Ok(scalar) = crate::lowering::types::scalar_carrier(&element) {
        return store_scalar_element(expressions, place, rhs, &element, scalar, body, flow);
    }
    let destination = element_place(expressions, place, &element, false, body, flow)?;
    super::loans::store_borrowed(expressions, destination, rhs, &element, body, flow, site, before_return)
}

/// `view[i] = value` through `@mut [T]` with a scalar `T`: replace one
/// element in place (`wiki/spec/terminal-psi/byte_views.md#element-typed-slice-views`).
/// A scalar element needs no restoration window; the write displaces nothing
/// with custody. Evaluate the selector, then the value, then observe the same
/// view's length immediately before the write, which owes `index < length`.
fn store_scalar_element(
    expressions: &Expressions<'_>,
    place: usize,
    rhs: usize,
    element: &ValueType,
    scalar: crate::ScalarType,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
) -> Result<(), LoweringError> {
    let tree = expressions.tree;
    let site = site_at(tree, place);
    let refuse = |message: &str| LoweringError::new(&site, message);
    let (selector, root) = selected_view(expressions, place, element, false, body)?;
    if expressions.types.get(body.places[root.index()].ty) != Some(&TypeShape::MutableElementView { element: crate::ViewElement::Scalar(scalar) }) {
        return Err(refuse("element view descriptor disagrees with its element type"));
    }
    let index_ty = expressions
        .types
        .id_for(&super::access::INDEX_TYPE)
        .map_err(|e| refuse(&e.to_string()))?;
    let OperationResult::Scalar(index) = expressions.lower(selector, &super::access::INDEX_TYPE, body, flow)? else {
        return Err(refuse("element selector requires a scalar value"));
    };
    if body.values[index.index()].ty != index_ty {
        return Err(refuse("element selector requires a u64 value"));
    }
    let OperationResult::Scalar(value) = expressions.lower(rhs, element, body, flow)? else {
        return Err(refuse("element write requires a scalar value"));
    };
    let element_ty = expressions.types.id_for(element).map_err(|e| refuse(&e.to_string()))?;
    if body.values[value.index()].ty != element_ty {
        return Err(refuse("element write value disagrees with its element type"));
    }
    let OperationResult::Scalar(length) = body.push_operation(
        OperationOutput::Scalar(index_ty),
        OperationKind::ElementViewLength { source: root },
        site.clone(),
    ) else {
        unreachable!("a length observation produces a scalar")
    };
    let obligation = crate::BoundsObligationId(super::body::next_id(body.diagnostics.bounds.len()));
    body.diagnostics.bounds.push(site_at(tree, selector));
    body.push_operation(
        OperationOutput::Unit,
        OperationKind::ElementViewWrite {
            destination: root,
            path: Vec::new(),
            index,
            value,
            length,
            obligation,
        },
        site,
    );
    Ok(())
}

/// The exact typed selection of one element through a machine-parameter
/// `@mut [T]`: the exclusive core element primitive, its bound, the view's
/// exclusive loan and, for a move, the restoration debt. Returns the selector
/// node and the view's own descriptor.
fn selected_view(
    expressions: &Expressions<'_>,
    node: usize,
    element: &ValueType,
    moving: bool,
    body: &MachineBody,
) -> Result<(usize, crate::PlaceId), LoweringError> {
    let tree = expressions.tree;
    let resolved = tree.resolved();
    let file = resolved.file;
    let index = file.ungroup(node);
    let site = site_at(tree, index);
    let refuse = |message: &str| LoweringError::new(&site, message);
    let Some((base, selector, view)) = receiver(expressions, index) else {
        return Err(refuse("element place requires an exclusive element view"));
    };
    let exact_view = ValueType::Reference(Access::Mut, Box::new(ValueType::Slice(Box::new(element.clone()))));
    if view != exact_view {
        return Err(refuse("element place disagrees with its view's element type"));
    }
    // The typed selection: the exclusive core element primitive, its bound,
    // the view's exclusive loan and, for a move, the restoration debt.
    let reference = ValueType::Reference(Access::Mut, Box::new(element.clone()));
    let view_site = site_at(tree, base);
    let mut selection = None;
    let mut precondition = None;
    let mut borrowed = false;
    let mut restored = false;
    for row in tree.semantics_at(index) {
        match row {
            NodeSemantics::CollectionAccess(access)
                if selection.is_none()
                    && access.primitive == CollectionPrimitive::ElementMut
                    && &access.element == element
                    && access.result == reference
                    && matches!(access.selector_carriers.as_slice(), [only] if only == &super::access::INDEX_TYPE) =>
            {
                selection = Some(&access.machine);
            }
            NodeSemantics::Obligation(o) => match &o.kind {
                ObligationKind::CallPrecondition(machine) if precondition.is_none() => precondition = Some(machine),
                ObligationKind::Borrow { access: Access::Mut, place } if !borrowed && place == &view_site => borrowed = true,
                ObligationKind::RestoreBorrowed { place, ty } if moving && !restored && *place == file.node_id(index) && ty == element => restored = true,
                _ => return Err(refuse("element place has an unsupported obligation")),
            },
            NodeSemantics::Expression(e) if e.static_argument_slot.is_none() && (&e.ty == element || !moving && e.ty == reference) => {}
            _ => return Err(refuse("element place has unsupported typed records")),
        }
    }
    if selection.is_none() || precondition != selection || !borrowed || restored != moving {
        return Err(refuse("element place lacks its exclusive element selection, bound or loan"));
    }
    let (capture, steps) = expressions.prepared.typed().place_path(file.node_id(index));
    let base_id = file.node_id(file.ungroup(base));
    if capture != base_id
        || !matches!(steps.as_slice(), [step] if step.base == base_id
            && step.through == Some(Access::Mut)
            && step.projection == Some(Projection::Index(file.node_id(selector))))
    {
        return Err(refuse("element place disagrees with its typed place projection"));
    }
    // The root is the view's own parameter descriptor, never a copy.
    let root_node = file.ungroup(base);
    let Some(Symbol::Binding { binder_node, .. }) = (file.nodes[root_node].kind == NodeKind::PathExpression)
        .then(|| resolved.symbol(root_node))
        .flatten()
    else {
        return Err(refuse("element view must be named directly"));
    };
    let Some(OperationResult::Structural(root)) = expressions
        .bindings
        .iter()
        .rev()
        .find(|(n, _)| n == binder_node)
        .map(|(_, v)| *v)
    else {
        return Err(refuse("element view has no lowered descriptor"));
    };
    if body.places[root.index()].origin != crate::PlaceOrigin::Parameter {
        return Err(refuse("element window requires a machine-parameter view"));
    }
    Ok((selector, root))
}

/// Check the exact typed element selection, then evaluate the selector once
/// and observe the same view's length at this occurrence.
fn element_place(
    expressions: &Expressions<'_>,
    node: usize,
    element: &ValueType,
    moving: bool,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
) -> Result<ProjectedPlace, LoweringError> {
    let tree = expressions.tree;
    let site = site_at(tree, tree.resolved().file.ungroup(node));
    let refuse = |message: &str| LoweringError::new(&site, message);
    let (selector, root) = selected_view(expressions, node, element, moving, body)?;
    let element_ty = expressions
        .types
        .id_for(element)
        .map_err(|e| refuse(&e.to_string()))?;
    let Some(TypeShape::Nominal(data)) = expressions.types.get(element_ty) else {
        return Err(refuse("element window requires a nominal element"));
    };
    if expressions.types.get(body.places[root.index()].ty) != Some(&TypeShape::MutableElementView { element: crate::ViewElement::Nominal(*data) }) {
        return Err(refuse("element view descriptor disagrees with its element type"));
    }
    if expressions.data[data.get() - 1].carry.is_some() {
        return Err(refuse("element transfer requires the carrier's address stability evidence"));
    }
    // Capture the destination selector before evaluating the replacement.
    let index_ty = expressions
        .types
        .id_for(&super::access::INDEX_TYPE)
        .map_err(|e| refuse(&e.to_string()))?;
    let OperationResult::Scalar(value) = expressions.lower(selector, &super::access::INDEX_TYPE, body, flow)? else {
        return Err(refuse("element selector requires a scalar value"));
    };
    if body.values[value.index()].ty != index_ty {
        return Err(refuse("element selector requires a u64 value"));
    }
    let OperationResult::Scalar(length) = body.push_operation(
        OperationOutput::Scalar(index_ty),
        OperationKind::ElementViewLength { source: root },
        site.clone(),
    ) else {
        unreachable!("a length observation produces a scalar")
    };
    let obligation = crate::BoundsObligationId(super::body::next_id(body.diagnostics.bounds.len()));
    body.diagnostics.bounds.push(site_at(tree, selector));
    Ok(ProjectedPlace {
        root,
        path: vec![
            PlaceProjection::Referent,
            PlaceProjection::ViewElement {
                index: RuntimeIndex { selector: value, obligation },
                length,
            },
        ],
    })
}
