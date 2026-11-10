#![forbid(unsafe_code)]

/*
Discharge on the lowered form: every obligation that structure-only
elaboration minted is decided here, and an obligation counts as discharged
only when the proof kernel (`proof_admission::verify_obligation`) accepted a
certificate for it. Anything else is a `Refusal` with reason `Unproved`; there
is no interval verdict, no admitted route and no assumption.
(`wiki/drafts/designs/check_on_lowered_form.md`, "Route one";
`wiki/drafts/designs/stage_trust.md`, rule 2.)

Per machine it does three things:

- Fills the `knowledge` table from the lowered form alone: each entry
  `requires` clause at the entry block, each operation's result equation on
  its normal successor (Terminal semantics states it), and each callee
  `ensures` clause, under the call's actuals, after the call. Nothing comes
  from source shapes, names or any Psi 05 or validation fact.
- States each obligation's goal over Terminal values: an operation's safety
  goal is Terminal semantics' own canonical goal, a call requirement is the
  callee's `requires` clause under the call's actuals, a normal guarantee is
  this machine's `ensures` clause.
- Passes the knowledge whose program point dominates the obligation's to one
  prover (`prove`), which emits a certificate, and asks the kernel to decide
  it.

Kinds outside `OperationSafety`, `CallRequirement` and `NormalGuarantee` are
refused as `Unsupported`; elaboration mints none of them yet.
*/

mod graph;
mod prove;

#[cfg(test)]
mod tests;

use proof_admission::{
    AcceptedFact, CertificateEnvelope, EvidenceRoute, Obligation, ObligationClass,
    ProofSystemMarker, verify_obligation,
};
use semantic_vocabulary::{
    EvidenceIdentity, IdTable, InvocationSite, MachineId, ObligationId, Proposition,
    PropositionContext, PropositionId, PsiSemanticId, ScalarTerm, ScalarType, ValueId,
};
use structure_elaboration::Elaboration;
use terminal_check_tables::edges::{Actual, Callee};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::{Justification, KnowledgeRow};
use terminal_check_tables::{
    Checked, Construct, KnowledgeId, MachineTables, ProgramPoint, Refusal, RefusalReason,
};
use terminal_codec::ObligationEvidenceSeal;
use terminal_psi::TerminalMachine;

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

