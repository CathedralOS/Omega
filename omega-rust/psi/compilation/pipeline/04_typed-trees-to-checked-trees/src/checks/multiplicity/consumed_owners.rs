//! Owned places a state body has already transferred or consumed.
//!
//! A by-value move out of a local, an owned parameter, or an owned `self`
//! receiver hands the place's custody to its destination; the place then
//! holds no established value until a whole store re-establishes it
//! (wiki/spec/language/ownership.md, "Paths and control flow": the
//! permission context tracks exact established places). Linear
//! validation rejects a second transfer of a tracked claim, but a read, a
//! borrow, a method receiver or a projected move produces no permission event
//! of its own, so `drop(g); g.handle` and `consume(g); g.handle` used to
//! check. Early disposal is an ordinary consuming call, so neither the callee
//! name nor a cleanup hook decides anything here.
//!
//! The tracker rides the occurrence-ordered replay that borrowed-storage
//! windows already use (`ownership_occurrences::statement_steps`): each evaluated
//! place is observed before its own move applies, so `take(g.handle, g)`
//! reads before it moves while `take(g); g.handle` reads a consumed owner.
//! Its limits keep it from inventing moves the ownership lane did not record:
//!
//! - Only owned custody chains are tracked. A move that crosses a reference
//!   (a `&mut` receiver, parameter, local or field) opens a borrowed-storage
//!   window instead, and a move of a reference value itself transfers a loan
//!   carrier, not storage; both keep their existing rules.
//! - A move on a transition arm leaves the state when that arm is taken; a
//!   guard miss falls through to the next arm still holding the value, so
//!   those moves retire when the arm's statement ends.
//! - A conditional move — inside one match arm or a short-circuit operand —
//!   leaves the owner consumed on some paths only. Sibling arms still hold
//!   the value, so the move applies once its statement completes; every later
//!   use then reads a possibly consumed owner. A move no evaluated occurrence
//!   claimed (an operand of an unreachable arm) never applies.
//! - A use at, below or above a consumed place observes absent storage. Partial
//!   moves preserve disjoint siblings, not a whole ancestor value to pass to
//!   another machine. Selected structural cleanup uses its exact residual
//!   frontier without evaluating that ancestor as an authored value.
//! - A store to the consumed place or to an enclosing owner re-establishes
//!   it. A store into a strict projection of a consumed place would write
//!   through storage that holds no value and rejects.
//! - Logical assembly facts and parser-synthesized destructure markers are
//!   proof reads, not operational uses, and are not checked.
//!
//! The diagnostics are reported only after linear validation succeeds (see
//! `check_linear_obligations`): a repeated whole transfer is also a use of a
//! consumed owner, and its established "already transferred or consumed"
//! diagnostic stays the one reported for that shape.

use validation::CanonicalPlace;

use super::borrowed_windows::place_spelling;
use super::linear_obligations::type_reference_is_reference;
use super::owned_selection::place_paths_overlap;

use diagnostics::Diagnostic;
use typed_trees::statement::StatementNode;

/// One owned place whose custody an earlier occurrence transferred.
struct ConsumedPlace {
    root: facts::PlaceRoot,
    path: Vec<facts::PlaceSegment>,
    spelling: String,
    moved_statement: usize,
}

#[derive(Default)]
pub(super) struct ConsumedOwners {
    consumed: Vec<ConsumedPlace>,
    /// Conditional moves of the current statement; they join `consumed` when
    /// the statement completes.
    pending: Vec<ConsumedPlace>,
    /// `consumed` rows that predate the current statement. A transition
    /// statement truncates back to this length when it ends.
    statement_start: usize,
}

impl ConsumedOwners {
    pub(super) fn begin_statement(&mut self) {
        self.statement_start = self.consumed.len();
        self.pending.clear();
    }

