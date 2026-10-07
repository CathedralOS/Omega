//! Source custody of post-call policies, independent of stage 05's emitter.
//!
//! A policy is not another provider call. It belongs immediately after the
//! exact original call, consumes its raw result and ordered operands, and is
//! the sole executable user of that raw result. Checking this relationship
//! prevents a valid standalone adapter from disguising a missing, substituted
//! or bypassed source policy. The artifact identity commits the checked pair.

use checked_trees::{CheckedArithmeticPolicyAdapter, CheckedTrees, NominalMachineUseSite};
use language_core::operator_spelling::OperatorSpelling;
use lowered_psi::LoweredPsi;
use numerics::float_semantics::FloatFormat;
use semantic_vocabulary::{IeeeFloatFormat, ScalarType};
use terminal_psi::{FloatResultPolicy, FloatSaturationRule, OperationKind};

pub(super) fn replay(checked: &CheckedTrees, lowered: &LoweredPsi) -> Result<(), &'static str> {
    // Surviving call companions are retained by Psi optimization. Reconcile
    // both directions: deleting a call and its adapter must not leave a
    // recorded policy-bearing source occurrence without an executable pair.
    let source_policies = lowered
        .source_call_occurrences
        .iter()
        .filter(|source| {
            let Some(NominalMachineUseSite::Expression(expression)) = source.source_site else {
                return false;
            };
            checked
                .facts
                .operators
                .resolved_expression_selections()
                .any(|(candidate, origin, target, policy)| {
                    candidate == expression
                        && target == source.source_target
                        && policy != CheckedArithmeticPolicyAdapter::None
                        && super::named_call_origin_matches(checked, origin, source)
                })
        })
        .count();
    let mut matched = 0;
    let mut actual = 0;
    for machine in &lowered.semantic_module.machines {
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
                let ScalarType::IeeeFloat(raw_format) = raw.scalar_type else {
                    continue;
                };
                let mut sources = lowered
                    .source_call_occurrences
                    .iter()
                    .filter(|source| source.terminal_site.operation() == Some(operation.id));
                let (Some(source), None) = (sources.next(), sources.next()) else {
                    return Err("float boundary call has no unique source occurrence");
                };
                let Some(NominalMachineUseSite::Expression(expression)) = source.source_site else {
                    continue;
                };
                let mut selections = checked
                    .facts
                    .operators
                    .resolved_expression_selections()
                    .filter(|(candidate, origin, _, _)| {
                        *candidate == expression
                            && super::named_call_origin_matches(checked, *origin, source)
                    });
                let Some((_, _, target, policy)) = selections.next() else {
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
                if selections.next().is_some() || target != source.source_target {
                    return Err("float result policy changed its exact checked selection");
                }
                let control = &checked.facts.flow.control;
                let target_state = checked
                    .typed
                    .machine_by_symbol(target)
                    .and_then(|machine| checked.typed.machine_states(machine).first())
                    .map_or(target, |state| state.symbol);
                let calls = control
                    .states
                    .iter()
                    .filter(|(_, state)| state.state_symbol == source.source_state)
                    .flat_map(|(_, state)| control.calls.span_or_empty(state.calls))
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
                let declaration = lowered
                    .semantic_module
                    .boundary_machines
                    .iter()
                    .find(|declaration| declaration.id == *boundary)
                    .ok_or("float result policy lost its boundary declaration")?;
                if declaration.identity != requirement_identity(checked, target)? {
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
                        let rule = match signature.spelling {
                            Some(OperatorSpelling::Divide) => {
                                FloatSaturationRule::FiniteDivisionOverflow
                            }
                            Some(
                                OperatorSpelling::Add
                                | OperatorSpelling::Subtract
                                | OperatorSpelling::Multiply,
                            ) => FloatSaturationRule::FiniteOperandOverflow,
                            _ => return Err("saturating result lacks exact overflow semantics"),
                        };
                        let [left, right] = arguments.as_slice() else {
                            return Err("saturating result lost its binary operands");
                        };
                        FloatResultPolicy::Saturating {
                            raw_result: raw.id,
                            left: *left,
                            right: *right,
                            rule,
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
