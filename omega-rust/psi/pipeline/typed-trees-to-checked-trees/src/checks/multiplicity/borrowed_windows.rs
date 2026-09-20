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

    /// Inspect the invocation only after its operands have evaluated. An
    /// earlier operand may already have detached borrowed content; a later
    /// move must not retroactively fence this call.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn check_invocation(
        &self,
        program: &typed_trees::TypedTrees,
        state: &typed_trees::state::State,
        statement_index: usize,
        expression: ExpressionHandle,
        control: &checked_trees::FlowControlFacts,
        state_calls: &[checked_trees::FlowCallFact],
        service_reaches: &checked_trees::ServiceReachFacts,
        operators: &checked_trees::CheckedOperatorFacts,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if self.open.is_empty() {
            return;
        }
        if state_calls.iter().any(|call| {
            call.statement_index == statement_index
                && call.authored_expression == expression
                && !control.is_retired(state.symbol, call)
                && (call.suspension.direct_may_suspend
                    || call.suspension.transitive_may_suspend
                    || call.blocking.direct_may_block
                    || call.blocking.transitive_may_block)
        }) {
            for absent in &self.open {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot suspend or block at statement {statement_index} while `{}` is \
                     absent: restore the value moved out of borrowed storage at statement \
                     {} first",
                    absent.spelling, absent.opened_statement,
                )));
            }
        }
        if state_calls.iter().any(|call| {
            call.statement_index == statement_index
                && call.authored_expression == expression
                && !control.is_retired(state.symbol, call)
                && call_may_enter_boundary(
                    program,
                    service_reaches,
                    control,
                    operators,
                    state.symbol,
                    call,
                )
        }) || state_has_boundary_operator(
            program,
            control,
            operators,
            state.symbol,
            Some(statement_index),
            Some(expression),
        ) {
            for absent in &self.open {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot make a boundary or service call at statement {statement_index} \
                     while `{}` is absent: restore the value moved out of borrowed storage \
                     at statement {} first",
                    absent.spelling, absent.opened_statement,
                )));
            }
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
        let expected = crate::semantic_calls::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        )
        .map(|site| match site {
            crate::semantic_calls::CallSite::Expression { expression, .. } => expression,
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
fn state_has_boundary_operator(
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

/// Rebase a place written through a `&mut`-typed local onto the referent
/// storage it borrows. `let r = &mut self.f` makes `r.g` and `self.f.g` name
/// the same absent subtree; the loop also follows plain reference copies
/// (`let s = r`), recast chains, and later reassignments of the reference
/// local to the source the route currently captures.
fn resolve_storage_place(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    statement_index: usize,
    root_symbol: SymbolHandle,
    path: &[facts::PlaceSegment],
) -> (facts::PlaceRoot, Vec<facts::PlaceSegment>) {
    let mut root =
        crate::flow::normalized_event_place_root(program, facts::PlaceRoot::Symbol(root_symbol));
    let mut segments = path.to_vec();
    for _ in 0..16 {
        let facts::PlaceRoot::Symbol(symbol) = root else {
            break;
        };
        let Some(source) =
            reference_route_source(program, state, statements, statement_index, symbol)
        else {
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
        root = crate::flow::normalized_event_place_root(program, source.root);
        let mut rebased_place = CanonicalPlace {
            root,
            segments: rebased,
        };
        // A `&mut self.f` capture may record the attached field itself as the
        // root; join it with its receiver storage so it keys like `self.f`.
        crate::flow::normalize_attached_place_root(
            program,
            machine.symbol,
            state.symbol,
            &mut rebased_place,
        );
        root = rebased_place.root;
        segments = rebased_place.segments;
    }
    (root, segments)
}

/// The expression whose place a reference-typed local currently refers to:
/// its initializer, or the value of the latest whole-local reassignment.
/// Returns `None` when the symbol is not a reference-typed local or the route
/// cannot be replayed.
fn reference_route_source(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<ExpressionHandle> {
    let declaration = statements
        .iter()
        .take(statement_index)
        .find_map(|statement| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (local.symbol == symbol).then_some(local)
        })?;
    if !super::linear_obligations::type_reference_is_reference(program, declaration.type_reference)
    {
        return None;
    }
    statements
        .iter()
        .take(statement_index)
        .rev()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some(local.initial_value),
            StatementNode::Assignment(assignment) => {
                let target = crate::flow::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    assignment.target,
                )?;
                (target.root == facts::PlaceRoot::Symbol(symbol) && target.segments.is_empty())
                    .then_some(assignment.value)
            }
            _ => None,
        })
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
    if root == facts::PlaceRoot::Symbol(machine.symbol)
        && program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
            .and_then(|parameter| reference_access(program, parameter.type_reference))
            != Some(ReferenceAccess::Mutable)
    {
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
fn place_spelling(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    root: facts::PlaceRoot,
    path: &[facts::PlaceSegment],
) -> String {
    let mut spelling = match root {
        facts::PlaceRoot::Symbol(symbol)
            if symbol == machine.symbol
                && program
                    .state_parameters(state)
                    .iter()
                    .any(|parameter| parameter.is_self) =>
        {
            "self".to_string()
        }
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

/// Window checks replay evaluated occurrences, not a set of mentioned places.
/// This is scratch ordering for the existing ownership events, not a second
/// ownership classifier. Each observation precedes its exact move; invocations
/// follow their operands, and the caller applies the repair after these steps.
pub(super) enum WindowStep {
    Observe(CanonicalPlace),
    Move { event: usize, conditional: bool },
    Invoke(ExpressionHandle),
}

pub(super) fn statement_steps(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    moves: &[crate::flow::DiscoveredMoveEvent],
    operators: &checked_trees::CheckedOperatorFacts,
) -> Vec<WindowStep> {
    let mut order = WindowOrder {
        program,
        state: state.symbol,
        statement_index,
        operators,
        remaining: moves
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                crate::checks::multiplicity::linear_validation::event_statement_index(event.source)
                    == Some(statement_index)
            })
            .collect(),
        steps: Vec::new(),
    };
    match statement {
        StatementNode::LocalData(local) => order.expression(local.initial_value, false),
        StatementNode::Expression(expression) => order.expression(*expression, false),
        StatementNode::Assignment(assignment) => {
            order.expression(assignment.value, false);
            order.selectors(assignment.target, false);
        }
        StatementNode::Call(call) => {
            if let Some(receiver) = crate::flow::canonical_receiver_place_for_call_site(
                program,
                machine.symbol,
                state.symbol,
                &crate::semantic_calls::CallSite::Statement(call),
                statement_index,
            ) {
                order.steps.push(WindowStep::Observe(receiver));
            }
            // A statement receiver has no expression handle. Its extraction
            // still precedes the first positional argument.
            order.complete_moves(ExpressionHandle::invalid(), false);
            for argument in program.statement_table.expression_handles(call.arguments) {
                order.expression(*argument, false);
            }
            order
                .steps
                .push(WindowStep::Invoke(ExpressionHandle::invalid()));
        }
        StatementNode::RootBinding(binding) => {
            order.expression(binding.receiver, false);
            order.expression(binding.implementation_operand, false);
        }
        StatementNode::AssemblyFact(fact) => order.expression(fact.expression, false),
        StatementNode::Transition(transition) => {
            if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                order.expression(guard, false);
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    typed_trees::statement::TransitionTargetNode::Value(value) => {
                        order.expression(*value, true)
                    }
                    typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
                        for argument in program.expression_table.expression_handles(*arguments) {
                            order.expression(*argument, true);
                        }
                        order
                            .steps
                            .push(WindowStep::Invoke(ExpressionHandle::invalid()));
                    }
                    _ => {}
                }
            }
        }
    }
    // Production destinations have no evaluated source. Their existing caller
    // classification skips them. Any other unmatched move lacks ordering
    // custody and must not gain a borrowed-storage window.
    for (event, _) in order.remaining {
        order.steps.push(WindowStep::Move {
            event,
            conditional: true,
        });
    }
    order.steps
}

