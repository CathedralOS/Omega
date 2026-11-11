/*
The discharge decides the rows it is given, so a certificate for every row
says nothing about a row that is missing, names the wrong subject or sits at
the wrong point. This module closes that: before anything is decided it
recomputes, from the lowered form alone, what each machine owes and where,
and refuses tables that differ. The lowered form already carries every
identity needed, so nothing is taken from elaboration's word:

- An operation Terminal semantics gives a safety goal names its obligation
  (`ProofBearingScalarLeafSemantics::obligation`).
- A call names one obligation per `requires` clause of its callee
  (`requirement_obligations`), and the callee's contract says how many there
  are.
- An `ensures` clause names its published identity (`ContractClause`), and
  every normal return owes one guarantee per clause
  (control_flow.md: "All normal returns, including returns after a cycle,
  must establish the published guarantees.").

Each of those rows must have exactly the kind the lowered form owes and be
created at the point of what owes it: before the operation, or on the return
edge. The point decides which knowledge reaches the proof
(fact_validity.md: "A premise must dominate its use and remain valid on every
path to it."), so a row moved past a guard or past its own call would be
proved from facts that do not hold where the obligation sits. A row of these
kinds that nothing in the lowered form owes is refused too: it could only
stand in for one that is owed.

Two tables feed the proofs beside the obligation rows, and are checked the
same way. The invocation table says which callee and which actuals a call
requirement or a callee guarantee is stated over; the edge table says where
each edge goes, which is what dominance is computed from. Both are restated
by the lowered form's own call operations and terminators.

`DataDomain` rows are counted by `Storage::owed_are_minted`, which needs the
place walk to know which clauses a write owes; their point is checked here.

A disagreement is a defect of the route, not of the program, so it is a
`ForeignIdentity` refusal: `judge` returns it as `Err` and nothing is
accepted on it.
*/

use semantic_vocabulary::{
    BlockId, EdgeId, IdTable, InvocationSite, MachineId, ObligationId, PsiSemanticId, ScalarType,
    ValueId,
};
use terminal_check_tables::edges::{Actual, Callee, Destination};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::{
    Checked, Construct, MachineTables, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::{ModuleInvocationInputs, TerminalMachine, Terminator};

use crate::{refusal, semantics_refusal, terminal_machine};

fn index(raw: u64) -> usize {
    usize::try_from(raw - 1).unwrap_or(usize::MAX)
}

/// Where each edge of a terminator goes, as the edge table must state it.
fn successors(terminator: &Terminator) -> Vec<(EdgeId, Destination)> {
    match terminator {
        Terminator::Jump { edge, target, .. } => vec![(*edge, Destination::Block(*target))],
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => vec![
            (when_true.edge, Destination::Block(when_true.target)),
            (when_false.edge, Destination::Block(when_false.target)),
        ],
        Terminator::StructuralCase { cases, .. } => cases
            .iter()
            .map(|case| (case.edge, Destination::Block(case.target)))
            .collect(),
        Terminator::Return { edge, .. }
        | Terminator::ReturnUnit { edge, .. }
        | Terminator::ReturnStructural { edge, .. } => vec![(*edge, Destination::Return)],
        Terminator::InvokeNoNormal { edge, .. }
        | Terminator::ExternalCompletion { edge, .. }
        | Terminator::Crash { edge, .. } => vec![(*edge, Destination::NoNormal)],
    }
}

struct Minted<'a> {
    machine: MachineId,
    machines: &'a [TerminalMachine],
    tables: &'a MachineTables,
    /// Whether the lowered form has claimed each obligation row, by position.
    obligations: Vec<bool>,
    /// The same for each invocation row.
    invocations: Vec<bool>,
}

