//! Read a scalar leaf of an indexed affine place without loading the owner.
use super::branches::BlockFlow;
use super::expressions::{EmitOperation, Expressions, OperationOutput};
use crate::{BoundsObligationId, LoweringError, MachineBody, OperationKind, OperationResult, PlaceProjection, RuntimeIndex, TypeShape, site_at};
use omega_program_contracts::CollectionPrimitive;
use omega_syntax::NodeKind;
use omega_typed::{NodeSemantics, ObligationKind, Projection, ValueType};

pub(super) fn scalar_field(
    expressions: &Expressions<'_>,
    node: usize,
    expected: &ValueType,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
) -> Result<Option<OperationResult>, LoweringError> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let mut fields = Vec::new();
    let mut current = file.ungroup(node);
    while let (NodeKind::FieldAccess, [base]) = (file.nodes[current].kind, &file.nodes[current].children[..]) {
        fields.push((current, *base));
        current = file.ungroup(*base);
    }
    let (NodeKind::Index, [base, selector]) = (file.nodes[current].kind, &file.nodes[current].children[..]) else {
        return Ok(None);
    };
    let (&base, &selector) = (base, selector);
    if !tree
        .semantics_at(current)
        .any(|row| matches!(row, NodeSemantics::CollectionAccess(access) if access.primitive == CollectionPrimitive::Element))
    {
        return Ok(None);
    }

    let refuse = |node, message| LoweringError::new(&site_at(tree, node), message);
    let (mut source, stored) = super::references::original_place(expressions, base, body, flow)?;
    if matches!(stored.reference_parts(), Some((_, ValueType::Slice(_)))) {
        return Err(refuse(
            current,
            "scalar observation of an affine slice element needs a portable shared element loan",
        ));
    }
    let ValueType::Array(element, _) = stored.carrier() else {
        return Err(refuse(current, "indexed scalar observation requires its original fixed-array place"));
    };
    let references: Vec<_> = source
        .path
        .iter()
        .enumerate()
        .filter_map(|(i, step)| (*step == PlaceProjection::Referent).then_some(i))
        .collect();
    if !matches!(references.as_slice(), [] | [0]) {
        return Err(refuse(current, "indexed scalar observation cannot defer a replaceable reference crossing"));
    }
    let (_, place_steps) = tree.forest().place_path(file.node_id(current));
    if !place_steps.last().is_some_and(|step| {
        step.base == file.node_id(file.ungroup(base)) && step.through.is_none() && step.projection == Some(Projection::Index(file.node_id(selector)))
    }) {
        return Err(refuse(current, "indexed scalar observation lost its exact selector projection"));
    }
    super::access::check_indexed_place(tree, current, base, element, super::access::IndexedPlaceUse::ScalarRead)?;

    let OperationResult::Scalar(index) = expressions.lower(selector, &super::access::INDEX_TYPE, body, flow)? else {
        return Err(refuse(selector, "indexed read selector needs its scalar evaluation"));
    };
    if expressions.types.id_for(&super::access::INDEX_TYPE).ok() != Some(body.values[index.index()].ty) {
        return Err(refuse(selector, "indexed read selector needs the exact u64 value"));
    }
    let obligation = BoundsObligationId(super::body::next_id(body.diagnostics.bounds.len()));
    body.diagnostics.bounds.push(site_at(tree, selector));
    source
        .path
        .push(PlaceProjection::RuntimeIndex(RuntimeIndex { selector: index, obligation }));

    let mut carrier = element.as_ref();
    for (field_node, receiver) in fields.into_iter().rev() {
        let (_, steps) = tree.forest().place_path(file.node_id(field_node));
        let field = steps
            .last()
            .and_then(|step| match &step.projection {
                Some(Projection::Field(member)) if step.through.is_none() && step.base == file.node_id(file.ungroup(receiver)) => Some(member),
                _ => None,
            })
            .ok_or_else(|| refuse(field_node, "indexed scalar observation needs its exact field projection"))?;
        let field = crate::identity::declaration(field, &site_at(tree, field_node))?;
        let owner_type = expressions
            .types
            .id_for(carrier)
            .map_err(|error| LoweringError::new(&site_at(tree, field_node), error.to_string()))?;
        let Some(TypeShape::Nominal(owner)) = expressions.types.get(owner_type) else {
            return Err(refuse(field_node, "indexed scalar observation needs a nominal field carrier"));
        };
        let owner = &expressions.data[owner.get() - 1];
        let member = if let Some(case) = owner
            .cases
            .iter()
            .find(|case| case.fields.iter().any(|member| member.declaration == field))
        {
            source
                .path
                .push(PlaceProjection::Case(case.declaration.clone()));
            case.fields.iter().find(|member| member.declaration == field)
        } else {
            owner.fields.iter().find(|member| member.declaration == field)
        }
        .ok_or_else(|| refuse(field_node, "indexed field does not belong to its selected carrier"))?;
        carrier = tree
            .semantics_at(field_node)
            .find_map(|row| match row {
                NodeSemantics::Expression(e) if e.static_argument_slot.is_none() => Some(&e.ty),
                _ => None,
            })
            .ok_or_else(|| refuse(field_node, "indexed field read lacks its typed carrier"))?;
        if expressions.types.id_for(carrier).ok() != Some(member.carrier) {
            return Err(refuse(field_node, "indexed field read disagrees with its selected type"));
        }
        source.path.push(PlaceProjection::Field(field));
    }
    if carrier != expected
        || tree
            .semantics_at(node)
            .filter(|row| {
                matches!(
                    row, NodeSemantics::Obligation(obligation) if matches!(&obligation.kind, ObligationKind::CopyRead(ty) if ty == expected)
                )
            })
            .count()
            != 1
    {
        return Err(refuse(node, "indexed read requires the selected scalar's exact copy obligation"));
    }
    let ty = expressions
        .types
        .id_for(expected)
        .map_err(|error| LoweringError::new(&site_at(tree, node), error.to_string()))?;
    Ok(Some(body.push_operation(
        OperationOutput::Scalar(ty),
        OperationKind::ProjectedScalarRead { source },
        site_at(tree, node),
    )))
}
