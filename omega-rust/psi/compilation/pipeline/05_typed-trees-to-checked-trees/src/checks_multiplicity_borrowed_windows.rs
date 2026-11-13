/*
    Borrowed-storage invariant windows through exclusive borrows (ch11).

    A consuming move out of `&mut`-reachable storage opens a repair obligation
    on the exact absent place: while the window is open the place is absent
    from the owner, so every read, borrow, move, or store that touches it or an
    enclosing owner rejects, and every exit edge (transition or return)
    requires the obligation discharged. The replacement is an ordinary store
    of a value carrying the hole's exact type; disjoint sibling work between
    the extraction and the repair is unrestricted.

    Windows are keyed on the resolved *storage* place, not the access route:
    a hole opened through a local `&mut` reborrow (`let r = &mut self.f; let x
    = r.g`) is the same hole the owner path (`self.f.g = ..`) repairs, because
    both name one referent subtree. Repair route liveness stays with the
    ordinary exclusive-loan checks; the window only records that the place is
    absent until a store of the required type reseats it.

    Extraction requires a readable exclusive chain: every reference crossed by
    the moved place's prefixes (the `self` receiver for machine-rooted places,
    a `&mut` parameter root, or a `&mut` field/local inside the path) must be
    `&mut`. Shared and write-only links keep the existing plain rejection.

    Replay observes each evaluated place before applying its exact discovered
    move, then checks invocations after their operands. Grouping all moves at
    statement entry rejects earlier reads and calls; exempting all mentions of
    a moved place hides later stale uses. The occurrence retained by discovery
    distinguishes those cases without reclassifying ownership from syntax.
    A repair store runs only after its value and destination selectors finish.
*/

use crate::checks::borrows::StorageSelectorFacts;
use crate::checks::borrows::storage_path_compatibility;
use crate::checks::reference_access;
use crate::flow::reference_route_source;
use crate::flow::reference_source_place;
use validation::CanonicalPlace;

use language_core::ReferenceAccess;
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

pub(super) use arm_windows::{ArmWindowPlan, ArmWindowVerdict};
pub(super) use boundary_checks::{
    authored_route_exclusive, borrowed_transfer_diagnostic, check_call_occurrences,
    check_recorded_invocations, state_has_boundary_operator,
};
pub(super) use windows::BorrowedStorageWindows;

mod arm_windows {
    /*
        arm windows slice of borrowed_windows.
    */
    use super::super::owned_selection::reachable_arms;
    use crate::flow::DiscoveredMoveEvent;
    use crate::flow::FlowOwnershipEventSource;
    use crate::values::evaluate_closed_boolean_expression;
    use validation::CanonicalPlace;

    use super::{FrameWalk, StorageWalk, place_spelling};

    use crate::checks::borrows::{StorageSelectorFacts, storage_path_compatibility};
    use crate::flow::expressions;

    use checked_trees::facts::StorageWindowInvocation;
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::statement::StatementNode;

    pub(super) type ArmHandle = arena::Handle<typed_trees::expression::TableMatchArm>;

    pub(super) fn arm_key(handle: ArmHandle) -> (u32, u32) {
        (handle.arena_index(), handle.generation())
    }

    pub(super) fn expression_key(handle: ExpressionHandle) -> (u32, u32) {
        (handle.arena_index(), handle.generation())
    }

    /* One `match` expression inside a statement's value evaluation. */
    pub(super) struct MatchWindowFrame {
        /*
            The match expression itself: its `Invoke` step crosses the step stream
            exactly once, after every reachable arm's own steps — the join point
            where an agreeing arm debt becomes one hole on the joined edge.
        */
        expression: ExpressionHandle,
        /* The arm span is the frame's identity and its membership roster. */
        arms: arena::HandleSpan<typed_trees::expression::TableMatchArm>,
        /*
            Arms that can evaluate: shadowed patterns and post-covered arms never
            run, so agreement holds them to nothing.
        */
        reachable: Vec<ArmHandle>,
        /*
            The arm whose value subtree encloses this match, invalid for a
            statement-rooted one. A nested match that agrees lifts its debt into
            this arm's set.
        */
        enclosing_arm: ArmHandle,
        /*
            Whether the match sits in a statement-conditional position — a
            short-circuit operand or transition target that may not evaluate. A
            conditional join cannot promise the hole on the statement's
            fall-through edge. Arm membership is not conditional in this sense:
            the arm's own edges are the frame's domain.
        */
        conditional: bool,
    }

    /* What the event loop should do with one arm-attributed borrowed move. */
    pub(in super::super) enum ArmWindowVerdict {
        /*
            Every reachable arm of the enclosing match agrees on this absent
            place. The hole is recorded as pending and commits at the frame's own
            `Invoke` step — the join — so sibling arms' uses of the place still
            see it present on their own edge.
        */
        Open,
        /*
            A sibling arm already opened this place through this match — the
            debts agree, so the join carries one hole — but this event's authored
            route must still be an exclusive chain.
        */
        SiblingRoute,
        /*
            The join edges disagree (or the move sits under a match that could not
            establish agreement): keep the plain rejection.
        */
        Reject,
        /* The move evaluates inside an arm that can never be selected. */
        DeadArm,
    }

