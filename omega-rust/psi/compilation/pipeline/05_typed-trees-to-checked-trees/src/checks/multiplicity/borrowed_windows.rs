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

mod arm_windows;
mod boundary_checks;
mod windows;

enum ProjectionStep {
    Field,
    Index {
        symbolic: bool,
        expression: typed_trees::expression::ExpressionHandle,
    },
}

/// Field and index steps of one place expression, root symbol first.
/// Borrows and recasts are transparent. A call anywhere in an index operand
/// keeps that selector authored (`[index]`) even after its value is proved.
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

/// A proved selector and the fixed element it selected are the same debt.
/// Field and case segments must still match; a literal index must match its
/// numeral. An unresolved `Index` matches only the fixed element custody
/// recorded for that same move.
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

/// The shared storage-resolution walk context: the program, the machine/state
/// pair under evaluation, and that state's statement list. Every resolution
/// helper restated this tuple at its call site, and each window owner restated
/// it again on every method; the walk carries it once.
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

    /// The join-agreement comparison: two arms carry the same resolved debts,
    /// regardless of the order their moves recorded them in.
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

    /// Rebase a place written through a `&mut`-typed local onto the referent
    /// storage it borrows. `let r = &mut self.f` makes `r.g` and `self.f.g` name
    /// the same absent subtree; the loop also follows plain reference copies
    /// (`let s = r`), recast chains, and later reassignments of the reference
    /// local to the source the route currently captures.
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
            // A `&mut self.f` capture may record the attached field itself as the
            // root; join it with its receiver storage so it keys like `self.f`.

            root = rebased_place.root;
            segments = rebased_place.segments;
        }
        (root, segments)
    }

    /// Diagnostic spelling of a restoration debt.
    ///
    /// Custody freezes a proved selector (`first_slot()` ensures `result == 0`)
    /// to `FixedIndex` so a later store to element 1 does not repair element 0.
    /// The reader-facing name stays the authored selector: `tokens[index]`, the
    /// same spelling an unresolved `Index` segment already uses. A literal
    /// index keeps its numeral.
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

    /// Spell `tokens[index]` when the debt was opened through a call selector.
    ///
    /// Canonical projection and permission events may both freeze
    /// `first_slot()` (ensures `result == 0`) to `FixedIndex`. Custody keeps
    /// that coordinate. The diagnostic names the selector the author wrote.
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

    /// Every reference crossed by the extracted place must be `&mut`: the `self`
    /// receiver for machine-rooted storage, then each proper prefix of the path
    /// (a `&mut` parameter root, a `&mut` field inside owned storage, a `&mut`
    /// local the window resolved through). Shared or write-only links cannot
    /// carry a restoration window.
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

/// The frame-scan context: the program and the checked operator facts the
/// nested arm frames are collected under. The recursive frame walk restated
/// this pair at every step; the owner carries it once.
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

/// An authored-facing name for a resolved storage place: `self`-rooted
/// storage spells through the receiver, other roots spell their own symbol,
/// and segments spell as field/index/case projections.
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
