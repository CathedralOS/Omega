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
mod branch_warnings;
mod callable_arrivals;
mod ceilings;
mod crashes;
mod describe;
mod entry_inputs;
mod formation;
mod graph;
mod joins;
mod known;
mod machine_inputs;
mod minted;
mod operational;
mod places;
mod prove;
mod rankings;
mod result_summaries;
mod reuse;
mod verdict;

pub use ceilings::Axis as CeilingAxis;
pub use formation::{ReplayScope, prepare_replay};
pub use places::{ConflictingLoan, LoanConflict};
pub use rankings::FiniteIntegerRanking;
pub use reuse::{
    Carried, CarriedForm, Selection, reverify, reverify_at_call, reverify_specialization,
};
pub use terminal_check_tables::compilation::{
    CallableFormationCause, CallableFormationDefect, CallableFormationFailure,
};
pub use verdict::{
    AcceptedClosure, CompilationRefusal, ConflictingUse, ConsumptionSite, ExceededReach,
    PublicationError, RoutePublication, RouteVerdict, RuleViolation, UnestablishedUse,
    UnprovedObligation, judge, judge_compilation,
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
    BlockId, CanonicalStructuralPathSegment, EvidenceIdentity, IdTable, IntegerMathTerm,
    InvocationSite, MachineId, ObligationId, OperationId, PlaceId, Proposition, PropositionContext,
    ScalarTerm, ScalarType, StructuralCaseId, StructuralTypeId, ValueId,
};
use terminal_check_tables::edges::{Actual, Callee, Destination};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::program::Elaboration;
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
    /// The canonical finite integer relation and its complete authenticated
    /// private bindings. Every internal edge comparison has a separate kernel
    /// certificate; no Truth proposition stands in for well-foundedness.
    FiniteIntegerRanking(FiniteIntegerRanking),
}

