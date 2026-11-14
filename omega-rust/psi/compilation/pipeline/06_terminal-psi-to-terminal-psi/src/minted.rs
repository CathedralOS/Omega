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
- A crashing terminator owes coverage for its cause, and a no-normal call or
  a call that may return and crash owes it per cause among its
  continuations (calls_and_outcomes.md, "Crash"). An edge that passes scalar
  arguments owes its destination's predicate (control_flow.md: "one
  obligation for every actual incoming edge").

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
The clauses they are goals of come from a table beside the module
(`Elaboration::domains`); `domains_are_declared` refuses that table unless it
is the `requires` each type declaration of the module states.

A disagreement is a defect of the route, not of the program, so it is a
`ForeignIdentity` refusal: `judge` returns it as `Err` and nothing is
accepted on it.
*/

use semantic_vocabulary::{
    BlockId, EdgeId, IdTable, InvocationSite, MachineId, ObligationId, Proposition, PsiSemanticId,
    ScalarType, StructuralTypeId, ValueId,
};
use terminal_check_tables::edges::{Actual, Callee, Destination};
use terminal_check_tables::obligations::{CrashSite, ObligationKind};
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{
    Checked, Construct, MachineTables, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::{
    BoundaryParameterKind, ModuleInvocationInputs, StructuralTypeDeclaration, TerminalMachine,
    TerminalMachineResult, Terminator, ValueDeclaration,
};

use crate::{refusal, semantics_refusal};

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
    module: &'a terminal_psi::TerminalModule,
    tables: &'a MachineTables,
    /// Whether the lowered form has claimed each obligation row, by position.
    obligations: Vec<bool>,
    /// The same for each invocation row.
    invocations: Vec<bool>,
}

