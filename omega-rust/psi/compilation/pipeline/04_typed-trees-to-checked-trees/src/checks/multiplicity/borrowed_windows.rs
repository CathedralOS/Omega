//! Borrowed-storage invariant windows through exclusive borrows (ch11).
//!
//! A consuming move out of `&mut`-reachable storage opens a repair obligation
//! on the exact absent place: while the window is open the place is absent
//! from the owner, so every read, borrow, move, or store that touches it or an
//! enclosing owner rejects, and every exit edge (transition or return)
//! requires the obligation discharged. The replacement is an ordinary store
//! of a value carrying the hole's exact type; disjoint sibling work between
//! the extraction and the repair is unrestricted.
//!
//! Windows are keyed on the resolved *storage* place, not the access route:
//! a hole opened through a local `&mut` reborrow (`let r = &mut self.f; let x
//! = r.g`) is the same hole the owner path (`self.f.g = ..`) repairs, because
//! both name one referent subtree. Repair route liveness stays with the
//! ordinary exclusive-loan checks; the window only records that the place is
//! absent until a store of the required type reseats it.
//!
//! Extraction requires a readable exclusive chain: every reference crossed by
//! the moved place's prefixes (the `self` receiver for machine-rooted places,
//! a `&mut` parameter root, or a `&mut` field/local inside the path) must be
//! `&mut`. Shared and write-only links keep the existing plain rejection.
//!
//! Replay observes each evaluated place before applying its exact discovered
//! move, then checks invocations after their operands. Grouping all moves at
//! statement entry rejects earlier reads and calls; exempting all mentions of
//! a moved place hides later stale uses. The occurrence retained by discovery
//! distinguishes those cases without reclassifying ownership from syntax.
//! A repair store runs only after its value and destination selectors finish.

use super::owned_selection::place_paths_overlap;
use crate::flow::CanonicalPlace;
use checked_trees::facts::StorageWindowInvocation;
use diagnostics::Diagnostic;
use language_core::ReferenceAccess;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern};
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// One absent place inside exclusive borrowed storage. `root`/`path` name the
/// resolved storage place (a `&mut`-local route is already rebased onto the
/// referent root); `spelling` is the authored-facing name used in diagnostics.
struct OpenWindow {
    root: facts::PlaceRoot,
    path: Vec<facts::PlaceSegment>,
    spelling: String,
    required_type: TypeReferenceHandle,
    opened_statement: usize,
    /// Set once an exit edge reports the still-open debt; later edges and the
    /// implicit return do not repeat the identical diagnostic.
    exit_reported: bool,
}

#[derive(Default)]
pub(super) struct BorrowedStorageWindows {
    open: Vec<OpenWindow>,
}

impl BorrowedStorageWindows {
    /// Select the still-present old referent after RHS evaluation. A repair
    /// into an exact hole creates no second death; replacing an enclosing
    /// value retains the absent children's exclusions. This is the same
    /// window state that checks reads, moves, invocations and restoration.
    pub(super) fn replacement_death(
        &self,
        program: &typed_trees::TypedTrees,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statements: &[StatementNode],
        statement_index: usize,
        assignment: &typed_trees::statement::TableAssignment,
    ) -> Option<(
        CanonicalPlace,
        TypeReferenceHandle,
        Vec<Vec<facts::PlaceSegment>>,
    )> {
        let target = crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            statement_index,
            assignment.target,
        )?;
        let borrowed = (0..=target.segments.len()).any(|length| {
            let prefix = CanonicalPlace {
                root: target.root,
                segments: target.segments[..length].to_vec(),
            };
            crate::flow::canonical_place_type_reference(
                program,
                state.symbol,
                statement_index,
                &prefix,
            )
            .is_some_and(|reference| {
                super::linear_obligations::type_reference_is_reference(program, reference)
            })
        });
        if !borrowed
            || !exclusive_storage_chain(
                program,
                machine,
                state,
                statement_index,
                target.root,
                &target.segments,
            )
        {
            return None;
        }
        let destination_type = crate::flow::canonical_place_type_reference(
            program,
            state.symbol,
            statement_index,
            &target,
        )?;
        // Death belongs to the old stored type, including its declared
        // refinements and generic substitutions. The RHS proves a new value;
        // it cannot redefine the old value's disposal obligations.
        let value_type = validation::assignment_value_type(program, destination_type);
        if program.type_multiplicity(value_type) != language_semantics::Multiplicity::Affine {
            return None;
        }
        let facts::PlaceRoot::Symbol(symbol) = target.root else {
            return None;
        };
        let (root, path) = resolve_storage_place(
            program,
            machine,
            state,
            statements,
            statement_index,
            symbol,
            &target.segments,
        );
        if self
            .open
            .iter()
            .any(|absent| absent.root == root && path.starts_with(&absent.path))
        {
            return None;
        }
        let excluded = self
            .open
            .iter()
            .filter(|absent| absent.root == root && absent.path.starts_with(&path))
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

