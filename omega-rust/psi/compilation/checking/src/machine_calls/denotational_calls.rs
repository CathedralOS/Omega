//! Shared eligibility for calls whose result is used denotationally.
//!
//! Quotient operations and fact-position call projections must agree on the
//! operational meaning of "pure and unconditionally terminating".  This
//! module consumes the existing whole-program summaries; it never performs a
//! second expression-local effect inference. Concrete calls extend the shared
//! checked-machine walk; exact sealed total value operators close a leaf.
//! A total, value-only static callable closes a leaf through its exact public
//! contract. Unresolved calls supply no denotational authority.

use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

mod normal_return;
mod specification;
pub use normal_return::normal_return_call_candidate;
pub(crate) use normal_return::plain_value_call_target;
pub(crate) use specification::validate_specification_calls;
pub use specification::{SpecificationCallRequest, admit_specification_calls};

/// A static callable's exact public telescope can close a value-only leaf.
/// This consumes the declared promise; it never inspects a selected private
/// implementation or supplies an unresolved caller with stronger guarantees.
pub(crate) fn abstract_value_call_signature(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
) -> Option<&typed_trees::signature::StateSignature> {
    let contract = program.retained_static_machine_contract(target)?;
    let signature = contract.signature();
    if !matches!(
        &signature.termination_guarantee,
        language_semantics::TerminationGuarantee::Terminates { premises }
            if premises.is_empty()
    ) || !specification::signature_has_closed_value_effects(program, signature)
    {
        return None;
    }
    let proof_only = crate::proof_only_classification(program);
    let forbidden = |reference| {
        crate::proof_contracts::quotients::type_has_forbidden_denotational_content(
            program,
            &proof_only,
            reference,
        )
    };
    if program
        .state_signature_parameters(signature)
        .iter()
        .any(|parameter| forbidden(parameter.type_reference))
        || (signature.return_type.is_valid() && forbidden(signature.return_type))
    {
        return None;
    }
    Some(signature)
}

/// A concrete value application's operational eligibility, before its checked
/// termination summary exists. Specification formation retains the exact call
/// request and completes that obligation with the same checking invocation's
/// termination facts; conditional runtime result capture has a different owner.
pub fn checked_value_call_candidate(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    operational: &flow_effects::OperationalPlan,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
) -> Result<(), &'static str> {
    if let Some(memos) = crate::frozen_program::frozen_program_memos(program) {
        let key = (machine.symbol, state.symbol);
        if let Ok(candidates) = memos.checked_value_call_candidates.lock()
            && let Some(result) = candidates.get(&key).copied()
        {
            return result;
        }
        let result =
            unchecked_value_call_candidate(program, machine, state, operational, service_reaches);
        if let Ok(mut candidates) = memos.checked_value_call_candidates.lock() {
            candidates.insert(key, result);
        }
        return result;
    }
    unchecked_value_call_candidate(program, machine, state, operational, service_reaches)
}

fn unchecked_value_call_candidate(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    operational: &flow_effects::OperationalPlan,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
) -> Result<(), &'static str> {
    if !has_pure_effect_closure(
        program,
        machine.symbol,
        state.symbol,
        program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.is_mutable),
        operational,
        service_reaches,
    ) {
        return Err("the selected closure has effects, reaches, or unresolved call targets");
    }
    if !has_observation_free_checked_closure(program, machine.symbol, operational) {
        return Err("the selected closure contains runtime observations or mutable storage");
    }
    if !has_no_crash_routes(program, machine.symbol, operational) {
        return Err("the selected closure publishes a crash route");
    }
    Ok(())
}

/// Whether every call inside this expression denotes one value per spelling,
/// so two equal terms name one observation. A call that is neither a
/// repeatable value-only normal return nor an already-admitted denotational
/// projection produces a fresh observation on each invocation, and two
/// identical spellings are then two different values. Callers that compare
/// terms built from concrete operands must decline such an expression:
/// `an empty write frame alone does not make a call repeatable`.
///
/// `admitted` reports calls a consumer already holds a denotational record
/// for, so a stage with its own projection table does not re-derive identity
/// here. Pass `&|_| false` with no such table.
pub fn expression_values_have_call_identity(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    operational: &flow_effects::OperationalPlan,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    admitted: &dyn Fn(ExpressionHandle) -> bool,
) -> bool {
    let mut pending = vec![expression];
    let mut seen = Vec::new();
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            return false;
        }
        if seen.contains(&expression) {
            continue;
        }
        seen.push(expression);
        let node = program.expression_table.expression(expression);
        if let ExpressionNode::Call(call) = node
            && !admitted(expression)
            && normal_return_call_candidate(program, call, operational, service_reaches).is_err()
            && !super::fact_call_projections::has_checked_denotational_call_identity(
                program,
                call,
                operational,
                service_reaches,
            )
        {
            return false;
        }
        crate::expression_traversal::children(program, node, |child| {
            if child.is_valid() {
                pending.push(child);
            }
        });
    }
    true
}