    /*
        Per-statement agreement over moves evaluated inside `match` arms.

        A move on one arm's edge cannot be repaired before the match joins, so the
        edges must agree on the absent places they leave open: an arm-attributed
        borrowed move opens a window only when every reachable arm resolves the
        same absent storage places, including debts an agreeing nested match lifts
        into the arm that evaluates it. Disagreement keeps the plain rejection
        from the unconditional gate it replaces.
    */
    pub(in super::super) struct ArmWindowPlan<'facts> {
        captures: StorageSelectorFacts<'facts>,
        /*
            Every `match` in the statement, pre-order: an enclosing match's frame
            precedes the frames nested inside its arms.
        */
        frames: Vec<MatchWindowFrame>,
        /* arm handle identity → frame index, for every arm of every frame. */
        frame_by_arm: std::collections::BTreeMap<(u32, u32), usize>,
        /*
            expression handle identity → the innermost arm whose value subtree
            contains it. Call-flow move events arrive unattributed; their call
            sites resolve through this map to the evaluating arm.
        */
        arm_of_expression: std::collections::BTreeMap<(u32, u32), ArmHandle>,
        /*
            Resolved storage places each arm's borrowed moves open, keyed by the
            arm's handle identity.
        */
        arm_debts: std::collections::BTreeMap<
            (u32, u32),
            Vec<(facts::PlaceRoot, Vec<facts::PlaceSegment>)>,
        >,
        /* frame index → every arm on its enclosing chain is reachable. */
        live: Vec<bool>,
        /* frame index → its reachable arms agreed on identical debts. */
        agreed: Vec<bool>,
        /*
            frame index → this match agreed and every match enclosing it agreed:
            the join debt is consistent on the whole path to the statement. A
            nested match that agrees inside a disagreeing arm cannot open the
            hole — the outer join would still leave one edge without it.
        */
        rooted_agreed: Vec<bool>,
        /*
            Resolved debts this plan already opened per frame — sibling arms of an
            agreeing match contribute one hole per place, not one hole per event.
        */
        opened: Vec<(usize, facts::PlaceRoot, Vec<facts::PlaceSegment>)>,
        /* frame index → the agreed debt set its reachable arms carry. */
        agreed_debts: Vec<Vec<(facts::PlaceRoot, Vec<facts::PlaceSegment>)>>,
        /*
            frame index → the match itself sits in a statement-conditional
            position, or a match enclosing it does. Pending debts then can never
            commit to the statement edge.
        */
        rooted_conditional: Vec<bool>,
        /*
            Places already absent on one arm's own edge, in step order, as
            (recording arm, resolved root, resolved path). A sibling arm's hole
            does not appear on this edge — only the join shares it.
        */
        edge_absent: Vec<(ArmHandle, facts::PlaceRoot, Vec<facts::PlaceSegment>)>,
        /*
            Agreed debts awaiting their root frame's join: (root frame index,
            move event index, resolved place). The caller commits them through
            `open` when the match's `Invoke` step crosses; the place fields dedupe
            one hole per place at the join even when several frames contributed.
        */
        pending: Vec<(usize, usize, facts::PlaceRoot, Vec<facts::PlaceSegment>)>,
        /*
            Expressions evaluated conditionally on their own arm's edge — a
            short-circuit operand inside the arm's value, for example. A move
            there cannot promise the hole on that edge.
        */
        edge_conditional: std::collections::BTreeSet<(u32, u32)>,
    }

    impl<'facts> ArmWindowPlan<'facts> {
        /*
            Collect the statement's `match` frames and the expression→arm map.
            The walk mirrors `statement_steps`: subjects and patterns evaluate in
            the enclosing context, arm values in their arm, and short-circuit
            right operands stay conditional on both their edge and the statement.
        */
        pub(in super::super) fn new(
            program: &dyn ::validation::CheckingInput,
            statement: &StatementNode,
            operators: &checked_trees::CheckedOperatorFacts,
            captures: StorageSelectorFacts<'facts>,
        ) -> Self {
            let mut plan = Self {
                captures,
                frames: Vec::new(),
                frame_by_arm: std::collections::BTreeMap::new(),
                arm_of_expression: std::collections::BTreeMap::new(),
                arm_debts: std::collections::BTreeMap::new(),
                live: Vec::new(),
                agreed: Vec::new(),
                rooted_agreed: Vec::new(),
                opened: Vec::new(),
                agreed_debts: Vec::new(),
                rooted_conditional: Vec::new(),
                edge_absent: Vec::new(),
                pending: Vec::new(),
                edge_conditional: std::collections::BTreeSet::new(),
            };
            let mut roots: Vec<ExpressionHandle> = Vec::new();
            match statement {
                StatementNode::LocalData(local) => roots.push(local.initial_value),
                StatementNode::Expression(expression) => roots.push(*expression),
                StatementNode::Assignment(assignment) => {
                    roots.push(assignment.value);
                    roots.push(assignment.target);
                }
                StatementNode::Call(call) => roots
                    .extend_from_slice(program.statement_table.expression_handles(call.arguments)),
                _ => {}
            }
            for root in roots {
                let frames = FrameWalk::new(program, operators);
                plan.collect_frames(&frames, root, ArmHandle::invalid(), false, false);
            }
            for (index, frame) in plan.frames.iter().enumerate() {
                for ordinal in 0..frame.arms.len() {
                    let arm = frame.arms.handle_at(ordinal as u32);
                    plan.frame_by_arm.insert(arm_key(arm), index);
                }
            }
            plan
        }

        /* Whether the statement contains any `match` worth planning over. */
        pub(in super::super) fn has_frames(&self) -> bool {
            !self.frames.is_empty()
        }

        pub(in super::super) fn has_edge_absent(&self) -> bool {
            !self.edge_absent.is_empty()
        }

        /*
            Record one eligible arm-scoped borrowed move: a non-empty borrowed
            place path that is neither the statement's production target nor a
            nominal-drop crossing. Unattributed events resolve through the
            expression→arm map (call-flow argument moves).
        */
        pub(in super::super) fn add_borrowed_move(
            &mut self,
            walk: &StorageWalk<'_>,
            statement_index: usize,
            event: &DiscoveredMoveEvent,
            path: &[facts::PlaceSegment],
        ) {
            let arm = self.effective_arm(walk, event);
            if !arm.is_valid() {
                return;
            }
            let Some(root_symbol) = event.root.as_symbol() else {
                return;
            };
            self.arm_debts
                .entry(arm_key(arm))
                .or_default()
                .push(walk.resolve_storage_place(statement_index, root_symbol, path));
        }

        /*
            Compute liveness and agreement. `live` follows the frame's enclosing
            chain (pre-order visits parents first); agreement runs innermost-first
            so each nested match contributes its agreed debt to the arm that
            evaluates it, and a nested disagreement poisons that arm.
        */
        pub(in super::super) fn seal(&mut self, walk: &StorageWalk<'_>) {
            let count = self.frames.len();
            self.live = vec![false; count];
            for index in 0..count {
                let enclosing_arm = self.frames[index].enclosing_arm;
                self.live[index] = !enclosing_arm.is_valid()
                    || self
                        .frame_by_arm
                        .get(&arm_key(enclosing_arm))
                        .is_some_and(|parent| {
                            self.live[*parent]
                                && self.frames[*parent].reachable.contains(&enclosing_arm)
                        });
            }
            self.agreed = vec![false; count];
            let mut agreed_debts: Vec<Vec<(facts::PlaceRoot, Vec<facts::PlaceSegment>)>> =
                vec![Vec::new(); count];
            let mut contributions: std::collections::BTreeMap<
                (u32, u32),
                Option<Vec<(facts::PlaceRoot, Vec<facts::PlaceSegment>)>>,
            > = std::collections::BTreeMap::new();
            for index in (0..count).rev() {
                if !self.live[index] {
                    continue;
                }
                let mut arm_sets: Vec<Vec<(facts::PlaceRoot, Vec<facts::PlaceSegment>)>> = self
                    .frames[index]
                    .reachable
                    .iter()
                    .map(|arm| {
                        self.arm_debts
                            .get(&arm_key(*arm))
                            .cloned()
                            .unwrap_or_default()
                    })
                    .collect();
                let mut rejected = arm_sets.iter().any(|debts| {
                    debts.iter().enumerate().any(|(position, debt)| {
                        debts[..position].iter().any(|prior| {
                            walk.same_storage_debt(
                                self.captures,
                                (prior.0, &prior.1),
                                (debt.0, &debt.1),
                            )
                        })
                    })
                });
                if !rejected {
                    for (position, arm) in self.frames[index].reachable.iter().enumerate() {
                        match contributions.remove(&arm_key(*arm)) {
                            Some(Some(inner)) => {
                                for debt in inner {
                                    if !arm_sets[position].iter().any(|prior| {
                                        walk.same_storage_debt(
                                            self.captures,
                                            (prior.0, &prior.1),
                                            (debt.0, &debt.1),
                                        )
                                    }) {
                                        arm_sets[position].push(debt);
                                    }
                                }
                            }
                            Some(None) => {
                                rejected = true;
                                break;
                            }
                            None => {}
                        }
                    }
                }
                if !rejected {
                    let first = arm_sets.first().map(Vec::as_slice).unwrap_or(&[]);
                    rejected = arm_sets
                        .iter()
                        .any(|debts| !walk.same_place_set(self.captures, debts, first));
                    if !rejected {
                        agreed_debts[index] = first.to_vec();
                    }
                }
                self.agreed[index] = !rejected;
                let enclosing_arm = self.frames[index].enclosing_arm;
                if enclosing_arm.is_valid() {
                    contributions.insert(
                        arm_key(enclosing_arm),
                        self.agreed[index].then(|| agreed_debts[index].clone()),
                    );
                }
            }
            self.rooted_agreed = vec![false; count];
            self.rooted_conditional = vec![false; count];
            for index in 0..count {
                let enclosing_arm = self.frames[index].enclosing_arm;
                let parent = enclosing_arm
                    .is_valid()
                    .then(|| self.frame_by_arm.get(&arm_key(enclosing_arm)))
                    .flatten()
                    .copied();
                self.rooted_agreed[index] =
                    self.agreed[index] && parent.is_none_or(|parent| self.rooted_agreed[parent]);
                self.rooted_conditional[index] = self.frames[index].conditional
                    || parent.is_some_and(|parent| self.rooted_conditional[parent]);
            }
            self.agreed_debts = agreed_debts;
        }

        /*
            Decide what one arm-scoped borrowed move does under the computed
            agreement. `None` marks an unconditional event — the caller keeps the
            ordinary `open` path. An `Open` verdict records the debt against this
            arm's own edge immediately and stages the join-level window under its
            root frame's pending list; `commit_pending` drains it when the match's
            `Invoke` crosses, so a sibling arm's reads still see the place present.
        */
        pub(in super::super) fn verdict(
            &mut self,
            walk: &StorageWalk<'_>,
            statement_index: usize,
            event_index: usize,
            event: &DiscoveredMoveEvent,
            path: &[facts::PlaceSegment],
        ) -> Option<ArmWindowVerdict> {
            let arm = self.effective_arm(walk, event);
            if !arm.is_valid() {
                return None;
            }
            let Some(&frame_index) = self.frame_by_arm.get(&arm_key(arm)) else {
                return Some(ArmWindowVerdict::Reject);
            };
            if !self.live.get(frame_index).copied().unwrap_or(false)
                || !self.frames[frame_index].reachable.contains(&arm)
            {
                return Some(ArmWindowVerdict::DeadArm);
            }
            let site = self.event_site_expression(walk, event);
            if (site.is_valid() && self.edge_conditional.contains(&expression_key(site)))
                || !self.rooted_agreed[frame_index]
                || self.rooted_conditional[frame_index]
            {
                return Some(ArmWindowVerdict::Reject);
            }
            let Some(root_symbol) = event.root.as_symbol() else {
                return Some(ArmWindowVerdict::Reject);
            };
            let resolved = walk.resolve_storage_place(statement_index, root_symbol, path);
            self.edge_absent.push((arm, resolved.0, resolved.1.clone()));
            let root = self.root_frame(frame_index);
            if self.opened.iter().any(|(frame, root, path)| {
                *frame == frame_index
                    && walk.same_storage_debt(
                        self.captures,
                        (*root, path),
                        (resolved.0, &resolved.1),
                    )
            }) || self.pending.iter().any(|(pending_root, _, proot, ppath)| {
                *pending_root == root
                    && walk.same_storage_debt(
                        self.captures,
                        (*proot, ppath),
                        (resolved.0, &resolved.1),
                    )
            }) {
                return Some(ArmWindowVerdict::SiblingRoute);
            }
            self.opened
                .push((frame_index, resolved.0, resolved.1.clone()));
            self.pending
                .push((root, event_index, resolved.0, resolved.1));
            Some(ArmWindowVerdict::Open)
        }

        /*
            The root (statement-level) frame an agreeing move's pending hole
            commits under: nested agreement lifts debts upward, so the hole exists
            on the statement edge only once the outermost match joins.
        */
        fn root_frame(&self, mut frame: usize) -> usize {
            loop {
                let enclosing_arm = self.frames[frame].enclosing_arm;
                let Some(&parent) = enclosing_arm
                    .is_valid()
                    .then(|| self.frame_by_arm.get(&arm_key(enclosing_arm)))
                    .flatten()
                else {
                    return frame;
                };
                frame = parent;
            }
        }

        /*
            Drain the pending debts of the root frame whose `Invoke` step this
            `expression` is, returning their move event indices so the caller can
            run them through `open`. Nested frames' debts already live in the
            root's pending list, so one join commits the whole lifted set.
        */
        pub(in super::super) fn commit_pending(
            &mut self,
            expression: ExpressionHandle,
        ) -> Vec<usize> {
            let key = expression_key(expression);
            let Some(root) = self.frames.iter().position(|frame| {
                !frame.enclosing_arm.is_valid() && expression_key(frame.expression) == key
            }) else {
                return Vec::new();
            };
            let (committed, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending)
                .into_iter()
                .partition(|(root_frame, _, _, _)| *root_frame == root);
            self.pending = kept;
            committed
                .into_iter()
                .map(|(_, event, _, _)| event)
                .collect()
        }

        /*
            Whether a hole recorded on `recorded_arm`'s edge is also present on
            `arm`'s edge: the recording arm is the query arm itself or an arm of a
            match nested inside its value subtree — the nested join leaves the
            hole behind for the rest of the enclosing edge.
        */
        fn edge_covers(&self, recorded_arm: ArmHandle, arm: ArmHandle) -> bool {
            let mut cursor = recorded_arm;
            loop {
                if cursor == arm {
                    return true;
                }
                let Some(&frame) = self.frame_by_arm.get(&arm_key(cursor)) else {
                    return false;
                };
                cursor = self.frames[frame].enclosing_arm;
                if !cursor.is_valid() {
                    return false;
                }
            }
        }

        /*
            The arm-local counterpart of [`BorrowedStorageWindows::check_use`]: a
            read inside an arm's edge while that edge's own extraction is absent.
            Sibling edges' holes never apply, and the join-level ledger entry
            exists only after the match commits.
        */
        pub(in super::super) fn check_edge_use(
            &self,
            walk: &StorageWalk<'_>,
            statement_index: usize,
            expression: ExpressionHandle,
            use_place: &CanonicalPlace,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if self.edge_absent.is_empty() {
                return;
            }
            let program = walk.program;
            let Some(&arm) = self.arm_of_expression.get(&expression_key(expression)) else {
                return;
            };
            if !arm.is_valid() {
                return;
            }
            let Some(root_symbol) = use_place.root.as_symbol() else {
                return;
            };
            let (root, storage_path) =
                walk.resolve_storage_place(statement_index, root_symbol, &use_place.segments);
            for (edge_arm, absent_root, absent_path) in &self.edge_absent {
                if !self.edge_covers(*edge_arm, arm)
                    || *absent_root != root
                    || !storage_path_compatibility(
                        program,
                        self.captures,
                        &storage_path,
                        absent_path,
                    )
                    .0
                {
                    continue;
                }
                let spelling = place_spelling(program, root, &storage_path);
                let absent_spelling = place_spelling(program, *absent_root, absent_path);
                diagnostics.push(Diagnostic::error(format!(
                    "cannot use `{spelling}` while `{absent_spelling}` is absent from borrowed \
                 storage: the value moved out at statement {statement_index} must be restored first",
                )));
            }
        }

        /*
            Preserve each selected arm's open frontier at its own invocation.
            Sibling arms do not lend their holes to this evaluated occurrence.
        */
        pub(in super::super) fn record_edge_invocation(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement: usize,
            expression: ExpressionHandle,
            cleanup: &mut checked_trees::ContextualCleanupFacts,
        ) {
            let Some(&arm) = self.arm_of_expression.get(&expression_key(expression)) else {
                return;
            };
            if !arm.is_valid() {
                return;
            }
            for (_, root, path) in self
                .edge_absent
                .iter()
                .filter(|(edge_arm, _, _)| self.edge_covers(*edge_arm, arm))
            {
                let segments = cleanup.segments.insert_many(path.iter().copied());
                cleanup.window_invocations.append(StorageWindowInvocation {
                    machine,
                    state,
                    statement,
                    expression,
                    absent: facts::Place {
                        root: *root,
                        segments,
                    },
                    opened_statement: None,
                });
            }
        }

        /*
            The arm this move evaluates on: the recorded `source_arm`, or — for
            call-flow events, which the discovery sink emits unattributed — the
            arm whose subtree contains the resolved call site.
        */
        fn effective_arm(&self, walk: &StorageWalk<'_>, event: &DiscoveredMoveEvent) -> ArmHandle {
            let program = walk.program;
            let machine = walk.machine;
            let state = walk.state;
            if event.source_arm.is_valid() {
                return event.source_arm;
            }
            let FlowOwnershipEventSource::Call {
                statement_index,
                call_ordinal,
                ..
            } = event.source
            else {
                return ArmHandle::invalid();
            };
            let Some(validation::CallSite::Expression { expression, .. }) =
                validation::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    call_ordinal,
                )
            else {
                return ArmHandle::invalid();
            };
            self.arm_of_expression
                .get(&expression_key(expression))
                .copied()
                .unwrap_or_default()
        }

        /*
            The expression whose evaluation position carries this event: the
            recorded operand, or the resolved call site for call-flow events that
            name a statement-local call by ordinal instead.
        */
        fn event_site_expression(
            &self,
            walk: &StorageWalk<'_>,
            event: &DiscoveredMoveEvent,
        ) -> ExpressionHandle {
            let program = walk.program;
            let machine = walk.machine;
            let state = walk.state;
            if event.expression.is_valid() {
                return event.expression;
            }
            let FlowOwnershipEventSource::Call {
                statement_index,
                call_ordinal,
                ..
            } = event.source
            else {
                return ExpressionHandle::invalid();
            };
            match validation::find_call_site(
                program,
                machine.symbol,
                state.symbol,
                statement_index,
                call_ordinal,
            ) {
                Some(validation::CallSite::Expression { expression, .. }) => expression,
                _ => ExpressionHandle::invalid(),
            }
        }

        fn collect_frames(
            &mut self,
            frames: &FrameWalk<'_>,
            expression: ExpressionHandle,
            enclosing_arm: ArmHandle,
            statement_conditional: bool,
            edge_conditional: bool,
        ) {
            let program = frames.program;
            let operators = frames.operators;
            macro_rules! frame {
                ($expression:expr) => {
                    self.collect_frames(
                        frames,
                        $expression,
                        enclosing_arm,
                        statement_conditional,
                        edge_conditional,
                    )
                };
            }
            if !expression.is_valid() {
                return;
            }
            self.arm_of_expression
                .insert(expression_key(expression), enclosing_arm);
            if edge_conditional {
                self.edge_conditional.insert(expression_key(expression));
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::SavedSubject(_) | ExpressionNode::MachineValue(_) => {}
                ExpressionNode::Match(dispatch) => {
                    self.frames.push(MatchWindowFrame {
                        expression,
                        arms: dispatch.arms,
                        reachable: reachable_arms(program, dispatch.arms)
                            .into_iter()
                            .map(|(handle, _)| handle)
                            .collect(),
                        enclosing_arm,
                        conditional: statement_conditional,
                    });
                    frame!(dispatch.subject);
                    for (ordinal, arm) in expressions(program)
                        .match_arms(dispatch.arms)
                        .iter()
                        .enumerate()
                    {
                        let arm_handle = dispatch.arms.handle_at(ordinal as u32);
                        for pattern in program.expression_table.match_pattern_operands(arm.pattern)
                        {
                            self.collect_frames(
                                frames,
                                pattern,
                                enclosing_arm,
                                true,
                                edge_conditional,
                            );
                        }
                        self.collect_frames(
                            frames,
                            arm.value,
                            arm_handle,
                            statement_conditional,
                            false,
                        );
                    }
                }
                ExpressionNode::Atomic(atomic) => {
                    frame!(atomic.value);
                    frame!(atomic.result);
                }
                ExpressionNode::ArrayLiteral(values) => {
                    for value in program.expression_table.expression_handles(*values) {
                        frame!(*value);
                    }
                }
                ExpressionNode::Binary(binary) => {
                    frame!(binary.left);
                    if matches!(
                        binary.operator,
                        typed_trees::expression::BinaryOperator::And
                            | typed_trees::expression::BinaryOperator::Or
                    ) {
                        let evaluate_when =
                            binary.operator == typed_trees::expression::BinaryOperator::And;
                        let left = evaluate_closed_boolean_expression(
                            program,
                            operators,
                            binary.left,
                            &[],
                        );
                        if left != Some(!evaluate_when) {
                            self.collect_frames(
                                frames,
                                binary.right,
                                enclosing_arm,
                                statement_conditional || left.is_none(),
                                edge_conditional || left.is_none(),
                            );
                        }
                    } else {
                        frame!(binary.right);
                    }
                }
                ExpressionNode::Borrow(borrow) => {
                    frame!(borrow.target);
                }
                ExpressionNode::Call(call) => {
                    frame!(call.evaluated_callee());
                    for argument in program.expression_table.expression_handles(call.arguments) {
                        frame!(*argument);
                    }
                }
                ExpressionNode::Cast(cast) => {
                    frame!(cast.value);
                }
                ExpressionNode::Indexed(indexed) => {
                    frame!(indexed.collection);
                    frame!(indexed.index);
                }
                ExpressionNode::Member(member) => {
                    frame!(member.receiver);
                }
                ExpressionNode::Range(range) => {
                    frame!(range.start);
                    frame!(range.end);
                }
                ExpressionNode::StructLiteral(literal) => {
                    for field in program.expression_table.struct_fields(literal.fields) {
                        frame!(field.value);
                    }
                }
                ExpressionNode::Unary(unary) => {
                    frame!(unary.operand);
                }
                ExpressionNode::Name(_)
                | ExpressionNode::TypeExpression(_)
                | typed_trees::spelled_literal_expression_pattern!() => {}
            }
        }
    }
}
mod boundary_checks {
    /*
        boundary checks slice of borrowed_windows.
    */
    use super::{StorageWalk, place_spelling};

    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;

