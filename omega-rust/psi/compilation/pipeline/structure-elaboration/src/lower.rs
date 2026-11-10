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

use semantic_vocabulary::{
    BlockId, ContractId, CrashCause, EdgeId, IdTable, InvocationSite, MachineId, ObligationId,
    OperationId, Proposition, ScalarQualificationSetId, ScalarTerm, ScalarType, ValueId,
};
use terminal_check_tables::edges::{
    Actual, Callee, Destination, Disposition, EdgeRow, InvocationRow,
};
use terminal_check_tables::obligations::{CrashSite, ObligationKind, ObligationRow};
use terminal_check_tables::values::{ValueOrigin, ValueRow};
use terminal_check_tables::{
    Checked, Construct, MachineTables, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::{
    Block, BoundaryParameterKind, ContractClause, CrashPredicateTerm, MachineContract, Operation,
    OperationKind, OperationResult, RegisterTargetApplicability, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, Terminator, ValueDeclaration,
};

use crate::body::{Body, Expression, ExpressionKind, Signature, Statement, Strict, Target};
use crate::canonical;
use crate::outcome::Outcome;
use crate::{MachineSources, SourceOrigin, Unpositioned, Unserved, refuse};

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
        guarantees: None,
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
    /// The `NormalGuarantee` obligations of the first normal exit, one per
    /// `ensures` clause; Terminal Psi's `ContractClause` holds one per clause
    /// (README, "Findings").
    guarantees: Option<Vec<ObligationId>>,
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

    fn block(&mut self, source: SourceOrigin) -> BlockId {
        let block = self.blocks.push(Draft::default());
        self.sources.blocks.push(source);
        block
    }

    fn enter(&mut self, block: BlockId) {
        self.current = Some(block);
        self.point = Some(ProgramPoint::BlockEntry(block));
    }

    fn open(&self) -> bool {
        self.current.is_some()
    }

    fn current(&self) -> Checked<BlockId> {
        self.current
            .ok_or_else(|| self.internal("no open block to emit into"))
    }

    fn here(&self) -> Checked<ProgramPoint> {
        self.point
            .ok_or_else(|| self.internal("no program point in a terminated block"))
    }

    fn draft(&mut self) -> Checked<&mut Draft> {
        let block = self.current()?;
        let id = self.id;
        self.blocks.get_mut(block).ok_or(Refusal {
            reason: RefusalReason::ForeignIdentity,
            machine: id,
            concerns: Construct::Block(block),
            detail: String::from("block is past the end of this machine"),
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

    fn operation_identity(&mut self, source: SourceOrigin) -> OperationId {
        self.sources.operations.push(source)
    }

    /// The obligation guarding `operation`, created at the point before it.
    fn safety(&mut self, operation: OperationId, source: SourceOrigin) -> Checked<ObligationId> {
        let before = self.here()?;
        Ok(self.obligation(
            ObligationKind::OperationSafety { operation },
            before,
            source,
        ))
    }

    fn emit(
        &mut self,
        operation: OperationId,
        result: Option<ScalarType>,
        kind: OperationKind,
    ) -> Checked<Value> {
        let (result, value) = match result {
            Some(scalar_type) => {
                let value = self.value(scalar_type, ValueOrigin::Operation(operation));
                (
                    OperationResult::Scalar(declaration(value, scalar_type)),
                    Value::Scalar(value),
                )
            }
            None => (OperationResult::Unit, Value::Unit),
        };
        self.draft()?
            .operations
            .push(Operation::unsuspended(operation, result, kind));
        self.point = Some(ProgramPoint::AfterOperation(operation));
        Ok(value)
    }

    fn edge(&mut self, destination: Destination, source: SourceOrigin) -> Checked<EdgeId> {
        let edge = self.tables.edges.push(EdgeRow {
            source: self.current()?,
            destination,
            dispositions: Vec::new(),
        });
        self.sources.edges.push(source);
        Ok(edge)
    }

    fn terminate(&mut self, terminator: Terminator) -> Checked<()> {
        let draft = self.draft()?;
        if draft.terminator.is_some() {
            return Err(self.internal("block terminated twice"));
        }
        draft.terminator = Some(terminator);
        self.current = None;
        self.point = None;
        Ok(())
    }

    fn branch(
        &mut self,
        condition: ValueId,
        when_true: (BlockId, Vec<ValueId>),
        when_false: (BlockId, Vec<ValueId>),
        source: SourceOrigin,
    ) -> Checked<()> {
        let true_edge = self.edge(Destination::Block(when_true.0), source)?;
        let false_edge = self.edge(Destination::Block(when_false.0), source)?;
        self.terminate(Terminator::Conditional {
            condition,
            when_true: successor(true_edge, when_true.0, when_true.1),
            when_false: successor(false_edge, when_false.0, when_false.1),
        })
    }

    fn jump(
        &mut self,
        target: BlockId,
        arguments: Vec<ValueId>,
        source: SourceOrigin,
    ) -> Checked<()> {
        let edge = self.edge(Destination::Block(target), source)?;
        self.terminate(Terminator::Jump {
            edge,
            target,
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            cleanup_actions: Vec::new(),
        })
    }

    /// One `NormalGuarantee` per `ensures` clause on this normal exit.
    fn guarantee(&mut self, exit: EdgeId, source: SourceOrigin) -> Checked<()> {
        let mut minted = Vec::with_capacity(self.signature.ensures.len());
        for clause in 0..self.signature.ensures.len() {
            let clause = self.position(clause)?;
            minted.push(self.obligation(
                ObligationKind::NormalGuarantee { exit, clause },
                ProgramPoint::Edge(exit),
                source,
            ));
        }
        if self.guarantees.is_none() {
            self.guarantees = Some(minted);
        }
        Ok(())
    }

    fn return_value(&mut self, value: ValueId, source: SourceOrigin) -> Checked<()> {
        let edge = self.edge(Destination::Return, source)?;
        self.terminate(Terminator::Return {
            edge,
            value,
            cleanup_actions: Vec::new(),
        })?;
        self.guarantee(edge, source)
    }

    fn return_unit(&mut self, source: SourceOrigin) -> Checked<()> {
        let edge = self.edge(Destination::Return, source)?;
        self.terminate(Terminator::ReturnUnit {
            edge,
            cleanup_actions: Vec::new(),
        })?;
        self.guarantee(edge, source)
    }

    fn statement(&mut self, statement: &Statement) -> Checked<()> {
        if !self.open() {
            return Err(
                self.internal("a statement follows an unconditional arm or a no-successor exit")
            );
        }
        match statement {
            Statement::Bind { slot, value } => {
                let value = self.scalar(value)?;
                *self.slot_mut(*slot)? = Some(value);
                Ok(())
            }
            Statement::Evaluate(expression) => match self.no_normal(expression)? {
                Some((callee, arguments)) => {
                    self.invoke_no_normal(callee, arguments, expression.source)
                }
                None => self.expression(expression).map(drop),
            },
            Statement::Arm {
                guard: Some(guard),
                target,
                source,
            } => {
                let condition = self.scalar(guard)?;
                let arm = self.block(*source);
                let rest = self.block(SourceOrigin::Unpositioned(
                    Unpositioned::DispatchContinuation,
                ));
                self.branch(
                    condition,
                    (arm, Vec::new()),
                    (rest, Vec::new()),
                    guard.source,
                )?;
                let taken = self.decision(condition, true)?;
                let not_taken = self.decision(condition, false)?;
                self.enter(arm);
                self.path.push(taken);
                self.complete(target, *source)?;
                self.path.pop();
                self.enter(rest);
                self.path.push(not_taken);
                Ok(())
            }
            Statement::Arm {
                guard: None,
                target,
                source,
            } => self.complete(target, *source),
        }
    }

    fn complete(&mut self, target: &Target, source: SourceOrigin) -> Checked<()> {
        match target {
            Target::Unit => self.return_unit(source),
            Target::Crash(cause) => self.crash(*cause, source),
            Target::Value(expression) => match self.no_normal(expression)? {
                Some((callee, arguments)) => {
                    self.invoke_no_normal(callee, arguments, expression.source)
                }
                None => match self.expression(expression)? {
                    Value::Scalar(value) => self.return_value(value, expression.source),
                    Value::Unit => self.return_unit(expression.source),
                },
            },
        }
    }

    /// One branch decision on the path: the branch value equal to the Boolean
    /// its edge took.
    fn decision(&self, condition: ValueId, taken: bool) -> Checked<Proposition> {
        canonical::equal(
            ScalarTerm::Value {
                id: condition,
                scalar_type: ScalarType::Boolean,
            },
            ScalarTerm::Boolean(taken),
            self.id,
        )
    }

    fn outcome(&self, callee: MachineId) -> Checked<&'_ Outcome> {
        terminal_check_tables::row(self.outcomes, callee, self.id, Construct::Machine)
    }

    /// The callee and arguments of a call in terminal position whose callee
    /// has no normal completion; such a call is the block's terminator.
    fn no_normal<'e>(
        &self,
        expression: &'e Expression,
    ) -> Checked<Option<(MachineId, &'e [Expression])>> {
        match &expression.kind {
            ExpressionKind::Call { callee, arguments }
                if !self.outcome(*callee)?.completes_normally =>
            {
                Ok(Some((*callee, arguments)))
            }
            ExpressionKind::Call { .. }
            | ExpressionKind::Integer(_)
            | ExpressionKind::Boolean(_)
            | ExpressionKind::Slot(_)
            | ExpressionKind::Strict { .. }
            | ExpressionKind::Not(_)
            | ExpressionKind::BitwiseNot(_)
            | ExpressionKind::ShortCircuit { .. } => Ok(None),
        }
    }

    /// The fate of each conserved obligation on an edge with no normal
    /// successor. A crash abandons them (effects.md: "Crashes have explicit
    /// no-successor abandonment, not a missing cleanup list"; `Fate::Abandoned`),
    /// and a no-normal invocation consumes or transfers them. The subset
    /// mints no conserved obligation, and this lowering does not track which
    /// are live, so one existing here is refused rather than given a guessed
    /// fate; with none, the edge has no dispositions.
    fn no_successor_dispositions(&self) -> Checked<Vec<Disposition>> {
        for raw in 1..=self.tables.obligations.len() as u64 {
            let obligation = ObligationId::new(raw).ok_or_else(|| self.internal("obligation"))?;
            let row = terminal_check_tables::row(
                &self.tables.obligations,
                obligation,
                self.id,
                Construct::Obligation(obligation),
            )?;
            if row.kind.is_conserved() {
                return Err(refuse(
                    self.id,
                    Construct::Obligation(obligation),
                    Unserved::Completion("conserved obligation at a no-successor edge"),
                ));
            }
        }
        Ok(Vec::new())
    }

    fn no_successor_edge(&mut self, source: SourceOrigin) -> Checked<EdgeId> {
        let dispositions = self.no_successor_dispositions()?;
        let edge = self.tables.edges.push(EdgeRow {
            source: self.current()?,
            destination: Destination::NoNormal,
            dispositions,
        });
        self.sources.edges.push(source);
        Ok(edge)
    }

    /// `crash Cause;`: one `CrashCoverage` obligation for this site and
    /// cause, then the no-successor terminator. Its frontier lower bound is
    /// empty because the subset holds no claims.
    fn crash(&mut self, cause: CrashCause, source: SourceOrigin) -> Checked<()> {
        let site = self.current()?;
        let before = self.here()?;
        let site_guard =
            canonical::clauses(self.path.clone(), canonical::Repeated::Merge, self.id)?
                .into_iter()
                .map(CrashPredicateTerm::new)
                .collect();
        self.obligation(
            ObligationKind::CrashCoverage {
                site: CrashSite::Block(site),
                cause,
            },
            before,
            source,
        );
        let edge = self.no_successor_edge(source)?;
        self.terminate(Terminator::Crash {
            edge,
            cause,
            site_guard,
            frontier_lower_bound: Vec::new(),
        })
    }

    /// A call of a machine with no normal completion, in terminal position:
    /// the arguments, then the invocation as the block's terminator, with the
    /// callee's routes substituted as its crash continuations and one
    /// `CrashCoverage` obligation per surviving cause at this site.
    fn invoke_no_normal(
        &mut self,
        callee: MachineId,
        arguments: &[Expression],
        source: SourceOrigin,
    ) -> Checked<()> {
        let mut actuals = Vec::with_capacity(arguments.len());
        for argument in arguments {
            actuals.push(self.scalar(argument)?);
        }
        let crash_continuations =
            canonical::substituted_routes(&self.outcome(callee)?.crash_routes, &actuals, self.id)?;
        let site = self.current()?;
        let before = self.here()?;
        let edge = self.no_successor_edge(source)?;
        let requirements = self.invocation(
            InvocationSite::InvokeNoNormal(edge),
            callee,
            &actuals,
            before,
            source,
        )?;
        for bucket in &crash_continuations {
            self.obligation(
                ObligationKind::CrashCoverage {
                    site: CrashSite::Block(site),
                    cause: bucket.cause,
                },
                before,
                source,
            );
        }
        self.terminate(Terminator::InvokeNoNormal {
            edge,
            static_reach_binding: None,
            callee,
            arguments: actuals,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: requirements,
            crash_continuations,
        })
    }

    fn scalar(&mut self, expression: &Expression) -> Checked<ValueId> {
        match self.expression(expression)? {
            Value::Scalar(value) => Ok(value),
            Value::Unit => Err(self.internal("a Unit call is used as a value")),
        }
    }

    fn result_type(&self, expression: &Expression) -> Checked<ScalarType> {
        expression
            .result
            .ok_or_else(|| self.internal("an operand has no scalar type"))
    }

    fn expression(&mut self, expression: &Expression) -> Checked<Value> {
        let source = expression.source;
        match &expression.kind {
            ExpressionKind::Integer(value) => {
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    expression.result,
                    OperationKind::IntegerConstant { value: *value },
                )
            }
            ExpressionKind::Boolean(value) => {
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    expression.result,
                    OperationKind::BooleanConstant { value: *value },
                )
            }
            ExpressionKind::Slot(slot) => {
                let bound = *self.slot_mut(*slot)?;
                bound
                    .map(Value::Scalar)
                    .ok_or_else(|| self.internal("a slot is read before it is bound"))
            }
            ExpressionKind::Not(operand) => {
                let operand = self.scalar(operand)?;
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    expression.result,
                    OperationKind::BooleanNot { operand },
                )
            }
            ExpressionKind::BitwiseNot(operand) => {
                let operand = self.scalar(operand)?;
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    expression.result,
                    OperationKind::IntegerBitwiseNot { operand },
                )
            }
            ExpressionKind::Strict {
                operation,
                left: left_expression,
                right,
            } => {
                let left = self.scalar(left_expression)?;
                let right = self.scalar(right)?;
                let operand_type = self.result_type(left_expression)?;
                self.strict(
                    *operation,
                    operand_type,
                    left,
                    right,
                    expression.result,
                    source,
                )
            }
            ExpressionKind::ShortCircuit {
                conjunction,
                left,
                right,
            } => self.short_circuit(*conjunction, left, right),
            ExpressionKind::Call { callee, arguments } => {
                self.call(*callee, arguments, expression.result, source)
            }
        }
    }

    fn strict(
        &mut self,
        strict: Strict,
        operand_type: ScalarType,
        left: ValueId,
        right: ValueId,
        result: Option<ScalarType>,
        source: SourceOrigin,
    ) -> Checked<Value> {
        let operation = self.operation_identity(source);
        let kind = match strict {
            Strict::Add => OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            Strict::Subtract => OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            Strict::Multiply => OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            Strict::Divide => OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            Strict::Remainder => OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            Strict::ShiftLeft => OperationKind::ExactIntegerShiftLeft {
                value: left,
                count: right,
                obligation: self.safety(operation, source)?,
            },
            Strict::ShiftRight => OperationKind::ExactIntegerShiftRight {
                value: left,
                count: right,
                obligation: self.safety(operation, source)?,
            },
            Strict::BitwiseAnd => OperationKind::IntegerBitwiseAnd { left, right },
            Strict::BitwiseOr => OperationKind::IntegerBitwiseOr { left, right },
            Strict::BitwiseXor => OperationKind::IntegerBitwiseXor { left, right },
            Strict::Equal | Strict::NotEqual => match operand_type {
                ScalarType::Boolean => OperationKind::BooleanEqual { left, right },
                ScalarType::Integer(_) => OperationKind::IntegerEqual { left, right },
                ScalarType::IeeeFloat(_) => {
                    return Err(self.internal("a float equality reached lowering"));
                }
            },
            Strict::Less => OperationKind::IntegerLessThan { left, right },
            Strict::LessOrEqual => OperationKind::IntegerLessOrEqual { left, right },
            // `a > b` is `b < a`: both operands are already evaluated in
            // authored order, so swapping the operand identities changes no
            // evaluation.
            Strict::Greater => OperationKind::IntegerLessThan {
                left: right,
                right: left,
            },
            Strict::GreaterOrEqual => OperationKind::IntegerLessOrEqual {
                left: right,
                right: left,
            },
        };
        if strict != Strict::NotEqual {
            return self.emit(operation, result, kind);
        }
        // `!=` is the negation of the equality: Terminal Psi has no
        // inequality operation.
        let Value::Scalar(equal) = self.emit(operation, Some(ScalarType::Boolean), kind)? else {
            return Err(self.internal("an equality produced no value"));
        };
        let negation = self.operation_identity(source);
        self.emit(
            negation,
            result,
            OperationKind::BooleanNot { operand: equal },
        )
    }

    /// `left && right` / `left || right`: the left value decides on one edge
    /// and is itself the result there; the other edge evaluates the right
    /// operand in its own block. Both meet at a join block whose one Boolean
    /// parameter is the result.
    fn short_circuit(
        &mut self,
        conjunction: bool,
        left: &Expression,
        right: &Expression,
    ) -> Checked<Value> {
        let decided = self.scalar(left)?;
        let evaluate_right =
            self.block(SourceOrigin::Unpositioned(Unpositioned::ShortCircuitRight));
        let join = self.block(SourceOrigin::Unpositioned(Unpositioned::ShortCircuitJoin));
        let result = self.value(
            ScalarType::Boolean,
            ValueOrigin::BlockParameter {
                block: join,
                position: 0,
            },
        );
        let id = self.id;
        self.blocks
            .get_mut(join)
            .ok_or(Refusal {
                reason: RefusalReason::ForeignIdentity,
                machine: id,
                concerns: Construct::Block(join),
                detail: String::from("join block is past the end of this machine"),
            })?
            .parameters
            .push(declaration(result, ScalarType::Boolean));
        let (when_true, when_false) = if conjunction {
            ((evaluate_right, Vec::new()), (join, vec![decided]))
        } else {
            ((join, vec![decided]), (evaluate_right, Vec::new()))
        };
        self.branch(decided, when_true, when_false, left.source)?;
        self.enter(evaluate_right);
        let right_value = self.scalar(right)?;
        self.jump(
            join,
            vec![right_value],
            SourceOrigin::Unpositioned(Unpositioned::ShortCircuitJoin),
        )?;
        self.enter(join);
        Ok(Value::Scalar(result))
    }

    /// The invocation row of one call and its `CallRequirement` obligations,
    /// one per callee `requires` clause, created at the point before it.
    fn invocation(
        &mut self,
        site: InvocationSite,
        callee: MachineId,
        actuals: &[ValueId],
        before: ProgramPoint,
        source: SourceOrigin,
    ) -> Checked<Vec<ObligationId>> {
        let clauses =
            terminal_check_tables::row(self.signatures, callee, self.id, Construct::Machine)?
                .requires
                .len();
        let invocation = self.tables.invocations.push(InvocationRow {
            site,
            callee: Callee::Machine(callee),
            actuals: actuals.iter().copied().map(Actual::Value).collect(),
            requirements: Vec::new(),
        });
        let mut requirements = Vec::with_capacity(clauses);
        for clause in 0..clauses {
            let clause = self.position(clause)?;
            requirements.push(self.obligation(
                ObligationKind::CallRequirement { invocation, clause },
                before,
                source,
            ));
        }
        let id = self.id;
        self.tables
            .invocations
            .get_mut(invocation)
            .ok_or(Refusal {
                reason: RefusalReason::ForeignIdentity,
                machine: id,
                concerns: Construct::Invocation(invocation),
                detail: String::from("invocation is past the end of this machine"),
            })?
            .requirements
            .clone_from(&requirements);
        Ok(requirements)
    }

    fn call(
        &mut self,
        callee: MachineId,
        arguments: &[Expression],
        result: Option<ScalarType>,
        source: SourceOrigin,
    ) -> Checked<Value> {
        // A call is an operation only when its callee may return. One with
        // no normal completion is a terminator (`invoke_no_normal`), reached
        // only in terminal position.
        let outcome = self.outcome(callee)?;
        if !outcome.completes_normally {
            return Err(refuse(
                self.id,
                self.current.map_or(Construct::Machine, Construct::Block),
                Unserved::Call("no-normal call inside an expression"),
            ));
        }
        let mut actuals = Vec::with_capacity(arguments.len());
        for argument in arguments {
            actuals.push(self.scalar(argument)?);
        }
        // A callee that may return and also publishes crash routes makes
        // this operation a crash site: its routes, with each formal replaced
        // by the actual at its position, are the call's crash continuations,
        // and the site owes one `CrashCoverage` per surviving cause, as an
        // `InvokeNoNormal` terminator does at its block.
        let crash_continuations =
            canonical::substituted_routes(&self.outcome(callee)?.crash_routes, &actuals, self.id)?;
        let before = self.here()?;
        let operation = self.operation_identity(source);
        let requirements = self.invocation(
            InvocationSite::Operation(operation),
            callee,
            &actuals,
            before,
            source,
        )?;
        for bucket in &crash_continuations {
            self.obligation(
                ObligationKind::CrashCoverage {
                    site: CrashSite::Operation(operation),
                    cause: bucket.cause,
                },
                before,
                source,
            );
        }
        let kind = match result {
            Some(_) => OperationKind::Call {
                callee,
                arguments: actuals,
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                requirement_obligations: requirements,
                crash_continuations,
            },
            None => OperationKind::CallUnit {
                callee,
                arguments: actuals,
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: requirements,
                crash_continuations,
            },
        };
        self.emit(operation, result, kind)
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
        let ensures = match (&self.guarantees, self.signature.ensures.is_empty()) {
            (_, true) => Vec::new(),
            (Some(guarantees), false) => self
                .signature
                .ensures
                .iter()
                .zip(guarantees)
                .map(|(proposition, obligation)| ContractClause {
                    obligation: *obligation,
                    proposition: proposition.clone(),
                })
                .collect(),
            (None, false) => {
                return Err(self.internal("ensures on a machine with no normal exit"));
            }
        };
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
