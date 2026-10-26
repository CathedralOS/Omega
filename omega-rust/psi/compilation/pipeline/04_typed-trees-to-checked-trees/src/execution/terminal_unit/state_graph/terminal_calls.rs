//! Terminal machine calls in transition arms
//! ([state contracts](../../../../../../../../../wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms)).
//!
//! An arm is classified by its resolved target: a local state is an internal
//! jump (`successor` plans its edge), as is a machine entry retained in the
//! recursive component. A destination outside that component is an ordinary
//! invocation whose normal result completes this machine. Two placements
//! exist, and neither bypasses the ordinary call plan:
//!
//! - An unconditional final arm (`transition { _ -> self.finish() }`) is the
//!   state's last operation. The shared statement sequence plans it through
//!   `call_occurrences::terminal_call`, exactly as a single-state body
//!   does, so its occurrence counts against the state's call roster and its
//!   value result binds as the state's returned value; `completion` then
//!   completes the state with that result, whatever its kind.
//! - A guarded arm's call runs only when its arm is selected, so it cannot
//!   join the state's linear operations. `arm_call` plans it with the same
//!   `build_call_operation` as the arm's own operation
//!   (`CheckedConditionalReturnArm::Call`); emission places it in the arm's
//!   private completion block.
//!
//! Its subslice operands replay at the call-argument site the checker records
//! for an invocation (`values::scalar::expression_plans::transition`), like an
//! expression call's.
//!
//! The callee's normal result completes this machine. A scalar result is the
//! call's ordinary scalar binding: the unconditional arm binds it as the
//! state's returned value
//! (`CheckedScalarReturnPlan::Binding` at the transition), and a guarded arm
//! binds it at the next ordinal of the namespace its arm inherits and returns
//! it there. A structural result is likewise the unconditional call's own
//! binding, which the state returns (`ReturnStructural`). The two signatures
//! must agree exactly; checking rejects a terminal call whose result differs
//! from the caller's declared result.
//!
//! Scalar operand computations retain their nested calls. Structural operands
//! retain the ordinary preparation sequence; the shared argument schedule
//! evaluates both inside the selected arm in authored order.
//! Structural results use the ordinary call binding after operand preparation;
//! completion transfers that value instead of discarding it. An arm call still
//! retains ordinary exact claim transfers into the callee.
use super::{
    CheckFacts, CheckedComposedUnitControlTerminatorPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    ScalarCalleeClosure, TransitionExit, TransitionTargetNode, control,
};
use crate::execution::terminal_unit::calls::{ExpectedCallValueResult, build_call_operation};
use crate::execution::terminal_unit::control::{
    bind_scalar_call_result, bind_structural_call_result, checked_structural_result_type,
};
use crate::execution::terminal_unit::types::{ShapeCollector, machine_binders, state_flow};

/// The exact callable destination an arm invokes. Local state transfers and
/// entry re-entry remain state-graph edges; requirements keep their declaration
/// identity and ordinary machines keep their executable state identity.
pub(super) fn invoked_callable(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    transition: &typed_trees::statement::TableTransition,
) -> Option<symbols::SymbolHandle> {
    if transition.exit != TransitionExit::Ordinary
        || transition.continuation.is_valid()
        || !program
            .statement_table
            .transition_target_is_valid(transition.target)
    {
        return None;
    }
    let target = control::terminal_target_symbol(program, transition.target)?;
    validation::terminal_callable_target(program, machine, target).map(|(target, _)| target)
}

/// Only a destination outside the retained graph needs a call operation.
/// A fused machine entry still has its authored call contract, but execution
/// transfers to its retained state instead of allocating another activation.
pub(super) fn external_callable(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    fused: &super::FusedStates<'_>,
    transition: &typed_trees::statement::TableTransition,
) -> Option<symbols::SymbolHandle> {
    invoked_callable(program, machine, transition)
        .filter(|target| super::fused_target_index(program, fused, *target).is_none())
}

