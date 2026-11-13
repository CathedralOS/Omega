#![forbid(unsafe_code)]

/*
Discharge on the lowered form: every obligation that structure-only
elaboration minted is decided here, and an obligation counts as discharged
only when the proof kernel (`proof_admission::verify_obligation`) accepted a
certificate for it. Anything else is a `Refusal` with reason `Unproved`; there
is no interval verdict, no admitted route and no assumption.
(`wiki/drafts/designs/check_on_lowered_form.md`, "Route one";
`wiki/drafts/designs/stage_trust.md`, rule 2.)

Per machine it does five things:

- Checks that the rows it is about to decide are the ones the lowered form
  owes, each at the point of what owes it (`minted`). The lowered form names
  every obligation itself, so a table with a row missing, restated over
  another subject or moved to another point is refused before any proof.
- Fills the `knowledge` table from the lowered form alone: each entry
  `requires` clause at the entry block, each operation's result equation on
  its normal successor (Terminal semantics states it), each field read's
  equation with the one value its place holds on every path to it (`places`,
  the same walk a copy's leaves are resolved by), each `requires` clause of
  a data type that a read of established storage can state on its value,
  and each callee `ensures` clause, under the call's actuals, after the call.
  Nothing comes from source shapes, names or any Psi 05 or validation fact.
- Judges initialization: every operation and terminator that reads storage
  gets one verdict, `Unestablished` when some path to it supplies no
  established value for a scalar leaf it reads (`places`). It is a
  structural judgment over the write log and the graph, not an obligation,
  so it has no certificate.
- Follows each invariant window: a store or zero establishment that was not
  shown to keep a data type's clause opens one on its place, the one
  reaching walk finds every consumption point it reaches open, and the
  clause is proved again there by the same prover (`places`,
  `Storage::consumptions`). Zeroed storage of a gated type observed while
  its window is open is `Unestablished`.
- States each obligation's goal over Terminal values: an operation's safety
  goal is Terminal semantics' own canonical goal, a call requirement is the
  callee's `requires` clause under the call's actuals, a normal guarantee is
  this machine's `ensures` clause.
  A data type's `requires` clause is stated over the values its place holds
  once the owing operation completes (`places`, `Storage::domain_goal`).
  Every clause, whoever owns it, is stated over this machine's values by the
  one substitution (`substitute`).
- Passes the knowledge whose program point dominates the obligation's to one
  prover (`prove`), which emits a certificate, and asks the kernel to decide
  it.

A loop head's invariant, the `requires` of the machine lowered beneath it, is
proved on every arrival (`ArrivalInvariant`) and known from the head's entry
on (`arrivals`). Kinds outside those five are refused as `Unsupported`.
*/

mod arrivals;
mod callable_formations;
mod crashes;
mod describe;
mod entry_inputs;
mod graph;
mod joins;
mod known;
mod minted;
mod places;
mod prove;
mod refinement;
mod result_summaries;
mod reuse;
mod verdict;

pub use callable_formations::{
    CallableFormationCause, CallableFormationDefect, CallableFormationFailure,
    check_bound_callable_shapes,
};
pub use places::{ConflictingLoan, LoanConflict};
pub use refinement::prove_callable_refinement;
pub use reuse::{Selection, reverify, reverify_at_call, reverify_specialization};
pub use verdict::{
    AcceptedClosure, CompilationRefusal, ConflictingUse, ConsumptionSite, PublicationError,
    RoutePublication, RouteVerdict, UnestablishedUse, UnprovedObligation, judge, judge_compilation,
    judge_compilation_for,
};

#[cfg(test)]
pub(crate) use prove::has_rules_for;

#[cfg(test)]
mod tests;

use proof_admission::{
    AcceptedFact, CertificateEnvelope, EvidenceRoute, Obligation, ObligationClass,
    ProofSystemMarker, verify_obligation,
};
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, EdgeId, EvidenceIdentity, IdTable, IntegerMathTerm,
    InvocationSite, MachineId, ObligationId, OperationId, PlaceId, Proposition, PropositionContext,
    ScalarTerm, ScalarType, StructuralCaseId, StructuralTypeId, ValueId,
};
use structure_elaboration::Elaboration;
use terminal_check_tables::edges::{Actual, Callee, Destination};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::{Justification, KnowledgeRow};
use terminal_check_tables::{
    Checked, Construct, KnowledgeId, MachineTables, ProgramPoint, Refusal, RefusalReason,
    UnprovedCause,
};
use terminal_codec::ObligationEvidenceSeal;
use terminal_psi::{TerminalMachine, TerminalModule, Terminator};

use graph::Graph;

/// What the kernel accepted for one obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    /// The goal the kernel decided, over this machine's values.
    pub goal: Proposition,
    /// The knowledge rows passed as the certificate's assumptions, by index.
    pub assumptions: Vec<KnowledgeId>,
    /// The knowledge rows passed as its semantic axioms (operation result
    /// equations), by index.
    pub axioms: Vec<KnowledgeId>,
    pub certificate: CertificateEnvelope,
    /// The kernel's acceptance, including whether the mathematical core
    /// judged the certificate or refused a rule family it does not cover.
    pub fact: AcceptedFact,
}

/// What discharged one obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discharged {
    /// A certificate the kernel accepted.
    Certificate(Evidence),
    /// A `PublishedGuarantee`: the `NormalGuarantee` of every normal return
    /// edge for this clause was discharged by a certificate, and these are
    /// they. Empty when the machine has no normal return, where the clause
    /// holds vacuously.
    EveryExit(Vec<ObligationId>),
    /// A `DataDomain` clause that a store or a zero establishment was not
    /// shown to keep, with why: the write opens an invariant window.
    /// `wiki/spec/language/dependent_values.md`, "Invariant windows": "A
    /// write known to preserve the default domain keeps it established.
    /// Otherwise the write opens a window on the place, whose actual new
    /// contents remain available to ordinary flow reasoning. The domain must
    /// be proved again at the next consumption point". Nothing is owed at
    /// the write, so this is not a refusal; it is not a discharge either.
    /// The clause is owed at every consumption point the window reaches
    /// open, and each of those has its own verdict in
    /// `MachineDischarge::windows`, under this obligation's identity.
    ///
    /// A zero establishment (`let x: T;`) whose zero value does not satisfy
    /// the clause opens the same debt on storage that holds no valid value
    /// yet: "Zeroing storage is permitted independently of establishing a
    /// value." and "If zero satisfies the default domain, the value is
    /// zero-constructible. Otherwise the type is gated: construction or
    /// checked qualification must establish the domain before observation."
    Window(Box<Refusal>),
    /// A `DataDomain` clause of a store that writes no storage the clause
    /// reads, where the clause was not proved over the place's values: the
    /// clause is as true after the store as before it, so the store opens
    /// no window and closes none. dependent_values.md: "A write known to
    /// preserve the default domain keeps it established." It is a judgment
    /// over the clause's names and the stored place (`Storage::untouched`),
    /// with no certificate.
    Preserved,
    /// A `Loan` that may be formed where it is: no other loan of the same
    /// call overlaps it incompatibly, and a reborrow asks no more access
    /// than its parent holds (`places::loans`). loans.md: "Loan descent,
    /// access attenuation, temporal containment and restoration are
    /// resource judgments. Logical facts cannot create, amplify, transfer,
    /// extend, return, consume or duplicate authority." It is a structural
    /// judgment with no certificate, and the loan ends when its call
    /// returns, so it owes nothing after it.
    Lent,
}

impl Discharged {
    pub const fn certificate(&self) -> Option<&Evidence> {
        match self {
            Self::Certificate(evidence) => Some(evidence),
            Self::EveryExit(_) | Self::Window(_) | Self::Preserved | Self::Lent => None,
        }
    }
}

/// One consumption point an open invariant window reaches, and whether the
/// window's clause was proved again there. dependent_values.md: "The domain
/// must be proved again at the next consumption point" and "An unclosed
/// window rejects with both the opening write or storage cast and the
/// failing consumption point."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowUse {
    /// The `DataDomain` obligation the opening write left unproved
    /// (`Discharged::Window`): the window's identity, and the identity the
    /// certificate is issued under.
    pub opened_by: ObligationId,
    /// The consumption point: the operation, or for a terminator its edge
    /// or block.
    pub at: Construct,
    /// The kernel's acceptance of the clause over what the place holds at
    /// the consumption point. `Err` with reason `Unproved` when a store
    /// opened the window, and with reason `Unestablished` when a zero
    /// establishment did, as the storage then never held a valid value:
    /// either is a compile error of the program.
    pub verdict: Checked<Evidence>,
}