    /*
        The expression coordinate is checked evidence, not permission to relocate
        an invocation outside a restoration window. Rejoin it to the authored call
        ordinal before occurrence-ordered replay uses the row. Retired calls belong
        to their selected replacements and are not executed again here.
    */
    pub(in crate::checks::multiplicity) fn check_call_occurrences(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        calls: &[checked_trees::FlowCallFact],
        control: &checked_trees::FlowControlFacts,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for call in calls {
            if control.is_retired(state.symbol, call) {
                continue;
            }
            let expected = validation::find_call_site(
                program,
                machine.symbol,
                state.symbol,
                call.statement_index,
                call.call_ordinal,
            )
            .map(|site| match site {
                validation::CallSite::Expression { expression, .. } => expression,
                _ => ExpressionHandle::invalid(),
            });
            if expected != Some(call.authored_expression) {
                diagnostics.push(Diagnostic::error(format!(
                    "borrowed-storage call occurrence does not match statement {} invocation {} in `{}`",
                    call.statement_index, call.call_ordinal, machine.name.as_str(),
                )));
            }
        }
    }

    /*
        Complete invocation checks after selected cleanup closes the effect graph.
        Source recording retained each actual open frontier, including match-arm
        occurrences. This consumes final call effects without reapplying any move
        or reconstructing restoration from the later, repaired storage state.
    */
    pub(in crate::checks::multiplicity) fn check_recorded_invocations(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        for (_, invocation) in facts.contextual_cleanup.window_invocations.iter() {
            let Some(machine) = program.machine_by_symbol(invocation.machine) else {
                return Err(vec![Diagnostic::error(
                    "borrowed-storage invocation has no owning machine",
                )]);
            };
            let Some(state) = program.state_in_machine(machine, invocation.state) else {
                return Err(vec![Diagnostic::error(
                    "borrowed-storage invocation has no owning state",
                )]);
            };
            let Some(flow) = facts
                .flow
                .control
                .state(invocation.machine, invocation.state)
            else {
                return Err(vec![Diagnostic::error(
                    "borrowed-storage invocation has no final call flow",
                )]);
            };
            let calls = facts.flow.control.calls.span_or_empty(flow.calls);
            let selected = |call: &&checked_trees::FlowCallFact| {
                call.statement_index == invocation.statement
                    && call.authored_expression == invocation.expression
                    && !facts.flow.control.is_retired(state.symbol, call)
            };
            let may_suspend = calls.iter().filter(selected).any(|call| {
                call.suspension.direct_may_suspend
                    || call.suspension.transitive_may_suspend
                    || call.blocking.direct_may_block
                    || call.blocking.transitive_may_block
            });
            let may_enter_boundary = calls.iter().filter(selected).any(|call| {
                call_may_enter_boundary(
                    program,
                    &facts.service_reaches,
                    &facts.flow.control,
                    &facts.operators,
                    state.symbol,
                    call,
                )
            }) || state_has_boundary_operator(
                program,
                &facts.flow.control,
                &facts.operators,
                state.symbol,
                Some(invocation.statement),
                Some(invocation.expression),
            );
            if !may_suspend && !may_enter_boundary {
                continue;
            }
            let path = facts
                .contextual_cleanup
                .segments
                .span_or_empty(invocation.absent.segments);
            let spelling = place_spelling(program, invocation.absent.root, path);
            let extraction = invocation
                .opened_statement
                .map(|statement| format!(" at statement {statement}"))
                .unwrap_or_default();
            if may_suspend {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot suspend or block at statement {} while `{spelling}` is absent: restore the value moved out of borrowed storage{extraction} first",
                    invocation.statement,
                )));
            }
            if may_enter_boundary {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot make a boundary or service call at statement {} while `{spelling}` is absent: restore the value moved out of borrowed storage{extraction} first",
                    invocation.statement,
                )));
            }
        }
        crate::checks::finish(diagnostics)
    }

    /*
        Service reach and boundary invocation are different facts: an opaque
        boundary may declare empty reach. Follow the retained exact call topology
        through ordinary wrappers as well, rather than treating that empty row (or
        an empty write frame, which says nothing about reads) as non-observation.
        This query runs only for calls crossing an open window. Its local visited
        set closes cycles without inventing a new effect row or expanding paths.
    */
    pub(super) fn call_may_enter_boundary(
        program: &dyn ::validation::CheckingInput,
        service_reaches: &checked_trees::ServiceReachFacts,
        control: &checked_trees::FlowControlFacts,
        operators: &checked_trees::CheckedOperatorFacts,
        state: SymbolHandle,
        call: &checked_trees::FlowCallFact,
    ) -> bool {
        let empty = language_semantics::ServiceReachRowTable::EMPTY_ROW;
        if !call.boundary_edges.is_empty()
            || call.service_reach.direct != empty
            || call.service_reach.transitive != empty
        {
            return true;
        }
        let Some(target) = service_reaches.for_state(state).and_then(|state| {
            service_reaches.calls_for(state).iter().find(|target| {
                target.statement_index == call.statement_index
                    && target.call_ordinal == call.call_ordinal
                    && target.target_state == call.target_symbol
            })
        }) else {
            return true;
        };
        let mut pending = vec![target];
        let mut visited = Vec::new();
        while let Some(call) = pending.pop() {
            if call.inferred_direct != empty || call.inferred_transitive != empty {
                return true;
            }
            if let Some(builtin) = program
                .symbols
                .builtin_function_for_symbol(call.target_state)
            {
                if builtin.asm_intrinsic_service_name().is_some() {
                    return true;
                }
                continue;
            }
            let target = call.target_machine;
            if visited.contains(&target) {
                continue;
            }
            visited.push(target);
            let Some(machine) = program.machine_by_symbol(target) else {
                return true;
            };
            if !machine.supply_mode.is_checked_body() || !machine.body_is_present {
                return true;
            }
            let Some(reach) = service_reaches.for_machine(target) else {
                return true;
            };
            for state in service_reaches.states_for(reach) {
                if state_has_boundary_operator(program, control, operators, state.state, None, None)
                {
                    return true;
                }
                pending.extend(service_reaches.calls_for(state));
            }
        }
        false
    }

    /*
        Operator invocations are scheduled separately from ordinary calls. Use
        those records for both named and spelled operators so skipped operands do
        not become calls, and wrappers cannot hide an opaque operator invocation.
    */
    pub(in crate::checks::multiplicity) fn state_has_boundary_operator(
        program: &dyn ::validation::CheckingInput,
        control: &checked_trees::FlowControlFacts,
        operators: &checked_trees::CheckedOperatorFacts,
        state: SymbolHandle,
        statement: Option<usize>,
        expression: Option<ExpressionHandle>,
    ) -> bool {
        control.operator_invocations.iter().any(|(_, invocation)| {
            let (origin, target, occurrence) = if invocation.named_use.is_valid() {
                let usage = operators.named_uses.get(invocation.named_use);
                (
                    usage.origin,
                    usage.selected_operator_symbol,
                    usage.expression,
                )
            } else {
                let usage = operators.uses.get(invocation.operator_use);
                (
                    usage.origin,
                    usage.selected_operator_symbol,
                    usage.expression,
                )
            };
            matches!(origin, checked_trees::CheckedValueOrigin::StateStatement {
                state_symbol, statement_index, ..
            } if state_symbol == state && statement.is_none_or(|expected| expected == statement_index))
                && expression.is_none_or(|expected| expected == occurrence)
                && typed_trees::operator::declaration_by_symbol(program, target)
                    .is_some_and(|operator| operator.is_boundary)
        })
    }

    /*
        The shared diagnostic for a borrowed-storage transfer that cannot open or
        complete a window.
    */
    pub(in crate::checks::multiplicity) fn borrowed_transfer_diagnostic(
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "cannot transfer a non-copy value out of borrowed storage without replacing its \
         owner in `{}`, state `{}` (statement {})",
            machine.name.as_str(),
            state.name.as_str(),
            statement_index,
        ))
    }

    /*
        The exclusive-route half of [`BorrowedStorageWindows::open`] keyed on the
        authored spelling: sibling arms may name the same referent storage through
        different routes (a `&mut` local versus the owner path), so each arm's
        route is checked on its own links even though the join opens one hole.
    */
    pub(in crate::checks::multiplicity) fn authored_route_exclusive(
        walk: &StorageWalk<'_>,
        statement_index: usize,
        root: facts::PlaceRoot,
        path: &[facts::PlaceSegment],
    ) -> bool {
        matches!(root, facts::PlaceRoot::Symbol(_))
            && walk.exclusive_storage_chain(statement_index, root, path)
    }
}
mod windows {
    /*
        windows slice of borrowed_windows.
    */
    use super::super::linear_obligations::type_reference_is_reference;
    use checked_trees::CapturedPlaceContainment;
    use checked_trees::ContextualCleanupFacts;
    use facts::Place;
    use facts::PlaceRoot;
    use facts::PlaceSegment;
    use language_semantics::Multiplicity;
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::TableAssignment;
    use validation::{
        CanonicalPlace, assignment_stored_value_type, canonical_place_type_reference,
        expression_result_type_reference,
    };

