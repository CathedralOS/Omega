//! Completed instruction events follow the same call and branch flow as
//! ordinary guarantees. Their point retains captured operands; no statement
//! scan or source spelling can manufacture execution on a bypassing edge.
use super::{CallFlowContexts, FlowBuildContext};
use arena::HandleSpan;
use checked_trees::BorrowCallFact;
use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint};
use language_core::inline_assembly::AsmRegisterOperation;

pub(in crate::flow) fn append(
    program: &dyn validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &BorrowCallFact,
    exit: &mut CallFlowContexts,
) {
    let payload = match program
        .symbols
        .builtin_function_for_symbol(call.target_symbol)
        .and_then(|builtin| AsmRegisterOperation::from_intrinsic_name(builtin.name()))
        .filter(|operation| operation.is_admitted())
    {
        Some(operation) => FactPayload::InstructionEffect { operation },
        None => FactPayload::CallCompletion {
            target: call.target_symbol,
        },
    };
    let point = ProgramPoint::CallEnsures {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        statement_index: call.statement_index,
        call_ordinal: call.call_ordinal,
    };
    let fact = semantic.append_fact(Fact {
        place: FactPlace::Unknown,
        point,
        origin: FactOrigin::StatementTransfer,
        payload,
        ..Fact::default()
    });
    let mut references = HandleSpan::empty();
    semantic.append_ref(&mut references, fact);
    crate::flow::reference_spans::append_semantic_context(semantic, build, exit, point, references);
}