/// One operation or terminator that reads storage, and whether what it
/// reads is established there. The initialization judgment is structural: it
/// has no obligation row, so its verdicts stand beside the obligations'.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageUse {
    /// The operation, or for a terminator its edge.
    pub at: Construct,
    /// `Err` with reason `Unestablished` when some path to the use supplies
    /// no established value for a scalar leaf it reads: a compile error of
    /// the program.
    pub verdict: Checked<()>,
}

/// One machine's result.
#[derive(Debug, Clone)]
pub struct MachineDischarge {
    /// One verdict per obligation row, indexed by the same `ObligationId`.
    pub verdicts: IdTable<ObligationId, Checked<Discharged>>,
    /// One verdict per use of storage, in block and operation order, a
    /// block's terminator last.
    pub uses: Vec<StorageUse>,
    /// One verdict per consumption point each invariant window reaches
    /// open, in block and operation order.
    pub windows: Vec<WindowUse>,
    /// Joins whose inferred predicate some arrival could not establish. Each
    /// predicate was replaced by `Truth`, which every arrival establishes,
    /// so the join contributes nothing downstream (`joins`).
    pub weakened_joins: Vec<BlockId>,
    /// Each loan that may not be formed, with what it conflicts with. Its
    /// row's verdict is the `Conflict` refusal.
    pub conflicts: Vec<places::ConflictingLoan>,
}

impl MachineDischarge {
    /// The uses of storage that are not established: each is a rejection of
    /// the program by this route, as an `Unproved` obligation is.
    pub fn unestablished(&self) -> impl Iterator<Item = &Refusal> {
        self.uses
            .iter()
            .filter_map(|used| used.verdict.as_ref().err())
    }
}

#[derive(Debug, Clone)]
pub struct Discharge {
    pub machines: IdTable<MachineId, MachineDischarge>,
}

impl Discharge {
    /// One seal per discharged `OperationSafety` obligation, the only kind
    /// Omega 00 turns into an accepted fact. The seals travel beside the
    /// module in memory and are lost on a byte round trip
    /// (`ObligationEvidenceSeal`).
    pub fn operation_seals(
        &self,
        elaboration: &Elaboration,
    ) -> Checked<Vec<ObligationEvidenceSeal>> {
        let mut seals = Vec::new();
        for (machine, result) in self.machines.iter() {
            let tables = terminal_check_tables::row(
                &elaboration.tables.machines,
                machine,
                machine,
                Construct::Machine,
            )?;
            for (obligation, verdict) in result.verdicts.iter() {
                let Ok(Discharged::Certificate(evidence)) = verdict else {
                    continue;
                };
                let row = terminal_check_tables::row(
                    &tables.obligations,
                    obligation,
                    machine,
                    Construct::Obligation(obligation),
                )?;
                if !matches!(row.kind, ObligationKind::OperationSafety { .. }) {
                    continue;
                }
                let resolve = |rows: &[KnowledgeId]| {
                    rows.iter()
                        .map(|row| {
                            knowledge_proposition(tables, *row).cloned().ok_or_else(|| {
                                refusal(
                                    RefusalReason::ForeignIdentity,
                                    machine,
                                    Construct::Obligation(obligation),
                                    "evidence cites a knowledge row with no proposition",
                                )
                            })
                        })
                        .collect::<Checked<Vec<_>>>()
                };
                let seal = ObligationEvidenceSeal::seal(
                    machine,
                    obligation,
                    evidence.goal.clone(),
                    resolve(&evidence.assumptions)?,
                    resolve(&evidence.axioms)?,
                    evidence.certificate.clone(),
                )
                .map_err(|error| Refusal {
                    reason: RefusalReason::Unsupported,
                    machine,
                    concerns: Construct::Obligation(obligation),
                    detail: format!("evidence encoding {error:?}"),
                })?;
                seals.push(seal);
            }
        }
        Ok(seals)
    }
}

/// Decides every obligation of every machine and fills each machine's
/// `knowledge` table. A refusal here concerns the module itself (an identity
/// that does not resolve, knowledge already present); a refusal of one
/// obligation is its verdict.
pub fn discharge(elaboration: &mut Elaboration) -> Checked<Discharge> {
    discharge_with(elaboration, Knowledge::All)
}

/// Which reasoning `discharge_with` uses. The checker uses `All`; the other
/// two exist only so a measurement can count what branch facts and relations
/// between values each turn from `Unproved` to discharged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Knowledge {
    All,
    WithoutBranchConditions,
    /// No branch facts and no relations between values: literal intervals.
    LiteralIntervals,
}

/// `discharge`, with the knowledge set chosen.
pub fn discharge_with(elaboration: &mut Elaboration, knowledge: Knowledge) -> Checked<Discharge> {
    // Results before inputs: what a helper returns is a fact its caller
    // may pass on to the next private helper, whose inferred input
    // condition is described from the caller's facts.
    // And once more after inputs: a helper that omits `requires` was
    // summarized under no input, and what it returns because of the input
    // its callers establish is a further clause. A summary already in the
    // contract is not added twice.
    let mut unreached = Unreached::new(elaboration);
    result_summaries::infer(elaboration, knowledge, &mut unreached)?;
    entry_inputs::infer(elaboration, knowledge, &mut unreached)?;
    result_summaries::infer(elaboration, knowledge, &mut unreached)?;
    let module = &elaboration.module;
    let mut machines = IdTable::default();
    let identities: Vec<MachineId> = elaboration.tables.machines.keys().collect();
    for id in identities {
        let tables = elaboration.tables.machines.get_mut(id).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Machine,
                "machine tables are past the end",
            )
        })?;
        // A requirement has a contract and no body, so nothing about it is
        // decided here. Its guarantees are each selected implementation's to
        // prove; leaving them without a verdict would read as vacuously held.
        if elaboration.requirements.contains(&id) {
            machines.push(MachineDischarge {
                verdicts: tables.obligations.map(|obligation, _| {
                    Err(refusal(
                        RefusalReason::Unsupported,
                        id,
                        Construct::Obligation(obligation),
                        "a requirement's guarantee is proved by each implementation, not here",
                    ))
                }),
                uses: Vec::new(),
                windows: Vec::new(),
                weakened_joins: Vec::new(),
                conflicts: Vec::new(),
            });
            continue;
        }
        let beyond = unreached.values(terminal_machine(&module.machines, id, id)?, tables);
        machines.push(machine(
            module,
            &elaboration.domains,
            id,
            tables,
            knowledge,
            &beyond,
        )?);
    }
    Ok(Discharge { machines })
}

/// Values that have no defining operation in the machine that names them and
/// still stand for one the integer rules do not reach (`prove::reach`): a
/// formal whose input condition was inferred from a caller's unreached
/// actual, and the result of a call to a private helper whose body returns
/// an unreached value. A goal that depends on one is `Incomplete` when it is
/// not proved, as it would be were the operation in the same machine.
///
/// Only inferred conditions carry this across a call. A written contract is
/// the program's own claim: an arrival or a return that cannot establish it
/// is judged where the operation is, and a caller that needs more than the
/// contract states is rejected.
pub(crate) struct Unreached {
    formals: IdTable<MachineId, Vec<ValueId>>,
    results: IdTable<MachineId, bool>,
}

impl Unreached {
    fn new(elaboration: &Elaboration) -> Self {
        Self {
            formals: elaboration.tables.machines.map(|_, _| Vec::new()),
            results: elaboration.tables.machines.map(|_, _| false),
        }
    }

    /// The unreached values of the machine `lowered`: its marked formals and
    /// the results of its calls of marked helpers.
    pub(crate) fn values(&self, lowered: &TerminalMachine, tables: &MachineTables) -> Vec<ValueId> {
        let mut values = self.formals.get(lowered.id).cloned().unwrap_or_default();
        for call in tables.invocations.values() {
            if let Callee::Declared(semantic_vocabulary::TargetMachineCallable::Machine(callee)) =
                call.callee
                && self.results.get(callee) == Some(&true)
                && let InvocationSite::Operation(operation) = call.site
                && let Some(result) = operation_result(lowered, operation)
            {
                values.push(result);
            }
        }
        values
    }

    pub(crate) fn mark_formal(&mut self, machine: MachineId, formal: ValueId) {
        if let Some(formals) = self.formals.get_mut(machine)
            && !formals.contains(&formal)
        {
            formals.push(formal);
        }
    }

