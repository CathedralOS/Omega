//! arm windows slice of borrowed_windows.

use validation::CanonicalPlace;

use super::{place_spelling, resolve_storage_place, same_place_set, same_storage_debt};

use crate::checks::borrows::{StorageSelectorFacts, storage_path_compatibility};

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

/// One `match` expression inside a statement's value evaluation.
pub(super) struct MatchWindowFrame {
    /// The match expression itself: its `Invoke` step crosses the step stream
    /// exactly once, after every reachable arm's own steps — the join point
    /// where an agreeing arm debt becomes one hole on the joined edge.
    expression: ExpressionHandle,
    /// The arm span is the frame's identity and its membership roster.
    arms: arena::HandleSpan<typed_trees::expression::TableMatchArm>,
    /// Arms that can evaluate: shadowed patterns and post-covered arms never
    /// run, so agreement holds them to nothing.
    reachable: Vec<ArmHandle>,
    /// The arm whose value subtree encloses this match, invalid for a
    /// statement-rooted one. A nested match that agrees lifts its debt into
    /// this arm's set.
    enclosing_arm: ArmHandle,
    /// Whether the match sits in a statement-conditional position — a
    /// short-circuit operand or transition target that may not evaluate. A
    /// conditional join cannot promise the hole on the statement's
    /// fall-through edge. Arm membership is not conditional in this sense:
    /// the arm's own edges are the frame's domain.
    conditional: bool,
}

/// What the event loop should do with one arm-attributed borrowed move.
pub(in crate::checks::multiplicity) enum ArmWindowVerdict {
    /// Every reachable arm of the enclosing match agrees on this absent
    /// place. The hole is recorded as pending and commits at the frame's own
    /// `Invoke` step — the join — so sibling arms' uses of the place still
    /// see it present on their own edge.
    Open,
    /// A sibling arm already opened this place through this match — the
    /// debts agree, so the join carries one hole — but this event's authored
    /// route must still be an exclusive chain.
    SiblingRoute,
    /// The join edges disagree (or the move sits under a match that could not
    /// establish agreement): keep the plain rejection.
    Reject,
    /// The move evaluates inside an arm that can never be selected.
    DeadArm,
}

/// Per-statement agreement over moves evaluated inside `match` arms.
///
/// A move on one arm's edge cannot be repaired before the match joins, so the
/// edges must agree on the absent places they leave open: an arm-attributed
/// borrowed move opens a window only when every reachable arm resolves the
/// same absent storage places, including debts an agreeing nested match lifts
/// into the arm that evaluates it. Disagreement keeps the plain rejection
/// from the unconditional gate it replaces.
pub(in crate::checks::multiplicity) struct ArmWindowPlan<'facts> {
    captures: StorageSelectorFacts<'facts>,
    /// Every `match` in the statement, pre-order: an enclosing match's frame
    /// precedes the frames nested inside its arms.
    frames: Vec<MatchWindowFrame>,
    /// arm handle identity → frame index, for every arm of every frame.
    frame_by_arm: std::collections::BTreeMap<(u32, u32), usize>,
    /// expression handle identity → the innermost arm whose value subtree
    /// contains it. Call-flow move events arrive unattributed; their call
    /// sites resolve through this map to the evaluating arm.
    arm_of_expression: std::collections::BTreeMap<(u32, u32), ArmHandle>,
    /// Resolved storage places each arm's borrowed moves open, keyed by the
    /// arm's handle identity.
    arm_debts:
        std::collections::BTreeMap<(u32, u32), Vec<(facts::PlaceRoot, Vec<facts::PlaceSegment>)>>,
    /// frame index → every arm on its enclosing chain is reachable.
    live: Vec<bool>,
    /// frame index → its reachable arms agreed on identical debts.
    agreed: Vec<bool>,
    /// frame index → this match agreed and every match enclosing it agreed:
    /// the join debt is consistent on the whole path to the statement. A
    /// nested match that agrees inside a disagreeing arm cannot open the
    /// hole — the outer join would still leave one edge without it.
    rooted_agreed: Vec<bool>,
    /// Resolved debts this plan already opened per frame — sibling arms of an
    /// agreeing match contribute one hole per place, not one hole per event.
    opened: Vec<(usize, facts::PlaceRoot, Vec<facts::PlaceSegment>)>,
    /// frame index → the agreed debt set its reachable arms carry.
    agreed_debts: Vec<Vec<(facts::PlaceRoot, Vec<facts::PlaceSegment>)>>,
    /// frame index → the match itself sits in a statement-conditional
    /// position, or a match enclosing it does. Pending debts then can never
    /// commit to the statement edge.
    rooted_conditional: Vec<bool>,
    /// Places already absent on one arm's own edge, in step order, as
    /// (recording arm, resolved root, resolved path). A sibling arm's hole
    /// does not appear on this edge — only the join shares it.
    edge_absent: Vec<(ArmHandle, facts::PlaceRoot, Vec<facts::PlaceSegment>)>,
    /// Agreed debts awaiting their root frame's join: (root frame index,
    /// move event index, resolved place). The caller commits them through
    /// `open` when the match's `Invoke` step crosses; the place fields dedupe
    /// one hole per place at the join even when several frames contributed.
    pending: Vec<(usize, usize, facts::PlaceRoot, Vec<facts::PlaceSegment>)>,
    /// Expressions evaluated conditionally on their own arm's edge — a
    /// short-circuit operand inside the arm's value, for example. A move
    /// there cannot promise the hole on that edge.
    edge_conditional: std::collections::BTreeSet<(u32, u32)>,
}

