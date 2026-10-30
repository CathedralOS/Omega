//! Private entry facts are proved from actual arrivals, never lexical scope.
//! The enclosing declaration starts with arbitrary inputs and its own contract.
//! Its ordinary path owner substitutes completed actuals and retains guards and
//! prior citations. Every admissible private arrival must be observed; an opaque
//! call or escaping selection cannot donate a convenient premise. Recurrent
//! entries are observed afresh, never under a previous iteration's values.
//! Coverage and proof are separate: this coordinator observes every arrival;
//! the consumer owns its predicate, all-path judgment, and theorem eligibility.

use super::super::structural_judgment::StructuralJudge;
use super::{PrefixObservation, StructuralPrefixVisitor};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;

mod cycles;
mod scope;
pub(crate) use scope::{ClosedArrivals, ProgramPrivateArrivalIndex, private_invocation_index};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Arrival {
    pub(crate) caller: SymbolHandle,
    pub(crate) statement: usize,
    pub(crate) expression: Option<ExpressionHandle>,
}

pub(in crate::proof_contracts::contract_entailment) fn observe_private_arrivals<Request>(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    requirement: &mut Request,
    case_facts_only: bool,
) -> Option<bool>
where
    Request: StructuralPrefixVisitor + Clone,
{
    let root = scope::lexical_root(program, machine)?;
    let arrivals = match scope::closed_arrivals(program, root, machine) {
        Ok(arrivals) => arrivals,
        Err(reason) => {
            if super::super::struct_trace() {
                eprintln!("PRIVATE arrivals machine={} reason={reason}", machine.name);
            }
            return Some(false);
        }
    };
    let entry = program.machine_states(machine).first()?;
    let classification = crate::proof_only_classification(program);
    // Erased proof recursion retains its existing ranked induction owner.
    // These fresh runtime intervals cannot license another theorem's IH.
    if arrivals.entry_boundaries.iter().any(|symbol| {
        program
            .machine_by_symbol(*symbol)
            .is_none_or(|entry| classification.is_proof_machine(program, entry))
    }) {
        return Some(false);
    }
    let roots = program
        .machines()
        .iter()
        .filter(|candidate| {
            scope::is_root(program, candidate, root)
                || arrivals.entry_boundaries.contains(&candidate.symbol)
        })
        .collect::<Vec<_>>();
    // Each cached frontier replaces every non-anchor owner with all of its
    // incoming owners. Every trial starts from the original query state;
    // failed overapproximations cannot publish a negative judgment.
    for indices in &arrivals.caller_frontiers {
        let Some(caller_roots) = indices
            .iter()
            .map(|&index| program.machines().get(index))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        if same_frontier(&caller_roots, &roots)
            // Ranked mathematical induction retains its original owner.
            || caller_roots.iter().any(|caller| classification.is_proof_machine(program, caller))
        {
            continue;
        }
        let mut trial = requirement.clone();
        if observe_frontier(
            program,
            machine,
            entry.symbol,
            &caller_roots,
            &mut trial,
            case_facts_only,
            &arrivals,
        ) && trial.has_positive_proof()
        {
            *requirement = trial;
            return Some(true);
        }
    }
    Some(observe_frontier(
        program,
        machine,
        entry.symbol,
        &roots,
        requirement,
        case_facts_only,
        &arrivals,
    ))
}

fn same_frontier(left: &[&Machine], right: &[&Machine]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .all(|root| right.iter().any(|other| root.symbol == other.symbol))
}

fn observe_frontier(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    entry: SymbolHandle,
    roots: &[&Machine],
    requirement: &mut dyn StructuralPrefixVisitor,
    case_facts_only: bool,
    arrivals: &std::sync::Arc<ClosedArrivals>,
) -> bool {
    let classification = crate::proof_only_classification(program);
    let boundaries = roots.iter().map(|root| root.symbol).collect::<Vec<_>>();
    let mut observer = ArrivalRequirement {
        requirement,
        entry,
        root: SymbolHandle::invalid(),
        observed: Vec::new(),
        boundaries,
        closed: std::sync::Arc::clone(arrivals),
    };
    let mut diagnostics = Vec::new();
    let mut recognized = true;
    // Each executable application starts independently with arbitrary runtime
    // inputs under its own applied contract. Knowing the source template locates
    // that declaration group; it supplies neither caller values nor premises.
    for arrival_root in roots {
        observer.root = arrival_root.symbol;
        let Some(root_entry) = program.machine_states(arrival_root).first() else {
            return false;
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
            // This is an independently admitted invocation, just like an
            // ordinary prefix root. Retain its exact incoming values and
            // default-domain predicates, not merely explicit `requires`.
            StructuralJudge::from_runtime_state_requires(
                program,
                arrival_root,
                root_entry,
                &requires,
            )
        };
        if case_facts_only {
            judge.restrict_to_case_facts();
        }
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
            "PRIVATE arrivals machine={} roots={} recognized={} observed={} expected={} diagnostics={}",
            machine.name,
            roots.len(),
            recognized,
            observer.observed.len(),
            arrivals.expected.len(),
            diagnostics.len()
        );
    }
    recognized
        && diagnostics.is_empty()
        && !arrivals.expected.is_empty()
        && arrivals
            .expected
            .iter()
            .all(|arrival| observer.observed.contains(arrival))
}

struct ArrivalRequirement<'query> {
    requirement: &'query mut dyn StructuralPrefixVisitor,
    entry: SymbolHandle,
    root: SymbolHandle,
    observed: Vec<Arrival>,
    boundaries: Vec<SymbolHandle>,
    closed: std::sync::Arc<ClosedArrivals>,
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
        // Closure checked every direct/escaping private use. Opaque calls in
        // unrelated bodies cannot name this private occurrence; re-entry into
        // an enclosing root is covered by that root's independent invocation.
        // Ordinary completion still observes their actual writes and values.
        let boundary = self.boundaries.contains(&machine);
        (!boundary && !self.closed.bodies.contains(&machine))
            || (machine != self.root && boundary)
            || self.requirement.stops_at_machine(machine)
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
