//! Private entry facts are proved from actual arrivals, never lexical scope.
//! The enclosing declaration starts with arbitrary inputs and its own contract.
//! Its ordinary path owner substitutes completed actuals and retains guards and
//! prior citations. Every admissible private arrival must be observed; an opaque
//! call, escaping selection or private cycle cannot donate a convenient premise.
//! Coverage and proof are separate: this coordinator observes every arrival;
//! the consumer owns its predicate, all-path judgment, and theorem eligibility.

use super::super::structural_judgment::StructuralJudge;
use super::{PrefixObservation, StructuralPrefixVisitor};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;

mod scope;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Arrival {
    caller: SymbolHandle,
    statement: usize,
    expression: Option<ExpressionHandle>,
}

pub(in crate::proof_contracts::contract_entailment) fn observe_private_arrivals(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    requirement: &mut dyn StructuralPrefixVisitor,
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
    let mut observer = ArrivalRequirement {
        requirement,
        entry: entry.symbol,
        root: root.symbol,
        observed: Vec::new(),
    };
    let mut diagnostics = Vec::new();
    let mut recognized = true;
    // Each executable application starts independently with arbitrary runtime
    // inputs under its own applied contract. Knowing the source template locates
    // that declaration group; it supplies neither caller values nor premises.
    for arrival_root in program
        .machines()
        .iter()
        .filter(|candidate| scope::is_root(program, candidate, root))
    {
        let Some(root_entry) = program.machine_states(arrival_root).first() else {
            return Some(false);
        };
        if !super::super::structural_case_arms::prefix_site_is_reachable(
            program, root_entry, &observer,
        ) {
            continue;
        }
        let requires = super::machine_requires_facts(program, arrival_root);
        let mut judge = if classification.is_proof_machine(program, arrival_root) {
            StructuralJudge::from_requires(program, arrival_root, &requires, true)
        } else {
            StructuralJudge::from_runtime_requires(program, arrival_root, &requires)
        };
        if case_facts_only {
            judge.restrict_to_case_facts();
        }
        observer.root = arrival_root.symbol;
        recognized &= super::recognize_structural_case_arms(
            program,
            arrival_root,
            &judge,
            &classification,
            &mut diagnostics,
            Some(&mut observer),
        )
        .is_some();
    }
    if super::super::struct_trace() {
        eprintln!(
            "PRIVATE arrivals machine={} root={} recognized={} observed={} expected={} diagnostics={}",
            machine.name,
            root.name,
            recognized,
            observer.observed.len(),
            expected.len(),
            diagnostics.len()
        );
    }
    Some(
        recognized
            && diagnostics.is_empty()
            && !expected.is_empty()
            && expected
                .iter()
                .all(|arrival| observer.observed.contains(arrival)),
    )
}

struct ArrivalRequirement<'query> {
    requirement: &'query mut dyn StructuralPrefixVisitor,
    entry: SymbolHandle,
    root: SymbolHandle,
    observed: Vec<Arrival>,
}

impl StructuralPrefixVisitor for ArrivalRequirement<'_> {
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
    fn stops_at_machine(&self, machine: SymbolHandle) -> bool {
        self.requirement.stops_at_machine(machine)
    }
    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let program = observation.0;
        if let Some(root) = program.machine_by_symbol(self.root) {
            self.requirement.observe_private_arrival(root, observation);
        } else {
            self.requirement.unresolved(observation.1);
        }
    }
    fn entered_body(
        &mut self,
        caller: SymbolHandle,
        statement: usize,
        expression: Option<ExpressionHandle>,
        entry: SymbolHandle,
    ) {
        self.requirement
            .entered_body(caller, statement, expression, entry);
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
