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
use super::{Draft, Evaluation, Lowering, Reached, Value, declaration, successor};
use crate::body::{Expression, ExpressionKind, Selector, Statement, Strict, Target, ValueType};
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
            ExpressionKind::Call {
                callee: semantic_vocabulary::TargetMachineCallable::Machine(callee),
                arguments,
                ..
            } if self.heads.iter().any(|head| head.member == *callee) => Some((*callee, arguments)),
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

    pub(super) fn statement(&mut self, statement: &Statement) -> Evaluation<()> {
        if !self.open() {
            return Err(
                self.internal("a statement follows an unconditional arm or a no-successor exit")
            );
        }
        match statement {
            Statement::Bind { slot, value } => {
                let bound = match value.result {
                    Some(ValueType::Record(_, _, _)) => {
                        Bound::Record(continuing!(self.bind_record(value)))
                    }
                    Some(ValueType::Scalar(..) | ValueType::Rigid(_)) | None => {
                        Bound::Scalar(continuing!(self.scalar(value)))
                    }
                };
                *self.slot_mut(*slot)? = Some(bound);
                Ok(Reached::Value(()))
            }
            Statement::CaptureReference { slot, original } => {
                let bound = *self.slot_mut(*original)?;
                if !matches!(bound, Some(Bound::Record(_) | Bound::Home(_))) {
                    return Err(self.internal("a reference capture has no storage"));
                }
                *self.slot_mut(*slot)? = bound;
                Ok(Reached::Value(()))
            }
            Statement::Witness { slot, term } => {
                let witness = self.witness(term)?;
                *self.slot_mut(*slot)? = Some(Bound::Erased(witness));
                Ok(Reached::Value(()))
            }
            Statement::Declare { slot, home, source } => self
                .declare_local(*slot, home.as_deref(), *source)
                .map(Reached::Value),
            Statement::Store {
                destination,
                value,
                source,
            } => self.store(destination, value, *source),
            Statement::Dispatch {
                subject,
                arms,
                default,
                source,
            } => {
                let cases: Vec<usize> = arms.iter().map(|(case, _, _)| *case).collect();
                let (blocks, rest) =
                    continuing!(self.select(subject, &cases, default.is_some(), *source, None));
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
                Ok(Reached::Value(()))
            }
            Statement::Evaluate(expression) => {
                self.expression(expression).map(|reached| reached.map(drop))
            }
            Statement::Arm {
                guard: Some(guard),
                target,
                source,
            } => {
                let condition = continuing!(self.scalar(guard));
                let decided_in = self.current()?;
                let arm = self.block(*source);
                let rest = self.block(SourceOrigin::Unpositioned(
                    Unpositioned::DispatchContinuation,
                ));
                // Each arm only some arrivals take is a row the checker asks
                // whether any arrival can take (`MachineSources::branches`):
                // this arm, and the arm after it when that one takes
                // whatever this guard turned away.
                self.sources.branches.push(crate::Branch {
                    block: arm,
                    decided_in,
                    arm: *source,
                    condition: guard.source,
                });
                self.fallthrough = Some(crate::Branch {
                    block: rest,
                    decided_in,
                    arm: *source,
                    condition: guard.source,
                });
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
                Ok(Reached::Value(()))
            }
            Statement::Arm {
                guard: None,
                target,
                source,
            } => {
                if let Some(fallthrough) = self.fallthrough.take()
                    && self.point == Some(ProgramPoint::BlockEntry(fallthrough.block))
                {
                    self.sources.branches.push(crate::Branch {
                        arm: *source,
                        ..fallthrough
                    });
                }
                self.complete(target, *source).map(Reached::Value)
            }
        }
    }

    fn complete(&mut self, target: &Target, source: SourceOrigin) -> Checked<()> {
        match target {
            Target::Unit => self.return_unit(source),
            Target::Crash(cause) => self.crash(*cause, source),
            // A member's arguments are scalars, evaluated left to right like
            // any call's, and the edge carries them.
            Target::Value(expression) if let Some((member, arguments)) = self.loops(expression) => {
                let mut values = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    match self.scalar(argument)? {
                        Reached::Value(value) => values.push(value),
                        Reached::Ended(_) => return Ok(()),
                    }
                }
                self.arrive(member, values, expression.source)
            }
            // An arm whose every path ends at a call that never returns is
            // complete there.
            Target::Value(expression) => {
                let value = self.expression(expression);
                match value? {
                    Reached::Ended(_) => Ok(()),
                    Reached::Value(Value::Scalar(value)) => {
                        self.return_value(value, expression.source)
                    }
                    Reached::Value(Value::Record(root)) => {
                        self.return_record(root, expression.source)
                    }
                    Reached::Value(Value::Unit) => self.return_unit(expression.source),
                }
            }
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

    /// The edge of a terminator with no normal successor, with the fate of
    /// each conserved obligation live on it. A crash abandons them
    /// (effects.md: "Crashes have explicit no-successor abandonment, not a
    /// missing cleanup list"), process exit abandons them
    /// (`Fate::Abandoned`), and a no-normal invocation consumes the loans
    /// it carries (`dispositions`, from `invoke_no_normal`).
    ///
    /// A `Loan` is the one conserved kind the subset mints. Every other
    /// loan was formed for one call that returned and ended when it did, so
    /// none but the ones in `dispositions` is live here. This lowering does
    /// not track which obligations of any other conserved kind are live, so
    /// one existing in the machine is refused rather than given a guessed
    /// fate.
    pub(super) fn no_successor_edge(
        &mut self,
        dispositions: Vec<Disposition>,
        source: SourceOrigin,
    ) -> Checked<EdgeId> {
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
        let edge = self.no_successor_edge(Vec::new(), source)?;
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
        arms: &[(Selector, Expression)],
        default: Option<&Expression>,
        scalar_type: ScalarType,
        source: SourceOrigin,
        capture: Option<arena::Handle<typed_trees::expression::CompletedValueCapture>>,
    ) -> Evaluation<Value> {
        if let Some(ValueType::Scalar(subject_type, _)) = subject.result {
            return self.scalar_match(subject, subject_type, arms, default, scalar_type);
        }
        let cases = arms
            .iter()
            .map(|(selector, _)| {
                selector
                    .case()
                    .ok_or_else(|| self.internal("a value pattern on a sum subject"))
            })
            .collect::<Checked<Vec<usize>>>()?;
        let (blocks, rest) =
            continuing!(self.select(subject, &cases, default.is_some(), source, capture));
        let bodies = arms
            .iter()
            .map(|(_, value)| value)
            .zip(blocks)
            .chain(default.zip(rest));
        let mut arrivals = Arrivals::default();
        for (value, (block, chosen)) in bodies {
            self.enter(block);
            self.selected.push(chosen);
            let reached = self.arm(value, &mut arrivals);
            self.selected.pop();
            reached?;
        }
        self.join(arrivals, scalar_type)
    }

    /// Value-position `match` over a scalar: the subject's one value, then
    /// for each value arm in authored order its equality test and a branch
    /// to the arm's block or on to the next test; after the last test, the
    /// default. Every arm block jumps to a join whose one parameter is the
    /// result, as over a sum. patterns.md: a dispatch "evaluates its subject
    /// once, tests alternatives in authored order, and executes only the
    /// selected arm". Each branch edge carries its decision, so an arm is
    /// checked knowing its test held and every earlier one failed.
    fn scalar_match(
        &mut self,
        subject: &Expression,
        subject_type: ScalarType,
        arms: &[(Selector, Expression)],
        default: Option<&Expression>,
        scalar_type: ScalarType,
    ) -> Evaluation<Value> {
        let Some(default) = default else {
            return Err(self.internal("a scalar match without a default arm"));
        };
        let tested = continuing!(self.scalar(subject));
        let mut arrivals = Arrivals::default();
        for (selector, value) in arms {
            let Selector::Value(pattern) = selector else {
                return Err(self.internal("a case pattern on a scalar subject"));
            };
            let expected = match self.scalar(pattern)? {
                Reached::Value(value) => value,
                Reached::Ended(call) => {
                    // Earlier arms may already arrive. Only the remaining tests
                    // end here, so retain those arrivals rather than ending the match.
                    arrivals.ended = Some(call);
                    return self.join(arrivals, scalar_type);
                }
            };
            let Value::Scalar(condition) = self.strict(
                Strict::Equal,
                subject_type,
                tested,
                expected,
                Some(ScalarType::Boolean),
                pattern.source,
            )?
            else {
                return Err(self.internal("an equality test without a value"));
            };
            let arm = self.block(value.source);
            let rest = self.block(SourceOrigin::Unpositioned(
                Unpositioned::DispatchContinuation,
            ));
            self.branch(
                condition,
                (arm, Vec::new()),
                (rest, Vec::new()),
                pattern.source,
            )?;
            self.enter(arm);
            self.arm(value, &mut arrivals)?;
            self.enter(rest);
        }
        self.arm(default, &mut arrivals)?;
        self.join(arrivals, scalar_type)
    }

    /// One arm of a value-position `match`, lowered in the open block: its
    /// value and where it arrives from, or the call at which it ended.
    fn arm(&mut self, value: &Expression, arrivals: &mut Arrivals) -> Checked<()> {
        self.joining += 1;
        let value = self.scalar(value);
        self.joining -= 1;
        match value? {
            Reached::Value(value) => {
                arrivals.values.push((self.current()?, self.here()?, value));
            }
            Reached::Ended(call) => arrivals.ended = Some(call),
        }
        Ok(())
    }

    /// The join of a value-position `match`: one block whose one parameter
    /// is the result, and a jump to it from each arm that arrives. An arm
    /// whose every path ends at a call that never returns ends there and does
    /// not arrive. The join is made once the arms are lowered, so a `match`
    /// none of whose arms arrives has no join: its every path ended, at the
    /// last arm's call.
    fn join(&mut self, arrivals: Arrivals, scalar_type: ScalarType) -> Evaluation<Value> {
        if arrivals.values.is_empty() {
            let call = arrivals
                .ended
                .ok_or_else(|| self.internal("a match with no arm"))?;
            return Ok(Reached::Ended(call));
        }
        let join = self.block(SourceOrigin::Unpositioned(Unpositioned::MatchJoin));
        let result = self.value(
            scalar_type,
            ValueOrigin::BlockParameter {
                block: join,
                position: 0,
            },
        );
        self.parameter(join, result, scalar_type)?;
        for (block, point, value) in arrivals.values {
            self.current = Some(block);
            self.point = Some(point);
            self.jump(
                join,
                vec![value],
                SourceOrigin::Unpositioned(Unpositioned::MatchJoin),
            )?;
        }
        self.enter(join);
        Ok(Reached::Value(Value::Scalar(result)))
    }

    /// `value as +D` for a domain with predicates: a jump, at the cast, to
    /// a block whose one parameter is the cast's result and whose invariant
    /// is the domain's predicates of that parameter. control_flow.md:
    /// "Every arrival proves the predicate under simultaneous
    /// destination-parameter to actual-argument substitution, before
    /// destination hypotheses become available." The edge is therefore
    /// where the cast's obligation is owed (domains.md: "Has predicates |
    /// Prove them for this value."), and the block is where the result is
    /// known to satisfy them. Nothing is computed: the parameter is the
    /// operand's value.
    pub(super) fn prove_membership(
        &mut self,
        operand: ValueId,
        scalar_type: ScalarType,
        clauses: &[Proposition],
        stated: &str,
        source: SourceOrigin,
    ) -> Checked<ValueId> {
        let id = self.id;
        let established = self.block(source);
        self.sources
            .memberships
            .push((established, stated.to_owned()));
        let result = self.value(
            scalar_type,
            ValueOrigin::BlockParameter {
                block: established,
                position: 0,
            },
        );
        self.parameter(established, result, scalar_type)?;
        let mut predicate = Vec::with_capacity(clauses.len());
        for clause in clauses {
            predicate.push(canonical::substitute(clause, &[result], id)?);
        }
        self.invariants.push(terminal_psi::ScalarBlockInvariant {
            machine: id,
            header: established,
            predicate: canonical::conjunction(predicate, id)?,
            arrivals: Vec::new(),
        });
        self.jump(established, vec![operand], source)?;
        self.enter(established);
        Ok(result)
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
    ) -> Evaluation<Value> {
        let decided = continuing!(self.scalar(left));
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
        // A right operand whose every path ends at a call that never returns
        // ends that branch only: the join has the left operand's arrival.
        if let Reached::Value(right_value) = right_value? {
            self.jump(
                join,
                vec![right_value],
                SourceOrigin::Unpositioned(Unpositioned::ShortCircuitJoin),
            )?;
        }
        self.enter(join);
        Ok(Reached::Value(Value::Scalar(result)))
    }
}

/// The arms of a value-position `match` lowered so far: for each that
/// arrives, its open block, program point and value; and the last call at
/// which an arm ended instead.
#[derive(Default)]
struct Arrivals {
    values: Vec<(BlockId, ProgramPoint, ValueId)>,
    ended: Option<SourceOrigin>,
}
