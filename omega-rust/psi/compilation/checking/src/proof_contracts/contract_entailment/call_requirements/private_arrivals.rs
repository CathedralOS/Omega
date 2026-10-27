//! Private entry facts are proved from actual arrivals, never lexical scope.
//! The enclosing declaration starts with arbitrary inputs and its own contract.
//! Its ordinary path owner substitutes completed actuals and retains guards and
//! prior citations. Every admissible private arrival must be observed; an opaque
//! call, escaping selection or private cycle cannot donate a convenient premise.

use super::super::structural_judgment::StructuralJudge;
use super::super::structural_prefix::{PrefixObservation, StructuralPrefixVisitor};
use super::{CallRequirement, Machine, SymbolHandle};
use typed_trees::expression::ExpressionHandle;

mod scope;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Arrival {
    caller: SymbolHandle,
    statement: usize,
    expression: Option<ExpressionHandle>,
}

pub(super) fn prove(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    requirement: &mut CallRequirement<'_>,
    case_facts_only: bool,
) -> Option<bool> {
    let root = scope::lexical_root(program, machine)?;
    let expected = match scope::closed_arrivals(program, root, machine) {
        Ok(expected) => expected,
        Err(reason) => {
            if super::super::struct_trace() {
                eprintln!("PRIVATE arrivals machine={} reason={reason}", machine.name);
            }
            return Some(false);
        }
    };
    let entry = program.machine_states(machine).first()?;
    let classification = crate::proof_only_classification(program);
    let requires = super::machine_requires_facts(program, root);
    let mut judge = if classification.is_proof_machine(program, root) {
        StructuralJudge::from_requires(program, root, &requires, true)
    } else {
        StructuralJudge::from_runtime_requires(program, root, &requires)
    };
    if case_facts_only {
        judge.restrict_to_case_facts();
    }
    requirement.seen = false;
    requirement.proven = true;
    let mut observer = ArrivalRequirement {
        requirement,
        entry: entry.symbol,
        root: root.symbol,
        observed: Vec::new(),
    };
    let mut diagnostics = Vec::new();
    let recognized = super::recognize_structural_case_arms(
        program,
        root,
        &judge,
        &classification,
        &mut diagnostics,
        Some(&mut observer),
    );
    if super::super::struct_trace() {
        eprintln!(
            "PRIVATE arrivals machine={} root={} recognized={} seen={} proven={} observed={} expected={} diagnostics={}",
            machine.name,
            root.name,
            recognized.is_some(),
            observer.requirement.seen,
            observer.requirement.proven,
            observer.observed.len(),
            expected.len(),
            diagnostics.len()
        );
    }
    Some(
        recognized.is_some()
            && diagnostics.is_empty()
            && observer.requirement.seen
            && observer.requirement.proven
            && !expected.is_empty()
            && expected
                .iter()
                .all(|arrival| observer.observed.contains(arrival)),
    )
}

struct ArrivalRequirement<'query, 'program> {
    requirement: &'query mut CallRequirement<'program>,
    entry: SymbolHandle,
    root: SymbolHandle,
    observed: Vec<Arrival>,
}

impl StructuralPrefixVisitor for ArrivalRequirement<'_, '_> {
    fn site(&self) -> super::super::structural_prefix::StructuralPrefixSite {
        self.requirement.site()
    }
    fn is_complete(&self) -> bool {
        self.requirement.is_complete()
    }
    fn entered(&mut self) {
        self.requirement.entered();
    }
    fn unresolved(&mut self, machine: &Machine) {
        self.requirement.unresolved(machine);
    }
    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let (program, machine, state, mut judge, environment) = observation;
        if !judge.has_runtime_body_values()
            && let Some(root) = program.machine_by_symbol(self.root)
        {
            super::intake_state_induction(
                program,
                root,
                state,
                self.requirement.statement_index,
                environment,
                &mut judge,
            );
        }
        self.requirement
            .observe((program, machine, state, judge, environment));
    }
    fn entered_body(
        &mut self,
        caller: SymbolHandle,
        statement: usize,
        expression: Option<ExpressionHandle>,
        entry: SymbolHandle,
    ) {
        if entry == self.entry {
            let arrival = Arrival {
                caller,
                statement,
                expression,
            };
            if !self.observed.contains(&arrival) {
                self.observed.push(arrival);
            }
        }
    }
}