    pub(crate) fn mark_result(&mut self, machine: MachineId) {
        if let Some(result) = self.results.get_mut(machine) {
            *result = true;
        }
    }
}

fn refusal(
    reason: RefusalReason,
    machine: MachineId,
    concerns: Construct,
    detail: &str,
) -> Refusal {
    Refusal {
        reason,
        machine,
        concerns,
        detail: detail.to_owned(),
    }
}

fn terminal_machine(
    machines: &[TerminalMachine],
    id: MachineId,
    from: MachineId,
) -> Checked<&TerminalMachine> {
    usize::try_from(id.get() - 1)
        .ok()
        .and_then(|position| machines.get(position))
        .filter(|machine| machine.id == id)
        .ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                from,
                Construct::Machine,
                "machine is not at its position in the module",
            )
        })
}

fn machine(
    module: &TerminalModule,
    domains: &IdTable<StructuralTypeId, Vec<Proposition>>,
    id: MachineId,
    tables: &mut MachineTables,
    knowledge: Knowledge,
    unreached: &[ValueId],
) -> Checked<MachineDischarge> {
    let machines = &module.machines;
    let structural_types = &module.structural_types;
    let invariants = &module.scalar_block_invariants;
    let lowered = terminal_machine(machines, id, id)?;
    if !tables.knowledge.is_empty() {
        return Err(refusal(
            RefusalReason::Unsupported,
            id,
            Construct::Machine,
            "knowledge rows already present",
        ));
    }
    let Prepared {
        types,
        graph,
        context,
    } = prepare(module, domains, id, tables, knowledge)?;
    let storage = places::Storage::new(
        id,
        lowered,
        structural_types,
        domains,
        &graph,
        tables,
        &types,
    )?;
    let mut decided: Vec<Option<Checked<Discharged>>> =
        (0..tables.obligations.len()).map(|_| None).collect();
    // Loans are judged by structure, not proved (`places::loans`).
    let (lent, conflicts) = storage.loans(module, tables)?;
    for (obligation, verdict) in lent {
        let slot = decided
            .get_mut(usize::try_from(obligation.get() - 1).unwrap_or(usize::MAX))
            .ok_or_else(|| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    Construct::Obligation(obligation),
                    "a loan row past the end of this machine",
                )
            })?;
        *slot = Some(verdict.map(|()| Discharged::Lent));
    }
    // Declared block invariants (a loop head's, the `requires` of the
    // machine lowered beneath it) and inferred join predicates are one kind
    // of row: each is known from its block's entry on and is proved on every
    // arrival before it may justify anything downstream (fact_validity.md:
    // "Every arrival must check before the destination predicate can justify
    // downstream obligations."). A declared one holds because every arrival
    // proves it, and the closure is accepted only when every obligation is
    // discharged (`judge`).
    let declared = joins::prepare(
        lowered,
        id,
        tables,
        &graph,
        &context,
        invariants,
        knowledge == Knowledge::All,
    )?;
    let weakened_joins = joins::check(
        &joins::Machine {
            module,
            lowered,
            storage: &storage,
            id,
            types: &types,
            graph: &graph,
            context: &context,
            relations: knowledge != Knowledge::LiteralIntervals,
            declared: &declared,
            unreached,
        },
        tables,
        &mut decided,
    )?;
    // Certificates next; a published clause is then decided from its exits.
    for (index, (obligation, row)) in tables.obligations.iter().enumerate() {
        if decided[index].is_some() {
            continue;
        }
        if let ObligationKind::PublishedGuarantee { .. } = row.kind {
            continue;
        }
        let verdict = goal(
            module, lowered, &storage, id, obligation, &row.kind, tables, &types,
        )
        .and_then(|goal| {
            decide(
                &context,
                &graph,
                tables,
                id,
                obligation,
                row.created_at,
                goal,
                knowledge != Knowledge::LiteralIntervals,
                unreached,
            )
            .map(Discharged::Certificate)
        });
        decided[index] = Some(verdict);
    }
    let mut verdicts: IdTable<ObligationId, Checked<Discharged>> = IdTable::default();
    for ((obligation, row), verdict) in tables.obligations.iter().zip(&decided) {
        verdicts.push(match (verdict, row.kind) {
            (Some(verdict), _) => verdict.clone(),
            (None, ObligationKind::PublishedGuarantee { clause }) => {
                published(tables, id, obligation, clause, &decided)
            }
            (None, _) => Err(refusal(
                RefusalReason::Unsupported,
                id,
                Construct::Obligation(obligation),
                "obligation left undecided",
            )),
        });
    }
    // A store or a zero establishment that was not shown to keep a clause
    // does not fail: it opens an invariant window, and the clause is owed
    // at the consumption points the window reaches (dependent_values.md,
    // "Invariant windows"). A construction makes a value and must prove its
    // clauses where it stands, so its unproved clause stays a refusal.
    let mut windows: Vec<places::Window> = Vec::new();
    let mut restoring: Vec<(places::Owner, OperationId)> = Vec::new();
    let mut settled = IdTable::default();
    for ((obligation, row), (_, verdict)) in tables.obligations.iter().zip(verdicts.iter()) {
        let ObligationKind::DataDomain { operation, clause } = row.kind else {
            settled.push(verdict.clone());
            continue;
        };
        let concerns = Construct::Obligation(obligation);
        let stores = storage.store(operation)?;
        let zeroes = storage.zero_establishment(operation)?;
        settled.push(match verdict {
            // data_and_literals.md: "Construction and every restoring write
            // prove the owning default-domain clauses."
            Ok(Discharged::Certificate(_)) if stores => {
                restoring.push((storage.owner(operation, clause, concerns)?, operation));
                verdict.clone()
            }
            Err(unproved)
                if matches!(unproved.reason, RefusalReason::Unproved(_)) && (stores || zeroes) =>
            {
                let owner = storage.owner(operation, clause, concerns)?;
                if stores && storage.untouched(operation, &owner, concerns)? {
                    Ok(Discharged::Preserved)
                } else {
                    windows.push(places::Window {
                        obligation,
                        opened_by: operation,
                        owner,
                        initial: zeroes,
                        restored: Vec::new(),
                    });
                    Ok(Discharged::Window(Box::new(unproved.clone())))
                }
            }
            verdict => verdict.clone(),
        });
    }
    for window in &mut windows {
        window.restored = restoring
            .iter()
            .filter(|(owner, _)| *owner == window.owner)
            .map(|(_, operation)| *operation)
            .collect();
    }
    // A store not shown to keep a clause while that clause's window on the
    // same place is already open opens no second window: the window stays
    // open and the store only changes the contents it holds
    // (dependent_values.md: "whose actual new contents remain available to
    // ordinary flow reasoning"). Its consumption points are the first
    // window's, which names the write that opened it. Zeroed storage is the
    // exception: a store into it makes the leaf hold a written value, so a
    // store that does not keep the clause keeps its own window.
    let mut opened: Vec<places::Window> = Vec::with_capacity(windows.len());
    for window in windows {
        let mut within = false;
        for earlier in &opened {
            if !earlier.initial
                && earlier.owner == window.owner
                && storage.open_before(tables, earlier, window.opened_by)?
            {
                within = true;
                break;
            }
        }
        if !within {
            opened.push(window);
        }
    }
    let windows = opened;
    let mut uses = storage.uses(tables)?;
    let mut closures = Vec::new();
    for consumption in storage.consumptions(tables, &windows)? {
        let window = windows.get(consumption.window).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                id,
                consumption.at,
                "a consumption names no window",
            )
        })?;
        let proved = consumption.goal.and_then(|goal| {
            decide(
                &context,
                &graph,
                tables,
                id,
                window.obligation,
                consumption.point,
                goal,
                knowledge != Knowledge::LiteralIntervals,
                unreached,
            )
        });
        let verdict = match proved {
            // Zeroed storage of a gated type that is observed before its
            // domain holds was never an established value: the use itself
            // is unestablished, which is the initialization judgment's
            // verdict on it.
            Err(unproved)
                if window.initial && matches!(unproved.reason, RefusalReason::Unproved(_)) =>
            {
                let unestablished = Refusal {
                    reason: RefusalReason::Unestablished,
                    machine: id,
                    concerns: consumption.at,
                    detail: format!(
                        "zeroed storage of a gated type is used before its requires holds: {}",
                        unproved.detail
                    ),
                };
                let used = uses
                    .iter_mut()
                    .find(|used| used.at == consumption.at)
                    .ok_or_else(|| {
                        refusal(
                            RefusalReason::ForeignIdentity,
                            id,
                            consumption.at,
                            "a consumption point is no use of storage",
                        )
                    })?;
                if used.verdict.is_ok() {
                    used.verdict = Err(unestablished.clone());
                }
                Err(unestablished)
            }
            proved => proved,
        };
        closures.push(WindowUse {
            opened_by: window.obligation,
            at: consumption.at,
            verdict,
        });
    }
    Ok(MachineDischarge {
        verdicts: settled,
        uses,
        windows: closures,
        weakened_joins,
        conflicts,
    })
}

