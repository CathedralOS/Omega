//! Exact source-actual replay for service arguments of ordinary Unit calls.
//!
//! Calls transport an existing whole parameter or borrowed record field. The
//! actual's formal position, source root and path are recovered independently;
//! unrelated arguments and surrounding operations impose no body-shape rule.

use super::{CheckedTrees, LoweringError, unsupported};
use crate::emission::call_source_custody::authored;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypeShape,
};

struct StatePlan<'a> {
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    parameters: &'a [CheckedUnitStructuralParameterPlan],
    operations: &'a [CheckedUnitEffectOperationPlan],
}

pub(super) fn validate(checked: &CheckedTrees) -> Result<(), LoweringError> {
    let flow = &checked.facts.flow.terminal_unit_effects;
    let plans = flow
        .machines
        .iter()
        .map(|plan| StatePlan {
            machine: plan.machine,
            state: plan.state,
            parameters: &plan.structural_parameters,
            operations: &plan.operations,
        })
        .chain(
            flow.composed_machines
                .iter()
                .filter(|plan| matches!(plan.result, checked_trees::CheckedControlResultPlan::Unit))
                .flat_map(|plan| {
                    plan.states.iter().map(move |state| StatePlan {
                        machine: plan.machine,
                        state: state.state,
                        parameters: &state.structural_parameters,
                        operations: &state.operations,
                    })
                }),
        )
        .collect::<Vec<_>>();
    for caller in &plans {
        for call in caller
            .operations
            .iter()
            .filter_map(|operation| operation.call())
            .filter(|call| {
                !call.is_boundary() && *call.result() == checked_trees::CheckedUnitCallResult::Unit
            })
        {
            let targets = plans
                .iter()
                .filter(|plan| {
                    plan.machine == call.target.machine && plan.state == call.target.state
                })
                .collect::<Vec<_>>();
            let target_source = checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == call.target.machine)
                .and_then(|machine| {
                    checked
                        .machine_states(machine)
                        .iter()
                        .find(|state| state.symbol == call.target.state)
                });
            let mut has_service = targets.iter().any(|plan| {
                plan.parameters
                    .iter()
                    .any(|parameter| parameter.fused_service_erasure.is_some())
            });
            if let Some(state) = target_source {
                for source in checked.state_parameters(state) {
                    has_service |= carrier(checked, source.type_reference)?.is_some();
                }
            }
            if !has_service {
                continue;
            }
            let [target] = targets.as_slice() else {
                return unsupported("service call has no unique direct or composed Unit target");
            };
            let target_source = target_source.ok_or(LoweringError::Unsupported(
                "service call lost its exact target state",
            ))?;
            if target.parameters.len() != call.structural_arguments.len() {
                return unsupported(
                    "service call structural arguments differ from its target signature",
                );
            }
            for (position, source) in checked.state_parameters(target_source).iter().enumerate() {
                if let Some(declared) = carrier(checked, source.type_reference)? {
                    let mut matching = target
                        .parameters
                        .iter()
                        .filter(|formal| formal.position as usize == position);
                    let formal = matching.next().ok_or(LoweringError::Unsupported(
                        "service target omitted its declared formal",
                    ))?;
                    if matching.next().is_some() {
                        return unsupported("service target duplicates a declared formal");
                    }
                    validate_parameter(checked, source, formal, &declared)?;
                }
            }
            let source_call = authored::locate(
                checked,
                caller.machine,
                caller.state,
                call.coordinate,
                target.machine,
                target.state,
            )?;
            let (source_machine, source_state) =
                crate::declarations::state_declaration(checked, caller.state)?;
            for (ordinal, (formal, argument)) in target
                .parameters
                .iter()
                .zip(&call.structural_arguments)
                .enumerate()
            {
                let source_formal = checked
                    .state_parameters(target_source)
                    .get(formal.position as usize)
                    .ok_or(LoweringError::Unsupported(
                        "service target formal position is invalid",
                    ))?;
                let Some(target_carrier) = carrier(checked, source_formal.type_reference)? else {
                    if formal.fused_service_erasure.is_some() {
                        return unsupported("ordinary target formal fabricates a service receipt");
                    }
                    continue;
                };
                validate_parameter(checked, source_formal, formal, &target_carrier)?;
                if call
                    .claim_transfers()
                    .iter()
                    .any(|transfer| transfer.argument_index as usize == ordinal)
                {
                    return unsupported("service argument fabricates a permission-claim transfer");
                }
                let expression = source_call
                    .structural_argument(formal.position, formal.is_self)
                    .ok_or(LoweringError::Unsupported(
                        "service argument lost its exact authored actual position",
                    ))?;
                let source_index =
                    argument
                        .source_parameter_index()
                        .ok_or(LoweringError::Unsupported(
                            "service argument must transport an existing parameter occurrence",
                        ))?;
                let parameter = caller.parameters.get(source_index as usize).ok_or(
                    LoweringError::Unsupported(
                        "service argument source parameter index is invalid",
                    ),
                )?;
                let source_parameter = checked
                    .state_parameters(source_state)
                    .get(parameter.position as usize)
                    .ok_or(LoweringError::Unsupported(
                        "service argument source position is invalid",
                    ))?;
                let source_shell =
                    super::structural_carrier_type(checked, source_parameter.type_reference)?;
                let source_access = match checked.type_reference_table.type_reference(source_shell)
                {
                    checked_trees::types::TypeReferenceNode::Reference { access, .. } => {
                        match access {
                            language_semantics::ReferenceAccess::Shared => {
                                CheckedStructuralAccess::SharedBorrow
                            }
                            language_semantics::ReferenceAccess::Mutable => {
                                CheckedStructuralAccess::MutableBorrow
                            }
                            language_semantics::ReferenceAccess::WriteOnly => {
                                CheckedStructuralAccess::WriteOnlyBorrow
                            }
                        }
                    }
                    _ => CheckedStructuralAccess::Owned,
                };
                if source_parameter.relevance.is_erased() || parameter.access != source_access {
                    return unsupported("service source root changed its declared loan access");
                }
                let (root, path, explicit_access) = super::source_path(
                    checked,
                    source_machine,
                    Some(source_state),
                    source_parameter.type_reference,
                    expression,
                )?;
                if !(root == source_parameter.symbol
                    || (source_parameter.is_self && root == source_machine.symbol))
                    || path != argument.path
                    || parameter.is_self != source_parameter.is_self
                {
                    return unsupported(
                        "service argument substituted its authored source root or field path",
                    );
                }
                let place_expression = match checked.expression_table.expression(expression) {
                    checked_trees::expression::ExpressionNode::Borrow(borrow) => borrow.target,
                    _ => expression,
                };
                let source_type = validation::declared_place_type_raw(
                    &checked.typed,
                    source_machine,
                    Some(source_state),
                    place_expression,
                )
                .ok_or(LoweringError::Unsupported(
                    "service argument lost its exact declared carrier",
                ))?;
                let actual = carrier(checked, source_type)?.ok_or(LoweringError::Unsupported(
                    "service argument is not a declared Binding occurrence",
                ))?;
                if actual.requirement != target_carrier.requirement
                    || actual.base_type_identity != target_carrier.base_type_identity
                    || argument.type_identity != target_carrier.base_type_identity
                    || argument.access != target_carrier.access
                    || explicit_access.unwrap_or(actual.access) != argument.access
                    || !super::checked_access_can_supply(parameter.access, argument.access)
                {
                    return unsupported(
                        "service argument changed its carrier, requirement, or loan access",
                    );
                }
                if path.is_empty() {
                    validate_parameter(checked, source_parameter, parameter, &actual)?;
                } else {
                    if argument.access == CheckedStructuralAccess::Owned
                        || parameter.fused_service_erasure.is_some()
                    {
                        return unsupported(
                            "service field argument must borrow its existing record occurrence",
                        );
                    }
                    validate_field(checked, &parameter.type_identity, &path, &target_carrier)?;
                }
            }
        }
    }
    Ok(())
}

