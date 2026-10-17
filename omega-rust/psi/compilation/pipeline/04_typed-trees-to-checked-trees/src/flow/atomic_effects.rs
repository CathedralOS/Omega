//! Performed atomic memory effects join the same historical event contexts as
//! register instructions. The normalized atomic decoder owns source meaning;
//! no ordinary assignment or fence can publish a release-store event.
use super::FlowBuildContext;
use arena::HandleSpan;
use checked_trees::{FlowConstraintRef, FlowSemanticContextRef};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, InstructionEffectOperation, ProgramPoint,
};

pub(super) fn append(
    program: &dyn validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &typed_trees::statement::StatementNode,
    contexts: &mut HandleSpan<FlowSemanticContextRef>,
    constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    if release_store(program, machine, state, statement).is_none() {
        return;
    }
    let point = ProgramPoint::Statement {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        statement_index,
    };
    let fact = semantic.append_fact(Fact {
        point,
        place: FactPlace::Unknown,
        origin: FactOrigin::StatementTransfer,
        payload: FactPayload::InstructionEffect {
            operation: InstructionEffectOperation::AtomicReleaseStore,
        },
        ..Fact::default()
    });
    let mut references = HandleSpan::empty();
    semantic.append_ref(&mut references, fact);
    let mut completed = super::call_phases::CallFlowContexts {
        contexts: *contexts,
        constraints: *constraints,
    };
    super::reference_spans::append_semantic_context(
        semantic,
        build,
        &mut completed,
        point,
        references,
    );
    *contexts = completed.contexts;
    *constraints = completed.constraints;
}

pub(crate) fn release_store(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement: &typed_trees::statement::StatementNode,
) -> Option<validation::AtomicCarrierAccess> {
    let carrier = validation::atomic_assignment_carrier(program, statement.as_assignment()?)?;
    (matches!(
        carrier.operation,
        validation::AtomicAccessOperation::Store(
            language_core::atomic::MemoryOrdering::Publish
                | language_core::atomic::MemoryOrdering::GlobalOrder
        )
    ) && validation::place_is_atomic_storage(program, machine, Some(state), carrier.place))
    .then_some(carrier)
}