/// Reachable-machine DFS over `operational`'s checked call targets from
/// `machine_symbol`; `admits` returning `false` — or a machine the index
/// cannot resolve — refuses the whole walk.
fn each_reachable_checked_machine(
    program: &dyn crate::CheckingInput,
    machine_symbol: symbols::SymbolHandle,
    operational: &flow_effects::OperationalPlan,
    admits: impl Fn(&dyn crate::CheckingInput, &typed_trees::machine::Machine) -> bool,
) -> bool {
    let mut pending = vec![machine_symbol];
    let mut visited = Vec::new();
    while let Some(current) = pending.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.push(current);
        let Some(machine) = program.machine(current) else {
            return false;
        };
        if !admits(program, machine)
            || !push_checked_machine_call_targets(program, operational, current, &mut pending)
        {
            return false;
        }
    }
    true
}

pub(crate) fn has_observation_free_checked_closure(
    program: &dyn crate::CheckingInput,
    machine_symbol: symbols::SymbolHandle,
    operational: &flow_effects::OperationalPlan,
) -> bool {
    each_reachable_checked_machine(program, machine_symbol, operational, |program, machine| {
        if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            || !machine.body_is_present
            || !program.machine_owned_data(machine).is_empty()
            || !program
                .service_reach_rows
                .services(machine.service_reach_row)
                .is_empty()
        {
            if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                eprintln!(
                    "DENOTATION observation closure machine={:?} name={} supply={:?} body={} owned={} reach={}",
                    machine.symbol,
                    machine.name,
                    machine.supply_mode,
                    machine.body_is_present,
                    program.machine_owned_data(machine).len(),
                    program
                        .service_reach_rows
                        .services(machine.service_reach_row)
                        .len(),
                );
            }
            return false;
        }
        for state in program.machine_states(machine) {
            if program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.is_mutable)
            {
                if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                    eprintln!(
                        "DENOTATION observation closure machine={:?} name={} state={:?} has mutable parameters",
                        machine.symbol, machine.name, state.symbol,
                    );
                }
                return false;
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                use typed_trees::statement::StatementNode;
                // Statement calls and transition arguments retain their spans
                // in the statement table, independently of expression calls.
                // Its root query also owns optional continuation traversal.
                let allowed = match statement {
                    StatementNode::RootBinding(_)
                    | StatementNode::Assembly(_)
                    | StatementNode::AssemblyFact(_)
                    | StatementNode::Assignment(_) => false,
                    StatementNode::LocalData(local) => !local.is_mutable,
                    StatementNode::Call(call) => !call.receiver_symbol.is_valid(),
                    StatementNode::Expression(_) | StatementNode::Transition(_) => true,
                } && program
                    .statement_table
                    .expression_roots(statement)
                    .iter()
                    .all(|(_, expression)| {
                        expression_is_fact_observation_free(program, *expression)
                    });
                if !allowed {
                    if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                        eprintln!(
                            "DENOTATION observation closure machine={:?} name={} state={:?} statement={statement:?}",
                            machine.symbol, machine.name, state.symbol,
                        );
                    }
                    return false;
                }
            }
        }
        true
    })
}

fn expression_is_fact_observation_free(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    let node = program.expression_table.expression(expression);
    let allowed = match node {
        ExpressionNode::Match(dispatch) => {
            crate::expression_traversal::match_children(program, *dispatch)
                .all(|child| expression_is_fact_observation_free(program, child))
        }
        ExpressionNode::Atomic(_) | ExpressionNode::Borrow(_) => false,
        ExpressionNode::Binary(binary) => {
            expression_is_fact_observation_free(program, binary.left)
                && expression_is_fact_observation_free(program, binary.right)
        }
        ExpressionNode::Cast(cast) => expression_is_fact_observation_free(program, cast.value),
        ExpressionNode::Call(call) => {
            // A retained declaration qualifier is not a storage observation.
            // Rejoin it to the exact selected target through the shared call
            // owner; declaration spelling or namespace kind alone is no proof.
            (!call.receiver.is_valid() || program.call_has_no_runtime_receiver_for_target(call))
                && program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .all(|argument| expression_is_fact_observation_free(program, *argument))
        }
        ExpressionNode::Indexed(indexed) => {
            expression_is_fact_observation_free(program, indexed.collection)
                && expression_is_fact_observation_free(program, indexed.index)
        }
        ExpressionNode::Member(member) => {
            expression_is_fact_observation_free(program, member.receiver)
        }
        ExpressionNode::Range(range) => {
            expression_is_fact_observation_free(program, range.start)
                && expression_is_fact_observation_free(program, range.end)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .all(|field| expression_is_fact_observation_free(program, field.value)),
        ExpressionNode::Unary(unary) => expression_is_fact_observation_free(program, unary.operand),
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .all(|value| expression_is_fact_observation_free(program, *value)),
        typed_trees::literal_expression_pattern!() => true,
    };
    if !allowed && std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
        eprintln!("DENOTATION observation expression={expression:?} node={node:?}",);
    }
    allowed
}