    use super::{StorageWalk, place_spelling};

    use super::boundary_checks::*;

    use crate::checks::borrows::{StorageSelectorFacts, storage_path_compatibility};
    use crate::published::places::place_at;

    use checked_trees::facts::StorageWindowInvocation;
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::types::TypeReferenceHandle;

    /*
        One absent place inside exclusive borrowed storage. `root`/`path` name the
        resolved storage place (a `&mut`-local route is already rebased onto the
        referent root); `spelling` is the authored-facing name used in diagnostics.
    */
    pub(super) struct OpenWindow {
        root: PlaceRoot,
        path: Vec<PlaceSegment>,
        spelling: String,
        required_type: TypeReferenceHandle,
        opened_statement: usize,
        /*
            Set once an exit edge reports the still-open debt; later edges and the
            implicit return do not repeat the identical diagnostic.
        */
        exit_reported: bool,
    }

    pub(in crate::checks::multiplicity) struct BorrowedStorageWindows<'facts> {
        open: Vec<OpenWindow>,
        captures: StorageSelectorFacts<'facts>,
    }

    impl<'facts> BorrowedStorageWindows<'facts> {
        pub(in crate::checks::multiplicity) fn new(captures: StorageSelectorFacts<'facts>) -> Self {
            Self {
                open: Vec::new(),
                captures,
            }
        }

        pub(in crate::checks::multiplicity) fn has_open_windows(&self) -> bool {
            !self.open.is_empty()
        }

        /*
            Select the still-present old referent after RHS evaluation. A repair
            into an exact hole creates no second death; replacing an enclosing
            value retains the absent children's exclusions. This is the same
            window state that checks reads, moves, invocations and restoration.
        */
        pub(in crate::checks::multiplicity) fn replacement_death(
            &self,
            walk: &StorageWalk<'_>,
            statement_index: usize,
            assignment: &TableAssignment,
        ) -> Option<(CanonicalPlace, TypeReferenceHandle, Vec<Vec<PlaceSegment>>)> {
            let program = walk.program;
            let machine = walk.machine;
            let state = walk.state;
            let target = place_at(program, state.symbol, statement_index, assignment.target)?;
            let borrowed = (0..=target.segments.len()).any(|length| {
                let prefix = CanonicalPlace {
                    root: target.root,
                    segments: target.segments[..length].to_vec(),
                };
                canonical_place_type_reference(program, state.symbol, statement_index, &prefix)
                    .is_some_and(|reference| type_reference_is_reference(program, reference))
            });
            if !borrowed
                || !walk.exclusive_storage_chain(statement_index, target.root, &target.segments)
            {
                return None;
            }
            let value_type = assignment_stored_value_type(program, machine, state, assignment)?;
            if program.type_multiplicity(value_type) != Multiplicity::Affine {
                return None;
            }
            let symbol = target.root.as_symbol()?;
            let (root, path) =
                walk.resolve_storage_place(statement_index, symbol, &target.segments);
            if self.open.iter().any(|absent| {
                absent.root == root
                    && storage_path_contains(program, self.captures, &absent.path, &path)
            }) {
                return None;
            }
            let excluded = self
                .open
                .iter()
                .filter(|absent| {
                    absent.root == root
                        && storage_path_contains(program, self.captures, &path, &absent.path)
                })
                .map(|absent| absent.path.clone())
                .collect();
            Some((
                CanonicalPlace {
                    root,
                    segments: path,
                },
                value_type,
                excluded,
            ))
        }

        /*
            The repair obligation an eligible borrowed move creates. Returns the
            diagnostic to emit when the transfer cannot open a window — a
            non-symbol root, a shared or write-only link in the chain, or an
            overlap with an already absent place — and `None` once the window is
            recorded.
        */
        pub(in crate::checks::multiplicity) fn open(
            &mut self,
            walk: &StorageWalk<'_>,
            statement_index: usize,
            event: &crate::flow::DiscoveredMoveEvent,
            path: &[PlaceSegment],
        ) -> Option<Diagnostic> {
            let program = walk.program;
            let machine = walk.machine;
            let state = walk.state;
            let Some(root_symbol) = event.root.as_symbol() else {
                return Some(borrowed_transfer_diagnostic(
                    machine,
                    state,
                    statement_index,
                ));
            };
            if !walk.exclusive_storage_chain(statement_index, event.root, path) {
                return Some(borrowed_transfer_diagnostic(
                    machine,
                    state,
                    statement_index,
                ));
            }
            let (root, storage_path) =
                walk.resolve_storage_place(statement_index, root_symbol, path);
            if !walk.exclusive_storage_chain(statement_index, root, &storage_path) {
                return Some(borrowed_transfer_diagnostic(
                    machine,
                    state,
                    statement_index,
                ));
            }
            let spelling = walk.restoration_debt_spelling(
                statement_index,
                event.expression,
                root,
                &storage_path,
            );
            if let Some(absent) = self.open.iter().find(|absent| {
                absent.root == root
                    && storage_path_compatibility(
                        program,
                        self.captures,
                        &storage_path,
                        &absent.path,
                    )
                    .0
            }) {
                return Some(Diagnostic::error(format!(
                    "cannot transfer `{spelling}` out of borrowed storage while `{}` is absent: \
                 the value moved at statement {} has not been replaced",
                    absent.spelling, absent.opened_statement,
                )));
            }
            let event_place = CanonicalPlace {
                root: event.root,
                segments: path.to_vec(),
            };
            let required_type = event
                .value_type
                .is_valid()
                .then_some(event.value_type)
                .or_else(|| {
                    canonical_place_type_reference(
                        program,
                        state.symbol,
                        statement_index,
                        &event_place,
                    )
                })
                .unwrap_or_default();
            self.open.push(OpenWindow {
                root,
                path: storage_path,
                spelling,
                required_type,
                opened_statement: statement_index,
                exit_reported: false,
            });
            None
        }

        /*
            A move of a place that is — or still contains — an absent borrowed
            subtree cannot leave: the moved value would carry the hole with it.
            This is the ordinary-move twin of `open`'s overlap refusal and covers
            whole-owner moves (empty paths) the borrowed gate never sees.
        */
        pub(in crate::checks::multiplicity) fn refuse_move_over_open(
            &self,
            walk: &StorageWalk<'_>,
            statement_index: usize,
            event: &crate::flow::DiscoveredMoveEvent,
            path: &[PlaceSegment],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if self.open.is_empty() {
                return;
            }
            let program = walk.program;
            let Some(root_symbol) = event.root.as_symbol() else {
                return;
            };
            let (root, storage_path) =
                walk.resolve_storage_place(statement_index, root_symbol, path);
            let Some(absent) = self.open.iter().find(|absent| {
                absent.root == root
                    && storage_path_compatibility(
                        program,
                        self.captures,
                        &storage_path,
                        &absent.path,
                    )
                    .0
            }) else {
                return;
            };
            let spelling = place_spelling(program, root, &storage_path);
            diagnostics.push(Diagnostic::error(format!(
                "cannot transfer `{spelling}` while `{}` is absent: the value moved out of \
             borrowed storage at statement {} has not been replaced",
                absent.spelling, absent.opened_statement,
            )));
        }

        /*
            Retain the exact open frontier after operands finish. Cleanup effect
            closure is unavailable during source death selection, so invocation
            effects consume these occurrences later rather than replaying moves.
        */
        pub(in crate::checks::multiplicity) fn record_invocation(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement: usize,
            expression: ExpressionHandle,
            cleanup: &mut ContextualCleanupFacts,
        ) {
            for absent in &self.open {
                let segments = cleanup.segments.insert_many(absent.path.iter().copied());
                cleanup.window_invocations.append(StorageWindowInvocation {
                    machine,
                    state,
                    statement,
                    expression,
                    absent: Place {
                        root: absent.root,
                        segments,
                    },
                    opened_statement: Some(absent.opened_statement),
                });
            }
        }

        pub(in crate::checks::multiplicity) fn check_use(
            &self,
            walk: &StorageWalk<'_>,
            statement_index: usize,
            use_place: &CanonicalPlace,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if self.open.is_empty() {
                return;
            }
            let program = walk.program;
            let Some(root_symbol) = use_place.root.as_symbol() else {
                return;
            };
            let (root, storage_path) =
                walk.resolve_storage_place(statement_index, root_symbol, &use_place.segments);
            let Some(absent) = self.open.iter().find(|absent| {
                absent.root == root
                    && storage_path_compatibility(
                        program,
                        self.captures,
                        &storage_path,
                        &absent.path,
                    )
                    .0
            }) else {
                return;
            };
            let spelling = place_spelling(program, root, &storage_path);
            diagnostics.push(Diagnostic::error(format!(
                "cannot use `{spelling}` while `{}` is absent from borrowed storage: the \
                 value moved out at statement {} must be restored first",
                absent.spelling, absent.opened_statement,
            )));
        }

        /*
            A store whose target is the absent place — or a whole enclosing owner —
            discharges the obligation when the stored value carries the hole's
            exact type. Any other overlapping store touches absent storage.
        */
        pub(in crate::checks::multiplicity) fn check_repair(
            &mut self,
            walk: &StorageWalk<'_>,
            statement_index: usize,
            assignment: &TableAssignment,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if self.open.is_empty() {
                return;
            }
            let program = walk.program;
            let machine = walk.machine;
            let state = walk.state;
            let Some(target) = place_at(program, state.symbol, statement_index, assignment.target)
            else {
                return;
            };
            let Some(root_symbol) = target.root.as_symbol() else {
                return;
            };
            let (root, target_path) =
                walk.resolve_storage_place(statement_index, root_symbol, &target.segments);
            let mut repaired = Vec::new();
            for (index, absent) in self.open.iter().enumerate() {
                if absent.root != root
                    || !storage_path_compatibility(
                        program,
                        self.captures,
                        &target_path,
                        &absent.path,
                    )
                    .0
                {
                    continue;
                }
                if storage_path_contains(program, self.captures, &target_path, &absent.path) {
                    repaired.push(index);
                    continue;
                }
                let spelling = place_spelling(program, root, &target_path);
                diagnostics.push(Diagnostic::error(format!(
                    "cannot store `{spelling}` while `{}` is absent: the value moved out of \
                 borrowed storage at statement {} must be restored as a whole",
                    absent.spelling, absent.opened_statement,
                )));
            }
            for index in repaired.into_iter().rev() {
                let absent = self.open.remove(index);
                if absent.path.len() != target_path.len() {
                    continue;
                }
                let value_type =
                    expression_result_type_reference(program, machine, state, assignment.value);
                let exact = value_type.is_some_and(|value_type| {
                    absent.required_type.is_valid()
                        && program.normalized_type_identity(value_type)
                            == program.normalized_type_identity(absent.required_type)
                });
                if !exact {
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot restore `{}` from this value: the place must regain a value of \
                     the exact type statement {} moved out of borrowed storage",
                        absent.spelling, absent.opened_statement,
                    )));
                }
            }
        }

        /*
            Every exit edge — each transition arm and the result expression — hands
            the borrowed owner back; every open window must be discharged first.
            The diagnostics name the opening move and the absent place. Each window
            reports once: additional arms leaving the same region add no noise.
        */
        pub(in crate::checks::multiplicity) fn refuse_open_at_exit(
            &mut self,
            machine: &Machine,
            state: &State,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for absent in &mut self.open {
                if absent.exit_reported {
                    continue;
                }
                absent.exit_reported = true;
                diagnostics.push(Diagnostic::error(format!(
                    "cannot transfer a non-copy value out of borrowed storage without replacing \
                 its owner in `{}`, state `{}` (statement {}): `{}` must hold a value again \
                 before this state's exits consume the borrow",
                    machine.name.as_str(),
                    state.name.as_str(),
                    absent.opened_statement,
                    absent.spelling,
                )));
            }
        }
    }

    /*
        Restoration and old-value death select the same captured subtree. Raw
        expression-handle prefixes cannot recognize a re-spelled immutable index;
        structural spelling equality could instead conflate changed mutable values.
    */
    fn storage_path_contains(
        program: &dyn ::validation::CheckingInput,
        captures: StorageSelectorFacts<'_>,
        container: &[PlaceSegment],
        contained: &[PlaceSegment],
    ) -> bool {
        matches!(
            storage_path_compatibility(program, captures, container, contained).1,
            CapturedPlaceContainment::Same | CapturedPlaceContainment::LeftContainsRight
        )
    }
}

