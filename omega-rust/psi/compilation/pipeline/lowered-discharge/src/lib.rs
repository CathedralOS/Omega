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
  established value for a scalar leaf it reads: today the zeroed storage of
  a gated type (`places`). It is a structural judgment over the
  write log and the graph, not an obligation, so it has no certificate.
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

Kinds outside `OperationSafety`, `CallRequirement`, `NormalGuarantee` and
`DataDomain` are refused as `Unsupported`; elaboration mints none of them
yet.
*/

mod crashes;
mod graph;
mod joins;
mod minted;
mod places;
mod prove;
mod reuse;
mod verdict;

pub use reuse::{Selection, reverify, reverify_at_call, reverify_specialization};
pub use verdict::{AcceptedClosure, RouteVerdict, UnestablishedUse, UnprovedObligation, judge};

#[cfg(test)]
pub(crate) use prove::has_rules_for;

#[cfg(test)]
mod tests;

use proof_admission::{
    AcceptedFact, CertificateEnvelope, EvidenceRoute, Obligation, ObligationClass,
    ProofSystemMarker, verify_obligation,
};
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, EdgeId, EvidenceIdentity, IdTable, InvocationSite,
    MachineId, ObligationId, OperationId, PlaceId, Proposition, PropositionContext, ScalarTerm,
    ScalarType, StructuralCaseId, StructuralTypeId, ValueId,
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
use terminal_psi::{TerminalMachine, Terminator};

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
    /// A `DataDomain` clause of a zero establishment (`let x: T;`) that the
    /// zero value was not proved to satisfy, with why. It is not a refusal,
    /// because nothing is owed where storage is only zeroed
    /// (`wiki/spec/language/dependent_values.md`: "Zeroing storage is
    /// permitted independently of establishing a value."). It is not a
    /// discharge either: the type is gated ("If zero satisfies the default
    /// domain, the value is zero-constructible. Otherwise the type is gated:
    /// construction or checked qualification must establish the domain
    /// before observation."), so the zeroed contents are not an established
    /// value and every use they reach is `Unestablished` (`StorageUse`).
    Gated(Box<Refusal>),
}

impl Discharged {
    pub const fn certificate(&self) -> Option<&Evidence> {
        match self {
            Self::Certificate(evidence) => Some(evidence),
            Self::EveryExit(_) | Self::Gated(_) => None,
        }
    }
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
    /// Joins whose inferred predicate some arrival could not establish. Each
    /// predicate was replaced by `Truth`, which every arrival establishes,
    /// so the join contributes nothing downstream (`joins`).
    pub weakened_joins: Vec<BlockId>,
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
                weakened_joins: Vec::new(),
            });
            continue;
        }
        machines.push(machine(
            &module.machines,
            &module.structural_types,
            &elaboration.domains,
            id,
            tables,
            knowledge,
        )?);
    }
    Ok(Discharge { machines })
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
    machines: &[TerminalMachine],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    domains: &IdTable<StructuralTypeId, Vec<Proposition>>,
    id: MachineId,
    tables: &mut MachineTables,
    knowledge: Knowledge,
) -> Checked<MachineDischarge> {
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
    } = prepare(machines, structural_types, domains, id, tables, knowledge)?;
    let mut storage = places::Storage::new(
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
    // Join predicates first: each is checked on every arrival before it may
    // justify anything downstream.
    if knowledge == Knowledge::All {
        joins::infer(lowered, id, tables, &graph, &context)?;
    }
    let weakened_joins = joins::check(
        &joins::Machine {
            machines,
            lowered,
            storage: &storage,
            id,
            types: &types,
            graph: &graph,
            context: &context,
            relations: knowledge != Knowledge::LiteralIntervals,
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
            machines, lowered, &storage, id, obligation, &row.kind, tables, &types,
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
            )
            .map(Discharged::Certificate)
        });
        decided[index] = Some(verdict);
    }
    let mut verdicts = IdTable::default();
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
    // A zero establishment whose zero value does not satisfy a clause it
    // owes: the type is gated. Nothing is owed where storage is only zeroed
    // (dependent_values.md: "Zeroing storage is permitted independently of
    // establishing a value."), so the clause's verdict is not a refusal; the
    // zeroed contents are not an established value, and the initialization
    // judgment below refuses every use they reach.
    let mut gated: Vec<OperationId> = Vec::new();
    let mut settled = IdTable::default();
    let decided: &IdTable<ObligationId, Checked<Discharged>> = &verdicts;
    for ((_, row), (_, verdict)) in tables.obligations.iter().zip(decided.iter()) {
        settled.push(match (row.kind, verdict) {
            (ObligationKind::DataDomain { operation, .. }, Err(unproved))
                if matches!(unproved.reason, RefusalReason::Unproved(_))
                    && storage.zero_establishment(operation)? =>
            {
                if !gated.contains(&operation) {
                    gated.push(operation);
                }
                Ok(Discharged::Gated(Box::new(unproved.clone())))
            }
            // A store whose written value is not known to keep a clause does
            // not break the program: dependent_values.md, "Invariant windows":
            // "A write known to preserve the default domain keeps it
            // established. Otherwise the write opens a window on the place
            // ... The domain must be proved again at the next consumption
            // point". This route has no windows yet, so it cannot judge the
            // closure whole and hands it back; it never rejects a store for
            // a clause the specification lets a later store restore.
            (ObligationKind::DataDomain { operation, .. }, Err(unproved))
                if matches!(unproved.reason, RefusalReason::Unproved(_))
                    && storage.store(operation)? =>
            {
                Err(Refusal {
                    reason: RefusalReason::Unsupported,
                    machine: id,
                    concerns: unproved.concerns,
                    detail: format!(
                        "a store that would open an invariant window: {}",
                        unproved.detail
                    ),
                })
            }
            (_, verdict) => verdict.clone(),
        });
    }
    storage.gate(gated);
    let uses = storage.uses(tables)?;
    Ok(MachineDischarge {
        verdicts: settled,
        uses,
        weakened_joins,
    })
}

