//! Source custody of post-call policies, independent of stage 05's emitter.
//!
//! A policy is not another provider call. It belongs immediately after the
//! exact original call, consumes its raw result and ordered operands, and is
//! the sole executable user of that raw result. Checking this relationship
//! prevents a valid standalone adapter from disguising a missing, substituted
//! or bypassed source policy. The artifact identity commits the checked pair.

use checked_trees::{CheckedArithmeticPolicyAdapter, CheckedTrees, NominalMachineUseSite};
use lowered_psi::LoweredSourceCustody;
use numerics::float_semantics::FloatFormat;
use semantic_vocabulary::IeeeFloatFormat;
use terminal_psi::{FloatResultPolicy, OperationKind};

mod saturation_eligibility;

/// Independent source-correspondence audit for tests and artifact review, not
/// a second construction pass on the ordinary compilation route.
pub fn verify_boundary_result_policies(
    checked: &CheckedTrees,
    module: &terminal_psi::TerminalModule,
    source_custody: &LoweredSourceCustody,
) -> Result<(), &'static str> {
    // Surviving call companions are retained by Psi optimization. Reconcile
    // both directions: deleting a call and its adapter must not leave a
    // recorded policy-bearing source occurrence without an executable pair.
    let source_policies = source_custody
        .source_call_occurrences
        .iter()
        .filter(|source| {
            let Some(NominalMachineUseSite::Expression(expression)) = source.source_site else {
                return false;
            };
            let mut has_policy = false;
            checked
                .facts
                .operators
                .for_each_resolved_expression_selection(expression, |origin, target, policy, _| {
                    if !has_policy
                        && target == source.source_target
                        && policy != CheckedArithmeticPolicyAdapter::None
                        && source.matches_checked_origin(checked, origin)
                    {
                        has_policy = true;
                    }
                });
            has_policy
        })
        .count();
    let mut matched = 0;
    let mut actual = 0;
    for machine in &module.machines {
        for block in &machine.blocks {
            for (position, operation) in block.operations.iter().enumerate() {
                if matches!(operation.kind, OperationKind::FloatResultPolicy { .. }) {
                    actual += 1;
                }
                let OperationKind::BoundaryCall {
                    boundary,
                    arguments,
                    ..
                } = &operation.kind
                else {
                    continue;
                };
                let Some(raw) = operation.result.scalar() else {
                    continue;
                };
                let Some(raw_format) = raw.scalar_type.as_ieee_float() else {
                    continue;
                };
                let mut sources = source_custody
                    .source_call_occurrences
                    .iter()
                    .filter(|source| source.terminal_site.operation() == Some(operation.id));
                let (Some(source), None) = (sources.next(), sources.next()) else {
                    return Err("float boundary call has no unique source occurrence");
                };
                let Some(NominalMachineUseSite::Expression(expression)) = source.source_site else {
                    continue;
                };
                let mut first_selection = None;
                let mut multiple_selections = false;
                checked
                    .facts
                    .operators
                    .for_each_resolved_expression_selection(
                        expression,
                        |origin, target, policy, _| {
                            if source.matches_checked_origin(checked, origin) {
                                if first_selection.is_none() {
                                    first_selection = Some((target, policy));
                                } else {
                                    multiple_selections = true;
                                }
                            }
                        },
                    );
                let Some((target, policy)) = first_selection else {
                    if typed_trees::operator::declaration_by_symbol(
                        &checked.typed,
                        source.source_target,
                    )
                    .is_some()
                    {
                        return Err("float result policy lost its checked selection");
                    }
                    continue;
                };
                if multiple_selections || target != source.source_target {
                    return Err("float result policy changed its exact checked selection");
                }
                let control = &checked.facts.flow.control;
                let target_state = checked
                    .typed
                    .machine_by_symbol(target)
                    .and_then(|machine| checked.typed.machine_states(machine).first())
                    .map_or(target, |state| state.symbol);
                let calls = control
                    .state_by_symbol(source.source_state)
                    .into_iter()
                    .flat_map(|state| control.calls.span_or_empty(state.calls))
                    .filter(|call| {
                        call.statement_index == source.statement_index
                            && call.call_ordinal == source.call_ordinal
                            && call.authored_expression == expression
                            && call.target_symbol == target_state
                    })
                    .count();
                if calls != 1 {
                    return Err("float result policy changed its source call coordinate");
                }
                let declaration = module
                    .boundary_machine(*boundary)
                    .ok_or("float result policy lost its boundary declaration")?;
                if module.identities.text(declaration.identity)
                    != Some(requirement_identity(checked, target)?.as_str())
                {
                    return Err("float result policy changed its original boundary target");
                }
                let Some(format) = policy.float_format() else {
                    continue;
                };
                let expected_format = match raw_format {
                    IeeeFloatFormat::Binary32 => FloatFormat::BINARY32,
                    IeeeFloatFormat::Binary64 => FloatFormat::BINARY64,
                };
                if format != expected_format {
                    return Err("float result policy changed its selected format");
                }
                let expected = match policy {
                    CheckedArithmeticPolicyAdapter::None => continue,
                    CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { .. } => {
                        FloatResultPolicy::Trapping { raw_result: raw.id }
                    }
                    CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly { .. } => {
                        let signature =
                            typed_trees::operator::declaration_by_symbol(&checked.typed, target)
                                .ok_or("saturating result lost its exact declaration")?;
                        let divisor = validation::float_saturation_divisor_parameter(
                            &checked.typed,
                            signature,
                        )
                        .ok_or("saturating result lost its raw semantic equation")?;
                        let adapter = block
                            .operations
                            .get(position + 1)
                            .ok_or("float call lost its result adapter")?;
                        let OperationKind::FloatResultPolicy {
                            policy:
                                FloatResultPolicy::Saturating {
                                    eligible,
                                    raw_result,
                                },
                        } = adapter.kind
                        else {
                            return Err("saturating result lost its adapter");
                        };
                        if raw_result != raw.id
                            || !saturation_eligibility::verify(
                                &machine.operations().collect::<Vec<_>>(),
                                eligible,
                                arguments,
                                &declaration.scalar_parameters,
                                divisor,
                            )
                        {
                            return Err("saturating result changed its exact input eligibility");
                        }
                        FloatResultPolicy::Saturating {
                            raw_result: raw.id,
                            eligible,
                        }
                    }
                };
                let adapter = block
                    .operations
                    .get(position + 1)
                    .ok_or("float call lost its result adapter")?;
                if adapter.kind != (OperationKind::FloatResultPolicy { policy: expected }) {
                    return Err("float call changed or omitted its exact result adapter");
                }
                let adapted = adapter
                    .result
                    .scalar()
                    .ok_or("float result adapter has no scalar result")?;
                if adapted.id == raw.id
                    || adapted.scalar_type != raw.scalar_type
                    || !adapted.qualifications.is_empty()
                    || !raw.qualifications.is_empty()
                {
                    return Err("float result adapter changed its result carrier");
                }
                // SSA values may leave this block only through its terminator.
                // No executable use may observe the raw result before or after
                // the adapter; proof-only mentions do not execute it again.
                let mut bypass = false;
                for consumer in &block.operations {
                    if consumer.id == adapter.id {
                        continue;
                    }
                    consumer.kind.clone().map_scalar_uses(&mut |value| {
                        bypass |= value == raw.id;
                        value
                    });
                }
                block.terminator.clone().map_scalar_uses(&mut |value| {
                    bypass |= value == raw.id;
                    value
                });
                if bypass {
                    return Err("float boundary raw result bypasses its policy adapter");
                }
                matched += 1;
            }
        }
    }
    if matched != actual || matched != source_policies {
        return Err("float result adapters do not match the exact source call roster");
    }
    Ok(())
}

fn requirement_identity(
    checked: &CheckedTrees,
    target: symbols::SymbolHandle,
) -> Result<String, &'static str> {
    if let Some(operator) = checked
        .typed
        .operators()
        .iter()
        .find(|operator| operator.is_boundary && operator.symbol == target)
    {
        return Ok(
            typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator),
        );
    }
    let machine = checked
        .typed
        .machine_by_symbol(target)
        .ok_or("float call lost its source requirement")?;
    if machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        || (machine.supply_mode == language_semantics::MachineSupplyMode::Boundary
            && machine.service_reach_is_installation_bound)
    {
        return checked
            .typed
            .machine_callable_identity_text(machine)
            .ok_or("float call lost its normalized requirement identity");
    }
    Ok(machine.name.as_str().to_owned())
}