    /// The repair obligation an eligible borrowed move creates. Returns the
    /// diagnostic to emit when the transfer cannot open a window — a
    /// non-symbol root, a shared or write-only link in the chain, or an
    /// overlap with an already absent place — and `None` once the window is
    /// recorded.
    pub(super) fn open(
        &mut self,
        program: &typed_trees::TypedTrees,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statements: &[StatementNode],
        statement_index: usize,
        event: &crate::flow::DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
    ) -> Option<Diagnostic> {
        let facts::PlaceRoot::Symbol(root_symbol) = event.root else {
            return Some(borrowed_transfer_diagnostic(
                machine,
                state,
                statement_index,
            ));
        };
        // The authored chain exposes the access of the route itself (a `&` or
        // `&write` local, a shared intermediate field); the resolved chain
        // exposes the referent's receiver and interior links. Both must be
        // exclusively mutable for an extraction to open a window.
        if !exclusive_storage_chain(program, machine, state, statement_index, event.root, path) {
            return Some(borrowed_transfer_diagnostic(
                machine,
                state,
                statement_index,
            ));
        }
        let (root, storage_path) = resolve_storage_place(
            program,
            machine,
            state,
            statements,
            statement_index,
            root_symbol,
            path,
        );
        if !exclusive_storage_chain(
            program,
            machine,
            state,
            statement_index,
            root,
            &storage_path,
        ) {
            return Some(borrowed_transfer_diagnostic(
                machine,
                state,
                statement_index,
            ));
        }
        let spelling = place_spelling(program, machine, state, root, &storage_path);
        if let Some(absent) = self
            .open
            .iter()
            .find(|absent| absent.root == root && place_paths_overlap(&storage_path, &absent.path))
        {
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
        let required_type = crate::flow::canonical_place_type_reference(
            program,
            state.symbol,
            statement_index,
            &event_place,
        )
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

    /// A move of a place that is — or still contains — an absent borrowed
    /// subtree cannot leave: the moved value would carry the hole with it.
    /// This is the ordinary-move twin of `open`'s overlap refusal and covers
    /// whole-owner moves (empty paths) the borrowed gate never sees.
    pub(super) fn refuse_move_over_open(
        &self,
        program: &typed_trees::TypedTrees,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statements: &[StatementNode],
        statement_index: usize,
        event: &crate::flow::DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if self.open.is_empty() {
            return;
        }
        let facts::PlaceRoot::Symbol(root_symbol) = event.root else {
            return;
        };
        let (root, storage_path) = resolve_storage_place(
            program,
            machine,
            state,
            statements,
            statement_index,
            root_symbol,
            path,
        );
        let Some(absent) = self
            .open
            .iter()
            .find(|absent| absent.root == root && place_paths_overlap(&storage_path, &absent.path))
        else {
            return;
        };
        let spelling = place_spelling(program, machine, state, root, &storage_path);
        diagnostics.push(Diagnostic::error(format!(
            "cannot transfer `{spelling}` while `{}` is absent: the value moved out of \
             borrowed storage at statement {} has not been replaced",
            absent.spelling, absent.opened_statement,
        )));
    }

    /// Retain the exact open frontier after operands finish. Cleanup effect
    /// closure is unavailable during source death selection, so invocation
    /// effects consume these occurrences later rather than replaying moves.
    pub(super) fn record_invocation(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        expression: ExpressionHandle,
        cleanup: &mut checked_trees::ContextualCleanupFacts,
    ) {
        for absent in &self.open {
            let segments = cleanup.segments.insert_many(absent.path.iter().copied());
            cleanup.window_invocations.append(StorageWindowInvocation {
                machine,
                state,
                statement,
                expression,
                absent: facts::Place {
                    root: absent.root,
                    segments,
                },
                opened_statement: Some(absent.opened_statement),
            });
        }
    }

    pub(super) fn check_use(
        &self,
        program: &typed_trees::TypedTrees,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statements: &[StatementNode],
        statement_index: usize,
        use_place: &CanonicalPlace,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if self.open.is_empty() {
            return;
        }
        let facts::PlaceRoot::Symbol(root_symbol) = use_place.root else {
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
        let Some(absent) = self
            .open
            .iter()
            .find(|absent| absent.root == root && place_paths_overlap(&storage_path, &absent.path))
        else {
            return;
        };
        let spelling = place_spelling(program, machine, state, root, &storage_path);
        diagnostics.push(Diagnostic::error(format!(
            "cannot use `{spelling}` while `{}` is absent from borrowed storage: the \
                 value moved out at statement {} must be restored first",
            absent.spelling, absent.opened_statement,
        )));
    }

    /// A store whose target is the absent place — or a whole enclosing owner —
    /// discharges the obligation when the stored value carries the hole's
    /// exact type. Any other overlapping store touches absent storage.
    pub(super) fn check_repair(
        &mut self,
        program: &typed_trees::TypedTrees,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statements: &[StatementNode],
        statement_index: usize,
        assignment: &typed_trees::statement::TableAssignment,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(target) = crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            statement_index,
            assignment.target,
        ) else {
            return;
        };
        let facts::PlaceRoot::Symbol(root_symbol) = target.root else {
            return;
        };
        let (root, target_path) = resolve_storage_place(
            program,
            machine,
            state,
            statements,
            statement_index,
            root_symbol,
            &target.segments,
        );
        let mut repaired = Vec::new();
        for (index, absent) in self.open.iter().enumerate() {
            if absent.root != root || !place_paths_overlap(&target_path, &absent.path) {
                continue;
            }
            if absent.path.starts_with(target_path.as_slice()) {
                repaired.push(index);
                continue;
            }
            let spelling = place_spelling(program, machine, state, root, &target_path);
            diagnostics.push(Diagnostic::error(format!(
                "cannot store `{spelling}` while `{}` is absent: the value moved out of \
                 borrowed storage at statement {} must be restored as a whole",
                absent.spelling, absent.opened_statement,
            )));
        }
        for index in repaired.into_iter().rev() {
            let absent = self.open.remove(index);
            if absent.path.len() != target_path.len() {
                // A whole-enclosing-owner store reseats every absent place
                // beneath it; upstream assignment typing already pinned the
                // stored value's type to the owner's.
                continue;
            }
            let value_type = validation::expression_result_type_reference(
                program,
                machine,
                state,
                assignment.value,
            );
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

    /// Every exit edge — each transition arm and the result expression — hands
    /// the borrowed owner back; every open window must be discharged first.
    /// The diagnostics name the opening move and the absent place. Each window
    /// reports once: additional arms leaving the same region add no noise.
    pub(super) fn refuse_open_at_exit(
        &mut self,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
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

/// The expression coordinate is checked evidence, not permission to relocate
/// an invocation outside a restoration window. Rejoin it to the authored call
/// ordinal before occurrence-ordered replay uses the row. Retired calls belong
/// to their selected replacements and are not executed again here.
pub(super) fn check_call_occurrences(
    program: &typed_trees::TypedTrees,
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
        let expected = crate::semantic::calls::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        )
        .map(|site| match site {
            crate::semantic::calls::CallSite::Expression { expression, .. } => expression,
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

/// Complete invocation checks after selected cleanup closes the effect graph.
/// Source recording retained each actual open frontier, including match-arm
/// occurrences. This consumes final call effects without reapplying any move
/// or reconstructing restoration from the later, repaired storage state.
pub(super) fn check_recorded_invocations(
    program: &typed_trees::TypedTrees,
    facts: &checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for (_, invocation) in facts.contextual_cleanup.window_invocations.iter() {
        let Some(machine) = crate::lookup::machine_by_symbol(program, invocation.machine) else {
            return Err(vec![Diagnostic::error(
                "borrowed-storage invocation has no owning machine",
            )]);
        };
        let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == invocation.state)
        else {
            return Err(vec![Diagnostic::error(
                "borrowed-storage invocation has no owning state",
            )]);
        };
        let Some(flow) = facts.flow.control.states.iter().find_map(|(_, flow)| {
            (flow.machine_symbol == invocation.machine && flow.state_symbol == invocation.state)
                .then_some(flow)
        }) else {
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
        let spelling = place_spelling(program, machine, state, invocation.absent.root, path);
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
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Service reach and boundary invocation are different facts: an opaque
/// boundary may declare empty reach. Follow the retained exact call topology
/// through ordinary wrappers as well, rather than treating that empty row (or
/// an empty write frame, which says nothing about reads) as non-observation.
/// This query runs only for calls crossing an open window. Its local visited
/// set closes cycles without inventing a new effect row or expanding paths.
fn call_may_enter_boundary(
    program: &typed_trees::TypedTrees,
    service_reaches: &checked_trees::ServiceReachFacts,
    control: &checked_trees::FlowControlFacts,
    operators: &checked_trees::CheckedOperatorFacts,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
) -> bool {
    let empty = language_semantics::ServiceReachRowTable::EMPTY_ROW;
    // The row-table reader maps unknown IDs to an empty slice. Only the
    // canonical empty identity establishes absence of service reach.
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
        // Compiler-owned builtins have no machine body. Resolve their exact
        // symbol, not the authored spelling; service-bearing assembly remains
        // a fence even if its retained reach row were missing that service.
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
        let Some(machine) = crate::lookup::machine_by_symbol(program, target) else {
            return true;
        };
        if !machine.supply_mode.is_checked_body() || !machine.body_is_present {
            return true;
        }
        let Some(reach) = service_reaches.for_machine(target) else {
            return true;
        };
        for state in service_reaches.states_for(reach) {
            if state_has_boundary_operator(program, control, operators, state.state, None, None) {
                return true;
            }
            pending.extend(service_reaches.calls_for(state));
        }
    }
    false
}

/// Operator invocations are scheduled separately from ordinary calls. Use
/// those records for both named and spelled operators so skipped operands do
/// not become calls, and wrappers cannot hide an opaque operator invocation.
pub(super) fn state_has_boundary_operator(
    program: &typed_trees::TypedTrees,
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

/// The shared diagnostic for a borrowed-storage transfer that cannot open or
/// complete a window.
pub(super) fn borrowed_transfer_diagnostic(
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

/// The exclusive-route half of [`BorrowedStorageWindows::open`] keyed on the
/// authored spelling: sibling arms may name the same referent storage through
/// different routes (a `&mut` local versus the owner path), so each arm's
/// route is checked on its own links even though the join opens one hole.
pub(super) fn authored_route_exclusive(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    root: facts::PlaceRoot,
    path: &[facts::PlaceSegment],
) -> bool {
    matches!(root, facts::PlaceRoot::Symbol(_))
        && exclusive_storage_chain(program, machine, state, statement_index, root, path)
}

type ArmHandle = arena::Handle<typed_trees::expression::TableMatchArm>;

fn arm_key(handle: ArmHandle) -> (u32, u32) {
    (handle.arena_index(), handle.generation())
}

fn expression_key(handle: ExpressionHandle) -> (u32, u32) {
    (handle.arena_index(), handle.generation())
}

/// One `match` expression inside a statement's value evaluation.
struct MatchWindowFrame {
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
pub(super) enum ArmWindowVerdict {
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
pub(super) struct ArmWindowPlan {
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

impl ArmWindowPlan {
    /// Collect the statement's `match` frames and the expression→arm map.
    /// The walk mirrors `statement_steps`: subjects and patterns evaluate in
    /// the enclosing context, arm values in their arm, and short-circuit
    /// right operands stay conditional on both their edge and the statement.
    pub(super) fn new(
        program: &typed_trees::TypedTrees,
        statement: &StatementNode,
        operators: &checked_trees::CheckedOperatorFacts,
    ) -> Self {
        let mut plan = Self {
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
                let arm = ArmHandle::from_parts(
                    frame.arms.start().arena_index() + ordinal as u32,
                    frame.arms.start().generation(),
                );
                plan.frame_by_arm.insert(arm_key(arm), index);
            }
        }
        plan
    }

    /// Whether the statement contains any `match` worth planning over.
    pub(super) fn has_frames(&self) -> bool {
        !self.frames.is_empty()
    }

    /// Record one eligible arm-scoped borrowed move: a non-empty borrowed
    /// place path that is neither the statement's production target nor a
    /// nominal-drop crossing. Unattributed events resolve through the
    /// expression→arm map (call-flow argument moves).
    pub(super) fn add_borrowed_move(
        &mut self,
        program: &typed_trees::TypedTrees,
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
        let facts::PlaceRoot::Symbol(root_symbol) = event.root else {
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
    pub(super) fn seal(&mut self) {
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
                let mut unique = debts.clone();
                unique.dedup();
                unique.len() != debts.len()
            });
            if !rejected {
                for (position, arm) in self.frames[index].reachable.iter().enumerate() {
                    match contributions.remove(&arm_key(*arm)) {
                        Some(Some(inner)) => {
                            for debt in inner {
                                if !arm_sets[position].contains(&debt) {
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
                rejected = arm_sets.iter().any(|debts| !same_place_set(debts, first));
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
    pub(super) fn verdict(
        &mut self,
        program: &typed_trees::TypedTrees,
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
        let facts::PlaceRoot::Symbol(root_symbol) = event.root else {
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
        if self
            .opened
            .contains(&(frame_index, resolved.0, resolved.1.clone()))
            || self.pending.iter().any(|(pending_root, _, proot, ppath)| {
                *pending_root == root && *proot == resolved.0 && *ppath == resolved.1
            })
        {
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
    pub(super) fn commit_pending(&mut self, expression: ExpressionHandle) -> Vec<usize> {
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
    pub(super) fn check_edge_use(
        &self,
        program: &typed_trees::TypedTrees,
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
        let facts::PlaceRoot::Symbol(root_symbol) = use_place.root else {
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
                || !place_paths_overlap(&storage_path, absent_path)
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
    pub(super) fn record_edge_invocation(
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
        program: &typed_trees::TypedTrees,
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
        let Some(crate::semantic::calls::CallSite::Expression { expression, .. }) =
            crate::semantic::calls::find_call_site(
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

    /// The expression whose evaluation position carries this event: the
    /// recorded operand, or the resolved call site for call-flow events that
    /// name a statement-local call by ordinal instead.
    fn event_site_expression(
        &self,
        program: &typed_trees::TypedTrees,
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
        match crate::semantic::calls::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            call_ordinal,
        ) {
            Some(crate::semantic::calls::CallSite::Expression { expression, .. }) => expression,
            _ => ExpressionHandle::invalid(),
        }
    }

    fn collect_frames(
        &mut self,
        program: &typed_trees::TypedTrees,
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
            ExpressionNode::Match(dispatch) => {
                self.frames.push(MatchWindowFrame {
                    expression,
                    arms: dispatch.arms,
                    reachable: super::owned_selection::reachable_arms(program, dispatch.arms)
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
                    let arm_handle = ArmHandle::from_parts(
                        dispatch.arms.start().arena_index() + ordinal as u32,
                        dispatch.arms.start().generation(),
                    );
                    if let MatchPattern::Value(pattern) = arm.pattern {
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
            ExpressionNode::Name(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
}

/// The join-agreement comparison: two arms carry the same resolved debts,
/// regardless of the order their moves recorded them in.
fn same_place_set(
    left: &[(facts::PlaceRoot, Vec<facts::PlaceSegment>)],
    right: &[(facts::PlaceRoot, Vec<facts::PlaceSegment>)],
) -> bool {
    left.len() == right.len() && left.iter().all(|place| right.contains(place))
}

/// Rebase a place written through a `&mut`-typed local onto the referent
/// storage it borrows. `let r = &mut self.f` makes `r.g` and `self.f.g` name
/// the same absent subtree; the loop also follows plain reference copies
/// (`let s = r`), recast chains, and later reassignments of the reference
/// local to the source the route currently captures.
fn resolve_storage_place(
    program: &typed_trees::TypedTrees,
    _machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    statement_index: usize,
    root_symbol: SymbolHandle,
    path: &[facts::PlaceSegment],
) -> (facts::PlaceRoot, Vec<facts::PlaceSegment>) {
    let mut root = facts::PlaceRoot::Symbol(root_symbol);
    let mut segments = path.to_vec();
    for _ in 0..16 {
        let facts::PlaceRoot::Symbol(symbol) = root else {
            break;
        };
        let Some(source) = crate::flow::reference_route_source(
            program,
            state,
            statements,
            statement_index,
            symbol,
        ) else {
            break;
        };
        let Some(source) = reference_source_place(program, state.symbol, statement_index, source)
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
        // A `&mut self.f` capture may record the attached field itself as the
        // root; join it with its receiver storage so it keys like `self.f`.

        root = rebased_place.root;
        segments = rebased_place.segments;
    }
    (root, segments)
}

/// The referent place a reference initializer captures: explicit borrows and
/// whole-place recasts unwrap to their target, and a plain place expression
/// names the place the new local aliases.
fn reference_source_place(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    initializer: ExpressionHandle,
) -> Option<CanonicalPlace> {
    let mut expression = initializer;
    loop {
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            ExpressionNode::Cast(cast) if cast.form.is_recast() => expression = cast.value,
            _ => break,
        }
    }
    crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )
}

/// The access of a possibly-constrained reference type.
fn reference_access(
    program: &typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<ReferenceAccess> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { access, .. } => Some(*access),
        TypeReferenceNode::Constrained { base_type, .. } => reference_access(program, *base_type),
        _ => None,
    }
}

/// Every reference crossed by the extracted place must be `&mut`: the `self`
/// receiver for machine-rooted storage, then each proper prefix of the path
/// (a `&mut` parameter root, a `&mut` field inside owned storage, a `&mut`
/// local the window resolved through). Shared or write-only links cannot
/// carry a restoration window.
fn exclusive_storage_chain(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    root: facts::PlaceRoot,
    path: &[facts::PlaceSegment],
) -> bool {
    if root == facts::PlaceRoot::Symbol(machine.symbol) {
        return false;
    }
    (0..path.len()).all(|length| {
        let prefix = CanonicalPlace {
            root,
            segments: path[..length].to_vec(),
        };
        crate::flow::canonical_place_type_reference(program, state.symbol, statement_index, &prefix)
            .and_then(|type_reference| reference_access(program, type_reference))
            .is_none_or(|access| access == ReferenceAccess::Mutable)
    })
}

/// An authored-facing name for a resolved storage place: `self`-rooted
/// storage spells through the receiver, other roots spell their own symbol,
/// and segments spell as field/index/case projections.
pub(super) fn place_spelling(
    program: &typed_trees::TypedTrees,
    _machine: &typed_trees::machine::Machine,
    _state: &typed_trees::state::State,
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