/// Fills one machine's knowledge table from its lowered form: entry
/// `requires`, result equations and field-read equations, branch
/// conditions, dispatch bindings and callee guarantees.
fn fill_knowledge(
    module: &TerminalModule,
    id: MachineId,
    lowered: &TerminalMachine,
    tables: &mut MachineTables,
    knowledge: Knowledge,
    types: &IdTable<ValueId, ScalarType>,
    storage: &places::Storage<'_>,
) -> Checked<()> {
    for (clause, proposition) in lowered.contract.requires.iter().enumerate() {
        // A clause over a field of a record formal is about what that field
        // held at entry, which is a value when some operation names the
        // field. One that names no such value states nothing here.
        let stated = match substitute(proposition, &mut storage.at_entry(tables)) {
            Ok(stated) => stated,
            Err(Unbound::Unproved(_)) => continue,
            Err(unbound) => return Err(unbound.refusal(id, Construct::Machine)),
        };
        push_knowledge(
            tables,
            stated,
            ProgramPoint::BlockEntry(lowered.entry),
            Justification::EntryRequirement {
                clause: position(clause, id)?,
            },
        );
    }
    for block in &lowered.blocks {
        for operation in &block.operations {
            if let Some(equation) = result_equation(id, operation, types)? {
                push_knowledge(
                    tables,
                    equation,
                    ProgramPoint::AfterOperation(operation.id),
                    Justification::OperationSemantics(operation.id),
                );
            }
            // A field read's result is a fresh value; what it equals comes
            // from the write log, not from the operation alone.
            if let Some(equation) = storage.read_equation(tables, operation)? {
                push_knowledge(
                    tables,
                    equation,
                    ProgramPoint::AfterOperation(operation.id),
                    Justification::OperationSemantics(operation.id),
                );
            }
            // And what the type of the storage it reads says of that value.
            for premise in storage.read_premises(tables, operation)? {
                push_knowledge(
                    tables,
                    premise,
                    ProgramPoint::AfterOperation(operation.id),
                    Justification::OperationSemantics(operation.id),
                );
            }
        }
        // A conditional's condition holds on its true edge and fails on its
        // false edge. Only a block that edge alone enters is dominated by it.
        if let Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } = &block.terminator
            && knowledge == Knowledge::All
        {
            for (edge, holds) in [(when_true.edge, true), (when_false.edge, false)] {
                push_knowledge(
                    tables,
                    Proposition::Equal(
                        ScalarTerm::value(*condition, ScalarType::Boolean),
                        ScalarTerm::Boolean(holds),
                    ),
                    ProgramPoint::Edge(edge),
                    Justification::BranchCondition { edge, holds },
                );
            }
        }
    }
    // What each edge of a dispatch binds: its payload parameters, each
    // equal to what the payload leaf holds, and each under the clauses the
    // type of that storage states of it, as a payload read would be. The
    // rows hold because that edge was taken and are available on it.
    if knowledge == Knowledge::All {
        for block in &lowered.blocks {
            let equations = storage.payload_equations(tables, block)?;
            let premises = storage.payload_premises(tables, block)?;
            for (edge, proposition) in equations.into_iter().chain(
                premises
                    .into_iter()
                    .map(|(edge, _, premise)| (edge, premise)),
            ) {
                push_knowledge(
                    tables,
                    proposition,
                    ProgramPoint::Edge(edge),
                    Justification::DispatchBinding { edge },
                );
            }
        }
    }
    for invocation in tables.invocations.keys().collect::<Vec<_>>() {
        let row = terminal_check_tables::row(
            &tables.invocations,
            invocation,
            id,
            Construct::Invocation(invocation),
        )?;
        // A call with no normal completion is a terminator. Its callee's
        // guarantees "apply only on normal return" (calls_and_outcomes.md), and
        // it has none, so it adds no knowledge.
        let operation = match row.site {
            InvocationSite::Operation(operation) => operation,
            InvocationSite::InvokeNoNormal(_) => continue,
            _ => {
                return Err(refusal(
                    RefusalReason::Unsupported,
                    id,
                    Construct::Invocation(invocation),
                    "invocation outside an operation or a no-normal call",
                ));
            }
        };
        let callee = callee(module, id, invocation, &row.callee)?;
        let actuals = scalar_actuals(&row.actuals);
        let erased = callable_erased_actuals(module, lowered, callee, id, invocation, row.site)?;
        let ensures = module.callable_ensures(callee).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Invocation(invocation),
                "callee has no canonical guarantee contract",
            )
        })?;
        for (clause, ensures) in ensures.enumerate() {
            let proposition = substitute(
                ensures,
                &mut ErasedTerms {
                    terms: &erased,
                    runtime: &mut ScalarActuals {
                        actuals: &actuals,
                        result: operation_result(lowered, operation),
                        tables,
                    },
                },
            )
            .map_err(|unbound| unbound.refusal(id, Construct::Invocation(invocation)))?;
            push_knowledge(
                tables,
                proposition,
                ProgramPoint::AfterOperation(operation),
                Justification::CalleeGuarantee {
                    invocation,
                    clause: position(clause, id)?,
                },
            );
        }
    }

    Ok(())
}

/// What deciding one machine's obligations needs: value types, dominance,
/// and the proposition context; filling the knowledge table on the way.
pub(crate) struct Prepared {
    pub(crate) types: IdTable<ValueId, ScalarType>,
    pub(crate) graph: Graph,
    pub(crate) context: PropositionContext,
}

/// Fills `tables.knowledge` from the lowered form and builds what deciding
/// needs. Knowledge already present is kept: it is this machine's own.
pub(crate) fn prepare(
    module: &TerminalModule,
    domains: &IdTable<StructuralTypeId, Vec<Proposition>>,
    id: MachineId,
    tables: &mut MachineTables,
    knowledge: Knowledge,
) -> Checked<Prepared> {
    let machines = &module.machines;
    let structural_types = &module.structural_types;
    let lowered = terminal_machine(machines, id, id)?;
    let fill = tables.knowledge.is_empty();
    let types = tables.values.map(|_, row| row.scalar_type);
    // Before anything is decided: the rows must be the ones the lowered form
    // owes, at its points, or deciding every row proves nothing (`minted`).
    // A consumer re-checking a saved proof decides rows too, so it checks
    // them the same way.
    minted::follow_the_lowered_form(id, module, lowered, tables, &types)?;
    minted::domains_are_declared(id, structural_types, domains)?;
    let graph = Graph::new(id, lowered, tables)?;
    if fill {
        let storage = places::Storage::new(
            id,
            lowered,
            structural_types,
            domains,
            &graph,
            tables,
            &types,
        )?;
        storage.owed_are_minted(tables)?;
        fill_knowledge(module, id, lowered, tables, knowledge, &types, &storage)?;
    }
    // A value of a type parameter has no type the kernel could hold it at,
    // so the context does not list it (`concrete`).
    let context = PropositionContext::from_value_types(
        types
            .iter()
            .filter(|(value, _)| {
                tables
                    .values
                    .get(*value)
                    .is_none_or(|row| row.rigid.is_none())
            })
            .map(|(value, scalar_type)| (value, *scalar_type)),
    )
    .map_err(|_| {
        refusal(
            RefusalReason::Unsupported,
            id,
            Construct::Machine,
            "value types do not form a proposition context",
        )
    })?;
    Ok(Prepared {
        types,
        graph,
        context,
    })
}