/// Complete a state whose unconditional final arm invokes a machine outside
/// the retained graph. The statement sequence already planned that call as
/// the state's last operation (statement `statement_ordinal`, the
/// statement's only outer call) and bound its value result as the state's
/// returned value. The callee's normal result completes this machine
/// unchanged: Unit returns, a scalar returns its binding, and a structural
/// value transfers its binding.
pub(super) fn completion(
    operations: &[CheckedUnitEffectOperationPlan],
    statement_ordinal: u32,
    result: &checked_trees::CheckedControlResultPlan,
    scalar_result: Option<&checked_trees::CheckedUnitScalarResultBindingPlan>,
    structural_result: Option<&checked_trees::CheckedUnitStructuralReturnPlan>,
) -> Option<CheckedComposedUnitControlTerminatorPlan> {
    let mut calls = operations
        .iter()
        .filter_map(CheckedUnitEffectOperationPlan::call)
        .filter(|call| {
            call.coordinate.statement_index == statement_ordinal
                && call.coordinate.call_ordinal == 0
        });
    let call = calls.next()?;
    if calls.next().is_some() {
        return None;
    }
    if *call.result() == checked_trees::CheckedUnitCallResult::NoNormal {
        return Some(CheckedComposedUnitControlTerminatorPlan::InvokeNoNormal {
            coordinate: call.coordinate,
        });
    }
    match (result, call.result()) {
        (
            checked_trees::CheckedControlResultPlan::Unit,
            checked_trees::CheckedUnitCallResult::Unit,
        ) => Some(CheckedComposedUnitControlTerminatorPlan::ReturnUnit),
        (
            checked_trees::CheckedControlResultPlan::Scalar { primitive_type },
            checked_trees::CheckedUnitCallResult::Scalar(bound),
        ) if bound.primitive_type == *primitive_type
            && bound.statement_index == statement_ordinal
            && scalar_result == Some(bound) =>
        {
            Some(CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                completion: checked_trees::CheckedScalarReturnPlan::Binding(*bound),
            })
        }
        (
            checked_trees::CheckedControlResultPlan::Structural(expected),
            checked_trees::CheckedUnitCallResult::Structural { binding, .. },
        ) if binding.statement_index == statement_ordinal
            && binding.type_identity == expected.type_identity
            && binding.multiplicity == expected.multiplicity =>
        {
            let returned = structural_result?;
            (returned.source
                == CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: binding.binding_ordinal,
                })
            .then(|| {
                let mut returned = returned.clone();
                returned.completion = checked_trees::CheckedStructuralReturnCompletion::Call {
                    coordinate: call.coordinate,
                };
                CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result: returned }
            })
        }
        _ => None,
    }
}

