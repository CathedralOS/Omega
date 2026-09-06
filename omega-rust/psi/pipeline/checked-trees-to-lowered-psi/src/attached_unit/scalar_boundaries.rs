//! Boundary-wrapper catalog roots belong to the same module as their Unit callers.

use super::*;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Scalar calls with structural arguments retain the same authored parameter
/// and permission identities as Unit calls; scalar production does not erase them.
pub(super) fn validate_call_source(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    operation: &CheckedUnitEffectOperationPlan,
    target: &CheckedBoundaryScalarReturnMachinePlan,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::ScalarCall {
        coordinate,
        target_machine,
        target_state,
        structural_arguments,
        claim_transfers,
        ..
    } = operation
    else {
        return unsupported("scalar wrapper source custody requires an ordinary scalar call");
    };
    let authored =
        crate::call_source_custody::authored::locate_source(checked, caller.state, *coordinate)?;
    let (source_machine, source_state) =
        crate::scalar_source_custody::authored_state(checked, caller.state)?;
    if source_machine.symbol != caller.machine
        || *target_machine != target.machine
        || *target_state != target.state
        || authored.target_machine != target.machine
        || authored.target_state != target.state
        || authored.boundary
        || structural_arguments.len() != target.structural_parameters.len()
    {
        return unsupported("scalar wrapper structural source disagrees with its exact call");
    }
    structural_calls::validate_consumer(
        checked,
        caller,
        operation,
        &target.structural_parameters,
        &target.entry_claims,
    )?;
    let mut expected_transfers = Vec::new();
    for (argument_index, (argument, parameter)) in structural_arguments
        .iter()
        .zip(&target.structural_parameters)
        .enumerate()
    {
        if argument
            .source_structural_result_binding_ordinal()
            .is_some()
        {
            // The existing result owner checks exact local identity, source
            // order, whole-affine custody and absence of transferred claims.
            continue;
        }
        let source_index = argument.source_parameter_index().ok_or(
            LoweringError::Unsupported(
                "scalar wrapper argument requires an entry parameter or established structural result",
            ),
        )?;
        let retained = caller
            .structural_parameters
            .get(source_index as usize)
            .ok_or(LoweringError::Unsupported(
                "scalar wrapper argument has no source parameter",
            ))?;
        let source = checked
            .state_parameters(source_state)
            .get(retained.position as usize)
            .ok_or(LoweringError::Unsupported(
                "scalar wrapper parameter lost its authored position",
            ))?;
        let expression = authored
            .structural_arguments
            .iter()
            .find_map(|(position, expression)| {
                (*position == parameter.position).then_some(*expression)
            })
            .or_else(|| {
                if !parameter.is_self {
                    return None;
                }
                let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
                    authored.source_site
                else {
                    return None;
                };
                match checked.expression_table.expression(expression) {
                    ExpressionNode::Call(call) => Some(call.receiver),
                    _ => None,
                }
            })
            .ok_or(LoweringError::Unsupported(
                "scalar wrapper parameter has no exact authored argument",
            ))?;
        let (root, path, explicit_access) =
            source_path(checked, source_machine, source.type_reference, expression)?;
        if !source.symbol.is_valid()
            || !(root == source.symbol || (source.is_self && root == caller.machine))
            || retained.is_self != source.is_self
            || path != argument.path
            || explicit_access.is_some_and(|access| access != argument.access)
        {
            return unsupported("scalar wrapper structural argument substituted its source place");
        }
        if argument.access != checked_trees::CheckedStructuralAccess::Owned {
            continue;
        }
        for claim in caller.entry_claims.iter().filter(|claim| {
            claim.parameter_index == source_index
                && (argument.path.is_empty() || claim.path == argument.path)
        }) {
            if claim.claim_identity == PermissionClaimIdentity::Unknown {
                return unsupported("scalar wrapper transfer has no exact source claim");
            }
            expected_transfers.push(checked_trees::CheckedUnitClaimTransferPlan {
                claim_identity: claim.claim_identity,
                argument_index: u32::try_from(argument_index).map_err(|_| {
                    LoweringError::Unsupported("scalar wrapper argument count exceeds u32")
                })?,
            });
        }
    }
    if *claim_transfers != expected_transfers {
        return unsupported("scalar wrapper transfers disagree with their source entry claims");
    }
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.machine_symbol == caller.machine
                && event.state_symbol == caller.state
                && event.source
                    == language_semantics::PermissionEventSource::Call {
                        statement_index: coordinate.statement_index as usize,
                        call_ordinal: coordinate.call_ordinal as usize,
                        target_symbol: authored.source_target,
                    }
                && event.kind == language_semantics::PermissionEventKind::Transfer
                && event.access == language_semantics::PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live)
                .then_some(event)
        })
        .collect::<Vec<_>>();
    if events.len() != expected_transfers.len()
        || expected_transfers.iter().any(|transfer| {
            events
                .iter()
                .filter(|event| event.claim_identity == transfer.claim_identity)
                .count()
                != 1
        })
    {
        return unsupported("scalar wrapper transfers lost their exact checked call events");
    }
    Ok(())
}

enum SourceProjection {
    Field(ExpressionHandle, symbols::SymbolHandle),
    Index(u64),
}

fn source_path(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    source_type: TypeReferenceHandle,
    mut expression: ExpressionHandle,
) -> Result<
    (
        symbols::SymbolHandle,
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        Option<checked_trees::CheckedStructuralAccess>,
    ),
    LoweringError,
