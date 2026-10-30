//! Fence orderings are selected from the captured operand's live case evidence.
//!
//! The sealed declaration supplies the exact case handles. An ordinary call
//! still evaluates its operands, including effectful producers, before this
//! proof selects the event. A copied enum keeps its completion facts; no
//! initializer or later state of its former storage is substituted here.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use diagnostics::Diagnostic;
use language_core::atomic::FenceOrdering;

pub(super) fn check(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (_, state) in facts.flow.control.states.iter() {
        for call in facts.flow.control.calls.span_or_empty(state.calls) {
            let Some(contract) = contract(program, call) else {
                continue;
            };
            if let Err(reason) = ordering(program, facts, state, call, &contract) {
                let mut diagnostic = Diagnostic::error(format!(
                    "Atomic::fence requires one proof-static FenceOrdering case at operand completion: {reason}"
                ));
                if let Some(span) = call.authored_source_span {
                    diagnostic = diagnostic.with_source_span(span);
                }
                diagnostics.push(diagnostic);
            }
        }
    }
}

pub(crate) fn contract(
    program: &dyn validation::CheckingInput,
    call: &FlowCallFact,
) -> Option<validation::AtomicFenceContract> {
    let declaration = program
        .operators()
        .iter()
        .find(|declaration| declaration.symbol == call.target_symbol)?;
    validation::exact_toolchain_atomic_fence_contract(program, declaration)
}

pub(crate) fn ordering(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    contract: &validation::AtomicFenceContract,
) -> Result<FenceOrdering, &'static str> {
    if contract.declaration != call.target_symbol {
        return Err("call lost its selected fence declaration");
    }
    let arguments =
        super::contracts::call_values::captured_argument_expressions(program, facts, state, call)
            .ok_or("call lost its captured argument roster")?;
    let [expression] = arguments.as_slice() else {
        return Err("expected exactly one ordering argument");
    };
    let [operand] = facts
        .flow
        .control
        .operands
        .span(call.argument_operands())
        .ok_or("ordering capture is absent")?
    else {
        return Err("ordering capture is not unique");
    };
    if operand.expression != *expression
        || operand.state_symbol != state.state_symbol
        || operand.statement_index != call.statement_index
        || program.normalized_type_identity(operand.type_reference)
            != program.normalized_type_identity(contract.ordering_type)
    {
        return Err("ordering capture changed its expression, epoch or sealed type");
    }
    let captured = crate::flow::captured_operand_values(
        program,
        &facts.flow,
        &facts.semantic,
        state,
        crate::flow::CapturedOperand::recorded(
            &facts.flow.control,
            call.argument_operands().start(),
        )
        .ok_or("ordering capture lost its recorded identity")?,
        contract.ordering_type,
        &[],
    )
    .ok_or("ordering lost its captured value provenance")?;
    let mut candidates = contract.cases.iter().filter_map(|(case, ordering)| {
        (!captured.is_empty()
            && captured.iter().all(|value| {
                case_at_capture(
                    program,
                    facts,
                    state,
                    value.observation.row(),
                    &value.observation_projection.remaining,
                    *case,
                ) || (value.projection.remaining.is_empty()
                    && validation::structural_case_constructor(
                        program,
                        value.projection.expression,
                        value.type_reference,
                    )
                    .is_some_and(|constructor| {
                        constructor.case == *case && constructor.fields.is_empty()
                    }))
                    || case_at_capture(
                        program,
                        facts,
                        state,
                        value.operand.row(),
                        &value.projection.remaining,
                        *case,
                    )
            }))
        .then_some(*ordering)
    });
    let selected = candidates.next().ok_or("no live ordering case is proved")?;
    candidates
        .next()
        .is_none()
        .then_some(selected)
        .ok_or("ordering case proof is ambiguous")
}

fn case_at_capture(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    operand: &checked_trees::FlowOperandFact,
    remaining: &[facts::PlaceSegment],
    case: symbols::SymbolHandle,
) -> bool {
    let Some(mut subject) = validation::canonical_place_from_expression_in_state(
        program,
        operand.state_symbol,
        operand.statement_index,
        operand.expression,
    ) else {
        return false;
    };
    subject.extend_segments(remaining);
    let contexts = facts
        .flow
        .semantic_constraint_contexts(operand.constraints)
        .collect::<Vec<_>>();
    crate::flow::place_case_has_value(
        program,
        &facts.semantic,
        &contexts,
        state.machine_symbol,
        operand.state_symbol,
        operand.statement_index,
        &subject,
        case,
        true,
    )
}
