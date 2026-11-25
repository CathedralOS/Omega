//! Transport private member inputs along the lowered component's actual edges.

use super::{CallSites, canonical, conjuncts, describe_calls};
use crate::{ProofSearchActivity, ReplayScope, Unreached, joins, terminal_machine};
use arena::IdTable;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ContentPlaceVersion, MachineId, PlaceId, Proposition,
    ScalarTerm, ScalarType, StructuralCaseId, ValueId,
};
use std::collections::{BTreeSet, VecDeque};
use terminal_check_tables::Checked;
use terminal_semantics::inhabitation::Inhabitation;
use terminal_semantics::substitution::clauses::{
    ClauseBindings, substitute_clause_with_computation_names,
};

/// What survived one physical transfer, expressed in the two members' own formal
/// names. Reusing the frame does not establish that an overwritten field survived.
struct Transfer {
    callee: MachineId,
    arrival: usize,
    preserved: Vec<(ScalarTerm, ScalarTerm)>,
}
pub(super) struct Transfers(IdTable<MachineId, Vec<Transfer>>);

/// Membership for one arrival. Its original condition remains in proof order
/// for later consumers. Only facts learned through recursion need new owners.
struct ArrivalFacts<'a> {
    original: Box<[&'a Proposition]>,
    inferred: BTreeSet<Proposition>,
}

impl<'a> ArrivalFacts<'a> {
    fn new(predicate: &'a Proposition) -> Self {
        let mut original: Box<[_]> = conjuncts(predicate).iter().collect();
        original.sort_unstable();
        Self {
            original,
            inferred: BTreeSet::new(),
        }
    }

    fn contains(&self, fact: &Proposition) -> bool {
        self.original.binary_search(&fact).is_ok() || self.inferred.contains(fact)
    }

    fn insert(&mut self, fact: &Proposition) -> bool {
        if self.contains(fact) {
            return false;
        }
        self.inferred.insert(fact.clone())
    }
}

impl Transfers {
    pub(super) fn insert(
        &mut self,
        owner: MachineId,
        callee: MachineId,
        arrival: usize,
        mut preserved: Vec<(ScalarTerm, ScalarTerm)>,
    ) {
        preserved.sort_unstable();
        self.0
            .get_mut(owner)
            .expect("member indexed")
            .push(Transfer {
                callee,
                arrival,
                preserved,
            });
    }
}

