/*
Lowering of one closed body to a Terminal Psi machine and its check tables.
Every identity is minted by pushing its row, so a `ValueId`, `EdgeId`,
`ObligationId`, `InvocationId` or root `PlaceId` is the same integer in the
module and in `MachineTables`, and each block, operation, edge and obligation
also gets a `MachineSources` row in lockstep. Records, their places and the
write log are in `places`.

Evaluation order is `wiki/spec/language/expressions.md`, "Evaluation
schedule": strict operands left then right, call arguments left to right,
`&&`/`||` evaluate the right operand only when the left does not decide, and
only the selected transition arm runs. Each operation is emitted after its
operands, so block order is evaluation order.

Obligations are created, never discharged:

- `OperationSafety` for each partial operation (control_flow.md: "Each
  partial operation still proves its own safety"). Partial means the
  operation's `integer_policy_bridge` has formation conditions; for `Exact`
  integers that is every add, subtract, multiply, divide, remainder and shift.
- `CallRequirement` per callee `requires` clause per call (fact_validity.md:
  "one exact obligation for every callee `requires` clause").
- `NormalGuarantee` per `ensures` clause per normal exit (control_flow.md:
  "All normal returns ... must establish the published guarantees.").
- `CrashCoverage` per crash site and cause (calls_and_outcomes.md: "The
  verifier proves every site guard from independent entry/CFG facts and
  checks same-cause coverage"). A site is a block whose terminator leaves
  without a successor (a `Crash` with its one cause, or an `InvokeNoNormal`
  with one obligation per cause among the callee's substituted routes), or a
  call operation whose callee may return and also publishes crash routes,
  again with one obligation per cause among the substituted routes.

Each is created at the point before the construct it guards: the program point
after the preceding operation, or the block entry when none precedes it.

A crash is a `Crash` terminator carrying its cause and its site guard: the
exact incoming conjunction of branch decisions on the path to it ("The guard
retains the exact incoming conjunction"), each stated as the branch value
equal to the Boolean its edge took. A call of a machine with no normal
completion is an `InvokeNoNormal` terminator: there is no normal successor to
return through ("A no-normal-result terminal transfer publishes no fictional
result or normal-return guarantee"). Both edges go to `Destination::NoNormal`.
*/

mod calls;
mod control;
mod operations;
pub(crate) mod places;

use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IdTable, MachineId, ObligationId, PlaceId, Proposition,
    ScalarQualificationSetId, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_check_tables::obligations::{ObligationKind, ObligationRow};
