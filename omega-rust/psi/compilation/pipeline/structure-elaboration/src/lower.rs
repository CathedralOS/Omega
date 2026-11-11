/*
Lowering of one closed scalar body to a Terminal Psi machine and its check
tables. Every identity is minted by pushing its row, so a `ValueId`,
`EdgeId`, `ObligationId` or `InvocationId` is the same integer in the module
and in `MachineTables`, and each also gets a `MachineSources` row in lockstep.

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

use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IdTable, MachineId, ObligationId, Proposition,
    ScalarQualificationSetId, ScalarType, ValueId,
};
use terminal_check_tables::obligations::{ObligationKind, ObligationRow};
use terminal_check_tables::values::{ValueOrigin, ValueRow};
use terminal_check_tables::{
    Checked, Construct, MachineTables, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::{
    Block, BoundaryParameterKind, ContractClause, MachineContract, Operation,
    RegisterTargetApplicability, SuccessorEdge, TerminalMachine, TerminalMachineResult, Terminator,
    ValueDeclaration,
};

use crate::body::{Body, Signature};
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
) -> Checked<Lowered> {
    let signature = terminal_check_tables::row(signatures, id, id, Construct::Machine)?;
    let mut lowering = Lowering {
        id,
        signature,
        signatures,
        outcomes,
        path: Vec::new(),
        tables: MachineTables::default(),
        sources: MachineSources::default(),
        blocks: IdTable::default(),
        current: None,
        point: None,
        slots: vec![None; body.slots.len()],
        published: Vec::new(),
    };
    let entry = lowering.block(SourceOrigin::Unpositioned(Unpositioned::MachineEntry));
    let mut parameters = Vec::with_capacity(signature.parameters.len());
    for (position, scalar_type) in signature.parameters.iter().enumerate() {
        let value = lowering.tables.values.push(ValueRow {
            scalar_type: *scalar_type,
            origin: ValueOrigin::MachineParameter {
                position: lowering.position(position)?,
            },
            selects: None,
        });
        // Clause propositions name formal `i` as value `i + 1`.
        if value.get() != position as u64 + 1 {
            return Err(lowering.internal("formals are not the first values"));
        }
        parameters.push(declaration(value, *scalar_type));
        *lowering.slot_mut(position)? = Some(value);
    }
    // The scalar result pseudo-value comes right after the formals, which is
    // how `ensures` clauses name `result` (`Signature::result`).
    let result = match signature.result {
        None => None,
        Some(scalar_type) => {
            let value = lowering.value(scalar_type, ValueOrigin::MachineResult);
            if value.get() != signature.parameters.len() as u64 + 1 {
                return Err(lowering.internal("the result does not follow the formals"));
            }
            Some(declaration(value, scalar_type))
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
    lowering.finish(entry, parameters, result)
}

struct Lowering<'a> {
    id: MachineId,
    signature: &'a Signature,
    signatures: &'a IdTable<MachineId, Signature>,
    outcomes: &'a IdTable<MachineId, Outcome>,
    /// The branch decisions on the path to the open block, in program order:
    /// each the branch value equal to the Boolean its edge took. Every arm
    /// completes the machine and short-circuit joins rejoin their own
    /// branch, so the path to a statement is one sequence.
    path: Vec<Proposition>,
    tables: MachineTables,
    sources: MachineSources,
    blocks: IdTable<BlockId, Draft>,
    /// The open block, `None` once it is terminated and no block is entered.
    current: Option<BlockId>,
    /// The program point after the last thing emitted in `current`.
    point: Option<ProgramPoint>,
    slots: Vec<Option<ValueId>>,
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

    fn slot_mut(&mut self, slot: usize) -> Checked<&mut Option<ValueId>> {
        let id = self.id;
        self.slots.get_mut(slot).ok_or(Refusal {
            reason: RefusalReason::ForeignIdentity,
            machine: id,
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
        parameters: Vec<ValueDeclaration>,
        result: Option<ValueDeclaration>,
    ) -> Checked<Lowered> {
        let result = match result {
            None => TerminalMachineResult::Unit,
            Some(declaration) => TerminalMachineResult::Scalar(declaration),
        };
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
        let parameter_order = vec![BoundaryParameterKind::Scalar; parameters.len()];
        let machine = TerminalMachine {
            assembly_target_requirement: RegisterTargetApplicability::Any,
            id: self.id,
            declaration: None,
            attachment: None,
            nominal_cleanup_subject: None,
            parameters,
            parameter_order,
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result,
            structural_places: Vec::new(),
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
        // SSA scalars: the lowered form has no structural places, so the
        // place, root and write tables are empty by construction.
        Ok(Lowered {
            machine,
            tables: self.tables,
            sources: self.sources,
        })
    }
}
