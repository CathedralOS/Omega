//! Observations at one executed state prefix.
//!
//! The structural path owner supplies preceding locals, guards and captured
//! normal results to each visitor. A visitor cannot obtain facts from a later
//! statement or a recurrent successor. State Requires describe this invocation;
//! machine Requires apply here only when this is the machine's entry state.

use super::citations::{CitationFacts, machine_requires_facts};
use super::structural_case_arms::recognize_structural_state_prefix;
use super::structural_judgment::{StructuralJudge, StructuralTerm};
use symbols::SymbolHandle;
use typed_trees::{machine::Machine, state::State};

#[derive(Clone, Copy)]
pub(super) struct StructuralPrefixSite {
    pub state: SymbolHandle,
    pub statement_index: usize,
    pub in_transition_target: bool,
}

pub(super) trait StructuralPrefixVisitor {
    fn site(&self) -> StructuralPrefixSite;

    #[allow(clippy::too_many_arguments)]
    fn visit(
        &mut self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        judge: &StructuralJudge<'_>,
        environment: &[(String, StructuralTerm)],
        hypotheses: &[(String, StructuralTerm)],
        equations: &[(StructuralTerm, StructuralTerm)],
        guarantees: &CitationFacts,
    );
}

pub(super) fn observe_runtime_state_prefix(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    visitor: &mut dyn StructuralPrefixVisitor,
) -> Option<()> {
    let entry = program.machine_states(machine).first()?;
    let mut requires = if state.symbol == entry.symbol {
        machine_requires_facts(program, machine)
    } else {
        Vec::new()
    };
    requires.extend(
        crate::proof_contracts::proof_facts::contract_expression_facts(
            program,
            program.state_contracts(state).iter(),
            typed_trees::signature::SignatureContractKind::Requires,
        ),
    );
    let judge = StructuralJudge::from_runtime_state_requires(program, machine, state, &requires);
    let mut diagnostics = Vec::new();
    recognize_structural_state_prefix(
        program,
        machine,
        state,
        &judge,
        &crate::proof_only_classification(program),
        &mut diagnostics,
        visitor,
    )?;
    diagnostics.is_empty().then_some(())
}