impl Minted<'_> {
    fn foreign(&self, concerns: Construct, detail: &str) -> Refusal {
        refusal(
            RefusalReason::ForeignIdentity,
            self.machine,
            concerns,
            detail,
        )
    }

    /// The row `obligation` must be exactly `kind`, created at `point`, and
    /// owed by nothing else.
    fn claim(
        &mut self,
        obligation: ObligationId,
        kind: ObligationKind,
        point: ProgramPoint,
    ) -> Checked<()> {
        let concerns = Construct::Obligation(obligation);
        let row = self.tables.obligations.get(obligation).ok_or_else(|| {
            self.foreign(concerns, "the lowered form owes an obligation with no row")
        })?;
        if row.kind != kind {
            return Err(self.foreign(
                concerns,
                "an obligation row is not what the lowered form owes under its identity",
            ));
        }
        if row.created_at != point {
            return Err(self.foreign(
                concerns,
                "an obligation is not created at the point of what owes it",
            ));
        }
        let machine = self.machine;
        let claimed = self
            .obligations
            .get_mut(index(obligation.get()))
            .ok_or_else(|| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    machine,
                    concerns,
                    "the lowered form owes an obligation with no row",
                )
            })?;
        if std::mem::replace(claimed, true) {
            return Err(self.foreign(concerns, "two constructs owe one obligation row"));
        }
        Ok(())
    }

    /// One call of the lowered form: its invocation row restates the callee
    /// and the actuals, and it owes one `CallRequirement` per `requires`
    /// clause of the callee, in clause order, at `before`.
    fn call(
        &mut self,
        site: InvocationSite,
        inputs: ModuleInvocationInputs<'_>,
        before: ProgramPoint,
        concerns: Construct,
    ) -> Checked<()> {
        let mut rows = self
            .tables
            .invocations
            .iter()
            .filter(|(_, row)| row.site == site);
        let (Some((invocation, row)), None) = (rows.next(), rows.next()) else {
            return Err(self.foreign(concerns, "a call does not have exactly one invocation row"));
        };
        let concerns = Construct::Invocation(invocation);
        if row.callee != Callee::Machine(inputs.callee) {
            return Err(self.foreign(concerns, "an invocation row names another callee"));
        }
        // The scalar lane and the record lane, each in argument order.
        let mut scalars = inputs.arguments.iter();
        let mut records = inputs.structural_arguments.iter();
        for actual in &row.actuals {
            let restated = match actual {
                Actual::Value(value) => scalars.next() == Some(value),
                Actual::Place(place) => records.next().is_some_and(|argument| {
                    argument.path.is_empty() && self.tables.roots.get(argument.place) == Some(place)
                }),
            };
            if !restated {
                return Err(self.foreign(
                    concerns,
                    "an invocation row's actuals are not the call's arguments",
                ));
            }
        }
        if scalars.next().is_some() || records.next().is_some() {
            return Err(self.foreign(
                concerns,
                "an invocation row's actuals are not the call's arguments",
            ));
        }
        let clauses = terminal_machine(self.machines, inputs.callee, self.machine)?
            .contract
            .requires
            .len();
        if inputs.requirement_obligations.len() != clauses
            || row.requirements != inputs.requirement_obligations
        {
            return Err(self.foreign(
                concerns,
                "a call does not owe one requirement per requires clause of its callee",
            ));
        }
        for (clause, obligation) in inputs.requirement_obligations.iter().enumerate() {
            let clause = crate::position(clause, self.machine)?;
            self.claim(
                *obligation,
                ObligationKind::CallRequirement { invocation, clause },
                before,
            )?;
        }
        if let Some(claimed) = self.invocations.get_mut(index(invocation.get())) {
            *claimed = true;
        }
        Ok(())
    }
}