    /// Apply one ordered move. Moves that do not transfer owned storage of
    /// this activation are ignored rather than guessed at.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn record_move(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        event: &crate::flow::DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
        conditional: bool,
    ) {
        let place = storage_place(
            program,
            machine,
            state,
            &CanonicalPlace {
                root: event.root,
                segments: path.to_vec(),
            },
        );
        if !owned_custody_chain(program, machine, state, statement_index, &place) {
            return;
        }
        let consumed = ConsumedPlace {
            spelling: place_spelling(program, machine, state, place.root, &place.segments),
            root: place.root,
            path: place.segments,
            moved_statement: statement_index,
        };
        if conditional {
            self.pending.push(consumed);
        } else {
            self.consumed.push(consumed);
        }
    }

    /// A read, borrow, receiver or move overlapping a consumed place touches
    /// storage that holds no complete established value.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn check_use(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement: &StatementNode,
        statement_index: usize,
        use_place: &CanonicalPlace,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if self.consumed.is_empty() || !is_operational(statement) {
            return;
        }
        let place = storage_place(program, machine, state, use_place);
        let Some(consumed) = self.consumed_through(&place) else {
            return;
        };
        let spelling = place_spelling(program, machine, state, place.root, &place.segments);
        push_once(
            diagnostics,
            format!(
                "cannot use `{spelling}` in machine `{}` state `{}` (statement \
                 {statement_index}) after `{}` was transferred or consumed at statement {}: it \
                 holds no value; read what is needed before the transfer or assign a whole \
                 value to `{}` first",
                machine.name.as_str(),
                state.name.as_str(),
                consumed.spelling,
                consumed.moved_statement,
                consumed.spelling,
            ),
        );
    }

    /// A whole store at or above a consumed place re-establishes it; a store
    /// strictly inside one writes through an owner that holds no value.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn check_store(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        assignment: &typed_trees::statement::TableAssignment,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if self.consumed.is_empty() {
            return;
        }
        let Some(target) = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            statement_index,
            assignment.target,
        ) else {
            return;
        };
        let target = storage_place(program, machine, state, &target);
        if let Some(consumed) = self.consumed.iter().find(|consumed| {
            consumed.root == target.root
                && target.segments.len() > consumed.path.len()
                && place_paths_overlap(&target.segments, &consumed.path)
        }) {
            let spelling = place_spelling(program, machine, state, target.root, &target.segments);
            push_once(
                diagnostics,
                format!(
                    "cannot store into `{spelling}` in machine `{}` state `{}` (statement \
                     {statement_index}) after `{}` was transferred or consumed at statement \
                     {}: assign a whole value to `{}` to re-establish it",
                    machine.name.as_str(),
                    state.name.as_str(),
                    consumed.spelling,
                    consumed.moved_statement,
                    consumed.spelling,
                ),
            );
            return;
        }
        self.consumed.retain(|consumed| {
            consumed.root != target.root || !consumed.path.starts_with(&target.segments)
        });
    }

    /// Close the current statement. A taken transition leaves the state, so
    /// its moves never reach the fall-through; every other statement's
    /// conditional moves now apply to later statements.
    pub(super) fn finish_statement(&mut self, leaves_on_transition: bool) {
        if leaves_on_transition {
            self.consumed.truncate(self.statement_start);
            self.pending.clear();
        } else {
            self.consumed.append(&mut self.pending);
        }
    }

    /// An ancestor observation also needs its missing subtree. Address-only
    /// traversal is not an observation: the occurrence owner already retains
    /// only each operation's maximal selected place, so disjoint siblings do
    /// not spuriously observe their common parent.
    fn consumed_through(&self, place: &CanonicalPlace) -> Option<&ConsumedPlace> {
        self.consumed.iter().find(|consumed| {
            consumed.root == place.root && place_paths_overlap(&place.segments, &consumed.path)
        })
    }
}

/// Repeated occurrences of one use in a statement report once.
fn push_once(diagnostics: &mut Vec<Diagnostic>, message: String) {
    if !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message == message)
    {
        diagnostics.push(Diagnostic::error(message));
    }
}

/// Assembly facts are logical and destructure markers are parser-synthesized
/// proof reads; neither evaluates the owner at run time.
fn is_operational(statement: &StatementNode) -> bool {
    match statement {
        StatementNode::AssemblyFact(_) => false,
        StatementNode::LocalData(local) => {
            local.role != language_core::LocalBindingRole::ArmPattern
        }
        _ => true,
    }
}

/// Key a place by its storage identity: `self` spellings re-root at the
/// machine exactly as move events do, and a bare attached-field root joins
/// its receiver storage.
fn storage_place(
    _program: &dyn ::validation::CheckingInput,
    _machine: &typed_trees::machine::Machine,
    _state: &typed_trees::state::State,
    place: &CanonicalPlace,
) -> CanonicalPlace {
    let storage = CanonicalPlace {
        root: (place.root),
        segments: place.segments.clone(),
    };

    storage
}

/// Whether `place` is owned storage of this activation reached without
/// crossing a reference: a local declared in this state, a by-value
/// parameter, and every prefix through the
/// place itself has a non-reference type.
fn owned_custody_chain(
    program: &dyn ::validation::CheckingInput,
    _machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    place: &CanonicalPlace,
) -> bool {
    let Some(root) = place.root.as_symbol() else {
        return false;
    };
    let parameters = program.state_parameters(state);
    let owned_root = program.state_local_declaration(state, root).is_some()
        || parameters.iter().any(|parameter| parameter.symbol == root);
    owned_root
        && (0..=place.segments.len()).all(|length| {
            validation::canonical_place_type_reference(
                program,
                state.symbol,
                statement_index,
                &CanonicalPlace {
                    root: place.root,
                    segments: place.segments[..length].to_vec(),
                },
            )
            .is_some_and(|type_reference| !type_reference_is_reference(program, type_reference))
        })
}