enum ProjectionStep {
    Field,
    Index {
        symbolic: bool,
        expression: typed_trees::expression::ExpressionHandle,
    },
}

/*
    Field and index steps of one place expression, root symbol first.
    Borrows and recasts are transparent. A call anywhere in an index operand
    keeps that selector authored (`[index]`) even after its value is proved.
*/
fn authored_projection(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<(SymbolHandle, Vec<ProjectionStep>)> {
    use typed_trees::expression::ExpressionNode;
    let mut expression = expression;
    let mut steps = Vec::new();
    for _ in 0..32 {
        if !expression.is_valid() {
            return None;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            ExpressionNode::Cast(cast) if cast.form.is_recast() => expression = cast.value,
            ExpressionNode::Member(member) => {
                steps.push(ProjectionStep::Field);
                expression = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                steps.push(ProjectionStep::Index {
                    symbolic: !matches!(
                        program.expression_table.expression(indexed.index),
                        ExpressionNode::Integer(_)
                    ),
                    expression: indexed.index,
                });
                expression = indexed.collection;
            }
            ExpressionNode::Name(path) => {
                steps.reverse();
                return Some((path.symbol, steps));
            }
            _ => return None,
        }
    }
    None
}

/*
    A proved selector and the fixed element it selected are the same debt.
    Field and case segments must still match; a literal index must match its
    numeral. An unresolved `Index` matches only the fixed element custody
    recorded for that same move.
*/
fn selector_path_matches(
    authored: &[facts::PlaceSegment],
    resolved: &[facts::PlaceSegment],
) -> bool {
    authored.len() == resolved.len()
        && authored
            .iter()
            .zip(resolved)
            .all(|(authored, resolved)| match (authored, resolved) {
                (
                    facts::PlaceSegment::Index { .. },
                    facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. },
                ) => true,
                _ => authored == resolved,
            })
}