pub(crate) fn unconditionally_terminates(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
) -> bool {
    let Some(machine) = program.machine(machine_symbol) else {
        return false;
    };
    matches!(
        &machine.termination_plan.checked_summary,
        language_semantics::TerminationGuarantee::Terminates { premises }
            if premises.is_empty()
    )
}

/// Whole-closure purity used by denotational calls.  The selected entry must
/// have no mutable runtime parameter; its machine has no service reach,
/// suspension, or blocking behavior; and every reachable call target is one
/// exact checked machine or a sealed closed-value operator leaf.
pub(crate) fn has_pure_effect_closure(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    has_mutable_parameters: bool,
    operational: &flow_effects::OperationalPlan,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
) -> bool {
    if has_mutable_parameters {
        return false;
    }

    let machine_summaries = operational
        .machines()
        .iter()
        .filter(|summary| summary.symbol == machine_symbol)
        .collect::<Vec<_>>();
    let [machine_summary] = machine_summaries.as_slice() else {
        return false;
    };
    if machine_summary.transitive_may_suspend || machine_summary.transitive_may_block {
        return false;
    }
    if operational
        .states
        .span_or_empty(machine_summary.states)
        .iter()
        .filter(|summary| summary.symbol == state_symbol)
        .count()
        != 1
    {
        return false;
    }

    let reach_summaries = service_reaches
        .machines()
        .iter()
        .filter(|summary| summary.machine == machine_symbol)
        .collect::<Vec<_>>();
    let [reach_summary] = reach_summaries.as_slice() else {
        return false;
    };
    if !service_reaches
        .services(reach_summary.inferred_transitive)
        .is_empty()
    {
        return false;
    }

    let mut pending = vec![machine_symbol];
    let mut visited = Vec::new();
    while let Some(current) = pending.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.push(current);
        if !push_checked_machine_call_targets(program, operational, current, &mut pending) {
            return false;
        }
    }
    true
}

/// Extend the concrete DFS through exact checked machine/state edges. A sealed
/// total value operator completes a leaf under the same eligibility owner for
/// purity, observations and crashes; a boundary spelling alone never does.
/// Exact total value-only static contracts also close leaves; missing
/// summaries and unresolved targets fail closed.
fn push_checked_machine_call_targets(
    program: &dyn crate::CheckingInput,
    operational: &flow_effects::OperationalPlan,
    current: symbols::SymbolHandle,
    pending: &mut Vec<symbols::SymbolHandle>,
) -> bool {
    let Some(machine) = program.machine(current) else {
        return false;
    };
    if machine.target_family_interface {
        let Some(members) = complete_family_members(program, machine) else {
            return false;
        };
        // A family dependency is not an executable call row. Its portable
        // source judgments nevertheless owe every retained member's closure.
        pending.extend(members.iter().map(|member| member.machine));
    }
    let summaries = operational
        .machines()
        .iter()
        .filter(|summary| summary.symbol == current)
        .collect::<Vec<_>>();
    let [summary] = summaries.as_slice() else {
        return false;
    };
    for state in operational.states.span_or_empty(summary.states) {
        for call in operational.calls.span_or_empty(state.calls) {
            if call.target_machine_symbol.is_valid() {
                let Some(machine) = program.machine(call.target_machine_symbol) else {
                    return false;
                };
                if !program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == call.target_state_symbol)
                {
                    return false;
                }
                pending.push(machine.symbol);
                continue;
            }
            let parameter = if call.static_machine_parameter.is_valid() {
                call.static_machine_parameter
            } else {
                call.target_state_symbol
            };
            if program
                .machine_parameter_signature_in(machine, parameter)
                .is_some()
            {
                if !call.static_machine_bindings.is_empty()
                    || abstract_value_call_signature(program, parameter).is_none()
                {
                    return false;
                }
                continue;
            }
            // An unresolved selection cannot acquire a nearby operator's
            // denotation. Only its own closed public promise closes a leaf.
            if call.static_machine_parameter.is_valid()
                || !call.static_machine_bindings.is_empty()
                || call.direct_may_suspend
                || call.direct_may_block
                || call.transitive_may_suspend
                || call.transitive_may_block
            {
                return false;
            }
            let Some(operator) =
                typed_trees::operator::declaration_by_symbol(program, call.target_operator_symbol)
            else {
                return false;
            };
            if (call.target_state_symbol.is_valid() && call.target_state_symbol != operator.symbol)
                || !selected_operator_has_closed_value_denotation(program, operator)
            {
                return false;
            }
        }
    }
    true
}