impl Minted<'_> {
    /// A row the lowered form owes but does not name: found by its exact
    /// kind, and claimed at `point`. A second row of the same kind stays
    /// unclaimed and is refused with the rows nothing owes.
    fn owed(
        &mut self,
        kind: ObligationKind,
        point: ProgramPoint,
        concerns: Construct,
    ) -> Checked<()> {
        let (obligation, _) = self
            .tables
            .obligations
            .iter()
            .find(|(_, row)| row.kind == kind)
            .ok_or_else(|| self.foreign(concerns, "a site owes an obligation with no row"))?;
        self.claim(obligation, kind, point)
    }

    /// The root a place lies beneath, by its parent links.
    fn root_of(
        &self,
        place: terminal_check_tables::ExactPlaceId,
    ) -> Option<semantic_vocabulary::PlaceId> {
        let mut at = place;
        // A parent is minted before its projections, so the chain is no
        // longer than the table.
        for _ in 0..=self.tables.places.len() {
            match self.tables.places.get(at)?.origin {
                terminal_check_tables::bindings::PlaceOrigin::Root(root) => return Some(root),
                terminal_check_tables::bindings::PlaceOrigin::Projection { parent, .. } => {
                    at = parent;
                }
            }
        }
        None
    }

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
        inputs: ModuleInvocationInputs<'_, semantic_vocabulary::TargetMachineCallable>,
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
        if row.callee != Callee::Declared(inputs.callee) {
            return Err(self.foreign(concerns, "an invocation row names another callee"));
        }
        // The scalar lane and the record lane, each in argument order.
        let mut scalars = inputs.arguments.iter();
        let mut records = inputs.structural_arguments.iter();
        for actual in &row.actuals {
            let restated = match actual {
                Actual::Value(value) => scalars.next() == Some(value),
                // An owned record argument is a root, passed whole. A loan
                // passes a place beneath a root; its full path is checked
                // against the place where loans are judged (`places::loans`).
                Actual::Place(place) => records.next().is_some_and(|argument| {
                    if argument.access == terminal_psi::StructuralAccess::Owned {
                        argument.path.is_empty()
                            && self.tables.roots.get(argument.place) == Some(place)
                    } else {
                        self.root_of(*place) == Some(argument.place)
                    }
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
        let callee = crate::callee(self.module, self.machine, invocation, row)?;
        if !inputs.claim_transfers.is_empty() || !inputs.completion_receipts.is_empty() {
            return Err(refusal(
                RefusalReason::Unsupported,
                self.machine,
                concerns,
                "call needs its exact claim-transfer and completion-receipt proof",
            ));
        }
        let clauses = self
            .module
            .callable_requires(callee)
            .ok_or_else(|| self.foreign(concerns, "callee has no canonical requires contract"))?
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
    module: &terminal_psi::TerminalModule,
    lowered: &TerminalMachine,
    tables: &MachineTables,
    types: &IdTable<ValueId, ScalarType>,
) -> Checked<()> {
    values_are_declared(id, lowered, tables)?;
    let mut minted = Minted {
        machine: id,
        module,
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
                        site: terminal_check_tables::obligations::ExecutionSite::Operation(
                            operation.id,
                        ),
                    },
                    before,
                )?;
            }
            for projection in operation.kind.structural_projections() {
                for segment in projection.path {
                    if let terminal_psi::StructuralPathSegment::RuntimeIndex {
                        obligation, ..
                    } = segment
                    {
                        minted.claim(
                            *obligation,
                            ObligationKind::OperationSafety {
                                site: terminal_check_tables::obligations::ExecutionSite::Operation(
                                    operation.id,
                                ),
                            },
                            before,
                        )?;
                    }
                }
            }
            if let Some(inputs) = operation.kind.declared_invocation() {
                minted.call(
                    InvocationSite::Operation(operation.id),
                    inputs,
                    before,
                    Construct::Operation(operation.id),
                )?;
            }
            // A call that may return and crash is a crash site of its own,
            // owing coverage per cause among its continuations.
            if let Some(inputs) = operation.kind.declared_invocation() {
                let routes = match inputs.callee {
                    terminal_psi::TargetMachineCallable::Machine(_) => inputs.crash_continuations,
                    terminal_psi::TargetMachineCallable::Boundary(boundary) => {
                        &module
                            .boundary_machine(boundary)
                            .ok_or_else(|| {
                                minted.foreign(
                                    Construct::Operation(operation.id),
                                    "call names an absent boundary",
                                )
                            })?
                            .crash_routes
                    }
                };
                for bucket in routes {
                    minted.owed(
                        ObligationKind::CrashCoverage {
                            site: CrashSite::Operation(operation.id),
                            cause: bucket.cause,
                        },
                        before,
                        Construct::Operation(operation.id),
                    )?;
                }
            }
            before = ProgramPoint::AfterOperation(operation.id);
        }
        for projection in block.terminator.storage_uses() {
            for segment in projection.path {
                if let terminal_psi::StructuralPathSegment::RuntimeIndex { obligation, .. } =
                    segment
                {
                    minted.claim(
                        *obligation,
                        ObligationKind::OperationSafety {
                            site: terminal_check_tables::obligations::ExecutionSite::Block(
                                block.id,
                            ),
                        },
                        before,
                    )?;
                }
            }
        }
        if let Some((site, inputs)) = block.terminator.declared_invocation() {
            minted.call(site, inputs, before, Construct::Block(block.id))?;
        }
        // A crashing terminator owes coverage at the point before it: its one
        // cause, or each cause among a no-normal call's continuations. An
        // edge that passes scalar arguments owes its destination's predicate
        // on that edge (control_flow.md: "one obligation for every actual
        // incoming edge").
        let site = CrashSite::Block(block.id);
        let concerns = Construct::Block(block.id);
        match &block.terminator {
            Terminator::Crash { cause, .. } => {
                let kind = ObligationKind::CrashCoverage {
                    site,
                    cause: *cause,
                };
                minted.owed(kind, before, concerns)?;
            }
            Terminator::InvokeNoNormal {
                crash_continuations,
                ..
            } => {
                for bucket in crash_continuations {
                    let kind = ObligationKind::CrashCoverage {
                        site,
                        cause: bucket.cause,
                    };
                    minted.owed(kind, before, concerns)?;
                }
            }
            Terminator::Jump {
                edge, arguments, ..
            } if !arguments.is_empty() => {
                let kind = ObligationKind::ArrivalInvariant { edge: *edge };
                minted.owed(kind, ProgramPoint::Edge(*edge), concerns)?;
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for successor in [when_true, when_false] {
                    if !successor.arguments.is_empty() {
                        let kind = ObligationKind::ArrivalInvariant {
                            edge: successor.edge,
                        };
                        minted.owed(kind, ProgramPoint::Edge(successor.edge), concerns)?;
                    }
                }
            }
            _ => {}
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
            | ObligationKind::PublishedGuarantee { .. }
            | ObligationKind::ArrivalInvariant { .. }
            | ObligationKind::CrashCoverage { .. } => {
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
            Callee::Declared(_) => minted.foreign(
                concerns,
                "an invocation row restates no call of this machine",
            ),
        });
    }
    Ok(())
}