/// One machine's result.
#[derive(Debug, Clone)]
pub struct MachineDischarge {
    /// The proposition of each knowledge row, minted in lockstep with the
    /// row: `PropositionId` n belongs to `KnowledgeId` n. `KnowledgeRow`
    /// points at "the module's proposition table", which Terminal Psi does
    /// not have (README, "Findings").
    pub propositions: IdTable<PropositionId, Proposition>,
    /// One verdict per obligation row, indexed by the same `ObligationId`.
    pub verdicts: IdTable<ObligationId, Checked<Evidence>>,
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
                let Ok(evidence) = verdict else {
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
                            result.proposition(*row).cloned().ok_or_else(|| {
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

impl MachineDischarge {
    fn proposition(&self, row: KnowledgeId) -> Option<&Proposition> {
        PropositionId::new(row.get()).and_then(|id| self.propositions.get(id))
    }
}

/// Decides every obligation of every machine and fills each machine's
/// `knowledge` table. A refusal here concerns the module itself (an identity
/// that does not resolve, knowledge already present); a refusal of one
/// obligation is its verdict.
pub fn discharge(elaboration: &mut Elaboration) -> Checked<Discharge> {
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
        machines.push(machine(&module.machines, id, tables)?);
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
    id: MachineId,
    tables: &mut MachineTables,
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
    let types = tables.values.map(|_, row| row.scalar_type);
    let graph = Graph::new(id, lowered, tables)?;
    let mut knowledge = Knowledge::default();

    for (clause, proposition) in lowered.contract.requires.iter().enumerate() {
        knowledge.push(
            tables,
            proposition.clone(),
            ProgramPoint::BlockEntry(lowered.entry),
            Justification::EntryRequirement {
                clause: position(clause, id)?,
            },
        );
    }
    for block in &lowered.blocks {
        for operation in &block.operations {
            if let Some(equation) = result_equation(id, operation, &types)? {
                knowledge.push(
                    tables,
                    equation,
                    ProgramPoint::AfterOperation(operation.id),
                    Justification::OperationSemantics(operation.id),
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
        let InvocationSite::Operation(operation) = row.site else {
            return Err(refusal(
                RefusalReason::Unsupported,
                id,
                Construct::Invocation(invocation),
                "invocation outside an operation",
            ));
        };
        let callee = callee(id, invocation, &row.callee)?;
        let actuals = actuals(id, invocation, &row.actuals)?;
        let contract = &terminal_machine(machines, callee, id)?.contract;
        for (clause, ensures) in contract.ensures.iter().enumerate() {
            let proposition =
                substitute(&ensures.proposition, &actuals, tables).map_err(|detail| {
                    refusal(
                        RefusalReason::Unsupported,
                        id,
                        Construct::Invocation(invocation),
                        detail,
                    )
                })?;
            knowledge.push(
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
    let mut verdicts = IdTable::default();
    for (obligation, row) in tables.obligations.iter() {
        let verdict =
            goal(machines, lowered, id, obligation, &row.kind, tables, &types).and_then(|goal| {
                decide(
                    &context,
                    &graph,
                    &knowledge,
                    tables,
                    id,
                    obligation,
                    row.created_at,
                    goal,
                )
            });
        verdicts.push(verdict);
    }
    Ok(MachineDischarge {
        propositions: knowledge.propositions,
        verdicts,
    })
}

#[derive(Default)]
struct Knowledge {
    propositions: IdTable<PropositionId, Proposition>,
}

impl Knowledge {
    fn push(
        &mut self,
        tables: &mut MachineTables,
        proposition: Proposition,
        available_from: ProgramPoint,
        justification: Justification,
    ) {
        let proposition = self.propositions.push(proposition);
        tables.knowledge.push(KnowledgeRow {
            proposition,
            available_from,
            justification,
            premises: Vec::new(),
        });
    }

    fn proposition(&self, row: KnowledgeId) -> Option<&Proposition> {
        PropositionId::new(row.get()).and_then(|id| self.propositions.get(id))
    }
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

fn actuals(
    machine: MachineId,
    invocation: terminal_check_tables::InvocationId,
    actuals: &[Actual],
) -> Checked<Vec<ValueId>> {
    actuals
        .iter()
        .map(|actual| match actual {
            Actual::Value(value) => Ok(*value),
            Actual::Place(_) => Err(refusal(
                RefusalReason::Unsupported,
                machine,
                Construct::Invocation(invocation),
                "structural actual",
            )),
        })
        .collect()
}

/// Replaces callee formal `i` (callee value `i + 1`) by the caller's actual
/// `i`, in the clause forms elaboration produces. Any other form is refused:
/// substituting under a construct this function does not know would be a
/// silent change of meaning.
fn substitute(
    proposition: &Proposition,
    actuals: &[ValueId],
    tables: &MachineTables,
) -> Result<Proposition, &'static str> {
    let term = |term: &ScalarTerm| -> Result<ScalarTerm, &'static str> {
        match term {
            ScalarTerm::Value { id, scalar_type } => {
                let actual = usize::try_from(id.get() - 1)
                    .ok()
                    .and_then(|position| actuals.get(position))
                    .ok_or("clause names a value that is not a formal")?;
                let row = tables
                    .values
                    .get(*actual)
                    .ok_or("actual is past the end of the caller's values")?;
                if row.scalar_type != *scalar_type {
                    return Err("actual type differs from the formal's");
                }
                Ok(ScalarTerm::value(*actual, *scalar_type))
            }
            ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_) => Ok(term.clone()),
            _ => Err("clause term outside the scalar subset"),
        }
    };
    Ok(match proposition {
        Proposition::Truth => Proposition::Truth,
        Proposition::Equal(left, right) => Proposition::Equal(term(left)?, term(right)?),
        Proposition::LessThan(left, right) => Proposition::LessThan(term(left)?, term(right)?),
        Proposition::LessOrEqual(left, right) => {
            Proposition::LessOrEqual(term(left)?, term(right)?)
        }
        Proposition::Conjunction(parts) => Proposition::Conjunction(
            parts
                .iter()
                .map(|part| substitute(part, actuals, tables))
                .collect::<Result<_, _>>()?,
        ),
        _ => return Err("clause form outside the scalar subset"),
    })
}

/// The goal of one obligation, over this machine's values.
fn goal(
    machines: &[TerminalMachine],
    lowered: &TerminalMachine,
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
            if semantics.obligation() != obligation {
                return Err(refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    concerns,
                    "operation names another obligation",
                ));
            }
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
            let callee = callee(id, invocation, &row.callee)?;
            let actuals = actuals(id, invocation, &row.actuals)?;
            let contract = &terminal_machine(machines, callee, id)?.contract;
            let clause = usize::try_from(clause)
                .ok()
                .and_then(|clause| contract.requires.get(clause))
                .ok_or_else(|| {
                    refusal(
                        RefusalReason::ForeignIdentity,
                        id,
                        concerns,
                        "callee has no such requires clause",
                    )
                })?;
            substitute(clause, &actuals, tables)
                .map_err(|detail| refusal(RefusalReason::Unsupported, id, concerns, detail))
        }
        ObligationKind::NormalGuarantee { clause, .. } => usize::try_from(clause)
            .ok()
            .and_then(|clause| lowered.contract.ensures.get(clause))
            .map(|clause| clause.proposition.clone())
            .ok_or_else(|| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    concerns,
                    "machine has no such ensures clause",
                )
            }),
        ObligationKind::Claim { .. }
        | ObligationKind::Loan { .. }
        | ObligationKind::RestorationWindow { .. }
        | ObligationKind::DomainDebt { .. }
        | ObligationKind::EstablishmentDebt { .. }
        | ObligationKind::ArrivalInvariant { .. }
        | ObligationKind::CrashCoverage { .. }
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
    knowledge: &Knowledge,
    tables: &MachineTables,
    id: MachineId,
    obligation: ObligationId,
    point: ProgramPoint,
    goal: Proposition,
) -> Checked<Evidence> {
    let unproved = |detail: String| Refusal {
        reason: RefusalReason::Unproved,
        machine: id,
        concerns: Construct::Obligation(obligation),
        detail,
    };
    let mut assumption_rows = Vec::new();
    let mut axiom_rows = Vec::new();
    let mut assumptions = Vec::new();
    let mut axioms = Vec::new();
    for (row, knowledge_row) in tables.knowledge.iter() {
        if !graph.dominates(knowledge_row.available_from, point)? {
            continue;
        }
        let proposition = knowledge.proposition(row).ok_or_else(|| {
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
            | Justification::BlockInvariant { .. }
            | Justification::Derived { .. } => {
                assumption_rows.push(row);
                assumptions.push(proposition.clone());
            }
        }
    }
    let mut prover = prove::Prover::new(context, &assumptions, &axioms, &tables.values);
    let proof = prover
        .prove(&goal)
        .map_err(|reason| unproved(reason.to_owned()))?;
    let identity = EvidenceIdentity::new(obligation.get())
        .ok_or_else(|| unproved(String::from("evidence identity")))?;
    let certificate = CertificateEnvelope {
        identity,
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof,
    };
    let fact = verify_obligation(
        context,
        &Obligation {
            id: obligation,
            proposition: goal.clone(),
            class: ObligationClass::Derivable,
        },
        &assumptions,
        &axioms,
        EvidenceRoute::CertificateDerived(certificate.clone()),
    )
    .map_err(|error| unproved(format!("kernel {error:?}")))?;
    Ok(Evidence {
        goal,
        assumptions: assumption_rows,
        axioms: axiom_rows,
        certificate,
        fact,
    })
}