use terminal_check_tables::values::{ValueOrigin, ValueRow};
use terminal_check_tables::{
    Checked, Construct, MachineTables, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::{
    Block, BoundaryParameterKind, ContractClause, MachineContract, Operation,
    RegisterTargetApplicability, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralResultDeclaration,
    SuccessorEdge, TerminalMachine, TerminalMachineResult, Terminator, ValueDeclaration,
};

use self::places::{Bound, Records};
use crate::body::{Body, Signature, ValueType};
use crate::outcome::Outcome;
use crate::{MachineSources, SourceOrigin, Unpositioned};

#[derive(Debug)]
pub(crate) struct Lowered {
    pub machine: TerminalMachine,
    pub tables: MachineTables,
    pub sources: MachineSources,
}

pub(crate) fn machine(
    id: MachineId,
    body: &Body,
    signatures: &IdTable<MachineId, Signature>,
    outcomes: &IdTable<MachineId, Outcome>,
    records: &Records<'_>,
) -> Checked<Lowered> {
    let signature = terminal_check_tables::row(signatures, id, id, Construct::Machine)?;
    let mut lowering = Lowering {
        id,
        signature,
        signatures,
        outcomes,
        records,
        roots: IdTable::default(),
        entry_contents: Vec::new(),
        path: Vec::new(),
        tables: MachineTables::default(),
        sources: MachineSources::default(),
        blocks: IdTable::default(),
        current: None,
        point: None,
        slots: vec![None; body.slots.len()],
        selected: Vec::new(),
        slot_types: &body.slots,
        published: Vec::new(),
    };
    let entry = lowering.block(SourceOrigin::Unpositioned(Unpositioned::MachineEntry));
    let mut formals = Formals::default();
    for (position, value_type) in signature.parameters.iter().enumerate() {
        match *value_type {
            ValueType::Scalar(scalar_type) => {
                let value = lowering.tables.values.push(ValueRow {
                    scalar_type,
                    origin: ValueOrigin::MachineParameter {
                        position: lowering.position(position)?,
                    },
                    selects: None,
                });
                // Clause propositions name the `k`-th scalar formal as value
                // `k + 1` (`Signature::parameters`).
                if value.get() != formals.scalars.len() as u64 + 1 {
                    return Err(lowering.internal("scalar formals are not the first values"));
                }
                formals.scalars.push(declaration(value, scalar_type));
                formals.order.push(BoundaryParameterKind::Scalar);
                *lowering.slot_mut(position)? = Some(Bound::Scalar(value));
            }
            // A record formal is a root place the machine owns for the
            // activation; what it holds at entry is entry content.
            ValueType::Record(record) => {
                let lane = lowering.position(formals.records.len())?;
                let (place, _) =
                    lowering.root(StructuralPlaceKind::Parameter { position: lane }, record)?;
                // Clause terms name the `k`-th record formal as place
                // `k + 1` (`Signature::requires`).
                if place.get() != formals.records.len() as u64 + 1 {
                    return Err(lowering.internal("record formals are not the first places"));
                }
                formals.records.push(StructuralParameterDeclaration {
                    place,
                    position: lane,
                    structural_type: record,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                });
                formals.order.push(BoundaryParameterKind::Structural);
                *lowering.slot_mut(position)? = Some(Bound::Record(place));
            }
        }
    }
    // The scalar result pseudo-value comes right after the scalar formals,
    // which is how `ensures` clauses name `result` (`Signature::result`). A
    // record result is the machine's result place, a root of its own.
    let result = match signature.result {
        None => TerminalMachineResult::Unit,
        Some(ValueType::Scalar(scalar_type)) => {
            let value = lowering.value(scalar_type, ValueOrigin::MachineResult);
            if value.get() != formals.scalars.len() as u64 + 1 {
                return Err(lowering.internal("the result does not follow the scalar formals"));
            }
            TerminalMachineResult::Scalar(declaration(value, scalar_type))
        }
        Some(ValueType::Record(record)) => {
            let (place, _) = lowering.root(StructuralPlaceKind::Result, record)?;
            TerminalMachineResult::Structural(StructuralResultDeclaration {
                place,
                structural_type: record,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                reference_sources: Vec::new(),
            })
        }
    };
    lowering.enter(entry);
    // One published identity per `ensures` clause, the one `ContractClause`
    // carries; every normal exit's guarantee names it.
    for clause in 0..signature.ensures.len() {
        let clause = lowering.position(clause)?;
        let published = lowering.obligation(
            ObligationKind::PublishedGuarantee { clause },
            ProgramPoint::BlockEntry(entry),
            SourceOrigin::Unpositioned(Unpositioned::MachineEntry),
        );
        lowering.published.push(published);
    }
    for statement in &body.statements {
        lowering.statement(statement)?;
    }
    if lowering.open() {
        match signature.result {
            None => {
                lowering.return_unit(SourceOrigin::Unpositioned(Unpositioned::ImplicitReturn))?
            }
            Some(_) => return Err(lowering.internal("a value machine fell off its last statement")),
        }
    }
    lowering.finish(entry, formals, result)
}

/// A machine's formals in Terminal Psi's two lanes, and the authored order
/// that interleaves them.
#[derive(Default)]
struct Formals {
    scalars: Vec<ValueDeclaration>,
    records: Vec<StructuralParameterDeclaration>,
    order: Vec<BoundaryParameterKind>,
}

struct Lowering<'a> {
    id: MachineId,
    signature: &'a Signature,
    signatures: &'a IdTable<MachineId, Signature>,
    outcomes: &'a IdTable<MachineId, Outcome>,
    records: &'a Records<'a>,
    /// What each root place is and the record type it holds, indexed by its
    /// `PlaceId`: the machine's structural place declarations.
    roots: IdTable<PlaceId, (StructuralPlaceKind, StructuralTypeId)>,
    /// The entry content value minted for a formal's scalar leaf, indexed by
    /// that leaf's `ExactPlaceId`; `None` until the leaf is first read.
    entry_contents: Vec<Option<ValueId>>,
    /// The branch decisions on the path to the open block, in program order:
    /// each the branch value equal to the Boolean its edge took. Every arm
    /// completes the machine, and a short-circuit or `match` rejoins its
    /// own branches, so the path to a statement is one sequence. A case a
    /// dispatch selected is not such a decision and is not here; see
    /// `selected`.
    path: Vec<Proposition>,
    tables: MachineTables,
    sources: MachineSources,
    blocks: IdTable<BlockId, Draft>,
    /// The open block, `None` once it is terminated and no block is entered.
    current: Option<BlockId>,
    /// The program point after the last thing emitted in `current`.
    point: Option<ProgramPoint>,
    slots: Vec<Option<Bound>>,
    /// The dispatch arms the open block lies in, outermost first: for a case
    /// arm the sum it selected on, the case, and the block parameter each
    /// scalar payload field arrived as.
    selected: Vec<places::Chosen>,
    /// The declared type of each slot, as the body states it.
    slot_types: &'a [ValueType],
    /// The `PublishedGuarantee` of each `ensures` clause, by position: the
    /// identity Terminal Psi's `ContractClause` holds.
    published: Vec<ObligationId>,
}