/// Refuses a value table that is not the values the lowered form declares:
/// each formal, the scalar result, each block parameter and each scalar
/// operation result, under its own identity, with its declared type and the
/// origin of what declares it.
///
/// The type of a value is what Terminal semantics states an operation's goal
/// over and what the kernel bounds the value by, and its origin is what the
/// payload judgment reads to learn which test a condition is the result of.
/// The one kind of row the lowered form does not declare is the content a
/// formal's leaf held at entry, which no operation produces; `places` checks
/// those against the formal's type. A declared value restated as one would
/// make a read of that leaf equal to it.
fn values_are_declared(
    id: MachineId,
    lowered: &TerminalMachine,
    tables: &MachineTables,
) -> Checked<()> {
    let foreign = |value: ValueId, detail: &str| {
        refusal(
            RefusalReason::ForeignIdentity,
            id,
            Construct::Value(value),
            detail,
        )
    };
    let mut declared: Vec<Option<(ScalarType, ValueOrigin)>> = vec![None; tables.values.len()];
    let mut declare = |value: ValueDeclaration, origin: ValueOrigin| -> Checked<()> {
        let slot = declared
            .get_mut(index(value.id.get()))
            .ok_or_else(|| foreign(value.id, "the lowered form declares a value with no row"))?;
        if slot.replace((value.scalar_type, origin)).is_some() {
            return Err(foreign(
                value.id,
                "the lowered form declares one value twice",
            ));
        }
        Ok(())
    };
    // A formal's position is its place among all the formals, erased ones
    // included. An erased scalar formal has no runtime lane, so the lowered
    // form does not say where among the formals it was written; its row
    // does. Those positions are accepted only when they strictly increase in
    // the contract's order and lie among the formals, and the runtime
    // formals then take the remaining positions in their own order.
    let erased = &lowered.contract.erased_scalar_formals;
    let total = lowered.parameter_order.len() + erased.len();
    let mut erased_positions = Vec::with_capacity(erased.len());
    for formal in erased {
        let position = match tables.values.get(formal.id).map(|row| row.origin) {
            Some(ValueOrigin::MachineParameter { position }) => position,
            _ => return Err(foreign(formal.id, "an erased formal's row is not a formal")),
        };
        let increasing = erased_positions.last().is_none_or(|last| *last < position);
        if !increasing || usize::try_from(position).map_or(true, |position| position >= total) {
            return Err(foreign(
                formal.id,
                "an erased formal's position is out of order or past the formals",
            ));
        }
        erased_positions.push(position);
        declare(*formal, ValueOrigin::MachineParameter { position })?;
    }
    let mut runtime_positions = (0..total)
        .map(|position| crate::position(position, id))
        .collect::<Checked<Vec<u32>>>()?
        .into_iter()
        .filter(|position| erased_positions.binary_search(position).is_err());
    let mut scalars = lowered.parameters.iter();
    for kind in &lowered.parameter_order {
        let Some(position) = runtime_positions.next() else {
            break;
        };
        if !matches!(kind, BoundaryParameterKind::Scalar) {
            continue;
        }
        let Some(formal) = scalars.next() else {
            break;
        };
        declare(*formal, ValueOrigin::MachineParameter { position })?;
    }
    if let Some(formal) = scalars.next() {
        return Err(foreign(
            formal.id,
            "a scalar formal has no position among the formals",
        ));
    }
    if let TerminalMachineResult::Scalar(result) = &lowered.result {
        declare(*result, ValueOrigin::MachineResult)?;
    }
    for block in &lowered.blocks {
        for (position, parameter) in block.parameters.iter().enumerate() {
            declare(
                *parameter,
                ValueOrigin::BlockParameter {
                    block: block.id,
                    position: crate::position(position, id)?,
                },
            )?;
        }
        for operation in &block.operations {
            if let Some(result) = operation.result.as_scalar() {
                declare(result, ValueOrigin::Operation(operation.id))?;
            }
        }
    }
    for ((value, row), declared) in tables.values.iter().zip(&declared) {
        match declared {
            Some((scalar_type, origin)) => {
                if row.scalar_type != *scalar_type || row.origin != *origin {
                    return Err(foreign(
                        value,
                        "a value row is not the value the lowered form declares under its identity",
                    ));
                }
            }
            None => {
                if !matches!(row.origin, ValueOrigin::InitialContent { .. }) {
                    return Err(foreign(
                        value,
                        "a value row that nothing in the lowered form declares",
                    ));
                }
            }
        }
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

/// Refuses a table of data `requires` clauses that is not the one the
/// module's type declarations state. The clauses are the goals of every
/// `DataDomain` obligation and the premises a read of established storage
/// learns from its type, so a clause weakened, strengthened, dropped, added
/// or moved to another type in the table would change what is proved for
/// every construction, store and read of that type without touching the
/// lowered form. The declaration is what the module carries into its
/// encoding and identity; the table must restate it exactly, in authored
/// order, since a `DataDomain` obligation names its clause by position.
///
/// A type the table has no row for is one whose declaration states none: a
/// scalar local's primitive home, declared after the data types.
pub(crate) fn domains_are_declared(
    id: MachineId,
    declarations: &[StructuralTypeDeclaration],
    domains: &IdTable<StructuralTypeId, Vec<Proposition>>,
) -> Checked<()> {
    let foreign = |detail: &str| {
        refusal(
            RefusalReason::ForeignIdentity,
            id,
            Construct::Machine,
            detail,
        )
    };
    for (structural_type, _) in domains.iter() {
        if !declarations
            .iter()
            .any(|declaration| declaration.id == structural_type)
        {
            return Err(foreign("a row of requires clauses names no declared type"));
        }
    }
    for declaration in declarations {
        let restated = domains.get(declaration.id).map_or(&[][..], Vec::as_slice);
        if restated != declaration.requires.as_slice() {
            return Err(foreign(
                "a data type's requires clauses are not the ones its declaration states",
            ));
        }
    }
    Ok(())
}