/*
    The shared storage-resolution walk context: the program, the machine/state
    pair under evaluation, and that state's statement list. Every resolution
    helper restated this tuple at its call site, and each window owner restated
    it again on every method; the walk carries it once.
*/
pub(in crate::checks::multiplicity) struct StorageWalk<'a> {
    program: &'a dyn ::validation::CheckingInput,
    machine: &'a typed_trees::machine::Machine,
    state: &'a typed_trees::state::State,
    statements: &'a [StatementNode],
}

impl<'a> StorageWalk<'a> {
    pub(in crate::checks::multiplicity) fn new(
        program: &'a dyn ::validation::CheckingInput,
        machine: &'a typed_trees::machine::Machine,
        state: &'a typed_trees::state::State,
        statements: &'a [StatementNode],
    ) -> Self {
        Self {
            program,
            machine,
            state,
            statements,
        }
    }

    /*
        The join-agreement comparison: two arms carry the same resolved debts,
        regardless of the order their moves recorded them in.
    */
    fn same_place_set(
        &self,
        captures: StorageSelectorFacts<'_>,
        left: &[(facts::PlaceRoot, Vec<facts::PlaceSegment>)],
        right: &[(facts::PlaceRoot, Vec<facts::PlaceSegment>)],
    ) -> bool {
        left.len() == right.len()
            && left.iter().all(|place| {
                right.iter().any(|candidate| {
                    self.same_storage_debt(
                        captures,
                        (place.0, &place.1),
                        (candidate.0, &candidate.1),
                    )
                })
            })
    }