/// A published `ensures` clause holds when every `Destination::Return` edge
/// has a certificate-discharged `NormalGuarantee` for it naming this
/// published identity; vacuously when there is no return edge.
/// control_flow.md: "All normal returns, including returns after a cycle,
/// must establish the published guarantees."
fn published(
    tables: &MachineTables,
    id: MachineId,
    published: ObligationId,
    clause: u32,
    decided: &[Option<Checked<Discharged>>],
) -> Checked<Discharged> {
    let unproved = |detail: &str| {
        refusal(
            RefusalReason::Unproved(UnprovedCause::ReturnDoesNotEstablish),
            id,
            Construct::Obligation(published),
            detail,
        )
    };
    let mut exits = Vec::new();
    for (edge, row) in tables.edges.iter() {
        if row.destination != Destination::Return {
            continue;
        }
        let guarantee = tables.obligations.iter().find(|(_, candidate)| {
            candidate.kind
                == ObligationKind::NormalGuarantee {
                    exit: edge,
                    clause,
                    published,
                }
        });
        let Some((obligation, _)) = guarantee else {
            return Err(unproved(
                "a normal return owes no guarantee for this clause",
            ));
        };
        let position = usize::try_from(obligation.get() - 1).unwrap_or(usize::MAX);
        match decided.get(position) {
            Some(Some(Ok(Discharged::Certificate(_)))) => exits.push(obligation),
            _ => return Err(unproved("a normal return does not establish this clause")),
        }
    }
    Ok(Discharged::EveryExit(exits))
}

fn push_knowledge(
    tables: &mut MachineTables,
    proposition: Proposition,
    available_from: ProgramPoint,
    justification: Justification,
) {
    let proposition = tables.propositions.push(proposition);
    tables.knowledge.push(KnowledgeRow {
        proposition,
        available_from,
        justification,
        premises: Vec::new(),
    });
}

/// The proposition a knowledge row states.
fn knowledge_proposition(tables: &MachineTables, row: KnowledgeId) -> Option<&Proposition> {
    let row = tables.knowledge.get(row)?;
    tables.propositions.get(row.proposition)
}

fn position(index: usize, machine: MachineId) -> Checked<u32> {
    u32::try_from(index).map_err(|_| {
        refusal(
            RefusalReason::Unsupported,
            machine,
            Construct::Machine,
            "clause position exceeds u32",
        )
    })
}

fn semantics_refusal(
    machine: MachineId,
    operation: semantic_vocabulary::OperationId,
) -> impl Fn(terminal_semantics::OperationSemanticError) -> Refusal {
    move |error| Refusal {
        reason: RefusalReason::Unsupported,
        machine,
        concerns: Construct::Operation(operation),
        detail: format!("operation semantics {error:?}"),
    }
}

/// The equation an operation establishes on its normal successor, as
/// Terminal semantics states it, or `None` for an operation with none (a
/// call).
fn result_equation(
    machine: MachineId,
    operation: &terminal_psi::Operation,
    types: &IdTable<ValueId, ScalarType>,
) -> Checked<Option<Proposition>> {
    let refuse = semantics_refusal(machine, operation.id);
    if let Some(semantics) =
        terminal_semantics::proof_bearing_scalar_leaf_semantics(operation, types)
            .map_err(&refuse)?
    {
        return Ok(Some(semantics.result_equation().clone()));
    }
    Ok(
        terminal_semantics::goal_free_scalar_leaf_semantics(operation, types)
            .map_err(&refuse)?
            .map(|semantics| semantics.result_equation().clone()),
    )
}

pub(crate) fn callee(
    module: &TerminalModule,
    machine: MachineId,
    invocation: terminal_check_tables::InvocationId,
    callee: &Callee,
) -> Checked<semantic_vocabulary::TargetMachineCallable> {
    let concerns = Construct::Invocation(invocation);
    let unavailable = |detail| refusal(RefusalReason::Unsupported, machine, concerns, detail);
    let Callee::Declared(callee) = *callee else {
        return Err(unavailable("call through a selector value"));
    };
    match callee {
        semantic_vocabulary::TargetMachineCallable::Machine(callee) => {
            terminal_machine(&module.machines, callee, machine)?;
        }
        semantic_vocabulary::TargetMachineCallable::Boundary(boundary) => {
            let declaration = module.boundary_machine(boundary).ok_or_else(|| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    machine,
                    concerns,
                    "boundary declaration is unavailable",
                )
            })?;
            if declaration.declaration.is_none()
                || declaration.role != terminal_psi::BoundaryDeclarationRole::Callable
                || declaration.completion != semantic_vocabulary::BoundaryCompletion::NormalReturn
            {
                return Err(unavailable(
                    "boundary call has no supported exact callable contract",
                ));
            }
            if declaration.declaration.as_ref().is_some_and(|checked| {
                checked.source.published_may_suspend || checked.source.published_may_block
            }) {
                return Err(unavailable(
                    "boundary call needs its suspension and blocking proof",
                ));
            }
            if !declaration.requires.is_empty()
                || !declaration.normal_qualifications.is_empty()
                || !declaration.scalar_domain_requirements.is_empty()
                || declaration.structural_parameters.iter().any(|formal| {
                    !formal.qualifications.is_empty()
                        || !formal.projected_qualifications.is_empty()
                        || !formal.additional_input_qualifications.is_empty()
                })
            {
                return Err(unavailable(
                    "boundary call needs its structural qualification proof",
                ));
            }
            if !declaration.reference_outputs.is_empty()
                || !declaration.result_reference_regions.is_empty()
                || !declaration.program_local_root_introductions.is_empty()
                || !declaration.content_guarantees.is_empty()
                || matches!(&declaration.result, terminal_psi::BoundaryMachineResult::Structural(result)
                    if !result.qualifications.is_empty() || !result.projected_qualifications.is_empty())
            {
                return Err(unavailable(
                    "boundary call needs its result, reference and content custody proof",
                ));
            }
            if !declaration.fixed_service_reach.is_empty()
                || !declaration.published_service_ceiling.is_empty()
            {
                return Err(unavailable(
                    "boundary call needs its nonempty declared reach proof",
                ));
            }
        }
    }
    Ok(callee)
}

fn callable_erased_actuals(
    module: &TerminalModule,
    caller: &TerminalMachine,
    callee: semantic_vocabulary::TargetMachineCallable,
    machine: MachineId,
    invocation: terminal_check_tables::InvocationId,
    site: InvocationSite,
) -> Checked<Vec<(ValueId, ScalarTerm)>> {
    match callee {
        semantic_vocabulary::TargetMachineCallable::Machine(callee) => erased_actuals(
            caller,
            &terminal_machine(&module.machines, callee, machine)?.contract,
            machine,
            invocation,
            site,
        ),
        // BoundaryCall has no erased operand lane, and structural reading
        // refuses a requirement whose contract would need one.
        semantic_vocabulary::TargetMachineCallable::Boundary(_) => Ok(Vec::new()),
    }
}

/// A call's scalar lane: the actuals a callee clause's formals stand for.
/// Elaboration numbers a machine's scalar formals apart from its record
/// formals (the `k`-th scalar formal is its value `k + 1`), so a clause term
/// is replaced by position in this lane. A record actual is a place and has
/// no value identity: a clause names a field of it, which stands for what
/// that field of the actual holds at the call (`places`, `AtCall`).
pub(crate) fn scalar_actuals(actuals: &[Actual]) -> Vec<ValueId> {
    let mut scalars = Vec::with_capacity(actuals.len());
    for actual in actuals {
        match actual {
            Actual::Value(value) => scalars.push(*value),
            Actual::Place(_) => {}
        }
    }
    scalars
}