pub(super) fn infer(
    scope: &mut ReplayScope<'_>,
    sites: &CallSites,
    unreached: &mut Unreached,
    inhabitation: &Inhabitation,
    activity: &mut ProofSearchActivity,
) -> Checked<IdTable<MachineId, Vec<Proposition>>> {
    let machines = &scope.source().tables.machines;
    let mut incoming = machines.map(|_, _| Vec::new());
    let mut transfers = Transfers(machines.map(|_, _| Vec::new()));
    for caller in machines.keys() {
        describe_calls(
            scope,
            caller,
            sites,
            Some(&mut transfers),
            &mut incoming,
            unreached,
            inhabitation,
            activity,
        )?;
    }

    // Every arrival counts, including ordinary calls from outside the component.
    // An impossible edge owes every predicate by its own checked unreachability.
    let mut available = incoming.map(|_, _| Vec::new());
    let mut offered = incoming.map(|_, _| BTreeSet::new());
    let mut sent = incoming.map(|_, _| BTreeSet::new());
    let mut pending = VecDeque::new();
    for (callee, arrivals) in incoming.iter() {
        if arrivals.is_empty() && transfers.0.get(callee).expect("member indexed").is_empty() {
            continue;
        }
        if !arrivals.is_empty() {
            *available.get_mut(callee).expect("member indexed") =
                arrivals.iter().map(ArrivalFacts::new).collect();
            let predicate = canonical(joins::connective(arrivals.clone(), false), callee)?;
            if predicate != Proposition::Truth {
                offered
                    .get_mut(callee)
                    .expect("member indexed")
                    .extend(conjuncts(&predicate).iter().cloned());
                scope.require(callee, predicate, sites, activity)?;
            }
        }
        enqueue_formed(scope, callee, &mut sent, &mut pending)?;
        offered
            .get_mut(callee)
            .expect("member indexed")
            .retain(|fact| !sent.get(callee).expect("member indexed").contains(fact));
    }
    // Substitutions only rename inputs that still have the same value.
    // They introduce neither arithmetic nor new literals, so the set of
    // distinct images of the finite initial predicates is finite.
    while let Some((owner, clause)) = pending.pop_front() {
        for edge in transfers.0.get(owner).expect("member indexed") {
            // Overwriting a cell can drop one conjunct inside an arrival's
            // disjunction. Project the surviving facts on every alternative.
            let image = joins::project(&clause, &mut |atom| {
                substitute_clause_with_computation_names(atom, &mut Missing, &edge.preserved).ok()
            });
            let image = canonical(image, edge.callee)?;
            if image == Proposition::Truth {
                continue;
            }
            let arrivals = available.get_mut(edge.callee).expect("callee indexed");
            if !arrivals[edge.arrival].insert(&image)
                || sent
                    .get(edge.callee)
                    .expect("callee indexed")
                    .contains(&image)
                || !arrivals
                    .iter()
                    .all(|facts| facts.contains(&image) || facts.contains(&Proposition::Falsehood))
            {
                continue;
            }
            // A candidate whose arithmetic could not form may become total
            // once another bound arrives. Offer it alongside that new bound.
            let candidates = offered.get_mut(edge.callee).expect("callee indexed");
            candidates.insert(image);
            scope.require(
                edge.callee,
                joins::connective(candidates.iter().cloned().collect(), true),
                sites,
                activity,
            )?;
            enqueue_formed(scope, edge.callee, &mut sent, &mut pending)?;
            candidates.retain(|fact| {
                !sent
                    .get(edge.callee)
                    .expect("callee indexed")
                    .contains(fact)
            });
        }
    }
    Ok(incoming)
}

fn enqueue_formed(
    scope: &ReplayScope<'_>,
    owner: MachineId,
    sent: &mut IdTable<MachineId, BTreeSet<Proposition>>,
    pending: &mut VecDeque<(MachineId, Proposition)>,
) -> Checked<()> {
    let member = terminal_machine(&scope.source().module.machines, owner, owner)?;
    let sent = sent.get_mut(owner).expect("member indexed");
    for clause in member.contract.requires.iter().flat_map(conjuncts) {
        if !matches!(clause, Proposition::Truth | Proposition::Falsehood)
            && sent.insert(clause.clone())
        {
            pending.push_back((owner, clause.clone()));
        }
    }
    Ok(())
}

// Every input must have an exact name in the transfer above. In particular,
// unrelated formals with the same numeric identity belong to different members.
struct Missing;
impl ClauseBindings for Missing {
    type Error = &'static str;
    fn value(&mut self, _: ValueId, _: ScalarType) -> Result<ScalarTerm, Self::Error> {
        Err("input is not preserved across this transfer")
    }
    fn field(
        &mut self,
        _: ContentPlaceVersion,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        Err("field is not preserved across this transfer")
    }
    fn case(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: StructuralCaseId,
    ) -> Result<bool, Self::Error> {
        Err("case is not preserved across this transfer")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Proposition::{Conjunction, Disjunction, Falsehood, Truth};

    #[test]
    fn arrival_facts_preserve_membership_and_original_proof_order() {
        let nested = Disjunction(vec![Falsehood, Truth]);
        let arrivals = [
            Conjunction(vec![nested.clone(), Falsehood, nested.clone()]),
            nested.clone(),
            Falsehood,
            Conjunction(vec![]),
        ];
        let originals = arrivals.clone();
        for arrival in &arrivals {
            let mut index = ArrivalFacts::new(arrival);
            let mut previous: BTreeSet<_> = conjuncts(arrival).iter().cloned().collect();
            for fact in [&Truth, &nested, &Falsehood, &Truth, &nested] {
                assert_eq!(index.contains(fact), previous.contains(fact));
                assert_eq!(index.insert(fact), previous.insert(fact.clone()));
                assert!(index.contains(fact));
            }
        }
        assert_eq!(arrivals, originals);
    }
}