#[derive(Default)]
struct Draft {
    parameters: Vec<ValueDeclaration>,
    operations: Vec<Operation>,
    terminator: Option<Terminator>,
}

enum Value {
    Scalar(ValueId),
    /// A root place holding the record value, which nothing else names.
    Record(PlaceId),
    Unit,
}

fn declaration(id: ValueId, scalar_type: ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        id,
        scalar_type,
        qualifications: ScalarQualificationSetId::ZERO,
    }
}

fn successor(edge: EdgeId, target: BlockId, arguments: Vec<ValueId>) -> SuccessorEdge {
    SuccessorEdge {
        edge,
        target,
        arguments,
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    }
}

impl Lowering<'_> {
    fn internal(&self, detail: &str) -> Refusal {
        Refusal {
            reason: RefusalReason::Unsupported,
            machine: self.id,
            concerns: self.current.map_or(Construct::Machine, Construct::Block),
            detail: format!("completion {detail}"),
        }
    }

    fn position(&self, index: usize) -> Checked<u32> {
        u32::try_from(index).map_err(|_| self.internal("position exceeds u32"))
    }

    fn slot_mut(&mut self, slot: usize) -> Checked<&mut Option<Bound>> {
        let id = self.id;
        self.slots.get_mut(slot).ok_or(Refusal {
            reason: RefusalReason::ForeignIdentity,
            machine: id,
            concerns: Construct::Machine,
            detail: String::from("slot is past the end of this body"),
        })
    }

    fn slot_type(&self, slot: usize) -> Checked<ValueType> {
        self.slot_types.get(slot).copied().ok_or(Refusal {
            reason: RefusalReason::ForeignIdentity,
            machine: self.id,
            concerns: Construct::Machine,
            detail: String::from("slot is past the end of this body"),
        })
    }

    fn value(&mut self, scalar_type: ScalarType, origin: ValueOrigin) -> ValueId {
        self.tables.values.push(ValueRow {
            scalar_type,
            origin,
            selects: None,
        })
    }

    fn obligation(
        &mut self,
        kind: ObligationKind,
        created_at: ProgramPoint,
        source: SourceOrigin,
    ) -> ObligationId {
        let obligation = self
            .tables
            .obligations
            .push(ObligationRow { kind, created_at });
        self.sources.obligations.push(source);
        obligation
    }

    fn finish(
        mut self,
        entry: BlockId,
        formals: Formals,
        result: TerminalMachineResult,
    ) -> Checked<Lowered> {
        let ensures = self
            .signature
            .ensures
            .iter()
            .zip(&self.published)
            .map(|(proposition, obligation)| ContractClause {
                obligation: *obligation,
                proposition: proposition.clone(),
            })
            .collect();
        let mut drafts = std::mem::take(&mut self.blocks);
        let mut blocks = Vec::with_capacity(drafts.len());
        for position in 1..=drafts.len() as u64 {
            let block =
                BlockId::new(position).ok_or_else(|| self.internal("block identity overflow"))?;
            let draft = drafts
                .get_mut(block)
                .ok_or_else(|| self.internal("block is past the end"))?;
            let operations = std::mem::take(&mut draft.operations);
            let block_parameters = std::mem::take(&mut draft.parameters);
            let Some(terminator) = draft.terminator.take() else {
                return Err(self.internal("a block was left without a terminator"));
            };
            let mut block = Block::plain(block, block_parameters, terminator);
            block.operations = operations;
            blocks.push(block);
        }
        let crash_routes =
            terminal_check_tables::row(self.outcomes, self.id, self.id, Construct::Machine)?
                .crash_routes
                .clone();
        let contract = MachineContract {
            crash_routes,
            requires: self.signature.requires.clone(),
            ensures,
            ..MachineContract::empty(
                ContractId::new(self.id.get()).ok_or_else(|| self.internal("contract identity"))?,
            )
        };
        let structural_places = self
            .roots
            .iter()
            .map(|(id, (kind, _))| StructuralPlaceDeclaration { id, kind: *kind })
            .collect();
        let machine = TerminalMachine {
            assembly_target_requirement: RegisterTargetApplicability::Any,
            id: self.id,
            declaration: None,
            attachment: None,
            nominal_cleanup_subject: None,
            parameters: formals.scalars,
            parameter_order: formals.order,
            structural_parameters: formals.records,
            ranked_scc: None,
            result,
            structural_places,
            entry_claims: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            declared_service_reach: Vec::new(),
            closed_reach_application: None,
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks,
            contract,
        };
        // A scalar machine has no structural place, so its place, root and
        // write tables are empty; `places` filled them for every record.
        Ok(Lowered {
            machine,
            tables: self.tables,
            sources: self.sources,
        })
    }
}