    fn same_storage_debt(
        &self,
        captures: StorageSelectorFacts<'_>,
        left: (facts::PlaceRoot, &[facts::PlaceSegment]),
        right: (facts::PlaceRoot, &[facts::PlaceSegment]),
    ) -> bool {
        left.0 == right.0
            && storage_path_compatibility(self.program, captures, left.1, right.1).1
                == checked_trees::CapturedPlaceContainment::Same
    }

    /*
        Rebase a place written through a `&mut`-typed local onto the referent
        storage it borrows. `let r = &mut self.f` makes `r.g` and `self.f.g` name
        the same absent subtree; the loop also follows plain reference copies
        (`let s = r`), recast chains, and later reassignments of the reference
        local to the source the route currently captures.
    */
    pub(super) fn resolve_storage_place(
        &self,
        statement_index: usize,
        root_symbol: SymbolHandle,
        path: &[facts::PlaceSegment],
    ) -> (facts::PlaceRoot, Vec<facts::PlaceSegment>) {
        let mut root = facts::PlaceRoot::Symbol(root_symbol);
        let mut segments = path.to_vec();
        for _ in 0..16 {
            let Some(symbol) = root.as_symbol() else {
                break;
            };
            let Some(source) = reference_route_source(
                self.program,
                self.state,
                self.statements,
                statement_index,
                symbol,
            ) else {
                break;
            };
            let Some(source) =
                reference_source_place(self.program, self.state.symbol, statement_index, source)
            else {
                break;
            };
            let facts::PlaceRoot::Symbol(..) = source.root else {
                break;
            };
            let mut rebased = source.segments;
            rebased.extend_from_slice(&segments);
            root = source.root;
            let rebased_place = CanonicalPlace {
                root,
                segments: rebased,
            };

            root = rebased_place.root;
            segments = rebased_place.segments;
        }
        (root, segments)
    }