impl Discharged {
    pub const fn certificate(&self) -> Option<&Evidence> {
        match self {
            Self::Certificate(evidence) => Some(evidence),
            Self::EveryExit(_)
            | Self::Window(_)
            | Self::Preserved
            | Self::Lent
            | Self::FiniteIntegerRanking(_) => None,
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
    /// loses its inferred facts; its original declared predicate, if any,
    /// remains mandatory (`joins`).
    pub weakened_joins: Vec<BlockId>,
    /// Each loan that may not be formed, with what it conflicts with. Its
    /// row's verdict is the `Conflict` refusal.
    pub conflicts: Vec<places::ConflictingLoan>,
    /// The authored arms no arrival can take (`branch_warnings`), for the
    /// advisory warning about them. Nothing is decided by them.
    pub impossible: Vec<ImpossibleArm>,
}

/// An authored transition arm whose condition contradicts the facts in force
/// before its branch, while those facts are consistent:
/// `wiki/spec/language/state_contracts.md`, "Unreachable branches": "A
/// checked contradiction between valid premises can establish that a
/// particular arrival is impossible", and "issue an unreachable-branch
/// warning when an authored branch contradicts its machine's entry contract
/// or established source facts". `premises` names the facts cited by the
/// accepted refutation, in authored order with the incoming condition itself
/// left out. It need not be a minimal explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpossibleArm {
    pub block: BlockId,
    pub premises: Vec<Justification>,
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
    /// One seal per discharged `OperationSafety` obligation, at an instruction
    /// or a pre-edge terminator read. The seals travel beside the
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
                    source_span: None,
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
/// Branch conditions, dispatch bindings and value relations are ordinary
/// premises, not selectable comparison modes. Explicit proof replay uses
/// the same preparation rules without changing source acceptance.
pub fn discharge(elaboration: &mut Elaboration) -> Checked<Discharge> {
    discharge_inner(elaboration, &[])
}

/// `discharge`, where each machine of `carried` is one stage 04 generated
/// from a generic machine whose check form was proved once. Such a machine
/// takes each certificate from that form, moved into it, and the kernel
/// decides it against the machine's own goal and hypotheses: no search runs
/// for it (generics.md, "Static type equality": the unspecialized body is
/// checked for every admitted use). An obligation for which no carried
/// proof is accepted (a kind the carrier does not move, or a machine that
/// is not the form's body) is decided as in any closed machine, so carrying
/// never accepts what the closed check would not, and never rejects what it
/// accepts.
pub fn discharge_carrying(
    elaboration: &mut Elaboration,
    carried: &[Carried<'_>],
) -> Checked<Discharge> {
    discharge_inner(elaboration, carried)
}

fn discharge_inner(elaboration: &mut Elaboration, carried: &[Carried<'_>]) -> Checked<Discharge> {
    for boundary in &elaboration.module.boundary_machines {
        terminal_semantics::boundary_result_lifetimes(
            &elaboration.module.structural_types,
            boundary,
        )
        .and_then(|_| {
            terminal_semantics::references::validate_declared_boundary_reference_outputs(
                &elaboration.module.structural_types,
                boundary,
            )
        })
        .map_err(|detail| {
            refusal(
                RefusalReason::Unsupported,
                elaboration
                    .module
                    .machines
                    .first()
                    .map(|machine| machine.id)
                    .unwrap_or_else(|| MachineId::new(1).expect("one is a machine identity")),
                Construct::Machine,
                detail,
            )
        })?;
    }
    terminal_semantics::references::validate_provider_reference_outputs(&elaboration.module)
        .map_err(|detail| {
            refusal(
                RefusalReason::Unsupported,
                elaboration
                    .module
                    .machines
                    .first()
                    .map(|machine| machine.id)
                    .unwrap_or_else(|| MachineId::new(1).expect("one is a machine identity")),
                Construct::Machine,
                detail,
            )
        })?;
    let mut scope = prepare_replay(elaboration)?;
    let elaboration = scope.source();
    minted::width_questions_are_minted(elaboration)?;
    operational::judge(elaboration)?;
    // A caller may retain old contents only outside the callee's actual
    // writes. Check every published frame before summaries or knowledge can
    // use it. Low-level proof reconstruction still states a conditional
    // judgment; this whole-module route owns admission of those premises.
    terminal_semantics::borrowed_storage::validate_write_frames(&elaboration.module).map_err(
        |detail| {
            refusal(
                RefusalReason::ForeignIdentity,
                elaboration
                    .module
                    .machines
                    .first()
                    .map(|machine| machine.id)
                    .unwrap_or_else(|| MachineId::new(1).expect("one is a machine identity")),
                Construct::Machine,
                &detail,
            )
        },
    )?;
    // Results before inputs: what a helper returns is a fact its caller
    // may pass on to the next private helper, whose inferred input
    // condition is described from the caller's facts.
    // And once more after inputs: a helper that omits `requires` was
    // summarized under no input, and what it returns because of the input
    // its callers establish is a further clause. A summary already in the
    // contract is not added twice.
    let diagnostic = elaboration
        .module
        .machines
        .first()
        .map(|machine| machine.id)
        .unwrap_or_else(|| MachineId::new(1).expect("one is a machine identity"));
    let inhabitation = prepare_inhabitation(&elaboration.module, diagnostic)?;
    let mut unreached = Unreached::new(elaboration);
    scope.infer_results(&mut unreached, &inhabitation)?;
    scope.infer_inputs(&mut unreached, &inhabitation)?;
    scope.infer_results(&mut unreached, &inhabitation)?;
    let (formed, domains, checking, sources, requirements) = scope.checking();
    let module = formed.source();
    let inputs = machine_inputs::PreparedMachineInputs::new(
        module,
        sources,
        &checking.machines,
        requirements,
        carried,
    );
    // Observe the same immutable type family used by inference above. The
    // existing module-scoped exclusion graph serves every machine; only this
    // borrowed declaration view is needed beside it for projected paths.
    let empty_contracts =
        terminal_semantics::inhabitation::TerminalContracts::new(module.structural_types.iter());
    let mut machines = IdTable::default();
    let identities: Vec<MachineId> = checking.machines.keys().collect();
    for id in identities {
        let tables = checking.machines.get_mut(id).ok_or_else(|| {
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
        let inputs = inputs.for_machine(id)?;
        if inputs.requirement {
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
                impossible: Vec::new(),
            });
            continue;
        }
        let beyond = unreached.values(terminal_machine(&module.machines, id, id)?, tables);
        machines.push(machine(
            formed,
            &inhabitation,
            domains,
            id,
            tables,
            &beyond,
            inputs.carried.map(|position| &carried[position]),
            inputs.branches,
            &empty_contracts,
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
        source_span: None,
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

#[allow(clippy::too_many_arguments)]
fn machine(
    formed: formation::FormedModule<'_>,
    inhabitation: &terminal_semantics::inhabitation::Inhabitation,
    domains: &IdTable<StructuralTypeId, Vec<Proposition>>,
    id: MachineId,
    tables: &mut MachineTables,
    unreached: &[ValueId],
    carried: Option<&Carried<'_>>,
    branches: &[terminal_check_tables::program::Branch],
    empty_contracts: &terminal_semantics::inhabitation::TerminalContracts<'_>,
) -> Checked<MachineDischarge> {
    let module = formed.source();
    let machines = &module.machines;
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
        reference_graph,
        ranking_bindings,
    } = prepare_with_inhabitation(formed, domains, id, tables, inhabitation)?;
    let references = std::cell::RefCell::new(
        terminal_semantics::descriptor_results::DescriptorAnalysis::with_borrowed_storage_slots(
            &module.structural_types,
            &reference_graph,
        ),
    );
    let storage = places::Storage::new(
        id,
        lowered,
        module,
        domains,
        &graph,
        tables,
        &types,
        &references,
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
    let declared = joins::prepare(lowered, id, tables, &graph, &context, invariants, &storage)?;
    let weakened_joins = joins::check(
        &joins::Machine {
            module,
            lowered,
            storage: &storage,
            id,
            types: &types,
            graph: &graph,
            context: &context,
            ranking_bindings: &ranking_bindings,
            declared: &declared,
            unreached,
        },
        tables,
        &mut decided,
    )?;
    // Certificates next; a published clause is then decided from its exits.
    // A machine stage 04 generated takes each one its generic form proved,
    // and searches only where none is carried (`discharge_carrying`).
    let carrier = carried.and_then(|carried| carried.carrier(tables, lowered));
    for (index, (obligation, row)) in tables.obligations.iter().enumerate() {
        if decided[index].is_some() {
            continue;
        }
        if let ObligationKind::PublishedGuarantee { .. } = row.kind {
            continue;
        }
        if let ObligationKind::WellFoundedness { component } = row.kind {
            decided[index] = Some(
                rankings::well_founded(lowered, &ranking_bindings, obligation, component)
                    .map(Discharged::FiniteIntegerRanking),
            );
            continue;
        }
        let verdict = goal(
            module,
            lowered,
            &storage,
            id,
            obligation,
            &row.kind,
            tables,
            &types,
            &ranking_bindings,
        )
        .and_then(|goal| {
            if let Some(carry) = &carrier
                && let Ok(evidence) = carry.certify(
                    &context,
                    &graph,
                    obligation,
                    &row.kind,
                    row.created_at,
                    goal.clone(),
                )
            {
                return Ok(Discharged::Certificate(evidence));
            }
            decide(
                &context,
                &graph,
                tables,
                id,
                obligation,
                row.created_at,
                goal,
                unreached,
            )
            .map(Discharged::Certificate)
        });
        decided[index] = Some(verdict);
    }
    // Published guarantees read normal-return verdicts, so finish that last
    // dependency pass before moving any certificate out of this table. Once
    // it is complete, settlement owns each result; no duplicate proof tree
    // or intermediate verdict table is needed.
    for (index, (obligation, row)) in tables.obligations.iter().enumerate() {
        if decided[index].is_none()
            && let ObligationKind::PublishedGuarantee { clause } = row.kind
        {
            decided[index] = Some(published(tables, id, obligation, clause, &decided));
        }
    }
    // A store or a zero establishment that was not shown to keep a clause
    // does not fail: it opens an invariant window, and the clause is owed
    // at the consumption points the window reaches (dependent_values.md,
    // "Invariant windows"). A construction makes a value and must prove its
    // clauses where it stands, so its unproved clause stays a refusal.
    let mut windows: Vec<places::Window> = Vec::new();
    let mut restoring: Vec<(places::Owner, OperationId)> = Vec::new();
    let mut settled = IdTable::default();
    for ((obligation, row), verdict) in tables.obligations.iter().zip(decided) {
        let verdict = verdict.unwrap_or_else(|| {
            Err(refusal(
                RefusalReason::Unsupported,
                id,
                Construct::Obligation(obligation),
                "obligation left undecided",
            ))
        });
        let ObligationKind::DataDomain { operation, clause } = row.kind else {
            settled.push(verdict);
            continue;
        };
        let concerns = Construct::Obligation(obligation);
        let stores = storage.store(operation)?;
        let zeroes = storage.zero_establishment(operation)?;
        settled.push(match verdict {
            // data_and_literals.md: "Construction and every restoring write
            // prove the owning default-domain clauses."
            Ok(Discharged::Certificate(evidence)) if stores => {
                restoring.push((storage.owner(operation, clause, concerns)?, operation));
                Ok(Discharged::Certificate(evidence))
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
                    Ok(Discharged::Window(Box::new(unproved)))
                }
            }
            verdict => verdict,
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
    let mut uses = storage.uses(tables, empty_contracts, inhabitation)?;
    // A store into a field of a byte domain is judged by the bytes it
    // supplies; a refused store is that operation's verdict.
    for judged in storage.byte_predicate_stores()? {
        match uses.iter_mut().find(|used| used.at == judged.at) {
            Some(used) if used.verdict.is_ok() => used.verdict = judged.verdict,
            Some(_) => {}
            None => uses.push(judged),
        }
    }
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
                    source_span: None,
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
    let impossible =
        branch_warnings::Analysis::new(&context, &graph, tables, lowered, id).run(branches);
    Ok(MachineDischarge {
        verdicts: settled,
        uses,
        windows: closures,
        weakened_joins,
        conflicts,
        impossible,
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
            // A read or a returning call establishes the applicable type
            // constraints on its exact observed storage versions.
            for premise in storage
                .read_premises(tables, operation)?
                .into_iter()
                .chain(storage.returned_type_premises(tables, operation)?)
            {
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
    for invocation in tables.invocations.keys().collect::<Vec<_>>() {
        let row = terminal_check_tables::row(
            &tables.invocations,
            invocation,
            id,
            Construct::Invocation(invocation),
        )?;
        // A call with no normal completion is a terminator. Its callee's
        // guarantees "apply only on normal return; external exit neither
        // establishes them nor necessarily violates them"
        // (calls_and_outcomes.md), and it has no return, so it adds no
        // knowledge.
        let operation = match row.site {
            InvocationSite::Operation(operation) => operation,
            InvocationSite::InvokeNoNormal(_) | InvocationSite::ExternalCompletion(_) => continue,
        };
        let callee = callee(module, id, invocation, row)?;
        let erased = callable_erased_actuals(
            module,
            lowered,
            callee,
            row.required_contract,
            id,
            invocation,
            row.site,
        )?;
        let ensures = module
            .invocation_ensures(callee, row.required_contract)
            .ok_or_else(|| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    Construct::Invocation(invocation),
                    "callee has no canonical guarantee contract",
                )
            })?;
        let mut ensures = ensures.peekable();
        if ensures.peek().is_none() {
            continue;
        }
        let mut returned = storage.after_call(
            tables,
            &row.actuals,
            module
                .invocation_structural_parameters(callee, row.required_contract)
                .ok_or_else(|| {
                    refusal(
                        RefusalReason::ForeignIdentity,
                        id,
                        Construct::Invocation(invocation),
                        "callee has no canonical structural formals",
                    )
                })?,
            match callee {
                semantic_vocabulary::TargetMachineCallable::Machine(machine) => module
                    .invocation_machine_contract(machine, row.required_contract)
                    .map(|contract| &contract.result),
                semantic_vocabulary::TargetMachineCallable::Boundary(_) => None,
            },
            operation,
            Construct::Invocation(invocation),
        )?;
        // Resolve this invocation's guarantees before publishing knowledge:
        // every clause reads the same retained inputs and actual result,
        // without holding a table borrow across its output mutation.
        let propositions = ensures
            .map(|ensures| {
                substitute(
                    ensures,
                    &mut ErasedTerms {
                        terms: &erased,
                        runtime: &mut returned,
                    },
                )
                .map_err(|unbound| unbound.refusal(id, Construct::Invocation(invocation)))
            })
            .collect::<Checked<Vec<_>>>()?;
        for (clause, proposition) in propositions.into_iter().enumerate() {
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
    reference_graph: Vec<terminal_semantics::descriptor_results::DescriptorMachine>,
    ranking_bindings: rankings::Bindings,
}

impl Prepared {
    fn references<'a>(
        &'a self,
        types: &'a [terminal_psi::StructuralTypeDeclaration],
    ) -> std::cell::RefCell<terminal_semantics::descriptor_results::DescriptorAnalysis<'a>> {
        std::cell::RefCell::new(
            terminal_semantics::descriptor_results::DescriptorAnalysis::with_borrowed_storage_slots(
                types,
                &self.reference_graph,
            ),
        )
    }
}

/// Fills `tables.knowledge` from the lowered form and builds what deciding
/// needs. Knowledge already present is kept: it is this machine's own.
pub(crate) fn prepare(
    formed: formation::FormedModule<'_>,
    domains: &IdTable<StructuralTypeId, Vec<Proposition>>,
    id: MachineId,
    tables: &mut MachineTables,
) -> Checked<Prepared> {
    let module = formed.source();
    let inhabitation = prepare_inhabitation(module, id)?;
    prepare_with_inhabitation(formed, domains, id, tables, &inhabitation)
}

fn prepare_inhabitation(
    module: &TerminalModule,
    id: MachineId,
) -> Checked<terminal_semantics::inhabitation::Inhabitation> {
    terminal_semantics::inhabitation::Inhabitation::prepare(
        &terminal_semantics::inhabitation::TerminalContracts::new(&module.structural_types),
    )
    .map_err(|_| {
        refusal(
            RefusalReason::ForeignIdentity,
            id,
            Construct::Machine,
            "structural type contracts do not form one inhabitation family",
        )
    })
}

pub(crate) fn prepare_with_inhabitation(
    formed: formation::FormedModule<'_>,
    domains: &IdTable<StructuralTypeId, Vec<Proposition>>,
    id: MachineId,
    tables: &mut MachineTables,
    inhabitation: &terminal_semantics::inhabitation::Inhabitation,
) -> Checked<Prepared> {
    let module = formed.source();
    let machines = &module.machines;
    let structural_types = &module.structural_types;
    let lowered = terminal_machine(machines, id, id)?;
    let fill = tables.knowledge.is_empty();
    let types = tables.values.map(|_, row| row.scalar_type);
    // Before anything is decided: the rows must be the ones the lowered form
    // owes, at its points, or deciding every row proves nothing (`minted`).
    // A consumer re-checking a saved proof decides rows too, so it checks
    // them the same way.
    minted::follow_the_lowered_form(id, module, lowered, tables, &types, inhabitation)?;
    minted::domains_are_declared(id, structural_types, domains)?;
    let graph = Graph::new(id, lowered, tables)?;
    let reference_graph = places::reference_graph(module, id)?;
    let ranking_bindings;
    {
        let references = std::cell::RefCell::new(
            terminal_semantics::descriptor_results::DescriptorAnalysis::with_borrowed_storage_slots(
                &module.structural_types,
                &reference_graph,
            ),
        );
        let storage = places::Storage::new(
            id,
            lowered,
            module,
            domains,
            &graph,
            tables,
            &types,
            &references,
        )?;
        ranking_bindings = rankings::admit(lowered, tables, &graph, &storage)?;
        if fill {
            storage.owed_are_minted(tables)?;
            fill_knowledge(module, id, lowered, tables, &types, &storage)?;
        }
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
        reference_graph,
        ranking_bindings,
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
        source_span: None,
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
    // A Trapping operation's normal successor is reached only where it did
    // not trap, and there it computed the Exact primitive's result.
    if let Some(semantics) =
        terminal_semantics::trapping_integer_semantics(operation, types).map_err(&refuse)?
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
    row: &terminal_check_tables::edges::InvocationRow,
) -> Checked<semantic_vocabulary::TargetMachineCallable> {
    let concerns = Construct::Invocation(invocation);
    let unavailable = |detail| refusal(RefusalReason::Unsupported, machine, concerns, detail);
    let Callee::Declared(callee) = row.callee else {
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
            {
                return Err(unavailable(
                    "boundary call has no supported exact callable contract",
                ));
            }
            // A declaration and its call site state one completion
            // (calls_and_outcomes.md: "an explicit closed external
            // completion kind and no normal result. Its invocation is a
            // terminal transfer ... with no normal successor"): a
            // requirement that completes externally is called only as a
            // terminator, one that returns only as an operation. Nothing
            // downstream may read an exit as a call that returns.
            let returns = matches!(row.site, InvocationSite::Operation(_));
            let external = matches!(row.site, InvocationSite::ExternalCompletion(_));
            match declaration.completion {
                semantic_vocabulary::BoundaryCompletion::NormalReturn if returns => {}
                semantic_vocabulary::BoundaryCompletion::External(_) if external => {
                    // process_exit.md: the canonical operation "permits no
                    // source crash".
                    if !declaration.crash_routes.is_empty() {
                        return Err(unavailable(
                            "external completion that declares a crash route",
                        ));
                    }
                }
                _ => {
                    return Err(refusal(
                        RefusalReason::ForeignIdentity,
                        machine,
                        concerns,
                        "boundary call site does not match its declaration's completion",
                    ));
                }
            }
            // A boundary that may suspend is judged with every other
            // possibly suspending call, by its crossing
            // (`operational::judge`, which every discharge runs first).
            // A formal's own domains are its type: the structural
            // qualification judgment holds each argument to exactly them, as
            // for any call. What a provider must additionally prove, or may
            // mint, is not yet judged.
            if !declaration.requires.is_empty()
                || !declaration.normal_qualifications.is_empty()
                || declaration
                    .structural_parameters
                    .iter()
                    .any(|formal| !formal.additional_input_qualifications.is_empty())
            {
                return Err(unavailable(
                    "boundary call needs its structural qualification proof",
                ));
            }
            if !declaration.result_reference_regions.is_empty()
                || !declaration.program_local_root_introductions.is_empty()
                || !declaration.content_guarantees.is_empty()
                || matches!(&declaration.result, terminal_psi::BoundaryMachineResult::Structural(result)
                    if !result.qualifications.is_empty() || !result.projected_qualifications.is_empty())
            {
                return Err(unavailable(
                    "boundary call needs its result, reference and content custody proof",
                ));
            }
            // What the requirement may reach is charged to this caller
            // where every call is (`ceilings`).
        }
    }
    Ok(callee)
}

fn callable_erased_actuals(
    module: &TerminalModule,
    caller: &TerminalMachine,
    callee: semantic_vocabulary::TargetMachineCallable,
    required_contract: Option<semantic_vocabulary::ContractId>,
    machine: MachineId,
    invocation: terminal_check_tables::InvocationId,
    site: InvocationSite,
) -> Checked<Vec<(ValueId, ScalarTerm)>> {
    match callee {
        semantic_vocabulary::TargetMachineCallable::Machine(callee) => erased_actuals(
            caller,
            module
                .invocation_machine_contract(callee, required_contract)
                .ok_or_else(|| {
                    refusal(
                        RefusalReason::ForeignIdentity,
                        machine,
                        Construct::Invocation(invocation),
                        "call lost its public contract association",
                    )
                })?,
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

pub(crate) use terminal_semantics::substitution::clauses::{
    ClauseBindings as Bindings, substitute_clause as substitute, substitute_clause_term as bound,
};

impl From<&'static str> for Unbound {
    fn from(detail: &'static str) -> Self {
        Self::Unsupported(detail)
    }
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
    pub(crate) runtime: &'a mut dyn Bindings<Error = Unbound>,
}

impl Bindings for ErasedTerms<'_> {
    type Error = Unbound;
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
        version: semantic_vocabulary::ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Unbound> {
        self.runtime.field(version, root, path, scalar_type)
    }

    fn extent(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, Unbound> {
        self.runtime.extent(root, path, scalar_type)
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
    type Error = Unbound;
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
        _: semantic_vocabulary::ContentPlaceVersion,
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
    ranking_bindings: &rankings::Bindings,
) -> Checked<Proposition> {
    let goal = stated_goal(
        module,
        lowered,
        storage,
        id,
        obligation,
        kind,
        tables,
        types,
        ranking_bindings,
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
    ranking_bindings: &rankings::Bindings,
) -> Checked<Proposition> {
    let concerns = Construct::Obligation(obligation);
    match *kind {
        ObligationKind::IntegerWidthPositive { parameter } => {
            let integer = semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Unsigned,
                64,
            )
            .expect("u64 is a supported carrier");
            Ok(Proposition::LessThan(
                ScalarTerm::integer(integer, semantic_vocabulary::IntegerValue::Unsigned(0))
                    .expect("zero is a u64 value"),
                ScalarTerm::value(parameter, ScalarType::Integer(integer)),
            ))
        }
        ObligationKind::OperationSafety { site } => storage.safety_goal(site, obligation, types),
        ObligationKind::CallRequirement { invocation, clause } => {
            let row = terminal_check_tables::row(
                &tables.invocations,
                invocation,
                id,
                Construct::Invocation(invocation),
            )?;
            let callee = callee(module, id, invocation, row)?;
            let erased = callable_erased_actuals(
                module,
                lowered,
                callee,
                row.required_contract,
                id,
                invocation,
                row.site,
            )?;
            let clauses = module
                .invocation_requires(callee, row.required_contract)
                .ok_or_else(|| {
                    refusal(
                        RefusalReason::ForeignIdentity,
                        id,
                        concerns,
                        "callee has no canonical requires contract",
                    )
                })?;
            let formals = module
                .invocation_structural_parameters(callee, row.required_contract)
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
            let over_actuals = |runtime: &mut dyn Bindings<Error = Unbound>| {
                substitute(
                    clause,
                    &mut ErasedTerms {
                        terms: &erased,
                        runtime,
                    },
                )
            };
            over_actuals(&mut storage.at_call(tables, &row.actuals, formals, row.site, concerns)?)
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
            substitute(
                &clause.proposition,
                &mut storage.at_return(tables, exit, concerns)?,
            )
            .map_err(|unbound| unbound.refusal(id, concerns))
        }
        ObligationKind::PublishedGuarantee { .. } => Err(refusal(
            RefusalReason::Unsupported,
            id,
            concerns,
            "a published clause is decided from its exits, not proved",
        )),
        ObligationKind::ArrivalInvariant { edge } => joins::arrival_goal(
            lowered,
            id,
            obligation,
            edge,
            tables,
            storage,
            &module.scalar_block_invariants,
        ),
        ObligationKind::CrashCoverage { site, cause } => {
            crashes::coverage_goal(module, lowered, tables, types, id, obligation, site, cause)
        }
        ObligationKind::RankComparison { component, edge } => {
            rankings::goal(lowered, ranking_bindings, obligation, component, edge)
        }
        ObligationKind::Claim { .. }
        | ObligationKind::Loan { .. }
        | ObligationKind::RestorationWindow { .. }
        | ObligationKind::DomainDebt { .. }
        | ObligationKind::EstablishmentDebt { .. }
        | ObligationKind::WellFoundedness { .. } => Err(refusal(
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
    unreached: &[ValueId],
) -> Checked<Evidence> {
    let hypotheses = hypotheses(graph, tables, id, obligation, point)?;
    // Search and the kernel first see only the facts connected to the goal
    // (`connected`); the full set is tried only when that search fails, so
    // the narrowing never refuses an obligation the full set proves.
    if let Some(narrowed) = connected(&goal, &hypotheses)
        && let Ok(certificate) = search_certificate(
            context,
            &tables.values,
            id,
            obligation,
            &goal,
            &narrowed.assumptions,
            &narrowed.axioms,
            unreached,
        )
    {
        return verify(context, id, obligation, goal, narrowed, certificate);
    }
    let certificate = search_certificate(
        context,
        &tables.values,
        id,
        obligation,
        &goal,
        &hypotheses.assumptions,
        &hypotheses.axioms,
        unreached,
    )?;
    verify(context, id, obligation, goal, hypotheses, certificate)
}

/// The hypotheses that share a value with `goal`, directly or through a
/// chain of other hypotheses, plus every hypothesis that names no value.
/// `None` when that is all of them.
///
/// A long machine accumulates thousands of available facts (a layout plan
/// that fills a 64-entry record array has over 5,000), and the kernel
/// re-checks every certificate in the context of every premise it is given,
/// so passing all of them made each obligation cost time linear in the
/// machine's size. A fact whose values are disjoint from the goal's
/// component can only matter by contradicting the others; the caller keeps
/// that case by retrying with the full set. Dropping premises cannot make
/// the kernel accept more, and the evidence records exactly the rows used.
fn connected(goal: &Proposition, hypotheses: &Hypotheses) -> Option<Hypotheses> {
    let premises = || hypotheses.assumptions.iter().chain(&hypotheses.axioms);
    // Dense indices for every value named anywhere, then one union-find
    // pass joining each premise's values.
    let mut values = Vec::new();
    let mut complete = Vec::with_capacity(hypotheses.assumptions.len() + hypotheses.axioms.len());
    for premise in premises() {
        complete.push(premise.visit_value_ids(|value| values.push(value)));
    }
    if !goal.visit_value_ids(|value| values.push(value)) {
        return None;
    }
    values.sort_unstable();
    values.dedup();
    let index = |value: ValueId| values.binary_search(&value).ok();
    let mut parent = (0..values.len()).collect::<Vec<_>>();
    fn root(parent: &mut [usize], mut node: usize) -> usize {
        while parent[node] != node {
            parent[node] = parent[parent[node]];
            node = parent[node];
        }
        node
    }
    for premise in premises() {
        let mut first = None;
        premise.visit_value_ids(|value| {
            let Some(node) = index(value) else { return };
            let node = root(&mut parent, node);
            match first {
                None => first = Some(node),
                Some(other) => parent[node] = root(&mut parent, other),
            }
        });
    }
    let mut goal_roots = Vec::new();
    goal.visit_value_ids(|value| {
        if let Some(node) = index(value) {
            goal_roots.push(root(&mut parent, node));
        }
    });
    goal_roots.sort_unstable();
    goal_roots.dedup();
    // A premise is kept when it names no value, could not be enumerated, or
    // names a value of the goal's component.
    let mut keep = |premise: &Proposition, complete: bool| {
        let mut named = false;
        let mut joined = false;
        premise.visit_value_ids(|value| {
            named = true;
            if let Some(node) = index(value) {
                joined |= goal_roots.binary_search(&root(&mut parent, node)).is_ok();
            }
        });
        !complete || !named || joined
    };
    let mut narrowed = Hypotheses {
        assumption_rows: Vec::new(),
        axiom_rows: Vec::new(),
        assumptions: Vec::new(),
        axioms: Vec::new(),
    };
    let (assumption_complete, axiom_complete) = complete.split_at(hypotheses.assumptions.len());
    for ((premise, row), complete) in hypotheses
        .assumptions
        .iter()
        .zip(&hypotheses.assumption_rows)
        .zip(assumption_complete)
    {
        if keep(premise, *complete) {
            narrowed.assumptions.push(premise.clone());
            narrowed.assumption_rows.push(*row);
        }
    }
    for ((premise, row), complete) in hypotheses
        .axioms
        .iter()
        .zip(&hypotheses.axiom_rows)
        .zip(axiom_complete)
    {
        if keep(premise, *complete) {
            narrowed.axioms.push(premise.clone());
            narrowed.axiom_rows.push(*row);
        }
    }
    (narrowed.assumptions.len() + narrowed.axioms.len()
        < hypotheses.assumptions.len() + hypotheses.axioms.len())
    .then_some(narrowed)
}

/// Declaration formation and executable obligations search the same proof
/// rules. Their callers provide the exact typed inputs and independent facts;
/// the resulting certificate still needs the kernel's acceptance.
#[allow(clippy::too_many_arguments)]
fn search_certificate(
    context: &PropositionContext,
    values: &dyn known::ProofValues,
    id: MachineId,
    obligation: ObligationId,
    goal: &Proposition,
    assumptions: &[Proposition],
    axioms: &[Proposition],
    unreached: &[ValueId],
) -> Checked<CertificateEnvelope> {
    let mut prover = prove::Prover::new(context, assumptions, axioms, values);
    let proof = prover.prove(goal).map_err(|reason| {
        // Classified by the goal's form, not by which search path failed
        // last, so the outcome does not depend on search order: the form as
        // written, and the form of the operations that define the values it
        // names (`prove::reach`).
        let cause = if prover.has_rules_for(goal) && !prover.beyond_rules(goal, unreached) {
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
    Ok(CertificateEnvelope {
        identity,
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof,
    })
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
        source_span: None,
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

/// Keep the actual program-point dependencies beside the kernel's accepted
/// certificate so a later consumer can reconstruct the same judgment.
pub(crate) fn verify(
    context: &PropositionContext,
    id: MachineId,
    obligation: ObligationId,
    goal: Proposition,
    hypotheses: Hypotheses,
    certificate: CertificateEnvelope,
) -> Checked<Evidence> {
    let fact = verify_certificate(
        context,
        id,
        obligation,
        &goal,
        &hypotheses.assumptions,
        &hypotheses.axioms,
        &certificate,
    )?;
    Ok(Evidence {
        goal,
        assumptions: hypotheses.assumption_rows,
        axioms: hypotheses.axiom_rows,
        certificate,
        fact,
    })
}

/// The kernel checks the exact goal and independent premises before this
/// certificate can justify a declaration or an executable obligation.
fn verify_certificate(
    context: &PropositionContext,
    id: MachineId,
    obligation: ObligationId,
    goal: &Proposition,
    assumptions: &[Proposition],
    axioms: &[Proposition],
    certificate: &CertificateEnvelope,
) -> Checked<AcceptedFact> {
    verify_obligation(
        context,
        &Obligation {
            id: obligation,
            proposition: goal.clone(),
            class: ObligationClass::Derivable,
        },
        assumptions,
        axioms,
        EvidenceRoute::CertificateDerived(certificate.clone()),
    )
    .map_err(|error| {
        unproved(
            id,
            obligation,
            UnprovedCause::KernelRefused,
            format!("kernel {error:?}"),
        )
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