/// Fills one machine's knowledge table from its lowered form: entry
/// `requires`, result equations and field-read equations, branch
/// conditions, dispatch bindings and callee guarantees.
fn fill_knowledge(
    machines: &[TerminalMachine],
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
    // equal to what the payload leaf holds. The row holds because that edge
    // was taken and is available on it.
    if knowledge == Knowledge::All {
        for block in &lowered.blocks {
            for (edge, equation) in storage.payload_equations(tables, block)? {
                push_knowledge(
                    tables,
                    equation,
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
        let callee = callee(id, invocation, &row.callee)?;
        let actuals = scalar_actuals(&row.actuals);
        let contract = &terminal_machine(machines, callee, id)?.contract;
        for (clause, ensures) in contract.ensures.iter().enumerate() {
            let proposition = substitute(
                &ensures.proposition,
                &mut ScalarActuals {
                    actuals: &actuals,
                    result: operation_result(lowered, operation),
                    tables,
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
    machines: &[TerminalMachine],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    domains: &IdTable<StructuralTypeId, Vec<Proposition>>,
    id: MachineId,
    tables: &mut MachineTables,
    knowledge: Knowledge,
) -> Checked<Prepared> {
    let lowered = terminal_machine(machines, id, id)?;
    let fill = tables.knowledge.is_empty();
    let types = tables.values.map(|_, row| row.scalar_type);
    // Before anything is decided: the rows must be the ones the lowered form
    // owes, at its points, or deciding every row proves nothing (`minted`).
    // A consumer re-checking a saved proof decides rows too, so it checks
    // them the same way.
    minted::follow_the_lowered_form(id, machines, lowered, tables, &types)?;
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
        fill_knowledge(machines, id, lowered, tables, knowledge, &types, &storage)?;
    }
    let context = PropositionContext::from_value_types(
        types
            .iter()
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

fn callee(
    machine: MachineId,
    invocation: terminal_check_tables::InvocationId,
    callee: &Callee,
) -> Checked<MachineId> {
    match callee {
        Callee::Machine(callee) => Ok(*callee),
        Callee::Selected { .. } => Err(refusal(
            RefusalReason::Unsupported,
            machine,
            Construct::Invocation(invocation),
            "call through a selector value",
        )),
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
        Proposition::Equal(left, right) => {
            Proposition::Equal(bound(left, bindings)?, bound(right, bindings)?)
        }
        Proposition::LessThan(left, right) => {
            Proposition::LessThan(bound(left, bindings)?, bound(right, bindings)?)
        }
        Proposition::LessOrEqual(left, right) => {
            Proposition::LessOrEqual(bound(left, bindings)?, bound(right, bindings)?)
        }
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
        _ => Err(Unbound::Unsupported(
            "clause term outside the scalar subset",
        )),
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
    machines: &[TerminalMachine],
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
        ObligationKind::OperationSafety { operation } => {
            let found = lowered
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|candidate| candidate.id == operation)
                .ok_or_else(|| {
                    refusal(
                        RefusalReason::ForeignIdentity,
                        id,
                        concerns,
                        "operation is not in this machine",
                    )
                })?;
            let semantics = terminal_semantics::proof_bearing_scalar_leaf_semantics(found, types)
                .map_err(semantics_refusal(id, operation))?
                .ok_or_else(|| {
                    refusal(
                        RefusalReason::Unsupported,
                        id,
                        concerns,
                        "operation has no safety goal",
                    )
                })?;
            // That this row is the one the operation names is `minted`'s
            // check, made before any goal is stated.
            semantics
                .canonical_goal()
                .kernel_proposition()
                .map_err(semantics_refusal(id, operation))
        }
        ObligationKind::CallRequirement { invocation, clause } => {
            let row = terminal_check_tables::row(
                &tables.invocations,
                invocation,
                id,
                Construct::Invocation(invocation),
            )?;
            let callee = terminal_machine(machines, callee(id, invocation, &row.callee)?, id)?;
            let InvocationSite::Operation(site) = row.site else {
                return Err(refusal(
                    RefusalReason::Unsupported,
                    id,
                    Construct::Invocation(invocation),
                    "invocation outside an operation",
                ));
            };
            let clause = usize::try_from(clause)
                .ok()
                .and_then(|clause| callee.contract.requires.get(clause))
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
            substitute(
                clause,
                &mut storage.at_call(tables, &row.actuals, callee, site, concerns)?,
            )
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
            crashes::coverage_goal(lowered, id, obligation, site, cause)
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
        // last, so the outcome does not depend on search order.
        let cause = if prove::has_rules_for(&goal) {
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
/// the comparison and connective forms a clause over scalars has. Any other
/// form is left as it is: it then still names the pseudo-value, which no
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
    match clause {
        Proposition::Equal(left, right) => Proposition::Equal(term(left), term(right)),
        Proposition::LessThan(left, right) => Proposition::LessThan(term(left), term(right)),
        Proposition::LessOrEqual(left, right) => Proposition::LessOrEqual(term(left), term(right)),
        Proposition::Conjunction(conjuncts) => Proposition::Conjunction(parts(conjuncts)),
        Proposition::Disjunction(disjuncts) => Proposition::Disjunction(parts(disjuncts)),
        other => other.clone(),
    }
}