/// Refuses `tables` unless its obligation rows, invocation rows and edge
/// rows are the ones `lowered` itself owes and states.
pub(crate) fn follow_the_lowered_form(
    id: MachineId,
    machines: &[TerminalMachine],
    lowered: &TerminalMachine,
    tables: &MachineTables,
    types: &IdTable<ValueId, ScalarType>,
) -> Checked<()> {
    let mut minted = Minted {
        machine: id,
        machines,
        tables,
        obligations: vec![false; tables.obligations.len()],
        invocations: vec![false; tables.invocations.len()],
    };

    // Every edge of a terminator has its own row, which goes where the
    // terminator does. A row no terminator owns is left alone: it can only
    // add a way into its destination, which takes dominance away and adds a
    // path to every backward walk, so nothing is proved or established on
    // its account.
    let mut edges = vec![false; tables.edges.len()];
    for block in &lowered.blocks {
        for (edge, destination) in successors(&block.terminator) {
            let concerns = Construct::Edge(edge);
            let row = tables
                .edges
                .get(edge)
                .ok_or_else(|| minted.foreign(concerns, "a terminator's edge has no row"))?;
            if row.source != block.id || row.destination != destination {
                return Err(minted.foreign(
                    concerns,
                    "an edge row does not go where its terminator does",
                ));
            }
            if std::mem::replace(&mut edges[index(edge.get())], true) {
                return Err(minted.foreign(concerns, "two terminators own one edge"));
            }
        }
    }

    for (clause, ensures) in lowered.contract.ensures.iter().enumerate() {
        minted.claim(
            ensures.obligation,
            ObligationKind::PublishedGuarantee {
                clause: crate::position(clause, id)?,
            },
            ProgramPoint::BlockEntry(lowered.entry),
        )?;
    }
    // The point before each operation, by `OperationId - 1`.
    let mut before_operation: Vec<Option<ProgramPoint>> = Vec::new();
    for block in &lowered.blocks {
        let mut before = ProgramPoint::BlockEntry(block.id);
        for operation in &block.operations {
            let slot = index(operation.id.get());
            if before_operation.len() <= slot {
                before_operation.resize(slot + 1, None);
            }
            before_operation[slot] = Some(before);
            if let Some(semantics) =
                terminal_semantics::proof_bearing_scalar_leaf_semantics(operation, types)
                    .map_err(semantics_refusal(id, operation.id))?
            {
                minted.claim(
                    semantics.obligation(),
                    ObligationKind::OperationSafety {
                        operation: operation.id,
                    },
                    before,
                )?;
            }
            if let Some(inputs) = operation.kind.module_invocation() {
                minted.call(
                    InvocationSite::Operation(operation.id),
                    inputs,
                    before,
                    Construct::Operation(operation.id),
                )?;
            }
            before = ProgramPoint::AfterOperation(operation.id);
        }
        if let Some(inputs) = block.terminator.module_invocation() {
            minted.call(
                InvocationSite::InvokeNoNormal(block.terminator.edge()),
                inputs,
                before,
                Construct::Block(block.id),
            )?;
        }
        if let Terminator::Return { edge, .. }
        | Terminator::ReturnUnit { edge, .. }
        | Terminator::ReturnStructural { edge, .. } = &block.terminator
        {
            minted.returns(lowered, block.id, *edge)?;
        }
    }

    for (obligation, row) in tables.obligations.iter() {
        let concerns = Construct::Obligation(obligation);
        match row.kind {
            ObligationKind::DataDomain { operation, .. } => {
                let before = before_operation
                    .get(index(operation.get()))
                    .copied()
                    .flatten()
                    .ok_or_else(|| {
                        minted.foreign(concerns, "a data requires obligation names no operation")
                    })?;
                if row.created_at != before {
                    return Err(minted.foreign(
                        concerns,
                        "an obligation is not created at the point of what owes it",
                    ));
                }
            }
            ObligationKind::OperationSafety { .. }
            | ObligationKind::CallRequirement { .. }
            | ObligationKind::NormalGuarantee { .. }
            | ObligationKind::PublishedGuarantee { .. } => {
                if !minted.obligations[index(obligation.get())] {
                    return Err(minted.foreign(
                        concerns,
                        "an obligation row that nothing in the lowered form owes",
                    ));
                }
            }
            // Kinds the route does not serve are refused where they are
            // decided (`goal`).
            ObligationKind::Claim { .. }
            | ObligationKind::Loan { .. }
            | ObligationKind::RestorationWindow { .. }
            | ObligationKind::DomainDebt { .. }
            | ObligationKind::EstablishmentDebt { .. }
            | ObligationKind::ArrivalInvariant { .. }
            | ObligationKind::CrashCoverage { .. }
            | ObligationKind::WellFoundedness { .. }
            | ObligationKind::RankComparison { .. } => {}
        }
    }
    // An invocation row is where a callee's `ensures` becomes knowledge
    // after a call, so one that restates no call would be a premise nothing
    // established.
    for ((invocation, row), claimed) in tables.invocations.iter().zip(&minted.invocations) {
        if *claimed {
            continue;
        }
        let concerns = Construct::Invocation(invocation);
        return Err(match row.callee {
            Callee::Selected { .. } => refusal(
                RefusalReason::Unsupported,
                id,
                concerns,
                "call through a selector value",
            ),
            Callee::Machine(_) => minted.foreign(
                concerns,
                "an invocation row restates no call of this machine",
            ),
        });
    }
    Ok(())
}

impl Minted<'_> {
    /// One normal return owes one `NormalGuarantee` per `ensures` clause, on
    /// its edge. The lowered form does not name these rows, so each is found
    /// by its kind; a second row of the same kind stays unclaimed and is
    /// refused with the rows nothing owes.
    fn returns(&mut self, lowered: &TerminalMachine, block: BlockId, exit: EdgeId) -> Checked<()> {
        for (clause, ensures) in lowered.contract.ensures.iter().enumerate() {
            let kind = ObligationKind::NormalGuarantee {
                exit,
                clause: crate::position(clause, self.machine)?,
                published: ensures.obligation,
            };
            let (obligation, _) = self
                .tables
                .obligations
                .iter()
                .find(|(_, row)| row.kind == kind)
                .ok_or_else(|| {
                    self.foreign(
                        Construct::Block(block),
                        "a normal return owes a guarantee with no row",
                    )
                })?;
            self.claim(obligation, kind, ProgramPoint::Edge(exit))?;
        }
        Ok(())
    }
}