impl<'facts> ArmWindowPlan<'facts> {
    /// Collect the statement's `match` frames and the expression→arm map.
    /// The walk mirrors `statement_steps`: subjects and patterns evaluate in
    /// the enclosing context, arm values in their arm, and short-circuit
    /// right operands stay conditional on both their edge and the statement.
    pub(in crate::checks::multiplicity) fn new(
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
            StatementNode::Call(call) => {
                roots.extend_from_slice(program.statement_table.expression_handles(call.arguments))
            }
            StatementNode::RootBinding(binding) => {
                roots.push(binding.receiver);
                if binding.implementation_operand.is_valid() {
                    roots.push(binding.implementation_operand);
                }
            }
            _ => {}
        }
        for root in roots {
            plan.collect_frames(program, root, ArmHandle::invalid(), false, false, operators);
        }
        for (index, frame) in plan.frames.iter().enumerate() {
            for ordinal in 0..frame.arms.len() {
                let arm = frame.arms.handle_at(ordinal as u32);
                plan.frame_by_arm.insert(arm_key(arm), index);
            }
        }
        plan
    }

    /// Whether the statement contains any `match` worth planning over.
    pub(in crate::checks::multiplicity) fn has_frames(&self) -> bool {
        !self.frames.is_empty()
    }

    pub(in crate::checks::multiplicity) fn has_edge_absent(&self) -> bool {
        !self.edge_absent.is_empty()
    }

    /// Record one eligible arm-scoped borrowed move: a non-empty borrowed
    /// place path that is neither the statement's production target nor a
    /// nominal-drop crossing. Unattributed events resolve through the
    /// expression→arm map (call-flow argument moves).
    pub(in crate::checks::multiplicity) fn add_borrowed_move(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statements: &[StatementNode],
        statement_index: usize,
        event: &crate::flow::DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
    ) {
        let arm = self.effective_arm(program, machine, state, event);
        if !arm.is_valid() {
            return;
        }
        let Some(root_symbol) = event.root.as_symbol() else {
            return;
        };
        self.arm_debts
            .entry(arm_key(arm))
            .or_default()
            .push(resolve_storage_place(
                program,
                machine,
                state,
                statements,
                statement_index,
                root_symbol,
                path,
            ));
    }

    /// Compute liveness and agreement. `live` follows the frame's enclosing
    /// chain (pre-order visits parents first); agreement runs innermost-first
    /// so each nested match contributes its agreed debt to the arm that
    /// evaluates it, and a nested disagreement poisons that arm.
    pub(in crate::checks::multiplicity) fn seal(
        &mut self,
        program: &dyn ::validation::CheckingInput,
    ) {
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
        // enclosing arm handle identity → the nested frame's contribution:
        // `Some` is its agreed debt set, `None` a nested disagreement that
        // keeps the arm's set from ever matching its siblings.
        let mut contributions: std::collections::BTreeMap<
            (u32, u32),
            Option<Vec<(facts::PlaceRoot, Vec<facts::PlaceSegment>)>>,
        > = std::collections::BTreeMap::new();
        for index in (0..count).rev() {
            if !self.live[index] {
                continue;
            }
            let mut arm_sets: Vec<Vec<(facts::PlaceRoot, Vec<facts::PlaceSegment>)>> = self.frames
                [index]
                .reachable
                .iter()
                .map(|arm| {
                    self.arm_debts
                        .get(&arm_key(*arm))
                        .cloned()
                        .unwrap_or_default()
                })
                .collect();
            // A second extraction of the same resolved place inside one arm
            // is a repeated-extraction on that path, not a sibling debt.
            let mut rejected = arm_sets.iter().any(|debts| {
                debts.iter().enumerate().any(|(position, debt)| {
                    debts[..position].iter().any(|prior| {
                        same_storage_debt(
                            program,
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
                                    same_storage_debt(
                                        program,
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
                    .any(|debts| !same_place_set(program, self.captures, debts, first));
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
        // An event opens only when its frame and every frame enclosing it
        // agreed; pre-order means each parent is already resolved.
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

    /// Decide what one arm-scoped borrowed move does under the computed
    /// agreement. `None` marks an unconditional event — the caller keeps the
    /// ordinary `open` path. An `Open` verdict records the debt against this
    /// arm's own edge immediately and stages the join-level window under its
    /// root frame's pending list; `commit_pending` drains it when the match's
    /// `Invoke` crosses, so a sibling arm's reads still see the place present.
    pub(in crate::checks::multiplicity) fn verdict(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statements: &[StatementNode],
        statement_index: usize,
        event_index: usize,
        event: &crate::flow::DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
    ) -> Option<ArmWindowVerdict> {
        let arm = self.effective_arm(program, machine, state, event);
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
        // A move under a short-circuit operand inside its own arm cannot
        // promise the hole on that arm's edge, and a match whose own position
        // is statement-conditional cannot promise it on the joined edge.
        let site = self.event_site_expression(program, machine, state, event);
        if (site.is_valid() && self.edge_conditional.contains(&expression_key(site)))
            || !self.rooted_agreed[frame_index]
            || self.rooted_conditional[frame_index]
        {
            return Some(ArmWindowVerdict::Reject);
        }
        let Some(root_symbol) = event.root.as_symbol() else {
            return Some(ArmWindowVerdict::Reject);
        };
        let resolved = resolve_storage_place(
            program,
            machine,
            state,
            statements,
            statement_index,
            root_symbol,
            path,
        );
        self.edge_absent.push((arm, resolved.0, resolved.1.clone()));
        let root = self.root_frame(frame_index);
        if self.opened.iter().any(|(frame, root, path)| {
            *frame == frame_index
                && same_storage_debt(
                    program,
                    self.captures,
                    (*root, path),
                    (resolved.0, &resolved.1),
                )
        }) || self.pending.iter().any(|(pending_root, _, proot, ppath)| {
            *pending_root == root
                && same_storage_debt(
                    program,
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

    /// The root (statement-level) frame an agreeing move's pending hole
    /// commits under: nested agreement lifts debts upward, so the hole exists
    /// on the statement edge only once the outermost match joins.
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

    /// Drain the pending debts of the root frame whose `Invoke` step this
    /// `expression` is, returning their move event indices so the caller can
    /// run them through `open`. Nested frames' debts already live in the
    /// root's pending list, so one join commits the whole lifted set.
    pub(in crate::checks::multiplicity) fn commit_pending(
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

    /// Whether a hole recorded on `recorded_arm`'s edge is also present on
    /// `arm`'s edge: the recording arm is the query arm itself or an arm of a
    /// match nested inside its value subtree — the nested join leaves the
    /// hole behind for the rest of the enclosing edge.
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

    /// The arm-local counterpart of [`BorrowedStorageWindows::check_use`]: a
    /// read inside an arm's edge while that edge's own extraction is absent.
    /// Sibling edges' holes never apply, and the join-level ledger entry
    /// exists only after the match commits.
    pub(in crate::checks::multiplicity) fn check_edge_use(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statements: &[StatementNode],
        statement_index: usize,
        expression: ExpressionHandle,
        use_place: &CanonicalPlace,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if self.edge_absent.is_empty() {
            return;
        }
        let Some(&arm) = self.arm_of_expression.get(&expression_key(expression)) else {
            return;
        };
        if !arm.is_valid() {
            return;
        }
        let Some(root_symbol) = use_place.root.as_symbol() else {
            return;
        };
        let (root, storage_path) = resolve_storage_place(
            program,
            machine,
            state,
            statements,
            statement_index,
            root_symbol,
            &use_place.segments,
        );
        for (edge_arm, absent_root, absent_path) in &self.edge_absent {
            if !self.edge_covers(*edge_arm, arm)
                || *absent_root != root
                || !storage_path_compatibility(program, self.captures, &storage_path, absent_path).0
            {
                continue;
            }
            let spelling = place_spelling(program, machine, state, root, &storage_path);
            let absent_spelling =
                place_spelling(program, machine, state, *absent_root, absent_path);
            diagnostics.push(Diagnostic::error(format!(
                "cannot use `{spelling}` while `{absent_spelling}` is absent from borrowed \
                 storage: the value moved out at statement {statement_index} must be restored first",
            )));
        }
    }

    /// Preserve each selected arm's open frontier at its own invocation.
    /// Sibling arms do not lend their holes to this evaluated occurrence.
    pub(in crate::checks::multiplicity) fn record_edge_invocation(
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

    /// The arm this move evaluates on: the recorded `source_arm`, or — for
    /// call-flow events, which the discovery sink emits unattributed — the
    /// arm whose subtree contains the resolved call site.
    fn effective_arm(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        event: &crate::flow::DiscoveredMoveEvent,
    ) -> ArmHandle {
        if event.source_arm.is_valid() {
            return event.source_arm;
        }
        let crate::flow::FlowOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            ..
        } = event.source
        else {
            return ArmHandle::invalid();
        };
        let Some(validation::CallSite::Expression { expression, .. }) = validation::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            call_ordinal,
        ) else {
            return ArmHandle::invalid();
        };
        self.arm_of_expression
            .get(&expression_key(expression))
            .copied()
            .unwrap_or_default()
    }

    /// The expression whose evaluation position carries this event: the
    /// recorded operand, or the resolved call site for call-flow events that
    /// name a statement-local call by ordinal instead.
    fn event_site_expression(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        event: &crate::flow::DiscoveredMoveEvent,
    ) -> ExpressionHandle {
        if event.expression.is_valid() {
            return event.expression;
        }
        let crate::flow::FlowOwnershipEventSource::Call {
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
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        enclosing_arm: ArmHandle,
        statement_conditional: bool,
        edge_conditional: bool,
        operators: &checked_trees::CheckedOperatorFacts,
    ) {
        if !expression.is_valid() {
            return;
        }
        self.arm_of_expression
            .insert(expression_key(expression), enclosing_arm);
        if edge_conditional {
            self.edge_conditional.insert(expression_key(expression));
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::Match(dispatch) => {
                self.frames.push(MatchWindowFrame {
                    expression,
                    arms: dispatch.arms,
                    reachable: super::super::owned_selection::reachable_arms(
                        program,
                        dispatch.arms,
                    )
                    .into_iter()
                    .map(|(handle, _)| handle)
                    .collect(),
                    enclosing_arm,
                    conditional: statement_conditional,
                });
                self.collect_frames(
                    program,
                    dispatch.subject,
                    enclosing_arm,
                    statement_conditional,
                    edge_conditional,
                    operators,
                );
                for (ordinal, arm) in program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .enumerate()
                {
                    let arm_handle = dispatch.arms.handle_at(ordinal as u32);
                    for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                        self.collect_frames(
                            program,
                            pattern,
                            enclosing_arm,
                            true,
                            edge_conditional,
                            operators,
                        );
                    }
                    // The arm's own subtree is a fresh edge: statement
                    // conditionality carries down, but the arm's selection
                    // is this frame's agreement domain, not an edge
                    // condition.
                    self.collect_frames(
                        program,
                        arm.value,
                        arm_handle,
                        statement_conditional,
                        false,
                        operators,
                    );
                }
            }
            ExpressionNode::Atomic(atomic) => {
                self.collect_frames(
                    program,
                    atomic.value,
                    enclosing_arm,
                    statement_conditional,
                    edge_conditional,
                    operators,
                );
                self.collect_frames(
                    program,
                    atomic.result,
                    enclosing_arm,
                    statement_conditional,
                    edge_conditional,
                    operators,
                );
            }
            ExpressionNode::ArrayLiteral(values) => {
                for value in program.expression_table.expression_handles(*values) {
                    self.collect_frames(
                        program,
                        *value,
                        enclosing_arm,
                        statement_conditional,
                        edge_conditional,
                        operators,
                    );
                }
            }
            ExpressionNode::Binary(binary) => {
                self.collect_frames(
                    program,
                    binary.left,
                    enclosing_arm,
                    statement_conditional,
                    edge_conditional,
                    operators,
                );
                if matches!(
                    binary.operator,
                    typed_trees::expression::BinaryOperator::And
                        | typed_trees::expression::BinaryOperator::Or
                ) {
                    let evaluate_when =
                        binary.operator == typed_trees::expression::BinaryOperator::And;
                    let left = crate::values::evaluate_closed_boolean_expression(
                        program,
                        operators,
                        binary.left,
                        &[],
                    );
                    if left != Some(!evaluate_when) {
                        self.collect_frames(
                            program,
                            binary.right,
                            enclosing_arm,
                            statement_conditional || left.is_none(),
                            edge_conditional || left.is_none(),
                            operators,
                        );
                    }
                } else {
                    self.collect_frames(
                        program,
                        binary.right,
                        enclosing_arm,
                        statement_conditional,
                        edge_conditional,
                        operators,
                    );
                }
            }
            ExpressionNode::Borrow(borrow) => {
                self.collect_frames(
                    program,
                    borrow.target,
                    enclosing_arm,
                    statement_conditional,
                    edge_conditional,
                    operators,
                );
            }
            ExpressionNode::Call(call) => {
                self.collect_frames(
                    program,
                    call.receiver,
                    enclosing_arm,
                    statement_conditional,
                    edge_conditional,
                    operators,
                );
                for argument in program.expression_table.expression_handles(call.arguments) {
                    self.collect_frames(
                        program,
                        *argument,
                        enclosing_arm,
                        statement_conditional,
                        edge_conditional,
                        operators,
                    );
                }
            }
            ExpressionNode::Cast(cast) => {
                self.collect_frames(
                    program,
                    cast.value,
                    enclosing_arm,
                    statement_conditional,
                    edge_conditional,
                    operators,
                );
            }
            ExpressionNode::Indexed(indexed) => {
                self.collect_frames(
                    program,
                    indexed.collection,
                    enclosing_arm,
                    statement_conditional,
                    edge_conditional,
                    operators,
                );
                self.collect_frames(
                    program,
                    indexed.index,
                    enclosing_arm,
                    statement_conditional,
                    edge_conditional,
                    operators,
                );
            }
            ExpressionNode::Member(member) => {
                self.collect_frames(
                    program,
                    member.receiver,
                    enclosing_arm,
                    statement_conditional,
                    edge_conditional,
                    operators,
                );
            }
            ExpressionNode::Range(range) => {
                self.collect_frames(
                    program,
                    range.start,
                    enclosing_arm,
                    statement_conditional,
                    edge_conditional,
                    operators,
                );
                self.collect_frames(
                    program,
                    range.end,
                    enclosing_arm,
                    statement_conditional,
                    edge_conditional,
                    operators,
                );
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    self.collect_frames(
                        program,
                        field.value,
                        enclosing_arm,
                        statement_conditional,
                        edge_conditional,
                        operators,
                    );
                }
            }
            ExpressionNode::Unary(unary) => {
                self.collect_frames(
                    program,
                    unary.operand,
                    enclosing_arm,
                    statement_conditional,
                    edge_conditional,
                    operators,
                );
            }
            ExpressionNode::Name(_) | typed_trees::spelled_literal_expression_pattern!() => {}
        }
    }
}
