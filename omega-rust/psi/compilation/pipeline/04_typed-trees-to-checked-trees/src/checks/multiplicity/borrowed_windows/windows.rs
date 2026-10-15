//! windows slice of borrowed_windows.

use validation::CanonicalPlace;

use super::{exclusive_storage_chain, place_spelling, resolve_storage_place};

use super::boundary_checks::*;

use crate::checks::borrows::{StorageSelectorFacts, storage_path_compatibility};

use checked_trees::facts::StorageWindowInvocation;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceHandle;

/// One absent place inside exclusive borrowed storage. `root`/`path` name the
/// resolved storage place (a `&mut`-local route is already rebased onto the
/// referent root); `spelling` is the authored-facing name used in diagnostics.
pub(super) struct OpenWindow {
    root: facts::PlaceRoot,
    path: Vec<facts::PlaceSegment>,
    spelling: String,
    required_type: TypeReferenceHandle,
    opened_statement: usize,
    /// Set once an exit edge reports the still-open debt; later edges and the
    /// implicit return do not repeat the identical diagnostic.
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

    /// Select the still-present old referent after RHS evaluation. A repair
    /// into an exact hole creates no second death; replacing an enclosing
    /// value retains the absent children's exclusions. This is the same
    /// window state that checks reads, moves, invocations and restoration.
    pub(in crate::checks::multiplicity) fn replacement_death(
        &self,
        program: &dyn ::validation::CheckingInput,
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
        let target = validation::canonical_place_from_expression_in_state(
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
            validation::canonical_place_type_reference(
                program,
                state.symbol,
                statement_index,
                &prefix,
            )
            .is_some_and(|reference| {
                super::super::linear_obligations::type_reference_is_reference(program, reference)
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
        // Death belongs to the old stored type, including its declared
        // refinements and generic substitutions. The RHS proves a new value;
        // it cannot redefine the old value's disposal obligations.
        // Rebinding retires the descriptor, whereas a write through it retires
        // its old contents. Peeling every reference would manufacture owned
        // disposal of a surviving pointee from a descriptor replacement.
        let value_type =
            validation::assignment_stored_value_type(program, machine, state, assignment)?;
        if program.type_multiplicity(value_type) != language_semantics::Multiplicity::Affine {
            return None;
        }
        let symbol = target.root.as_symbol()?;
        let (root, path) = resolve_storage_place(
            program,
            machine,
            state,
            statements,
            statement_index,
            symbol,
            &target.segments,
        );
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

    /// The repair obligation an eligible borrowed move creates. Returns the
    /// diagnostic to emit when the transfer cannot open a window — a
    /// non-symbol root, a shared or write-only link in the chain, or an
    /// overlap with an already absent place — and `None` once the window is
    /// recorded.
    pub(in crate::checks::multiplicity) fn open(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statements: &[StatementNode],
        statement_index: usize,
        event: &crate::flow::DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
    ) -> Option<Diagnostic> {
        let Some(root_symbol) = event.root.as_symbol() else {
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
        if let Some(absent) = self.open.iter().find(|absent| {
            absent.root == root
                && storage_path_compatibility(program, self.captures, &storage_path, &absent.path).0
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
                validation::canonical_place_type_reference(
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

    /// A move of a place that is — or still contains — an absent borrowed
    /// subtree cannot leave: the moved value would carry the hole with it.
    /// This is the ordinary-move twin of `open`'s overlap refusal and covers
    /// whole-owner moves (empty paths) the borrowed gate never sees.
    pub(in crate::checks::multiplicity) fn refuse_move_over_open(
        &self,
        program: &dyn ::validation::CheckingInput,
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
        let Some(root_symbol) = event.root.as_symbol() else {
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
        let Some(absent) = self.open.iter().find(|absent| {
            absent.root == root
                && storage_path_compatibility(program, self.captures, &storage_path, &absent.path).0
        }) else {
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
    pub(in crate::checks::multiplicity) fn record_invocation(
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

    pub(in crate::checks::multiplicity) fn check_use(
        &self,
        program: &dyn ::validation::CheckingInput,
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
        let Some(absent) = self.open.iter().find(|absent| {
            absent.root == root
                && storage_path_compatibility(program, self.captures, &storage_path, &absent.path).0
        }) else {
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
    pub(in crate::checks::multiplicity) fn check_repair(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statements: &[StatementNode],
        statement_index: usize,
        assignment: &typed_trees::statement::TableAssignment,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(target) = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            statement_index,
            assignment.target,
        ) else {
            return;
        };
        let Some(root_symbol) = target.root.as_symbol() else {
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
            if absent.root != root
                || !storage_path_compatibility(program, self.captures, &target_path, &absent.path).0
            {
                continue;
            }
            if storage_path_contains(program, self.captures, &target_path, &absent.path) {
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
    pub(in crate::checks::multiplicity) fn refuse_open_at_exit(
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

/// Restoration and old-value death select the same captured subtree. Raw
/// expression-handle prefixes cannot recognize a re-spelled immutable index;
/// structural spelling equality could instead conflate changed mutable values.
fn storage_path_contains(
    program: &dyn ::validation::CheckingInput,
    captures: StorageSelectorFacts<'_>,
    container: &[facts::PlaceSegment],
    contained: &[facts::PlaceSegment],
) -> bool {
    matches!(
        storage_path_compatibility(program, captures, container, contained).1,
        checked_trees::CapturedPlaceContainment::Same
            | checked_trees::CapturedPlaceContainment::LeftContainsRight
    )
}
