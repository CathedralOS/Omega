//! Two adjacent assignments can close an indexed ownership window under either
//! selector relation. Keep each schedule on its own edge until custody agrees.
use super::{Relation, SelectorPremise, element_place, receiver, relate};
use crate::machines::{
    branches::{BlockFlow, successor},
    expressions::{EmitOperation, Expressions, OperationOutput},
    loans::{Replacement, Vacancy, captured_place},
};
use crate::{
    ArithmeticPolicy, BlockTerminator, IntegerOperation, IntegerPrimitive, LoweringError, MachineBody, OperationKind, OperationResult, PlaceProjection,
    ProjectedPlace, ValueId, site_at,
};
use omega_syntax::NodeKind;
use omega_typed::ValueType;

fn element_selector(place: &ProjectedPlace) -> Option<ValueId> {
    let [
        PlaceProjection::Referent,
        PlaceProjection::ViewElement { index, .. },
    ] = &place.path[..]
    else {
        return None;
    };
    Some(index.selector)
}

/// The vacancy search can see a syntactically similar projection in a sibling
/// block. A runtime comparison may read only an available SSA definition.
fn available_selector(
    body: &MachineBody,
    flow: &BlockFlow,
    selector: ValueId,
) -> bool {
    let (active, operations) = flow.active_range(body.operations.len());
    match body.values[selector.index()].origin {
        crate::ValueOrigin::Parameter => true,
        crate::ValueOrigin::BlockParameter(block) => block == active || block.get() == 1,
        crate::ValueOrigin::Operation(op) | crate::ValueOrigin::Component { operation: op, .. } => {
            operations.contains(&op.index())
                || body
                    .blocks
                    .iter()
                    .any(|block| block.id.get() == 1 && block.operations.contains(&op.index()))
        }
    }
}

pub(super) fn emit_pair(
    expressions: &Expressions<'_>,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
    element: &ValueType,
    first: Replacement,
    open: ProjectedPlace,
    next_statement: Option<(usize, bool)>,
) -> Result<(), LoweringError> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let target = captured_place(body, &first.destination);
    let Some((opened, selected)) = element_selector(&open)
        .zip(element_selector(&target))
        .filter(|(opened, _)| open.root == target.root && available_selector(body, flow, *opened))
    else {
        return Err(first.ambiguous());
    };
    let Some((statement, next_before_return)) = next_statement else { return Err(first.ambiguous()) };
    let next = &file.nodes[statement];
    let (NodeKind::Assign, &[place, rhs]) = (next.kind, next.children.as_slice()) else {
        return Err(first.ambiguous());
    };
    let place = file.ungroup(place);
    let Some((_, selector, _)) = receiver(expressions, place) else {
        return Err(first.ambiguous());
    };
    // A later selector with effects or an RHS that opens its own window would
    // need a new ownership analysis. This pair only consumes a bound replacement.
    let rhs_node = &file.nodes[file.ungroup(rhs)];
    let bound_value = match (rhs_node.kind, rhs_node.children.as_slice()) {
        (NodeKind::Move, &[source]) => file.nodes[file.ungroup(source)].kind == NodeKind::PathExpression,
        (NodeKind::PathExpression, _) => true,
        _ => false,
    };
    if !bound_value || !matches!(file.nodes[file.ungroup(selector)].kind, NodeKind::PathExpression | NodeKind::Literal) {
        return Err(first.ambiguous());
    }
    crate::machines::body::check_container(tree, statement)?;
    let boolean = expressions
        .types
        .id_for(&ValueType::Bool)
        .map_err(|e| LoweringError::new(&first.site, e.to_string()))?;
    let OperationResult::Scalar(condition) = body.push_operation(
        OperationOutput::Scalar(boolean),
        OperationKind::Integer {
            operation: IntegerOperation {
                primitive: IntegerPrimitive::Equal,
                policy: ArithmeticPolicy::Exact,
                obligations: vec![],
            },
            operands: vec![opened, selected],
        },
        first.site.clone(),
    ) else {
        unreachable!("integer equality produces a Boolean")
    };
    let equal = flow.reserve();
    let different = flow.reserve();
    let join = flow.reserve();
    let when_true = successor(body, equal, vec![], first.site.clone());
    let when_false = successor(body, different, vec![], first.site.clone());
    flow.close(
        body,
        BlockTerminator::Conditional {
            condition,
            when_true,
            when_false,
        },
    );
    for (arm, first_is_vacant) in [(equal, true), (different, false)] {
        flow.open(body, arm, None);
        first.emit(expressions, body, flow, first_is_vacant)?;
        // Everything below runs after the first assignment on the chosen path.
        // Continuing after the pair needs matching custody at the join. At a
        // unit return the final store can instead owe identity to the checker.
        let destination = element_place(expressions, place, element, false, body, flow)?;
        if !next_before_return
            && !matches!(
                relate(body, &expressions.selector_premises, &captured_place(body, &destination), &open),
                Relation::Same
            )
        {
            return Err(first.ambiguous());
        }
        let site = site_at(tree, place);
        let second = Replacement::capture(expressions, destination, rhs, element, body, flow, site.clone())?;
        let mut premises = expressions.selector_premises.clone();
        premises.push(SelectorPremise::on_branch(opened, selected, first_is_vacant));
        let Vacancy::Known(vacant) = second.vacancy(&premises, body, flow, next_before_return)? else {
            return Err(first.ambiguous());
        };
        second.emit(expressions, body, flow, vacant)?;
        flow.jump_with_result(body, join, None, site);
    }
    flow.open(body, join, None);
    Ok(())
}