/// A fact denotes only normal values. Any reachable published crash route
/// makes the call partial in the fact language, even though the route is
/// separately covered for executable use.
pub(crate) fn has_no_crash_routes(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    operational: &flow_effects::OperationalPlan,
) -> bool {
    each_reachable_checked_machine(program, machine_symbol, operational, |program, machine| {
        if program.machine_declares_crash(machine) {
            return false;
        }
        if !machine.target_family_interface {
            return true;
        }
        // A private member's omitted clause is inference still owed by body
        // checking, not a published negative guarantee. Source denotation has
        // no checked body-crash fixed point to substitute for that obligation.
        complete_family_members(program, machine).is_some_and(|members| {
            members.iter().all(|member| {
                program
                    .machine(member.machine)
                    .is_some_and(|machine| program.machine_publishes_crash_ceiling(machine))
            })
        })
    })
}

fn complete_family_members<'program>(
    program: &'program dyn crate::CheckingInput,
    interface: &typed_trees::machine::Machine,
) -> Option<&'program [typed_trees::machine::TargetMachineFamilyMember]> {
    let members = super::calls::target_family_members(program, interface)?;
    for member in members {
        let machine = program.machine(member.machine)?;
        if !machine.supplies_checked_body()
            || !machine.body_is_present
            || machine.body_status != typed_trees::machine::MachineBodyStatus::Complete
            || machine.structural_type_equations_pending
        {
            return None;
        }
    }
    Some(members)
}

/// Rejoin a selected public operator with its exact authored implementation.
/// The operator remains the contract owner; its body only supplies the existing
/// concrete effect and deferred termination judgments. Boundary token shells
/// and subordinate states cannot provide this implementation authority.
pub(crate) fn selected_operator_authored_body<'program>(
    program: &'program dyn crate::CheckingInput,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Option<(
    &'program typed_trees::machine::Machine,
    &'program typed_trees::state::State,
)> {
    if operator.is_boundary
        || program.operator_contracts(operator).iter().any(|contract| {
            matches!(
                contract.kind,
                typed_trees::signature::SignatureContractKind::Crashes { .. }
            )
        })
    {
        return None;
    }
    let machine = program.machine(operator.symbol)?;
    if !machine.body_is_present {
        return None;
    }
    let entry = program.machine_states(machine).first()?;
    (operator.return_type == entry.return_type
        && program.operator_parameters(operator) == program.state_parameters(entry))
    .then_some((machine, entry))
}

/// Closed compiler catalogs independently establish a total value meaning.
/// A selected runtime slot must retain its exact sealed declaration and a
/// complete catalog result equation. Ordinary boundary admission or a matching
/// function-shaped guarantee is not denotational authority.
pub fn selected_operator_has_closed_value_denotation(
    program: &dyn crate::CheckingInput,
    operator: &typed_trees::operator::OperatorDefinition,
) -> bool {
    use crate::proof_contracts::float_projection_bindings;
    // A published crash ceiling describes possible abnormal completion, not
    // a total proof value. Closed kernel result equations describe normal
    // results only and cannot erase that separate selected interface effect.
    if program.operator_contracts(operator).iter().any(|contract| {
        matches!(
            contract.kind,
            typed_trees::signature::SignatureContractKind::Crashes { .. }
        )
    }) || program.machine(operator.symbol).is_some_and(|machine| {
        machine.suspends
            || machine.blocks
            || program.machine_declares_crash(machine)
            || !program
                .service_reach_rows
                .services(machine.service_reach_row)
                .is_empty()
            || !program.machine_owned_data(machine).is_empty()
    }) {
        return false;
    }
    if program
        .operator_parameters(operator)
        .iter()
        .any(|parameter| {
            parameter.is_mutable
                || crate::reference_shell(program, parameter.type_reference).is_some()
        })
    {
        return false;
    }
    if float_projection_bindings::semantic_operations::exact_toolchain_float_semantic_contract(
        program, operator,
    )
    .is_some()
    {
        return true;
    }
    let path = program.operator_path_members(operator.name);
    if let [namespace, name] = path
        && let Some(projection) =
            numerics::float_projection::FloatProjectionOperation::from_source_identity(
                namespace.as_str(),
                name.as_str(),
            )
        && float_projection_bindings::exact_toolchain_float_projection_primitive(
            program, operator, projection,
        )
        .is_some()
    {
        return true;
    }
    float_projection_bindings::semantic_operations::is_sealed_toolchain_float_operator(
        program, operator,
    ) && (float_projection_bindings::float_result_semantic_contract(program, operator).is_some()
        || float_projection_bindings::float_scalar_semantic_contract(program, operator).is_some())
}