fn carrier(
    checked: &CheckedTrees,
    reference: checked_trees::types::TypeReferenceHandle,
) -> Result<Option<checked_trees::CheckedServiceParameterCarrier>, LoweringError> {
    checked
        .bound_service_parameter_carrier(reference)
        .map_err(|_| LoweringError::Unsupported("service argument has an invalid declared carrier"))
}

pub(in crate::unit::attached_unit) fn validate_parameter(
    checked: &CheckedTrees,
    source: &checked_trees::signature::StateParameter,
    parameter: &CheckedUnitStructuralParameterPlan,
    carrier: &checked_trees::CheckedServiceParameterCarrier,
) -> Result<(), LoweringError> {
    let receipt = parameter
        .fused_service_erasure
        .as_ref()
        .ok_or(LoweringError::Unsupported(
            "service parameter lost its Fused receipt",
        ))?;
    let mut qualifications = carrier.qualifications.clone();
    qualifications.sort_by_key(|domain| domain.0);
    qualifications.dedup();
    if source.is_self
        || source.is_const
        || source.is_mutable != (carrier.access == CheckedStructuralAccess::MutableBorrow)
        || source.relevance.is_erased()
        || parameter.is_self
        || parameter.type_identity != carrier.base_type_identity
        || parameter.access != carrier.access
        || parameter.multiplicity != super::service_parameter_multiplicity(carrier.access)?
        || parameter.qualifications != qualifications
        || !parameter.projected_qualifications.is_empty()
        || receipt.source_parameter != source.symbol
        || receipt.carrier_type_identity != carrier.carrier_type_identity
        || receipt.requirement != carrier.requirement
        || checked.fused_service_erasure(carrier.requirement).is_none()
    {
        return unsupported(
            "service argument parameter receipt differs from its exact declaration",
        );
    }
    Ok(())
}

