/*
The control family of lowering: blocks and the open block, edges and
terminators, transition arms, short-circuit joins, crashes and returns.
A record value returns through `ReturnStructural`, naming the root that
holds it.
*/

use semantic_vocabulary::{
    BlockId, CrashCause, EdgeId, MachineId, ObligationId, PlaceId, Proposition, ScalarTerm,
    ScalarType, ValueId,
};
use terminal_check_tables::edges::{Destination, Disposition, EdgeRow};
use terminal_check_tables::obligations::{CrashSite, ObligationKind};
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{Checked, Construct, ProgramPoint, Refusal, RefusalReason};
use terminal_psi::{CrashPredicateTerm, Terminator};

use super::places::Bound;
use super::{Draft, Lowering, Value, declaration, successor};
use crate::body::{Expression, ExpressionKind, Statement, Target, ValueType};
use crate::canonical;
use crate::{SourceOrigin, Unpositioned, Unserved, refuse};

impl Lowering<'_> {
    pub(super) fn block(&mut self, source: SourceOrigin) -> BlockId {
        let block = self.blocks.push(Draft::default());
        self.sources.blocks.push(source);
        block
    }

    pub(super) fn enter(&mut self, block: BlockId) {
        self.current = Some(block);
        self.point = Some(ProgramPoint::BlockEntry(block));
    }

    pub(super) fn open(&self) -> bool {
        self.current.is_some()
    }

    pub(super) fn current(&self) -> Checked<BlockId> {
        self.current
            .ok_or_else(|| self.internal("no open block to emit into"))
    }

    pub(super) fn here(&self) -> Checked<ProgramPoint> {
        self.point
            .ok_or_else(|| self.internal("no program point in a terminated block"))
    }

    pub(super) fn draft(&mut self) -> Checked<&mut Draft> {
        let block = self.current()?;
        let id = self.id;
        self.blocks.get_mut(block).ok_or(Refusal {
            reason: RefusalReason::ForeignIdentity,
            machine: id,
            concerns: Construct::Block(block),
            detail: String::from("block is past the end of this machine"),
        })
    }

    pub(super) fn edge(
        &mut self,
        destination: Destination,
        source: SourceOrigin,
    ) -> Checked<EdgeId> {
        let edge = self.tables.edges.push(EdgeRow {
            source: self.current()?,
            destination,
            dispositions: Vec::new(),
        });
        self.sources.edges.push(source);
        Ok(edge)
    }

    pub(super) fn terminate(&mut self, terminator: Terminator) -> Checked<()> {
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

    /// An arrival at a member's loop head: the jump with the member's scalar
    /// formals as its arguments, and, when the head has an invariant, the
    /// one `ArrivalInvariant` obligation this edge owes it.
    /// control_flow.md: "Every arrival proves the predicate under
    /// simultaneous destination-parameter to actual-argument substitution,
    /// before destination hypotheses become available."
    pub(super) fn arrive(
        &mut self,
        member: MachineId,
        arguments: Vec<ValueId>,
        source: SourceOrigin,
    ) -> Checked<()> {
        let Some(head) = self.heads.iter().find(|head| head.member == member) else {
            return Err(self.internal("an arrival at a machine outside the call component"));
        };
        let target = head.block;
        if head.parameters.len() != arguments.len() {
            return Err(self.internal("an arrival does not match the head's parameters"));
        }
        let edge = self.edge(Destination::Block(target), source)?;
        self.terminate(Terminator::Jump {
            edge,
            target,
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            cleanup_actions: Vec::new(),
        })?;
        // The edge's `ArrivalInvariant` is minted with every other arrival's
        // (`Lowering::arrivals`), which also lists it on the head's
        // invariant row.
        Ok(())
    }

    /// The member and arguments of a call in terminal position whose callee
    /// lies in this machine's call component: such a call is a jump back.
    fn loops<'e>(&self, expression: &'e Expression) -> Option<(MachineId, &'e [Expression])> {
        match &expression.kind {
            ExpressionKind::Call { callee, arguments }
                if self.heads.iter().any(|head| head.member == *callee) =>
            {
                Some((*callee, arguments))
            }
            _ => None,
        }
    }

    /// One `NormalGuarantee` per `ensures` clause on this normal exit.
    fn guarantee(&mut self, exit: EdgeId, source: SourceOrigin) -> Checked<()> {
        for (position, published) in self.published.clone().into_iter().enumerate() {
            let clause = self.position(position)?;
            self.obligation(
                ObligationKind::NormalGuarantee {
                    exit,
                    clause,
                    published,
                },
                ProgramPoint::Edge(exit),
                source,
            );
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

    /// A record result returns the root that holds it.
    /// `wiki/spec/terminal-psi/calls_and_outcomes.md`, "Normal results":
    /// a structural result is one place, apart from the scalar lane. The
    /// plain record holds no claim, so none is returned.
    fn return_record(&mut self, source_root: PlaceId, source: SourceOrigin) -> Checked<()> {
        let edge = self.edge(Destination::Return, source)?;
        self.terminate(Terminator::ReturnStructural {
            edge,
            source: source_root,
            returned_claims: Vec::new(),
            cleanup_actions: Vec::new(),
        })?;
        self.guarantee(edge, source)
    }

    pub(super) fn return_unit(&mut self, source: SourceOrigin) -> Checked<()> {
        let edge = self.edge(Destination::Return, source)?;
        self.terminate(Terminator::ReturnUnit {
            edge,
            cleanup_actions: Vec::new(),
        })?;
        self.guarantee(edge, source)
    }

    pub(super) fn statement(&mut self, statement: &Statement) -> Checked<()> {
        if !self.open() {
            return Err(
                self.internal("a statement follows an unconditional arm or a no-successor exit")
            );
        }
        match statement {
            Statement::Bind { slot, value } => {
                let bound = match value.result {
                    Some(ValueType::Record(_)) => Bound::Record(self.bind_record(value)?),
                    Some(ValueType::Scalar(_) | ValueType::Qualified(..) | ValueType::Rigid(_)) | None => {
                        Bound::Scalar(self.scalar(value)?)
                    }
                };
                *self.slot_mut(*slot)? = Some(bound);
                Ok(())
            }
            Statement::Witness { slot, term } => {
                let witness = self.witness(term)?;
                *self.slot_mut(*slot)? = Some(Bound::Erased(witness));
                Ok(())
            }
            Statement::Declare { slot, source, .. } => self.declare_local(*slot, *source),
            Statement::Store {
                slot,
                path,
                value,
                source,
            } => self.store(*slot, path, value, *source),
            Statement::Dispatch {
                subject,
                arms,
                default,
                source,
            } => {
                let cases: Vec<usize> = arms.iter().map(|(case, _, _)| *case).collect();
                let (blocks, rest) = self.select(subject, &cases, default.is_some(), *source)?;
                // Each arm completes the machine, so what one binds (the home
                // a loan gives a local) is no other arm's.
                let bound = self.slots.clone();
                for ((_, target, source), (block, chosen)) in arms.iter().zip(blocks) {
                    self.enter(block);
                    self.selected.push(chosen);
                    self.complete(target, *source)?;
                    self.selected.pop();
                    self.slots.clone_from(&bound);
                }
                if let (Some((target, source)), Some((block, chosen))) = (default, rest) {
                    self.enter(block);
                    self.selected.push(chosen);
                    self.complete(target, *source)?;
                    self.selected.pop();
                    self.slots.clone_from(&bound);
                }
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
                // The arm completes the machine, so what it binds (the home a
                // loan gives a local) is not the continuation's.
                let bound = self.slots.clone();
                self.enter(arm);
                self.path.push(taken);
                self.complete(target, *source)?;
                self.path.pop();
                self.slots = bound;
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
                // A member's arguments are scalars, evaluated left to right
                // like any call's, and the edge carries them.
                _ if let Some((member, arguments)) = self.loops(expression) => {
                    let mut values = Vec::with_capacity(arguments.len());
                    for argument in arguments {
                        values.push(self.scalar(argument)?);
                    }
                    self.arrive(member, values, expression.source)
                }
                Some((callee, arguments)) => {
                    self.invoke_no_normal(callee, arguments, expression.source)
                }
                None => match self.expression(expression)? {
                    Value::Scalar(value) => self.return_value(value, expression.source),
                    Value::Record(root) => self.return_record(root, expression.source),
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

    /// The fate of each conserved obligation on an edge with no normal
    /// successor. A crash abandons them (effects.md: "Crashes have explicit
    /// no-successor abandonment, not a missing cleanup list"; `Fate::Abandoned`),
    /// and a no-normal invocation consumes or transfers them. The subset
    /// mints no conserved obligation live across an edge, and this lowering
    /// does not track which are live, so one existing here is refused rather
    /// than given a guessed fate; with none, the edge has no dispositions.
    ///
    /// A `Loan` is the one conserved kind the subset mints, and none is live
    /// here: each is formed for one call that returns, and ends when it does
    /// (`calls`; a call that never returns may not lend).
    fn no_successor_dispositions(&self) -> Checked<Vec<Disposition>> {
        for raw in 1..=self.tables.obligations.len() as u64 {
            let obligation = ObligationId::new(raw).ok_or_else(|| self.internal("obligation"))?;
            let row = terminal_check_tables::row(
                &self.tables.obligations,
                obligation,
                self.id,
                Construct::Obligation(obligation),
            )?;
            if row.kind.is_conserved() && !matches!(row.kind, ObligationKind::Loan { .. }) {
                return Err(refuse(
                    self.id,
                    Construct::Obligation(obligation),
                    Unserved::Completion("conserved obligation at a no-successor edge"),
                ));
            }
        }
        Ok(Vec::new())
    }

    pub(super) fn no_successor_edge(&mut self, source: SourceOrigin) -> Checked<EdgeId> {
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
        // The site guard is the conjunction of the branch decisions before
        // the crash, each a value equal to a Boolean. Which case a dispatch
        // selected is not a value, so inside an arm the guard cannot be
        // exact and the crash is refused rather than given a wider guard.
        if !self.selected.is_empty() {
            return Err(refuse(
                self.id,
                self.current.map_or(Construct::Machine, Construct::Block),
                Unserved::Transition("crash in a case arm"),
            ));
        }
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

    /// Value-position `match` over a sum: the dispatch, each arm's value in
    /// its own block, and a join whose one parameter is the result.
    /// control_flow.md: "Scalar arguments match the complete target
    /// telescope; their values may differ across arrivals." An arm is an
    /// expression, so it rebinds no slot and stores nothing: the result is
    /// the only thing that differs across the arrivals.
    pub(super) fn match_value(
        &mut self,
        subject: &Expression,
        arms: &[(usize, Expression)],
        default: Option<&Expression>,
        scalar_type: ScalarType,
        source: SourceOrigin,
    ) -> Checked<Value> {
        let cases: Vec<usize> = arms.iter().map(|(case, _)| *case).collect();
        let (blocks, rest) = self.select(subject, &cases, default.is_some(), source)?;
        let join = self.block(SourceOrigin::Unpositioned(Unpositioned::MatchJoin));
        let result = self.value(
            scalar_type,
            ValueOrigin::BlockParameter {
                block: join,
                position: 0,
            },
        );
        self.parameter(join, result, scalar_type)?;
        let bodies = arms
            .iter()
            .map(|(_, value)| value)
            .zip(blocks)
            .chain(default.zip(rest));
        for (value, (block, chosen)) in bodies {
            self.enter(block);
            self.selected.push(chosen);
            self.joining += 1;
            let value = self.scalar(value);
            self.joining -= 1;
            let value = value?;
            self.selected.pop();
            self.jump(
                join,
                vec![value],
                SourceOrigin::Unpositioned(Unpositioned::MatchJoin),
            )?;
        }
        self.enter(join);
        Ok(Value::Scalar(result))
    }

    /// Declares `value` as the next scalar parameter of `block`.
    pub(super) fn parameter(
        &mut self,
        block: BlockId,
        value: ValueId,
        scalar_type: ScalarType,
    ) -> Checked<()> {
        let id = self.id;
        self.blocks
            .get_mut(block)
            .ok_or(Refusal {
                reason: RefusalReason::ForeignIdentity,
                machine: id,
                concerns: Construct::Block(block),
                detail: String::from("block is past the end of this machine"),
            })?
            .parameters
            .push(declaration(value, scalar_type));
        Ok(())
    }

    /// `left && right` / `left || right`: the left value decides on one edge
    /// and is itself the result there; the other edge evaluates the right
    /// operand in its own block. Both meet at a join block whose one Boolean
    /// parameter is the result.
    pub(super) fn short_circuit(
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
        self.parameter(join, result, ScalarType::Boolean)?;
        let (when_true, when_false) = if conjunction {
            ((evaluate_right, Vec::new()), (join, vec![decided]))
        } else {
            ((join, vec![decided]), (evaluate_right, Vec::new()))
        };
        self.branch(decided, when_true, when_false, left.source)?;
        self.enter(evaluate_right);
        self.joining += 1;
        let right_value = self.scalar(right);
        self.joining -= 1;
        let right_value = right_value?;
        self.jump(
            join,
            vec![right_value],
            SourceOrigin::Unpositioned(Unpositioned::ShortCircuitJoin),
        )?;
        self.enter(join);
        Ok(Value::Scalar(result))
    }
}