    /*
        Diagnostic spelling of a restoration debt.

        Custody freezes a proved selector (`first_slot()` ensures `result == 0`)
        to `FixedIndex` so a later store to element 1 does not repair element 0.
        The reader-facing name stays the authored selector: `tokens[index]`, the
        same spelling an unresolved `Index` segment already uses. A literal
        index keeps its numeral.
    */
    pub(super) fn restoration_debt_spelling(
        &self,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
        resolved_root: facts::PlaceRoot,
        resolved_path: &[facts::PlaceSegment],
    ) -> String {
        self.authored_selector_spelling(statement_index, expression, resolved_root, resolved_path)
            .unwrap_or_else(|| place_spelling(self.program, resolved_root, resolved_path))
    }

    fn authored_selector_spelling(
        &self,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
        resolved_root: facts::PlaceRoot,
        resolved_path: &[facts::PlaceSegment],
    ) -> Option<String> {
        if !expression.is_valid() {
            return None;
        }
        let mut sources = Vec::new();
        if let Some(source) = self.reference_binding_source(statement_index, expression) {
            sources.push(source);
        }
        if let Some(initial) = self.reference_initializer(expression) {
            sources.push(initial);
        }
        sources.push(expression);
        for source in sources {
            if let Some(spelling) = self.spelling_for_call_selector(
                statement_index,
                source,
                resolved_root,
                resolved_path,
            ) {
                return Some(spelling);
            }
        }
        None
    }

    /*
        Spell `tokens[index]` when the debt was opened through a call selector.

        Canonical projection and permission events may both freeze
        `first_slot()` (ensures `result == 0`) to `FixedIndex`. Custody keeps
        that coordinate. The diagnostic names the selector the author wrote.
    */
    fn spelling_for_call_selector(
        &self,
        statement_index: usize,
        source: typed_trees::expression::ExpressionHandle,
        resolved_root: facts::PlaceRoot,
        resolved_path: &[facts::PlaceSegment],
    ) -> Option<String> {
        let (root_symbol, steps) = authored_projection(self.program, source)?;
        if !steps
            .iter()
            .any(|step| matches!(step, ProjectionStep::Index { symbolic: true, .. }))
        {
            return None;
        }
        let place =
            reference_source_place(self.program, self.state.symbol, statement_index, source)?;
        let root_symbol = place.root.as_symbol().unwrap_or(root_symbol);
        let (root, path) =
            self.resolve_storage_place(statement_index, root_symbol, &place.segments);
        if root != resolved_root || !selector_path_matches(&path, resolved_path) {
            return None;
        }
        let mut spelled = path;
        let mut indexes = steps.into_iter().filter_map(|step| match step {
            ProjectionStep::Index {
                symbolic,
                expression,
            } => Some((symbolic, expression)),
            ProjectionStep::Field => None,
        });
        for segment in &mut spelled {
            if !matches!(
                segment,
                facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. }
            ) {
                continue;
            }
            let (symbolic, index_expression) = indexes.next()?;
            if symbolic {
                *segment = facts::PlaceSegment::Index {
                    expression: index_expression,
                };
            }
        }
        if indexes.next().is_some() {
            return None;
        }
        Some(place_spelling(self.program, root, &spelled))
    }

    fn reference_binding_source(
        &self,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<typed_trees::expression::ExpressionHandle> {
        let typed_trees::expression::ExpressionNode::Name(path) =
            self.program.expression_table.expression(expression)
        else {
            return None;
        };
        reference_route_source(
            self.program,
            self.state,
            self.statements,
            statement_index,
            path.symbol,
        )
    }

    fn reference_initializer(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<typed_trees::expression::ExpressionHandle> {
        let typed_trees::expression::ExpressionNode::Name(path) =
            self.program.expression_table.expression(expression)
        else {
            return None;
        };
        let (_, declaration) = self
            .program
            .state_local_declaration(self.state, path.symbol)?;
        if !crate::checks::type_reference_is_reference(self.program, declaration.type_reference) {
            return None;
        }
        self.statements
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.symbol == path.symbol => {
                    Some(local.initial_value)
                }
                _ => None,
            })
    }

    /*
        Every reference crossed by the extracted place must be `&mut`: the `self`
        receiver for machine-rooted storage, then each proper prefix of the path
        (a `&mut` parameter root, a `&mut` field inside owned storage, a `&mut`
        local the window resolved through). Shared or write-only links cannot
        carry a restoration window.
    */
    fn exclusive_storage_chain(
        &self,
        statement_index: usize,
        root: facts::PlaceRoot,
        path: &[facts::PlaceSegment],
    ) -> bool {
        if root == facts::PlaceRoot::Symbol(self.machine.symbol) {
            return false;
        }
        (0..=path.len()).all(|length| {
            let prefix = CanonicalPlace {
                root,
                segments: path[..length].to_vec(),
            };
            validation::canonical_place_type_reference(
                self.program,
                self.state.symbol,
                statement_index,
                &prefix,
            )
            .and_then(|type_reference| reference_access(self.program, type_reference))
            .is_none_or(|access| access == ReferenceAccess::Mutable)
        })
    }
}

/*
    The frame-scan context: the program and the checked operator facts the
    nested arm frames are collected under. The recursive frame walk restated
    this pair at every step; the owner carries it once.
*/
pub(in crate::checks::multiplicity) struct FrameWalk<'a> {
    program: &'a dyn ::validation::CheckingInput,
    operators: &'a checked_trees::CheckedOperatorFacts,
}

impl<'a> FrameWalk<'a> {
    pub(in crate::checks::multiplicity) fn new(
        program: &'a dyn ::validation::CheckingInput,
        operators: &'a checked_trees::CheckedOperatorFacts,
    ) -> Self {
        Self { program, operators }
    }
}

/*
    An authored-facing name for a resolved storage place: `self`-rooted
    storage spells through the receiver, other roots spell their own symbol,
    and segments spell as field/index/case projections.
*/
pub(super) fn place_spelling(
    program: &dyn ::validation::CheckingInput,
    root: facts::PlaceRoot,
    path: &[facts::PlaceSegment],
) -> String {
    let mut spelling = match root {
        facts::PlaceRoot::Symbol(symbol) => program.symbols.name(symbol).to_string(),
        _ => "?".to_string(),
    };
    for segment in path {
        match segment {
            facts::PlaceSegment::Field { symbol } => {
                spelling.push('.');
                spelling.push_str(program.symbols.name(*symbol));
            }
            facts::PlaceSegment::FixedIndex { index } => {
                spelling.push_str(&format!("[{index}]"));
            }
            facts::PlaceSegment::Index { .. } => spelling.push_str("[index]"),
            facts::PlaceSegment::OpaqueRange => spelling.push_str("[?..?]"),
            facts::PlaceSegment::FixedRange { start, end } => {
                spelling.push_str(&format!("[{start}..{end}]"));
            }
            facts::PlaceSegment::Case { variant } => {
                spelling.push_str("::");
                spelling.push_str(program.symbols.name(*variant));
            }
        }
    }
    spelling
}