> {
    let mut projections = Vec::new();
    let mut access = None;
    let mut visited = Vec::new();
    let root = loop {
        if !checked.expression_table.expression_is_valid(expression)
            || visited.contains(&expression)
        {
            return unsupported("scalar wrapper structural argument has a stale or cyclic source");
        }
        visited.push(expression);
        match checked.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) if access.is_none() && projections.is_empty() => {
                access = Some(match borrow.access {
                    language_core::ReferenceAccess::Shared => {
                        checked_trees::CheckedStructuralAccess::SharedBorrow
                    }
                    language_core::ReferenceAccess::Mutable => {
                        checked_trees::CheckedStructuralAccess::MutableBorrow
                    }
                    language_core::ReferenceAccess::WriteOnly => {
                        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
                    }
                });
                expression = borrow.target;
            }
            ExpressionNode::Member(member)
                if member.case_variant.is_none() && member.member_symbol.is_valid() =>
            {
                projections.push(SourceProjection::Field(expression, member.member_symbol));
                expression = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                let ExpressionNode::Integer(index) =
                    checked.expression_table.expression(indexed.index)
                else {
                    return unsupported(
                        "scalar wrapper structural argument requires a literal index",
                    );
                };
                let index = index
                    .value_bignum()
                    .and_then(|value| value.to_u64())
                    .ok_or(LoweringError::Unsupported(
                        "scalar wrapper structural index exceeds u64",
                    ))?;
                projections.push(SourceProjection::Index(index));
                expression = indexed.collection;
            }
            ExpressionNode::Name(name)
                if name.symbol.is_valid()
                    && name.symbol == name.head_symbol
                    && checked
                        .expression_table
                        .name_path_members(name.members)
                        .len()
                        == 1 =>
            {
                break name.symbol;
            }
            _ => return unsupported("scalar wrapper structural argument is not a parameter place"),
        }
    };
    let mut type_reference = source_type;
    let mut path = Vec::new();
    for projection in projections.into_iter().rev() {
        type_reference = unqualified_source_type(checked, type_reference)?;
        match projection {
            SourceProjection::Field(expression, symbol) => {
                let owner = match checked.type_reference_table.type_reference(type_reference) {
                    TypeReferenceNode::Named { symbol, .. } => *symbol,
                    TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                    _ => return unsupported("scalar wrapper field has no declared record owner"),
                };
                let data = checked
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == owner)
                    .ok_or(LoweringError::Unsupported(
                        "scalar wrapper field owner is absent",
                    ))?;
                let symbol = validation::exact_self_field(&checked.typed, machine, expression)
                    .map_or(symbol, |field| field.symbol);
                let field = checked
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        checked_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                            Some(field)
                        }
                        _ => None,
                    })
                    .ok_or(LoweringError::Unsupported(
                        "scalar wrapper field substituted its declaration owner",
                    ))?;
                path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
                ));
                type_reference = field.type_reference;
            }
            SourceProjection::Index(index) => {
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: checked_trees::types::FixedArrayLength::Literal(length),
                } = checked.type_reference_table.type_reference(type_reference)
                else {
                    return unsupported("scalar wrapper index has no literal fixed-array owner");
                };
                if usize::try_from(index)
                    .ok()
                    .is_none_or(|index| index >= *length)
                {
                    return unsupported("scalar wrapper structural index is out of bounds");
                }
                path.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    index,
                ));
                type_reference = *element_type;
            }
        }
    }
    Ok((root, path, access))
}

fn unqualified_source_type(
    checked: &CheckedTrees,
    mut reference: TypeReferenceHandle,
) -> Result<TypeReferenceHandle, LoweringError> {
    let mut visited = Vec::new();
    loop {
        if !reference.is_valid() || visited.contains(&reference) {
            return unsupported("scalar wrapper source has an invalid type chain");
        }
        visited.push(reference);
        match checked.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            _ => return Ok(reference),
        }
    }
}

pub(super) fn retain_catalog_roots<'checked>(
    checked: &'checked CheckedTrees,
    callees: &[PreparedScalarCallee<'checked>],
    boundaries: &mut Vec<(&'checked CheckedBoundaryMachinePlan, String)>,
    type_roots: &mut Vec<String>,
    service_roots: &mut Vec<ServiceReachId>,
) -> Result<(), LoweringError> {
    for callee in callees {
        let PreparedScalarCallee::Boundary { plan, .. } = callee else {
            continue;
        };
        let boundary =
            crate::boundary_scalar_return::validate_boundary_scalar_return(checked, plan)?;
        if let Some((existing, _)) = boundaries
            .iter()
            .find(|(candidate, _)| candidate.machine == boundary.machine)
        {
            if *existing != boundary {
                return unsupported("scalar wrapper and Unit boundary declarations disagree");
            }
        } else {
            boundaries.push((
                boundary,
                checked_unit_boundary_identity(checked, boundary.machine)?,
            ));
        }
        type_roots.push(plan.attachment_type_identity.clone());
        type_roots.extend(
            plan.structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity.clone()),
        );
        collect_installation_machine_contract_services(
            checked,
            plan.machine,
            plan.contract_service_reach,
            plan.service_reach,
            service_roots,
        )?;
        let CheckedUnitEffectOperationPlan::BoundaryCall { service_reach, .. } =
            &plan.boundary_call
        else {
            return unsupported("scalar wrapper lost its boundary operation");
        };
        collect_service_summary(
            &checked.facts.service_reaches.rows,
            *service_reach,
            service_roots,
        )?;
    }
    Ok(())
}