fn validate_field(
    checked: &CheckedTrees,
    root_identity: &str,
    path: &[CheckedUnitStructuralPathSegment],
    carrier: &checked_trees::CheckedServiceParameterCarrier,
) -> Result<(), LoweringError> {
    let mut identity = root_identity;
    for (position, segment) in path.iter().enumerate() {
        let mut types = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .filter(|row| row.identity == identity);
        let Some(shape) = types.next() else {
            return unsupported("service field lost its checked record owner");
        };
        if types.next().is_some() {
            return unsupported("service field has ambiguous checked record owner");
        }
        let (
            CheckedUnitStructuralPathSegment::Field(name),
            CheckedUnitStructuralTypeShape::Record { fields },
        ) = (segment, &shape.shape)
        else {
            return unsupported("service field argument does not name a record-field route");
        };
        let mut matching = fields.iter().filter(|field| field.identity == *name);
        let Some(field) = matching.next() else {
            return unsupported("service field is absent from its checked owner");
        };
        if matching.next().is_some() || field.relevance.is_erased() {
            return unsupported("service field has invalid checked relevance or identity");
        }
        match &field.field_type {
            CheckedUnitStructuralFieldType::Structural { type_identity }
                if position + 1 < path.len() =>
            {
                identity = type_identity
            }
            CheckedUnitStructuralFieldType::FusedServiceBacked {
                provider_type_identity,
                erasure,
            } if position + 1 == path.len()
                && provider_type_identity == &carrier.base_type_identity
                && erasure.requirement == carrier.requirement =>
            {
                return Ok(());
            }
            _ => {
                return unsupported(
                    "service field argument substituted its exact Fused field receipt",
                );
            }
        }
    }
    unsupported("service field argument has no field receipt")
}
