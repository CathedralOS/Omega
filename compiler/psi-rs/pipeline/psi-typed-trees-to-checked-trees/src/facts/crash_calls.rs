use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;

use super::{encode_contract_fact_canonical, encode_expression_canonical, is_true_crash_route};

/// Materialize direct invocation-specific crash refinement while the typed
/// expressions are still available. The retained rows are entirely checked
/// data: downstream propagation can distinguish a proved-crash-free call from
/// an unexamined call without reopening source trees.
pub(super) fn attach_checked_crash_calls(
    program: &TypedTrees,
    flow: &psi_checked_trees::FlowFacts,
    content_conservation: &[psi_validation::ContentConservationSourcePlan],
    plans: &mut [psi_checked_trees::MachineContractPlan],
) {
    let mut calls_by_caller =
        Vec::<(SymbolHandle, Vec<psi_checked_trees::CheckedCrashCallSite>)>::new();
    for (_, state_flow) in flow.control.states.iter() {
        let caller_parameter_names = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == state_flow.machine_symbol)
            .and_then(|machine| program.machine_states(machine).first())
            .map(|entry| {
                program
                    .state_parameters(entry)
                    .iter()
                    .map(|parameter| parameter.name.as_str().to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for call_flow in flow.control.calls.span_or_empty(state_flow.calls) {
            let Some((target_machine_symbol, target_state_symbol)) =
                crate::contract_target_from_state_symbol(program, call_flow.target_symbol)
            else {
                continue;
            };
            let Some(target_machine) = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == target_machine_symbol)
            else {
                // Trait requirements and static machine parameters do not own
                // a local machine plan. Their pinned capsule is a later slice.
                continue;
            };
            let Some(target_plan) = plans
                .iter()
                .find(|plan| plan.machine == target_machine_symbol)
            else {
                continue;
            };
            if target_plan.crash.published().is_empty() {
                continue;
            }
            let Some(target_state) = program
                .machine_states(target_machine)
                .iter()
                .find(|state| state.symbol == target_state_symbol)
            else {
                continue;
            };
            let Some(call_site) = crate::find_call_site(
                program,
                state_flow.machine_symbol,
                state_flow.state_symbol,
                call_flow.statement_index,
                call_flow.call_ordinal,
            ) else {
                continue;
            };
            if matches!(call_site, crate::CallSite::TransitionNamed(_)) {
                // A named transition transfers within the current machine; it
                // is not an invocation of that machine's public crash ceiling.
                continue;
            }
            let target_parameters = program.state_parameters(target_state);
            let target_parameter_names = program
                .machine_states(target_machine)
                .first()
                .map(|entry| {
                    program
                        .state_parameters(entry)
                        .iter()
                        .map(|parameter| parameter.name.as_str().to_owned())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let route_expressions = crash_route_expressions_by_identity(
                program,
                target_machine,
                &target_parameter_names,
                content_conservation,
            );
            let arguments = crate::call_site_argument_expressions(program, &call_site);
            let mut surviving_buckets = Vec::new();
            for bucket in target_plan.crash.published() {
                let mut surviving_guards = Vec::new();
                for guard in bucket.alternative_guards() {
                    match guard {
                        psi_checked_trees::CrashRouteGuard::Truth => {
                            surviving_guards.push(psi_checked_trees::CrashRouteGuard::Truth);
                        }
                        psi_checked_trees::CrashRouteGuard::Predicate(identity) => {
                            let expression = *route_expressions.get(identity).expect(
                                "a canonical published crash route retains its typed producer expression",
                            );
                            match crate::checks::contracts::call_site_boolean_contract_expression_value(
                                program,
                                state_flow,
                                call_flow,
                                &call_site,
                                target_state_symbol,
                                target_parameters,
                                expression,
                            ) {
                                Some(false) => {}
                                Some(true) => surviving_guards
                                    .push(psi_checked_trees::CrashRouteGuard::Truth),
                                None => surviving_guards.push(
                                    psi_checked_trees::CrashRouteGuard::Predicate(
                                        canonical_instantiated_crash_route(
                                            program,
                                            expression,
                                            target_parameters,
                                            arguments,
                                            &caller_parameter_names,
                                            content_conservation,
                                        ),
                                    ),
                                ),
                            }
                        }
                    }
                }
                if let Some(bucket) = psi_checked_trees::CrashRouteBucket::new(
                    bucket.cause(),
                    bucket.containment_demand(),
                    surviving_guards,
                ) {
                    surviving_buckets.push(bucket);
                }
            }
            let caller_index = calls_by_caller
                .iter()
                .position(|(machine, _)| *machine == state_flow.machine_symbol)
                .unwrap_or_else(|| {
                    calls_by_caller.push((state_flow.machine_symbol, Vec::new()));
                    calls_by_caller.len() - 1
                });
            calls_by_caller[caller_index]
                .1
                .push(psi_checked_trees::CheckedCrashCallSite::new(
                    psi_checked_trees::CrashCallSiteLocation::new(
                        state_flow.state_symbol,
                        u32::try_from(call_flow.statement_index)
                            .expect("statement ordinal exceeds checked crash-call identity range"),
                        u32::try_from(call_flow.call_ordinal)
                            .expect("call ordinal exceeds checked crash-call identity range"),
                    ),
                    target_machine_symbol,
                    target_state_symbol,
                    target_plan.fingerprint,
                    surviving_buckets,
                ));
        }
    }

    for plan in plans {
        let checked_calls = calls_by_caller
            .iter_mut()
            .find(|(machine, _)| *machine == plan.machine)
            .map(|(_, calls)| std::mem::take(calls))
            .unwrap_or_default();
        plan.crash = plan
            .crash
            .clone()
            .with_checked_calls(checked_calls)
            .expect("one checked crash-call record occupies each invocation coordinate");
    }
}

fn crash_route_expressions_by_identity(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    parameter_names: &[String],
    content_conservation: &[psi_validation::ContentConservationSourcePlan],
) -> std::collections::BTreeMap<
    psi_checked_trees::CrashPredicateIdentity,
    psi_typed_trees::expression::ExpressionHandle,
> {
    let mut expressions = std::collections::BTreeMap::new();
    for contract in program.machine_contracts(machine) {
        if !matches!(
            contract.kind,
            psi_typed_trees::signature::SignatureContractKind::Crashes { .. }
        ) {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let psi_typed_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            if is_true_crash_route(program, fact) {
                continue;
            }
            let mut bytes = Vec::new();
            encode_contract_fact_canonical(
                program,
                fact,
                parameter_names,
                content_conservation,
                false,
                &mut bytes,
            );
            expressions.insert(
                psi_checked_trees::CrashPredicateIdentity::from_canonical_bytes(bytes),
                *expression,
            );
        }
    }
    expressions
}

/// Canonicalize a callee route after replacing bare formal parameters with
/// this invocation's argument expressions. The result lives in the caller's
/// positional namespace, so parameter renames on either side remain
/// irrelevant and checked consumers need no source handles.
fn canonical_instantiated_crash_route(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    target_parameters: &[psi_typed_trees::signature::StateParameter],
    arguments: &[psi_typed_trees::expression::ExpressionHandle],
    caller_parameter_names: &[String],
    content_conservation: &[psi_validation::ContentConservationSourcePlan],
) -> psi_checked_trees::CrashPredicateIdentity {
    let mut bytes = vec![1]; // ProofFact::Expression
    encode_instantiated_crash_expression(
        program,
        expression,
        target_parameters,
        arguments,
        caller_parameter_names,
        content_conservation,
        &mut bytes,
    );
    psi_checked_trees::CrashPredicateIdentity::from_canonical_bytes(bytes)
}

fn encode_instantiated_crash_expression(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    target_parameters: &[psi_typed_trees::signature::StateParameter],
    arguments: &[psi_typed_trees::expression::ExpressionHandle],
    caller_parameter_names: &[String],
    content_conservation: &[psi_validation::ContentConservationSourcePlan],
    out: &mut Vec<u8>,
) {
    use psi_typed_trees::expression::ExpressionNode;

    if let Some(conservation) = content_conservation
        .iter()
        .find(|candidate| candidate.source_expression == expression)
    {
        out.push(0xcc);
        out.extend(
            psi_language_semantics::content::content_conservation_plan_bytes(&conservation.plan),
        );
        return;
    }
    if !expression.is_valid() {
        out.push(0);
        return;
    }

    if let ExpressionNode::Name(path) = program.expression_table.expression(expression) {
        let name = program
            .expression_table
            .name_path_members(path.members)
            .last()
            .map(|name| name.as_str());
        let mut argument_index = 0usize;
        for parameter in target_parameters {
            let matches = (path.head_symbol.is_valid() && path.head_symbol == parameter.symbol)
                || (path.symbol.is_valid() && path.symbol == parameter.symbol)
                || name.is_some_and(|name| name == parameter.name.as_str());
            if parameter.is_self {
                if matches {
                    break;
                }
                continue;
            }
            let argument = arguments.get(argument_index).copied();
            argument_index = argument_index.saturating_add(1);
            if matches {
                if let Some(argument) = argument {
                    encode_expression_canonical(program, argument, caller_parameter_names, out);
                    return;
                }
                break;
            }
        }
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            out.push(1);
            out.push(binary.operator as u8);
            encode_instantiated_crash_expression(
                program,
                binary.left,
                target_parameters,
                arguments,
                caller_parameter_names,
                content_conservation,
                out,
            );
            encode_instantiated_crash_expression(
                program,
                binary.right,
                target_parameters,
                arguments,
                caller_parameter_names,
                content_conservation,
                out,
            );
        }
        ExpressionNode::Unary(unary) => {
            out.push(2);
            out.push(unary.operator as u8);
            encode_instantiated_crash_expression(
                program,
                unary.operand,
                target_parameters,
                arguments,
                caller_parameter_names,
                content_conservation,
                out,
            );
        }
        ExpressionNode::Integer(value) => {
            out.push(3);
            out.extend(value.text().as_bytes());
            out.push(0);
        }
        ExpressionNode::Boolean(value) => {
            out.push(4);
            out.push(u8::from(*value));
        }
        ExpressionNode::Name(path) => {
            out.push(5);
            for member in program.expression_table.name_path_members(path.members) {
                out.extend(member.as_str().as_bytes());
                out.push(b'.');
            }
            out.push(0);
        }
        ExpressionNode::Member(member) => {
            out.push(6);
            encode_instantiated_crash_expression(
                program,
                member.receiver,
                target_parameters,
                arguments,
                caller_parameter_names,
                content_conservation,
                out,
            );
            out.extend(member.member.as_str().as_bytes());
            out.push(0);
        }
        ExpressionNode::Call(call) => {
            out.push(7);
            out.extend(call.target.as_str().as_bytes());
            out.push(0);
            encode_instantiated_crash_expression(
                program,
                call.receiver,
                target_parameters,
                arguments,
                caller_parameter_names,
                content_conservation,
                out,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                encode_instantiated_crash_expression(
                    program,
                    *argument,
                    target_parameters,
                    arguments,
                    caller_parameter_names,
                    content_conservation,
                    out,
                );
            }
            out.push(0xfe);
        }
        other => {
            let _ = other;
            out.push(8);
            out.extend(program.expression_table.display_name(expression).as_bytes());
            out.push(0);
        }
    }
}