/// Why a clause could not be stated over this machine's values.
pub(crate) enum Unbound {
    /// The clause has a form or names something this route does not serve.
    Unsupported(&'static str),
    /// A name of the clause stands for nothing with a value identity where
    /// the clause is used, so the clause cannot be proved there.
    Unproved(&'static str),
    /// Resolving a name refused.
    Refused(Refusal),
}

impl Unbound {
    fn refusal(self, machine: MachineId, concerns: Construct) -> Refusal {
        match self {
            Self::Unsupported(detail) => {
                refusal(RefusalReason::Unsupported, machine, concerns, detail)
            }
            // The clause cannot be stated over values here, so no derivation
            // of it exists from what this point knows.
            Self::Unproved(detail) => refusal(
                RefusalReason::Unproved(UnprovedCause::NoDerivation),
                machine,
                concerns,
                detail,
            ),
            Self::Refused(refusal) => refusal,
        }
    }
}

/// What each name of a clause stands for where the clause is used. A clause
/// is written over its owner's names: a callee's formals, or the fields of
/// the value a data type's `requires` is about.
pub(crate) trait Bindings {
    /// The term a value of the clause's owner stands for.
    fn value(&mut self, id: ValueId, scalar_type: ScalarType) -> Result<ScalarTerm, Unbound>;
    /// The value the scalar field at `path` beneath `root` holds.
    fn field(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Unbound>;
    /// Whether the sum at `path` beneath `root` is `case`, by structure.
    fn case(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        case: StructuralCaseId,
    ) -> Result<bool, Unbound>;
}

/// A call's erased lane: each `[erased]` scalar formal of the callee with the
/// term the call supplies for it. `wiki/spec/proofs/contracts.md`, "Explicit
/// erased bindings": "Construction supplies the erased term". The check
/// tables hold no row for such an actual, because it is a term and not a
/// value or a place, so it is read from the call in the lowered form, where
/// Terminal Psi keeps it (`erased_arguments`). A term names values of the
/// caller or literals; any other form is refused, as in `substitute`.
fn erased_actuals(
    caller: &TerminalMachine,
    contract: &terminal_psi::MachineContract,
    id: MachineId,
    invocation: terminal_check_tables::InvocationId,
    site: InvocationSite,
) -> Checked<Vec<(ValueId, ScalarTerm)>> {
    let refuse = |reason, detail| refusal(reason, id, Construct::Invocation(invocation), detail);
    if contract.erased_scalar_formals.is_empty() {
        return Ok(Vec::new());
    }
    if !contract.erased_proof_formals.is_empty() {
        return Err(refuse(
            RefusalReason::Unsupported,
            "callee with an erased proof formal",
        ));
    }
    let supplied = caller
        .module_invocations()
        .find(|candidate| candidate.site == site)
        .ok_or_else(|| {
            refuse(
                RefusalReason::ForeignIdentity,
                "invocation is not in this machine",
            )
        })?
        .inputs
        .erased_arguments;
    if supplied.len() != contract.erased_scalar_formals.len() {
        return Err(refuse(
            RefusalReason::ForeignIdentity,
            "call supplies another number of erased terms than its callee declares",
        ));
    }
    contract
        .erased_scalar_formals
        .iter()
        .zip(supplied)
        .map(|(formal, term)| match term {
            ScalarTerm::Value { .. } | ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_) => {
                Ok((formal.id, term.clone()))
            }
            _ => Err(refuse(
                RefusalReason::Unsupported,
                "erased actual outside values and literals",
            )),
        })
        .collect()
}

/// A call's erased terms standing for the callee's `[erased]` scalar
/// formals, over the bindings of its runtime formals. An erased formal's
/// identity follows the callee's result pseudo-value
/// (`Signature::formal_values`), so it never collides with a runtime formal;
/// every other name is the runtime bindings' to resolve.
pub(crate) struct ErasedTerms<'a> {
    pub(crate) terms: &'a [(ValueId, ScalarTerm)],
    pub(crate) runtime: &'a mut dyn Bindings,
}

impl Bindings for ErasedTerms<'_> {
    fn value(&mut self, id: ValueId, scalar_type: ScalarType) -> Result<ScalarTerm, Unbound> {
        let Some((_, supplied)) = self.terms.iter().find(|(formal, _)| *formal == id) else {
            return self.runtime.value(id, scalar_type);
        };
        if supplied.scalar_type() != scalar_type {
            return Err(Unbound::Unsupported(
                "erased actual type differs from the formal's",
            ));
        }
        Ok(supplied.clone())
    }

    fn field(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Unbound> {
        self.runtime.field(root, path, scalar_type)
    }

    fn case(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        case: StructuralCaseId,
    ) -> Result<bool, Unbound> {
        self.runtime.case(root, path, case)
    }
}

/// A call's actuals standing for the callee's formals: the callee's `k`-th
/// scalar formal (callee value `k + 1`) is the caller's `k`-th scalar
/// actual. In a guarantee, the callee's result pseudo-value (the value after
/// its scalar formals) is `result`, the value the call defines
/// (calls_and_outcomes.md: "Normal guarantees refer to the actual result and
/// appropriate parameter versions."). A `requires` has no result, and a
/// clause naming one where none is given is refused.
pub(crate) struct ScalarActuals<'a> {
    pub(crate) actuals: &'a [ValueId],
    pub(crate) result: Option<ValueId>,
    pub(crate) tables: &'a MachineTables,
}

/// The caller's scalar actual a callee's scalar formal stands for.
pub(crate) fn scalar_actual(
    actuals: &[ValueId],
    tables: &MachineTables,
    id: ValueId,
    scalar_type: ScalarType,
) -> Result<ScalarTerm, Unbound> {
    let actual = usize::try_from(id.get() - 1)
        .ok()
        .and_then(|position| actuals.get(position))
        .ok_or(Unbound::Unsupported(
            "clause names a value that is not a formal",
        ))?;
    let row = tables.values.get(*actual).ok_or(Unbound::Unsupported(
        "actual is past the end of the caller's values",
    ))?;
    if row.scalar_type != scalar_type {
        return Err(Unbound::Unsupported(
            "actual type differs from the formal's",
        ));
    }
    Ok(ScalarTerm::value(*actual, scalar_type))
}

impl Bindings for ScalarActuals<'_> {
    fn value(&mut self, id: ValueId, scalar_type: ScalarType) -> Result<ScalarTerm, Unbound> {
        match self.result {
            Some(result) if id.get() == self.actuals.len() as u64 + 1 => {
                let row = self.tables.values.get(result).ok_or(Unbound::Unsupported(
                    "call result is past the end of the caller's values",
                ))?;
                if row.scalar_type != scalar_type {
                    return Err(Unbound::Unsupported(
                        "call result type differs from the callee's",
                    ));
                }
                Ok(ScalarTerm::value(result, scalar_type))
            }
            _ => scalar_actual(self.actuals, self.tables, id, scalar_type),
        }
    }

    fn field(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: ScalarType,
    ) -> Result<ScalarTerm, Unbound> {
        Err(Unbound::Unsupported(
            "clause term outside the scalar subset",
        ))
    }

    fn case(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: StructuralCaseId,
    ) -> Result<bool, Unbound> {
        Err(Unbound::Unsupported(
            "clause form outside the scalar subset",
        ))
    }
}

/// The one substitution: states a clause over this machine's values by
/// replacing each of its names with what `bindings` says it stands for. The
/// result names values and literals only; a place never survives it.
///
/// A case test is not a value: where the clause is used the case is known by
/// structure or not at all, so the test becomes `Truth` or `Falsehood` and
/// the connective around it is simplified. A conjunction with a false
/// conjunct is false and a disjunction with a true disjunct is true whatever
/// their other parts are, which is what lets a clause project a payload
/// under the test of its case (data_and_literals.md: "inactive payloads
/// cannot be projected"): the projection is only resolved when the test did
/// not already decide its connective.
///
/// Any other form is refused: substituting under a construct this function
/// does not know would be a silent change of meaning.
pub(crate) fn substitute(
    proposition: &Proposition,
    bindings: &mut dyn Bindings,
) -> Result<Proposition, Unbound> {
    Ok(match proposition {
        Proposition::Truth => Proposition::Truth,
        Proposition::Falsehood => Proposition::Falsehood,
        Proposition::Equal(left, right) => {
            Proposition::Equal(bound(left, bindings)?, bound(right, bindings)?)
        }
        Proposition::LessThan(left, right) => {
            Proposition::LessThan(bound(left, bindings)?, bound(right, bindings)?)
        }
        Proposition::LessOrEqual(left, right) => {
            Proposition::LessOrEqual(bound(left, bindings)?, bound(right, bindings)?)
        }
        Proposition::IntegerMathEqual(left, right) => Proposition::IntegerMathEqual(
            bound_mathematical(left, bindings)?,
            bound_mathematical(right, bindings)?,
        ),
        Proposition::IntegerMathLessThan(left, right) => Proposition::IntegerMathLessThan(
            bound_mathematical(left, bindings)?,
            bound_mathematical(right, bindings)?,
        ),
        Proposition::IntegerMathLessOrEqual(left, right) => Proposition::IntegerMathLessOrEqual(
            bound_mathematical(left, bindings)?,
            bound_mathematical(right, bindings)?,
        ),
        Proposition::StructuralCaseMembership { subject, case } => {
            if bindings.case(subject.root(), subject.path(), *case)? {
                Proposition::Truth
            } else {
                Proposition::Falsehood
            }
        }
        Proposition::Conjunction(parts) => connective(parts, bindings, true)?,
        Proposition::Disjunction(parts) => connective(parts, bindings, false)?,
        _ => {
            return Err(Unbound::Unsupported(
                "clause form outside the scalar subset",
            ));
        }
    })
}

