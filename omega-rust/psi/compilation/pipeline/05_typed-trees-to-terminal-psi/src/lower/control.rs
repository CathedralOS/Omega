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
use terminal_psi::{
    CrashPredicateTerm, StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
    Terminator,
};

use super::places::Bound;
use super::{Draft, Evaluation, Lowering, Reached, Value, declaration, successor};
use crate::body::{Expression, ExpressionKind, Selector, Statement, Strict, Target, ValueType};
use crate::canonical;
use crate::{SourceOrigin, Unpositioned, Unserved, refuse};

impl Lowering<'_> {
    pub(super) fn block(&mut self, source: SourceOrigin) -> BlockId {
        let block = self.blocks.push(Draft::default());
        self.sources.blocks.push(source);
        self.inherit_ranking(block);
        block
    }

    pub(super) fn enter(&mut self, block: BlockId) {
        self.current = Some(block);
        self.point = Some(ProgramPoint::BlockEntry(block));
        self.enter_ranking(block);
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
            source_span: None,
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
        self.jump_with_structural(target, arguments, Vec::new(), source)
    }

    pub(super) fn jump_with_structural(
        &mut self,
        target: BlockId,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        source: SourceOrigin,
    ) -> Checked<()> {
        let edge = self.edge(Destination::Block(target), source)?;
        self.terminate(Terminator::Jump {
            edge,
            target,
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments,
            cleanup_actions: Vec::new(),
        })
    }

    /// Arrive at a loop head with the same scalar and storage arguments a
    /// call would pass. Staged values are captured before their old homes
    /// retire, and a header invariant is proved for every arrival.
    /// control_flow.md: "Every arrival proves the predicate under
    /// simultaneous destination-parameter to actual-argument substitution,
    /// before destination hypotheses become available."
    pub(super) fn arrive(
        &mut self,
        member: MachineId,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
        source: SourceOrigin,
        acknowledgement: Option<terminal_psi::CallAcknowledgement>,
    ) -> Checked<()> {
        let Some(head) = self.heads.iter().find(|head| head.member == member) else {
            return Err(self.internal("an arrival at a machine outside the call component"));
        };
        let target = head.block;
        if head.parameters.len() != arguments.len()
            || head.structural_parameters.len() != structural_arguments.len()
        {
            return Err(self.internal("an arrival does not match the head's parameters"));
        }
        self.successor_ranking(member, &arguments, &structural_arguments)?;
        let edge = self.edge(Destination::Block(target), source)?;
        if let Some(acknowledgement) = acknowledgement {
            self.call_formations.push(terminal_psi::CallFormation {
                site: terminal_psi::CallSite::Edge(edge),
                acknowledgement,
                tail_callee: Some(member),
            });
        }
        self.terminate(Terminator::Jump {
            edge,
            target,
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments,
            cleanup_actions,
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

    /// A return or tail arrival leaves every enclosing computed match.
    /// Retain their real owner roots until the result/arguments are captured;
    /// the ordinary affine partition disposes only the unmoved remainder.
    fn selected_temporary_cleanup(&self) -> Vec<terminal_psi::TerminalAffineCleanupAction> {
        let mut homes = Vec::new();
        for home in self
            .selected
            .iter()
            .rev()
            .filter_map(|chosen| chosen.temporary())
        {
            if !homes.contains(&home) {
                homes.push(home);
            }
        }
        homes
            .into_iter()
            .map(terminal_psi::TerminalAffineCleanupAction::RetireHome)
            .collect()
    }

    fn return_value(&mut self, value: ValueId, source: SourceOrigin) -> Checked<()> {
        let edge = self.edge(Destination::Return, source)?;
        self.terminate(Terminator::Return {
            edge,
            value,
            cleanup_actions: self.selected_temporary_cleanup(),
        })?;
        self.guarantee(edge, source)
    }

    /// A record result returns the root that holds it.
    /// `wiki/spec/terminal-psi/calls_and_outcomes.md`, "Normal results":
    /// a structural result is one place, apart from the scalar lane. The
    /// plain record holds no claim, so none is returned.
    pub(super) fn return_record(
        &mut self,
        source_root: PlaceId,
        source: SourceOrigin,
    ) -> Checked<()> {
        let no_normal_value = self.empty_owned_value(source_root);
        // Keep the actual structural expression. Its impossible owned value
        // supplies no normal result, whatever type the destination expects,
        // so this edge owes no normal-return guarantee or receiver value.
        // The checker reconstructs that classification from the real root.
        let edge = self.edge(
            if no_normal_value {
                Destination::NoNormal
            } else {
                Destination::Return
            },
            source,
        )?;
        self.terminate(Terminator::ReturnStructural {
            edge,
            source: source_root,
            returned_claims: Vec::new(),
            cleanup_actions: self.selected_temporary_cleanup(),
        })?;
        if no_normal_value {
            Ok(())
        } else {
            self.guarantee(edge, source)
        }
    }

    pub(super) fn return_unit(&mut self, source: SourceOrigin) -> Checked<()> {
        let edge = self.edge(Destination::Return, source)?;
        self.terminate(Terminator::ReturnUnit {
            edge,
            cleanup_actions: self.selected_temporary_cleanup(),
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
                // A loan may have given this scalar binding a storage home.
                // Reassignment must write that same home: replacing only its
                // SSA binding would hide the write from outstanding loans.
                if matches!(*self.slot_mut(*slot)?, Some(Bound::Home(_))) {
                    let destination = Expression {
                        call_context: None,
                        kind: ExpressionKind::Slot(*slot),
                        result: Some(self.slot_type(*slot)?),
                        source: value.source,
                    };
                    return self.store(&destination, value, value.source);
                }
                let bound = match value.result {
                    Some(ValueType::Record(_, _, _)) => {
                        Bound::Record(continuing!(self.bind_record(value)))
                    }
                    Some(ValueType::Scalar(..) | ValueType::Rigid(_)) | None => {
                        let scalar = continuing!(self.scalar(value));
                        match self.lender_homes.get(*slot).copied().flatten() {
                            Some(home) => {
                                Bound::Home(self.establish_home(scalar, home, value.source)?)
                            }
                            None => Bound::Scalar(scalar),
                        }
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
            Statement::Decompose { subject } => self.decompose(subject),
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
                let (blocks, rest) = continuing!(self.select(
                    subject,
                    &cases,
                    default.as_ref().map(|(_, arm)| *arm),
                    *source,
                    None
                ));
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
            Statement::Evaluate {
                expression,
                discards_result,
            } => {
                // effects.md: a proved non-returning call has no result to
                // discard. Use the actual once-only evaluation, including its
                // published contract, rather than predicting that from syntax
                // or treating an inhabited result as disposable.
                match self.expression(expression)? {
                    Reached::Ended(origin) => Ok(Reached::Ended(origin)),
                    Reached::Value(_) if expression.result.is_none() || *discards_result => {
                        Ok(Reached::Value(()))
                    }
                    Reached::Value(_) => Err(refuse(
                        self.id,
                        Construct::Machine,
                        Unserved::Statement("expression statement that discards its value"),
                    )),
                }
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
            Target::Value(value) => self.complete_value(value),
        }
    }

    fn complete_value(&mut self, expression: &Expression) -> Checked<()> {
        if let ExpressionKind::Match {
            capture,
            subject,
            arms,
            default,
        } = &expression.kind
        {
            // The selected arm completes this invocation. Reuse the ordinary
            // match dispatch, then transfer/clean up that arm directly rather
            // than create a continuation after its recursive call.
            let bound = self.slots.clone();
            self.dispatch_match(
                subject,
                arms,
                default.as_deref(),
                expression.source,
                *capture,
                |lowering, value, _temporary| {
                    lowering.complete_value(value)?;
                    lowering.slots.clone_from(&bound);
                    Ok(())
                },
            )?;
            return Ok(());
        }
        if let Some((member, arguments)) = self.loops(expression) {
            let (values, structural, mut cleanup) =
                match self.tail_arguments(member, arguments, expression.source)? {
                    Reached::Value(arguments) => arguments,
                    Reached::Ended(_) => return Ok(()),
                };
            for action in self.selected_temporary_cleanup() {
                if !cleanup.contains(&action) {
                    cleanup.push(action);
                }
            }
            return self.arrive(
                member,
                values,
                structural,
                cleanup,
                expression.source,
                Some(expression.call_context.unwrap_or_default()),
            );
        }
        // An arm whose every path ends at a call that never returns is
        // complete there. Other values use their ordinary return transfer.
        match self.expression(expression)? {
            Reached::Ended(_) => Ok(()),
            Reached::Value(Value::Scalar(value)) => self.return_value(value, expression.source),
            Reached::Value(Value::Record(root)) => self.return_record(root, expression.source),
            Reached::Value(Value::Unit) => self.return_unit(expression.source),
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

    /// Continuing match arms supply one value to the same block telescope.
    /// Structural values use its ordinary ownership transfer; scalar values
    /// use its scalar lane. An arm that ends at a nonreturning call supplies
    /// no arrival, so it creates neither a result nor a cleanup successor.
    pub(super) fn match_value(
        &mut self,
        subject: &Expression,
        arms: &[(Selector, Expression)],
        default: Option<&Expression>,
        result_type: ValueType,
        source: SourceOrigin,
        capture: Option<arena::Handle<typed_trees::expression::CompletedValueCapture>>,
    ) -> Evaluation<Value> {
        let mut arrivals = Arrivals::default();
        if let Reached::Ended(call) = self.dispatch_match(
            subject,
            arms,
            default,
            source,
            capture,
            |lowering, value, temporary| lowering.arm(value, &mut arrivals, temporary),
        )? {
            arrivals.ended = Some(call);
        }
        self.join(arrivals, result_type)
    }

    /// Evaluate the selector once and enter each authored arm with its exact
    /// case/decision facts. The caller owns whether that arm joins a value or
    /// completes the invocation; dispatch itself introduces no continuation.
    fn dispatch_match(
        &mut self,
        subject: &Expression,
        arms: &[(Selector, Expression)],
        default: Option<&Expression>,
        source: SourceOrigin,
        capture: Option<arena::Handle<typed_trees::expression::CompletedValueCapture>>,
        mut finish: impl FnMut(&mut Self, &Expression, Option<PlaceId>) -> Checked<()>,
    ) -> Evaluation<()> {
        if let Some(ValueType::Scalar(subject_type, _)) = subject.result {
            return self.scalar_match(subject, subject_type, arms, default, finish);
        }
        let cases = arms
            .iter()
            .map(|(selector, _)| {
                selector
                    .case()
                    .ok_or_else(|| self.internal("a value pattern on a sum subject"))
            })
            .collect::<Checked<Vec<usize>>>()?;
        let decided_in = self.current()?;
        let (blocks, rest) = continuing!(self.select(
            subject,
            &cases,
            default.map(|value| value.source),
            source,
            capture
        ));
        let bodies = arms
            .iter()
            .map(|(_, value)| value)
            .zip(blocks)
            .chain(default.zip(rest));
        for (value, (block, chosen)) in bodies {
            self.authored_arm(block, decided_in, value.source, value.source);
            self.enter(block);
            let temporary = chosen.temporary();
            self.selected.push(chosen);
            let reached = finish(self, value, temporary);
            self.selected.pop();
            reached?;
        }
        Ok(Reached::Value(()))
    }

    /// Scalar patterns retain their authored order and decisions. A pattern
    /// that ends control preserves any earlier completed arms through the
    /// caller's ordinary value-arrival or invocation-completion owner.
    fn scalar_match(
        &mut self,
        subject: &Expression,
        subject_type: ScalarType,
        arms: &[(Selector, Expression)],
        default: Option<&Expression>,
        mut finish: impl FnMut(&mut Self, &Expression, Option<PlaceId>) -> Checked<()>,
    ) -> Evaluation<()> {
        let Some(default) = default else {
            return Err(self.internal("a scalar match without a default arm"));
        };
        let tested = continuing!(self.scalar(subject));
        let mut decided_in = None;
        for (selector, value) in arms {
            let Selector::Value(pattern) = selector else {
                return Err(self.internal("a case pattern on a scalar subject"));
            };
            let expected = continuing!(self.scalar(pattern));
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
            let here = self.current()?;
            let arm = self.block(value.source);
            let rest = self.block(SourceOrigin::Unpositioned(
                Unpositioned::DispatchContinuation,
            ));
            self.authored_arm(arm, here, value.source, pattern.source);
            self.branch(
                condition,
                (arm, Vec::new()),
                (rest, Vec::new()),
                pattern.source,
            )?;
            self.enter(arm);
            finish(self, value, None)?;
            self.enter(rest);
            decided_in = Some(here);
        }
        if let Some(decided_in) = decided_in {
            let block = self.current()?;
            self.authored_arm(block, decided_in, default.source, default.source);
        }
        finish(self, default, None)?;
        Ok(Reached::Value(()))
    }

    /// Records the authored arm whose first block is `block`, entered only
    /// from the decision that ends `decided_in`.
    fn authored_arm(
        &mut self,
        block: BlockId,
        decided_in: BlockId,
        arm: SourceOrigin,
        condition: SourceOrigin,
    ) {
        self.sources.branches.push(crate::Branch {
            block,
            decided_in,
            arm,
            condition,
        });
    }

    /// One arm of a value-position `match`, lowered in the open block: its
    /// value and where it arrives from, or the call at which it ended.
    fn arm(
        &mut self,
        value: &Expression,
        arrivals: &mut Arrivals,
        temporary: Option<PlaceId>,
    ) -> Checked<()> {
        let source = value.source;
        match self.expression(value)? {
            Reached::Value(value) => {
                if let Some(home) = temporary {
                    self.retire_homes(&[home], source)?;
                }
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
    fn join(&mut self, arrivals: Arrivals, result_type: ValueType) -> Evaluation<Value> {
        if arrivals.values.is_empty() {
            let call = arrivals
                .ended
                .ok_or_else(|| self.internal("a match with no arm"))?;
            return Ok(Reached::Ended(call));
        }
        let join = self.block(SourceOrigin::Unpositioned(Unpositioned::MatchJoin));
        let result = match result_type {
            ValueType::Scalar(scalar_type, _) => {
                let result = self.value(
                    scalar_type,
                    ValueOrigin::BlockParameter {
                        block: join,
                        position: 0,
                    },
                );
                self.parameter(join, result, scalar_type)?;
                Value::Scalar(result)
            }
            ValueType::Record(record, qualification, _) => {
                let (place, _) = self.root(
                    semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                        block: join,
                        position: 0,
                    },
                    record,
                )?;
                // Every arm delivers the match's one type, domains included;
                // the checker holds each arrival to them.
                let (qualifications, projected_qualifications) =
                    self.memberships(record, qualification)?;
                let parameter = StructuralParameterDeclaration {
                    place,
                    position: 0,
                    structural_type: record,
                    multiplicity: self.records.multiplicity(record, self.id)?,
                    access: StructuralAccess::Owned,
                    qualifications,
                    projected_qualifications,
                    additional_input_qualifications: Vec::new(),
                };
                let id = self.id;
                self.blocks
                    .get_mut(join)
                    .ok_or(Refusal {
                        reason: RefusalReason::ForeignIdentity,
                        machine: id,
                        concerns: Construct::Block(join),
                        source_span: None,
                        detail: String::from("a match join is past its function"),
                    })?
                    .structural_parameters
                    .push(parameter);
                Value::Record(place)
            }
            ValueType::Rigid(_) => {
                return Err(self.internal("a rigid match result has no established join"));
            }
        };
        for (block, point, value) in arrivals.values {
            self.current = Some(block);
            self.point = Some(point);
            let (scalars, structures) = match (&result, value) {
                (Value::Scalar(_), Value::Scalar(value)) => (vec![value], Vec::new()),
                (Value::Record(_), Value::Record(place)) => {
                    let ValueType::Record(record, qualification, _) = result_type else {
                        return Err(self.internal("a record join without a record type"));
                    };
                    let multiplicity = self.records.multiplicity(record, self.id)?;
                    let place = self.join_arrival(
                        place,
                        record,
                        qualification,
                        multiplicity,
                        SourceOrigin::Unpositioned(Unpositioned::MatchJoin),
                    )?;
                    (
                        Vec::new(),
                        vec![StructuralArgument::whole(place, StructuralAccess::Owned)],
                    )
                }
                _ => return Err(self.internal("a match arm does not supply its join's value lane")),
            };
            self.jump_with_structural(
                join,
                scalars,
                structures,
                SourceOrigin::Unpositioned(Unpositioned::MatchJoin),
            )?;
        }
        self.enter(join);
        Ok(Reached::Value(result))
    }

    /// Prove a cast's conditions before continuing in a block that records
    /// them. Domain addition and exact carrier conversion share this step;
    /// the block parameter keeps the operand's existing value. control_flow.md:
    /// "Every arrival proves the predicate under simultaneous
    /// destination-parameter to actual-argument substitution, before
    /// destination hypotheses become available." The edge is therefore
    /// where the cast's obligation is owed (domains.md: "Has predicates |
    /// Prove them for this value."), and the block is where the result is
    /// known to satisfy them. Nothing is computed: the parameter is the
    /// operand's value.
    pub(super) fn establish_predicates(
        &mut self,
        operand: ValueId,
        scalar_type: ScalarType,
        qualification: crate::body::Qualification,
        clauses: &[Proposition],
        stated: Option<&terminal_check_tables::program::CastCondition>,
        source: SourceOrigin,
    ) -> Checked<ValueId> {
        let id = self.id;
        let established = self.block(source);
        if let Some(stated) = stated {
            self.sources
                .predicate_establishments
                .push((established, stated.clone()));
        }
        let result = self.value(
            scalar_type,
            ValueOrigin::BlockParameter {
                block: established,
                position: 0,
            },
        );
        self.parameter(established, result, scalar_type)?;
        let original = self
            .value_qualifications
            .get((operand.get() - 1) as usize)
            .copied()
            .flatten()
            .ok_or_else(|| self.internal("cast operand has no declared qualification"))?;
        let only_removes_qualifications = original != qualification
            && qualification.domains.is_subset_of(&original.domains)
            && (qualification.policy.is_none() || qualification.policy == original.policy);
        let mut predicate = Vec::with_capacity(clauses.len());
        for clause in clauses {
            predicate.push(canonical::substitute(
                clause,
                &[result],
                &self.tables.values,
                id,
            )?);
        }
        // A predicate-free removal passes the same value. Keep it in ordinary
        // join scope so the dominating operand remains available at the result.
        if !predicate.is_empty() || !only_removes_qualifications {
            self.invariants.push(terminal_psi::ScalarBlockInvariant {
                machine: id,
                header: established,
                predicate: canonical::conjunction(predicate, id)?,
                arrivals: Vec::new(),
            });
        }
        let edge = semantic_vocabulary::EdgeId::new(self.tables.edges.len() as u64 + 1)
            .ok_or_else(|| self.internal("qualification edge identity overflow"))?;
        self.jump(established, vec![operand], source)?;
        let before = self.qualifications.scalar(id, scalar_type, original)?;
        let after = self.qualifications.scalar(id, scalar_type, qualification)?;
        // A logical predicate needs an ordinary same-value edge. Publish a
        // membership coercion only when the declared membership set changes.
        if before != after {
            self.qualifications
                .catalog
                .coercions
                .push(terminal_psi::ScalarQualificationCoercion {
                    machine: id,
                    edge,
                    argument_ordinal: 0,
                    source: operand,
                    destination: result,
                });
        }
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
                source_span: None,
                detail: String::from("block is past the end of this machine"),
            })?
            .parameters
            .push(declaration(value, scalar_type));
        Ok(())
    }

    /// A chain of one connective shares its result join. Each short-circuit
    /// edge passes the deciding operand, so the continuing edge retains its
    /// guard facts until the next operand is evaluated.
    pub(super) fn short_circuit(
        &mut self,
        conjunction: bool,
        left: &Expression,
        right: &Expression,
    ) -> Evaluation<Value> {
        let mut pending = vec![right, left];
        let mut operands = std::iter::from_fn(|| {
            while let Some(operand) = pending.pop() {
                if let ExpressionKind::ShortCircuit {
                    conjunction: nested,
                    left,
                    right,
                } = &operand.kind
                    && *nested == conjunction
                {
                    pending.extend([right.as_ref(), left.as_ref()]);
                } else {
                    return Some(operand);
                }
            }
            None
        });
        let first = operands
            .next()
            .expect("a binary connective has a left operand");
        let mut decided = continuing!(self.scalar(first));
        let mut decision_source = first.source;
        let mut operand = operands
            .next()
            .expect("a binary connective has a right operand");
        let mut evaluate_right =
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
        loop {
            let continuing = (evaluate_right, Vec::new());
            let short = (join, vec![decided]);
            let (when_true, when_false) = if conjunction {
                (continuing, short)
            } else {
                (short, continuing)
            };
            self.branch(decided, when_true, when_false, decision_source)?;
            self.enter(evaluate_right);
            // A nonreturning operand ends only its continuing path.
            let Reached::Value(value) = self.scalar(operand)? else {
                break;
            };
            let Some(next) = operands.next() else {
                self.jump(
                    join,
                    vec![value],
                    SourceOrigin::Unpositioned(Unpositioned::ShortCircuitJoin),
                )?;
                break;
            };
            decided = value;
            decision_source = operand.source;
            operand = next;
            evaluate_right =
                self.block(SourceOrigin::Unpositioned(Unpositioned::ShortCircuitRight));
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
    values: Vec<(BlockId, ProgramPoint, Value)>,
    ended: Option<SourceOrigin>,
}
