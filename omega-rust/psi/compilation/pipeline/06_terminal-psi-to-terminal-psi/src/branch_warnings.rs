//! Report an impossible authored arm from one kernel-accepted refutation, using
//! its actual premise citations rather than searching again to minimize them.
//! These results are advisory and never discharge a program obligation.

use proof_admission::{
    AcceptedFactRoute, CertificateEnvelope, EvidenceRoute, Obligation, ObligationClass,
    ProofSystemMarker, verify_obligation,
};
use semantic_vocabulary::{
    BlockId, EvidenceIdentity, MachineId, ObligationId, Proposition, PropositionContext,
};
use terminal_check_tables::edges::Destination;
use terminal_check_tables::program::Branch;
use terminal_check_tables::values::Justification;
use terminal_check_tables::{KnowledgeId, MachineTables, ProgramPoint};
use terminal_psi::TerminalMachine;

use crate::graph::Graph;
use crate::{ImpossibleArm, ProofSearchActivity, hypotheses, prove};

struct PointRefutation {
    point: ProgramPoint,
    // Outer None means unqueried; inner None means no accepted refutation.
    rows: Option<Option<Vec<KnowledgeId>>>,
}

/// All queries share one immutable machine/context. Reuse is local to this
/// advisory analysis, never across revisions or proof-obligation allowances.
pub(super) struct Analysis<'machine> {
    context: &'machine PropositionContext,
    graph: &'machine Graph,
    tables: &'machine MachineTables,
    lowered: &'machine TerminalMachine,
    machine: MachineId,
    points: Vec<PointRefutation>,
    #[cfg(test)]
    pub(super) searches: usize,
}

impl<'machine> Analysis<'machine> {
    pub(super) fn new(
        context: &'machine PropositionContext,
        graph: &'machine Graph,
        tables: &'machine MachineTables,
        lowered: &'machine TerminalMachine,
        machine: MachineId,
    ) -> Self {
        Self {
            context,
            graph,
            tables,
            lowered,
            machine,
            points: Vec::new(),
            #[cfg(test)]
            searches: 0,
        }
    }

    fn point(&self, block: BlockId, at_end: bool) -> Option<ProgramPoint> {
        let position = usize::try_from(block.get() - 1).ok()?;
        let lowered = self
            .lowered
            .blocks
            .get(position)
            .filter(|row| row.id == block)?;
        let after = at_end.then(|| lowered.operations.last()).flatten();
        Some(after.map_or(ProgramPoint::BlockEntry(block), |operation| {
            ProgramPoint::AfterOperation(operation.id)
        }))
    }

    fn refuted(
        &mut self,
        point: ProgramPoint,
        activity: &mut ProofSearchActivity,
    ) -> Option<&[KnowledgeId]> {
        let position = self
            .points
            .binary_search_by_key(&point, |query| query.point)
            .ok()?;
        let cached = &mut self.points[position].rows;
        if cached.is_none() {
            #[cfg(test)]
            {
                self.searches += 1;
            }
            *cached = Some(refutation(
                self.context,
                self.graph,
                self.tables,
                self.machine,
                point,
                activity,
            ));
        }
        cached.as_ref()?.as_deref()
    }

    /// Warn only at the first contradictory branch, not at later arms whose
    /// incoming context is already impossible. Missing/refused evidence stays
    /// quiet: failed search does not prove that an arrival is impossible.
    pub(super) fn run(
        &mut self,
        branches: &[Branch],
        activity: &mut ProofSearchActivity,
    ) -> Vec<ImpossibleArm> {
        // A machine can contain many blocks but only a few authored arms.
        // Prepare only their exact query points, not a table spanning all its
        // blocks or operations. Empty branch lists allocate nothing.
        self.points = branches
            .iter()
            .flat_map(|branch| {
                [
                    self.point(branch.decided_in, true),
                    self.point(branch.block, false),
                ]
            })
            .flatten()
            .map(|point| PointRefutation { point, rows: None })
            .collect();
        self.points.sort_unstable_by_key(|query| query.point);
        self.points.dedup_by_key(|query| query.point);
        let mut impossible = Vec::new();
        let tables = self.tables;
        for branch in branches {
            let Some(before) = self.point(branch.decided_in, true) else {
                continue;
            };
            if self.refuted(before, activity).is_some() {
                continue;
            }
            let Some(entry) = self.point(branch.block, false) else {
                continue;
            };
            let Some(rows) = self.refuted(entry, activity) else {
                continue;
            };
            let premises = rows
                .iter()
                .filter_map(|row| tables.knowledge.get(*row))
                .map(|row| row.justification)
                .filter(|justification| match justification {
                    Justification::BranchCondition { edge, .. } => !tables
                        .edges
                        .get(*edge)
                        .is_some_and(|row| row.destination == Destination::Block(branch.block)),
                    _ => true,
                })
                .collect();
            impossible.push(ImpossibleArm {
                block: branch.block,
                premises,
            });
        }
        impossible
    }

    #[cfg(test)]
    pub(super) fn query_count(&self) -> usize {
        self.points.len()
    }
}

/// One search and kernel decision. The accepted certificate already records
/// its ambient assumptions, including scopes discharged inside the proof; do
/// not recover those dependencies by walking syntax or retrying subsets.
fn refutation(
    context: &PropositionContext,
    graph: &Graph,
    tables: &MachineTables,
    machine: MachineId,
    point: ProgramPoint,
    activity: &mut ProofSearchActivity,
) -> Option<Vec<KnowledgeId>> {
    let probe = ObligationId::new(1)?;
    let known = hypotheses(graph, tables, machine, probe, point).ok()?;
    let mut prover = prove::Prover::new(
        context,
        &known.assumptions,
        &known.axioms,
        &tables.values,
        activity,
    );
    let proof = prover.prove(&Proposition::Falsehood).ok()?;
    let accepted = verify_obligation(
        context,
        &Obligation {
            id: probe,
            proposition: Proposition::Falsehood,
            class: ObligationClass::Derivable,
        },
        &known.assumptions,
        &known.axioms,
        EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: EvidenceIdentity::new(probe.get())?,
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof,
        }),
    )
    .ok()?;
    let AcceptedFactRoute::CertificateDerived { acceptance, .. } = accepted.route else {
        return None;
    };
    // Keep authored row order regardless of the certificate's citation order.
    // Each retained citation must identify the exact supplied proposition.
    let mut used = vec![false; known.assumptions.len()];
    for premise in acceptance.assumptions {
        if known.assumptions.get(premise.index) != Some(&premise.proposition) {
            return None;
        }
        *used.get_mut(premise.index)? = true;
    }
    Some(
        known
            .assumption_rows
            .into_iter()
            .zip(used)
            .filter_map(|(row, used)| used.then_some(row))
            .collect(),
    )
}