/// One term of a clause under substitution.
fn bound(term: &ScalarTerm, bindings: &mut dyn Bindings) -> Result<ScalarTerm, Unbound> {
    match term {
        ScalarTerm::Value { id, scalar_type } => bindings.value(*id, *scalar_type),
        ScalarTerm::IntegerField {
            root,
            path,
            scalar_type,
        } => bindings.field(*root, path, ScalarType::Integer(*scalar_type)),
        ScalarTerm::BooleanField { root, path } => bindings.field(*root, path, ScalarType::Boolean),
        ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_) => Ok(term.clone()),
        ScalarTerm::StructuralRead { .. } => Err(Unbound::Unsupported(
            "clause structural read is outside the scalar subset",
        )),
        _ if !term.operands().is_empty() => {
            let mut bound_term = term.clone();
            for operand in bound_term.operands_mut() {
                *operand = bound(operand, bindings)?;
            }
            Ok(bound_term)
        }
        _ => Err(Unbound::Unsupported(
            "clause term outside the scalar subset",
        )),
    }
}

/// One mathematical-integer term of a clause under substitution. An embedded
/// name is replaced by what `bindings` says it stands for, embedded again:
/// the value keeps its carrier, and a literal actual is the mathematical
/// integer it denotes (`IntegerMathTerm::embed`). The arithmetic around it is
/// over unbounded integers and is kept as written
/// (mathematical_values.md: "These terms denote unbounded mathematical
/// integers and cannot overflow").
fn bound_mathematical(
    term: &IntegerMathTerm,
    bindings: &mut dyn Bindings,
) -> Result<IntegerMathTerm, Unbound> {
    let embedded = |term: ScalarTerm| {
        IntegerMathTerm::embed(term).ok_or(Unbound::Unsupported(
            "clause embeds a term that is not an integer",
        ))
    };
    match term {
        IntegerMathTerm::IntegerLiteral(_) => Ok(term.clone()),
        IntegerMathTerm::MathValue { source_type, value } => {
            embedded(bindings.value(*value, ScalarType::Integer(*source_type))?)
        }
        IntegerMathTerm::Embed(scalar) => embedded(bound(scalar, bindings)?),
        IntegerMathTerm::Add(..)
        | IntegerMathTerm::Subtract(..)
        | IntegerMathTerm::Multiply(..)
        | IntegerMathTerm::ShiftLeft { .. } => {
            let mut bound_term = term.clone();
            for operand in bound_term.operands_mut() {
                *operand = bound_mathematical(operand, bindings)?;
            }
            Ok(bound_term)
        }
    }
}

/// A conjunction (`conjunction`) or disjunction under substitution. The
/// deciding constant is `Falsehood` for a conjunction and `Truth` for a
/// disjunction: one part equal to it decides the whole, and then a part
/// that could not be stated does not matter. The other constant is the
/// connective's unit and is dropped.
fn connective(
    parts: &[Proposition],
    bindings: &mut dyn Bindings,
    conjunction: bool,
) -> Result<Proposition, Unbound> {
    let (deciding, unit) = if conjunction {
        (Proposition::Falsehood, Proposition::Truth)
    } else {
        (Proposition::Truth, Proposition::Falsehood)
    };
    let mut kept = Vec::with_capacity(parts.len());
    let mut unstated = None;
    for part in parts {
        match substitute(part, bindings) {
            Ok(part) if part == deciding => return Ok(deciding),
            Ok(part) if part == unit => {}
            Ok(part) => kept.push(part),
            Err(unbound) => unstated = unstated.or(Some(unbound)),
        }
    }
    if let Some(unbound) = unstated {
        return Err(unbound);
    }
    Ok(match kept.len() {
        0 => unit,
        1 => kept.remove(0),
        _ if conjunction => Proposition::Conjunction(kept),
        _ => Proposition::Disjunction(kept),
    })
}

/// The goal of one obligation, over this machine's values.
#[allow(clippy::too_many_arguments)]
fn goal(
    module: &TerminalModule,
    lowered: &TerminalMachine,
    storage: &places::Storage<'_>,
    id: MachineId,
    obligation: ObligationId,
    kind: &ObligationKind,
    tables: &MachineTables,
    types: &IdTable<ValueId, ScalarType>,
) -> Checked<Proposition> {
    let goal = stated_goal(
        module, lowered, storage, id, obligation, kind, tables, types,
    )?;
    concrete(tables, id, Construct::Obligation(obligation), &goal)?;
    Ok(goal)
}