/// Plan a guarded arm's terminal machine call as the arm's own operation.
/// The outer flow call names this exact transition target after the guard's
/// earlier occurrences. Operand computations and structural preparation retain
/// their own calls and execute inside this arm's completion block.
///
/// A scalar-result machine forwards the callee's result: the call binds it
/// at `next_scalar_binding`, the ordinal the arm's namespace (the state's
/// retained bindings) gives its next result, and the arm returns that value.
#[allow(clippy::too_many_arguments)]
pub(super) fn arm_call(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
    local_results: &[(
        checked_trees::CheckedUnitStructuralResultBindingPlan,
        facts::PlaceRoot,
    )],
    preamble: &[checked_trees::CheckedUnitEffectOperationPlan],
    result: &checked_trees::CheckedControlResultPlan,
    next_scalar_binding: usize,
    transition: &typed_trees::statement::TableTransition,
    statement_ordinal: u32,
    trace: &control::LocalConstructionTrace,
) -> Option<checked_trees::CheckedConditionalReturnArm> {
    let target = invoked_callable(program, machine, transition)?;
    trace.phase("state graph: terminator: terminal call arm: completion");
    trace.statement(Some(statement_ordinal));
    // Bind the receiving result, including its checked qualifications. The
    // ordinary call builder checks the callee's carrier/access/multiplicity;
    // completed-call guarantees, not syntactic qualified-type equality,
    // justify the stronger receiving facts and exact returned claims.
    let no_normal = validation::call_target_state(program, target)
        .is_some_and(|state| validation::normal_completion_possible(program, state) == Some(false));
    let (scalar_result, mut structural_result) = if no_normal {
        (None, None)
    } else {
        match result {
            checked_trees::CheckedControlResultPlan::Unit => (None, None),
            checked_trees::CheckedControlResultPlan::Scalar { primitive_type } => (
                Some(checked_trees::CheckedUnitScalarResultBindingPlan {
                    statement_index: statement_ordinal,
                    binding_ordinal: u32::try_from(next_scalar_binding).ok()?,
                    primitive_type: *primitive_type,
                }),
                None,
            ),
            checked_trees::CheckedControlResultPlan::Structural(expected) => {
                let binding = checked_structural_result_type(
                    program,
                    shapes,
                    state.return_type,
                    &machine_binders(program, machine),
                )?;
                if binding.type_identity != expected.type_identity
                    || binding.multiplicity != expected.multiplicity
                {
                    return None;
                }
                (None, Some(binding))
            }
        }
    };
    trace.phase("state graph: terminator: terminal call arm: occurrence");
    trace.statement(Some(statement_ordinal));
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span_or_empty(flow.calls);
    let mut occurrences = calls.iter().filter(|call| {
        call.statement_index == statement_ordinal as usize
            && !facts.flow.control.is_retired(state.symbol, call)
            && match (
                program.statement_table.transition_target(transition.target),
                validation::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    call.statement_index,
                    call.call_ordinal,
                ),
            ) {
                (
                    TransitionTargetNode::Named { .. },
                    Some(validation::CallSite::TransitionNamed { .. }),
                ) => true,
                (
                    TransitionTargetNode::Value(expected),
                    Some(validation::CallSite::Expression { expression, .. }),
                ) => *expected == expression,
                _ => false,
            }
            && validation::transition_call_target(
                program,
                machine,
                state,
                call.statement_index,
                call.call_ordinal,
            ) == Some(transition.target)
    });
    let call = occurrences.next()?;
    // Flow custody retains the resolved authored symbol, which can name the
    // machine rather than its entry state. Compare executable destinations
    // without replacing that source identity in the ordinary call fact.
    if occurrences.next().is_some()
        || validation::terminal_callable_target(program, machine, call.target_symbol)
            .is_none_or(|(selected, _)| selected != target)
    {
        return None;
    }
    trace.phase("state graph: terminator: terminal call arm: operands");
    let mut structural_results = local_results.to_vec();
    let mut preparation = preamble.to_vec();
    let predecessor_count = preparation.len();
    let mut structural_count = super::returns::next_result_ordinal(preamble)?;
    let completion = control::structural_operands::plan_call_operands(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        state,
        structural,
        &[],
        entry_claims,
        &mut structural_results,
        &mut preparation,
        &mut structural_count,
        call,
        trace,
        &mut crate::execution::terminal_unit::borrowed_windows::OpenWindows::default(),
    )?;
    if completion == checked_trees::CheckedValueCompletion::NoNormal {
        return Some(
            checked_trees::CheckedConditionalReturnArm::EvaluateNoNormal {
                statement_ordinal,
                operations: checked_trees::CheckedOperandSequence::new(
                    preparation.split_off(predecessor_count),
                ),
            },
        );
    }
    if let Some(binding) = &mut structural_result {
        binding.statement_index = statement_ordinal;
        binding.binding_ordinal = u32::try_from(structural_count).ok()?;
    }
    trace.phase("state graph: terminator: terminal call arm: operand occurrences");
    trace.statement(Some(statement_ordinal));
    control::call_owns_statement_computations(
        program,
        facts,
        machine.symbol,
        state.symbol,
        call,
        calls,
        preamble,
    )?;
    trace.phase("state graph: terminator: terminal call arm: call operation");
    trace.statement(Some(statement_ordinal));
    let operation = build_call_operation(
        program,
        facts,
        Some(scalar_callees),
        machine,
        state,
        structural,
        &[],
        call,
        structural_result
            .as_ref()
            .map(ExpectedCallValueResult::Structural)
            .or_else(|| {
                scalar_result.map(|result| ExpectedCallValueResult::Scalar(result.primitive_type))
            }),
        &structural_results,
        &preparation,
        trace,
    )?;
    trace.phase("state graph: terminator: terminal call arm: result binding");
    let planned = operation.call()?;
    if planned.coordinate.statement_index != statement_ordinal {
        return None;
    }
    if *planned.result() == checked_trees::CheckedUnitCallResult::NoNormal {
        control::statement_sequence::consume_results(&mut preparation, &operation)?;
        preparation.drain(..predecessor_count);
        return Some(checked_trees::CheckedConditionalReturnArm::Call {
            operands: checked_trees::CheckedOperandSequence::new(preparation),
            operation,
            completions: checked_trees::CheckedOperandSequence::new(Vec::new()),
        });
    }
    let operation = match (scalar_result, structural_result) {
        (Some(result), None) => bind_scalar_call_result(facts, operation, result, true)?,
        (None, Some(result)) => {
            let mut operation = bind_structural_call_result(
                program,
                facts,
                machine.symbol,
                state.symbol,
                operation,
                result.clone(),
            )?;
            let (binding, discard) = operation.call_mut()?.structural_result_mut()?;
            if *binding != result {
                return None;
            }
            *discard = false;
            operation
        }
        (None, None) if *planned.result() == checked_trees::CheckedUnitCallResult::Unit => {
            operation
        }
        _ => return None,
    };
    trace.phase("state graph: terminator: terminal call arm: result consumption");
    control::statement_sequence::consume_results(&mut preparation, &operation)?;
    // The arm's result completes its caller, but that does not extend its
    // operand loans into caller cleanup. Retain the same normal-return
    // transaction as a statement or nested value call, on this selected arm.
    let completions = super::super::cleanup::anonymous::normal_call_completion(
        program,
        facts,
        shapes,
        machine,
        state,
        &structural_results,
        &mut preparation,
        &operation,
        trace,
    )?;
    preparation.drain(..predecessor_count);
    Some(checked_trees::CheckedConditionalReturnArm::Call {
        operands: checked_trees::CheckedOperandSequence::new(preparation),
        operation,
        completions: checked_trees::CheckedOperandSequence::new(completions),
    })
}