struct WindowOrder<'a> {
    program: &'a typed_trees::TypedTrees,
    state: SymbolHandle,
    statement_index: usize,
    operators: &'a checked_trees::CheckedOperatorFacts,
    remaining: Vec<(usize, &'a crate::flow::DiscoveredMoveEvent)>,
    steps: Vec<WindowStep>,
}

impl WindowOrder<'_> {
    fn complete_moves(&mut self, expression: ExpressionHandle, conditional: bool) {
        let mut position = 0;
        while position < self.remaining.len() {
            if self.remaining[position].1.expression == expression {
                let (event, _) = self.remaining.remove(position);
                self.steps.push(WindowStep::Move { event, conditional });
            } else {
                position += 1;
            }
        }
    }

    fn expression(&mut self, expression: ExpressionHandle, conditional: bool) {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Indexed(indexed) if self.selected_index(expression) => {
                self.expression(indexed.collection, conditional);
                self.expression(indexed.index, conditional);
            }
            ExpressionNode::Name(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::Borrow(_) => {
                // Reading a projection observes its maximal selected place,
                // not every enclosing owner. Computed receivers and selectors
                // still execute before that observation.
                self.selectors(expression, conditional);
                if let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                    self.program,
                    self.state,
                    self.statement_index,
                    expression,
                ) {
                    self.steps.push(WindowStep::Observe(place));
                }
            }
            ExpressionNode::Call(call) => {
                self.expression(call.receiver, conditional);
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                {
                    self.expression(*argument, conditional);
                }
            }
            ExpressionNode::Match(dispatch) => {
                self.expression(dispatch.subject, conditional);
                for (_, arm) in super::owned_selection::reachable_arms(self.program, dispatch.arms)
                {
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        self.expression(pattern, true);
                    }
                    self.expression(arm.value, true);
                }
            }
            ExpressionNode::Binary(binary) => {
                self.expression(binary.left, conditional);
                if matches!(
                    binary.operator,
                    typed_trees::expression::BinaryOperator::And
                        | typed_trees::expression::BinaryOperator::Or
                ) {
                    let evaluate_when =
                        binary.operator == typed_trees::expression::BinaryOperator::And;
                    // Only closed selected values can skip a subtree here.
                    // Runtime bindings may have changed during earlier operands.
                    let left = crate::values::evaluate_closed_boolean_expression(
                        self.program,
                        self.operators,
                        binary.left,
                        &[],
                    );
                    if left != Some(!evaluate_when) {
                        self.expression(binary.right, conditional || left.is_none());
                    }
                } else {
                    self.expression(binary.right, conditional);
                }
            }
            ExpressionNode::Unary(unary) => self.expression(unary.operand, conditional),
            ExpressionNode::Cast(cast) => self.expression(cast.value, conditional),
            ExpressionNode::Atomic(atomic) => self.expression(atomic.value, conditional),
            ExpressionNode::ArrayLiteral(values) => {
                for value in self.program.expression_table.expression_handles(*values) {
                    self.expression(*value, conditional);
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in self.program.expression_table.struct_fields(literal.fields) {
                    self.expression(field.value, conditional);
                }
            }
            ExpressionNode::Range(range) => {
                self.expression(range.start, conditional);
                self.expression(range.end, conditional);
            }
            _ => {}
        }
        // Ordinary calls and selected operators use their retained exact
        // invocation rows; syntax alone cannot invent an executed boundary.
        self.steps.push(WindowStep::Invoke(expression));
        self.complete_moves(expression, conditional);
    }

    fn selectors(&mut self, expression: ExpressionHandle, conditional: bool) {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Indexed(_) if self.selected_index(expression) => {
                self.expression(expression, conditional);
            }
            ExpressionNode::Name(_) => {}
            ExpressionNode::Member(member) => self.selectors(member.receiver, conditional),
            ExpressionNode::Indexed(indexed) => {
                self.selectors(indexed.collection, conditional);
                self.expression(indexed.index, conditional);
            }
            ExpressionNode::Borrow(borrow) => self.selectors(borrow.target, conditional),
            _ => self.expression(expression, conditional),
        }
    }

    /// A nominal index operator observes or consumes its collection operand;
    /// only intrinsic place indexing can bypass an enclosing-owner read.
    fn selected_index(&self, expression: ExpressionHandle) -> bool {
        self.operators.resolved_uses().any(|usage| {
            usage.expression == expression
                && usage.occurrence == checked_trees::CheckedOperatorOccurrence::Expression
                && matches!(usage.origin, checked_trees::CheckedValueOrigin::StateStatement {
                    state_symbol, statement_index, ..
                } if state_symbol == self.state && statement_index == self.statement_index)
                && typed_trees::operator::declaration_by_symbol(
                    self.program,
                    usage.selected_operator_symbol,
                )
                .is_some()
        })
    }
}