/// Refuses a proposition that names a value of a type parameter.
///
/// A generic check form declares such a value with a stand-in type
/// (`RigidType::DECLARED_AS`), because the lowered form has no other way to
/// declare one. Nothing may be proved about it from that declaration, and no
/// proof rule is yet marked as holding at every type, so a goal or a
/// hypothesis over such a value is refused as outside the route: every goal
/// passes here from `goal`, and every hypothesis from `hypotheses`.
fn concrete(
    tables: &MachineTables,
    id: MachineId,
    concerns: Construct,
    proposition: &Proposition,
) -> Checked<()> {
    let rigid = proposition.any_value_id(|value| {
        tables
            .values
            .get(value)
            .is_some_and(|row| row.rigid.is_some())
    });
    if rigid {
        return Err(refusal(
            RefusalReason::Unsupported,
            id,
            concerns,
            "a proposition names a value of a type parameter",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn stated_goal(
    module: &TerminalModule,
    lowered: &TerminalMachine,
    storage: &places::Storage<'_>,
    id: MachineId,
    obligation: ObligationId,
    kind: &ObligationKind,
    tables: &MachineTables,
    types: &IdTable<ValueId, ScalarType>,
) -> Checked<Proposition> {
    let concerns = Construct::Obligation(obligation);
    match *kind {
        ObligationKind::OperationSafety { site } => storage.safety_goal(site, obligation, types),
        ObligationKind::CallRequirement { invocation, clause } => {
            let row = terminal_check_tables::row(
                &tables.invocations,
                invocation,
                id,
                Construct::Invocation(invocation),
            )?;
            let callee = callee(module, id, invocation, &row.callee)?;
            let erased =
                callable_erased_actuals(module, lowered, callee, id, invocation, row.site)?;
            let clauses = module.callable_requires(callee).ok_or_else(|| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    concerns,
                    "callee has no canonical requires contract",
                )
            })?;
            let formals = module
                .callable_structural_parameters(callee)
                .ok_or_else(|| {
                    refusal(
                        RefusalReason::ForeignIdentity,
                        id,
                        concerns,
                        "callee has no canonical structural formals",
                    )
                })?;
            let clause = usize::try_from(clause)
                .ok()
                .and_then(|clause| clauses.get(clause))
                .ok_or_else(|| {
                    refusal(
                        RefusalReason::ForeignIdentity,
                        id,
                        concerns,
                        "callee has no such requires clause",
                    )
                })?;
            // A scalar formal is the scalar actual in its lane; a field of a
            // record formal is what that field of the record actual holds
            // when the call is made.
            let over_actuals = |runtime: &mut dyn Bindings| {
                substitute(
                    clause,
                    &mut ErasedTerms {
                        terms: &erased,
                        runtime,
                    },
                )
            };
            match row.site {
                InvocationSite::Operation(site) => over_actuals(&mut storage.at_call(
                    tables,
                    &row.actuals,
                    formals,
                    site,
                    concerns,
                )?),
                InvocationSite::InvokeNoNormal(_) => {
                    let actuals = scalar_actuals(&row.actuals);
                    over_actuals(&mut ScalarActuals {
                        actuals: &actuals,
                        result: None,
                        tables,
                    })
                }
                _ => {
                    return Err(refusal(
                        RefusalReason::Unsupported,
                        id,
                        Construct::Invocation(invocation),
                        "invocation outside a direct call",
                    ));
                }
            }
            .map_err(|unbound| unbound.refusal(id, concerns))
        }
        ObligationKind::DataDomain { operation, clause } => {
            storage.domain_goal(tables, operation, clause, concerns)
        }
        ObligationKind::NormalGuarantee { exit, clause, .. } => {
            let clause = usize::try_from(clause)
                .ok()
                .and_then(|clause| lowered.contract.ensures.get(clause))
                .ok_or_else(|| {
                    refusal(
                        RefusalReason::ForeignIdentity,
                        id,
                        concerns,
                        "machine has no such ensures clause",
                    )
                })?;
            // On a return edge the result pseudo-value stands for the value
            // that edge returns (state_contracts.md: "Synthetic `result`
            // denotes the exact returned value").
            Ok(match (&lowered.result, returned(lowered, exit)) {
                (terminal_psi::TerminalMachineResult::Scalar(result), Some(value)) => {
                    returned_for_result(&clause.proposition, result.id, value)
                }
                _ => clause.proposition.clone(),
            })
        }
        ObligationKind::PublishedGuarantee { .. } => Err(refusal(
            RefusalReason::Unsupported,
            id,
            concerns,
            "a published clause is decided from its exits, not proved",
        )),
        ObligationKind::ArrivalInvariant { edge } => {
            joins::arrival_goal(lowered, id, obligation, edge, tables)
        }
        ObligationKind::CrashCoverage { site, cause } => {
            crashes::coverage_goal(module, lowered, tables, id, obligation, site, cause)
        }
        ObligationKind::Claim { .. }
        | ObligationKind::Loan { .. }
        | ObligationKind::RestorationWindow { .. }
        | ObligationKind::DomainDebt { .. }
        | ObligationKind::EstablishmentDebt { .. }
        | ObligationKind::WellFoundedness { .. }
        | ObligationKind::RankComparison { .. } => Err(refusal(
            RefusalReason::Unsupported,
            id,
            concerns,
            "obligation kind outside this route",
        )),
    }
}

/// Proves `goal` from the knowledge available at `point` and has the kernel
/// decide the certificate.
#[allow(clippy::too_many_arguments)]
fn decide(
    context: &PropositionContext,
    graph: &Graph,
    tables: &MachineTables,
    id: MachineId,
    obligation: ObligationId,
    point: ProgramPoint,
    goal: Proposition,
    relations: bool,
    unreached: &[ValueId],
) -> Checked<Evidence> {
    let hypotheses = hypotheses(graph, tables, id, obligation, point)?;
    let mut prover = prove::Prover::new(
        context,
        &hypotheses.assumptions,
        &hypotheses.axioms,
        &tables.values,
        relations,
    );
    let proof = prover.prove(&goal).map_err(|reason| {
        // Classified by the goal's form, not by which search path failed
        // last, so the outcome does not depend on search order: the form as
        // written, and the form of the operations that define the values it
        // names (`prove::reach`).
        let cause = if prover.has_rules_for(&goal) && !prover.beyond_rules(&goal, unreached) {
            UnprovedCause::NoDerivation
        } else {
            UnprovedCause::NoRuleForGoalForm
        };
        unproved(id, obligation, cause, reason.to_owned())
    })?;
    let identity = EvidenceIdentity::new(obligation.get()).ok_or_else(|| {
        refusal(
            RefusalReason::ForeignIdentity,
            id,
            Construct::Obligation(obligation),
            "obligation identity is not an evidence identity",
        )
    })?;
    let certificate = CertificateEnvelope {
        identity,
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof,
    };
    verify(context, id, obligation, goal, hypotheses, certificate)
}

fn unproved(
    id: MachineId,
    obligation: ObligationId,
    cause: UnprovedCause,
    detail: String,
) -> Refusal {
    Refusal {
        reason: RefusalReason::Unproved(cause),
        machine: id,
        concerns: Construct::Obligation(obligation),
        detail,
    }
}

/// The knowledge available where an obligation sits, split as the kernel
/// takes it: assumptions and semantic axioms, with their rows.
pub(crate) struct Hypotheses {
    pub(crate) assumption_rows: Vec<KnowledgeId>,
    pub(crate) axiom_rows: Vec<KnowledgeId>,
    pub(crate) assumptions: Vec<Proposition>,
    pub(crate) axioms: Vec<Proposition>,
}

pub(crate) fn hypotheses(
    graph: &Graph,
    tables: &MachineTables,
    id: MachineId,
    obligation: ObligationId,
    point: ProgramPoint,
) -> Checked<Hypotheses> {
    let mut assumption_rows = Vec::new();
    let mut axiom_rows = Vec::new();
    let mut assumptions = Vec::new();
    let mut axioms = Vec::new();
    for (row, knowledge_row) in tables.knowledge.iter() {
        if !graph.dominates(knowledge_row.available_from, point)? {
            continue;
        }
        let proposition = knowledge_proposition(tables, row).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Obligation(obligation),
                "knowledge row has no proposition",
            )
        })?;
        concrete(tables, id, Construct::Obligation(obligation), proposition)?;
        match knowledge_row.justification {
            Justification::OperationSemantics(_) => {
                axiom_rows.push(row);
                axioms.push(proposition.clone());
            }
            Justification::EntryRequirement { .. }
            | Justification::CalleeGuarantee { .. }
            | Justification::BranchCondition { .. }
            | Justification::DispatchBinding { .. }
            | Justification::BlockInvariant { .. }
            | Justification::Derived { .. } => {
                assumption_rows.push(row);
                assumptions.push(proposition.clone());
            }
        }
    }
    Ok(Hypotheses {
        assumption_rows,
        axiom_rows,
        assumptions,
        axioms,
    })
}

/// Has the kernel decide `certificate` for `goal` under `hypotheses`. The
/// only acceptance path: an obligation is discharged exactly when this
/// returns evidence.
pub(crate) fn verify(
    context: &PropositionContext,
    id: MachineId,
    obligation: ObligationId,
    goal: Proposition,
    hypotheses: Hypotheses,
    certificate: CertificateEnvelope,
) -> Checked<Evidence> {
    let fact = verify_obligation(
        context,
        &Obligation {
            id: obligation,
            proposition: goal.clone(),
            class: ObligationClass::Derivable,
        },
        &hypotheses.assumptions,
        &hypotheses.axioms,
        EvidenceRoute::CertificateDerived(certificate.clone()),
    )
    .map_err(|error| {
        unproved(
            id,
            obligation,
            UnprovedCause::KernelRefused,
            format!("kernel {error:?}"),
        )
    })?;
    Ok(Evidence {
        goal,
        assumptions: hypotheses.assumption_rows,
        axioms: hypotheses.axiom_rows,
        certificate,
        fact,
    })
}

/// The scalar value an operation defines, if any: a call's result value.
pub(crate) fn operation_result(
    lowered: &TerminalMachine,
    operation: OperationId,
) -> Option<ValueId> {
    lowered
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|candidate| candidate.id == operation)
        .and_then(|found| match &found.result {
            terminal_psi::OperationResult::Scalar(declaration) => Some(declaration.id),
            _ => None,
        })
}

/// The value a `Return` terminator with this edge returns.
fn returned(lowered: &TerminalMachine, exit: EdgeId) -> Option<ValueId> {
    lowered
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            Terminator::Return { edge, value, .. } if *edge == exit => Some(*value),
            _ => None,
        })
}

/// `clause` with the result pseudo-value `result` replaced by `value`, in
/// the comparison and connective forms a clause over scalars or mathematical
/// integers has. Any other form is left as it is: it then still names the pseudo-value, which no
/// knowledge row describes, so it is unproved and never wrongly accepted.
fn returned_for_result(clause: &Proposition, result: ValueId, value: ValueId) -> Proposition {
    let term = |term: &ScalarTerm| match term {
        ScalarTerm::Value { id, scalar_type } if *id == result => {
            ScalarTerm::value(value, *scalar_type)
        }
        other => other.clone(),
    };
    let parts = |parts: &[Proposition]| {
        parts
            .iter()
            .map(|part| returned_for_result(part, result, value))
            .collect()
    };
    // An embedded `result` is the returned value embedded: the embedding
    // keeps the carrier, and the two have the same one.
    fn mathematical(term: &IntegerMathTerm, result: ValueId, value: ValueId) -> IntegerMathTerm {
        let mut returned = term.clone();
        match &mut returned {
            IntegerMathTerm::MathValue { value: id, .. } if *id == result => *id = value,
            other => {
                for operand in other.operands_mut() {
                    *operand = mathematical(operand, result, value);
                }
            }
        }
        returned
    }
    let math = |term: &IntegerMathTerm| mathematical(term, result, value);
    match clause {
        Proposition::Equal(left, right) => Proposition::Equal(term(left), term(right)),
        Proposition::LessThan(left, right) => Proposition::LessThan(term(left), term(right)),
        Proposition::LessOrEqual(left, right) => Proposition::LessOrEqual(term(left), term(right)),
        Proposition::IntegerMathEqual(left, right) => {
            Proposition::IntegerMathEqual(math(left), math(right))
        }
        Proposition::IntegerMathLessThan(left, right) => {
            Proposition::IntegerMathLessThan(math(left), math(right))
        }
        Proposition::IntegerMathLessOrEqual(left, right) => {
            Proposition::IntegerMathLessOrEqual(math(left), math(right))
        }
        Proposition::Conjunction(conjuncts) => Proposition::Conjunction(parts(conjuncts)),
        Proposition::Disjunction(disjuncts) => Proposition::Disjunction(parts(disjuncts)),
        other => other.clone(),
    }
}
