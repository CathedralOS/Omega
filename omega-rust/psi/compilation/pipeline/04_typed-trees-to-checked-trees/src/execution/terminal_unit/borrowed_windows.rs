//! Ordinary move-out/restore statements as borrowed-window operations. A
//! `let local: T = root.field...;` move opens a window and an owned store to
//! that exact place closes it. The replacement may be the removed value, a
//! construction, or a call result: its custody belongs to the ordinary result
//! bindings, not to the hole. An occupied destination first displaces its old
//! value; an already-open hole must not be read or disposed again. The lowered
//! emitter feeds these operations through `BorrowedWindowLedger`, and independent
//! verification reconstructs both restoration debt and the values' custody.

use std::collections::BTreeSet;

use super::{
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralResultBindingPlan, Multiplicity, StatementNode, SymbolHandle, TypedTrees,
};
use crate::execution::terminal_unit::structural_scalar_store::TargetSelectors;
use crate::execution::terminal_unit::types::ShapeCollector;

use crate::execution::terminal_unit::types::terminal_field_identity;
use crate::flow::CanonicalPlace;
use checked_trees::CheckFacts;
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::expression::ExpressionHandle;

/// The exact field chain beneath an exclusive borrowed structural parameter.
/// Whole-root moves and referent segments open no pinned window; a fixed or
/// retained runtime element step selects the element the window pins.
///
/// The place is first keyed on the resolved *storage* it names — the same
/// rebase the checker performs: a route through a reference-typed local
/// (`let r = &mut self; r.f`) names the same hole as the owner path
/// (`self.f`), so both must land on one window. `target` is the authored
/// expression the place canonicalized from: its retained `AssignmentIndex`
/// scalars resolve a runtime segment's element coordinate.
pub(in crate::execution::terminal_unit) fn window_place(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    statement_index: usize,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    place: &CanonicalPlace,
    target: ExpressionHandle,
) -> Option<(u32, Vec<CheckedUnitStructuralPathSegment>)> {
    window_place_admitting(
        program,
        facts,
        machine,
        state,
        statements,
        statement_index,
        structural_parameters,
        place,
        target,
        false,
    )
}

/// `window_place` admitting a whole-root store target: replacing the
/// referent of an exclusive borrowed parameter is a legitimate
/// `StoreStructuralField` destination (`path` empty), while a whole-root
/// move-out still opens no pinned window.
pub(in crate::execution::terminal_unit) fn window_place_admitting(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    statement_index: usize,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    place: &CanonicalPlace,
    target: ExpressionHandle,
    admit_whole_root: bool,
) -> Option<(u32, Vec<CheckedUnitStructuralPathSegment>)> {
    let PlaceRoot::Symbol(root_symbol) = place.root else {
        return None;
    };
    let (root, segments) = resolve_storage_place(
        program,
        state,
        statements,
        statement_index,
        root_symbol,
        &place.segments,
    );
    let PlaceRoot::Symbol(symbol) = root else {
        return None;
    };
    let position = program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == symbol)?;
    let position = u32::try_from(position).ok()?;
    let (parameter_index, parameter) = structural_parameters
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.position == position)?;
    if parameter.access != CheckedStructuralAccess::MutableBorrow
        || parameter.multiplicity == Multiplicity::Linear
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
    {
        return None;
    }
    if segments.is_empty() && !admit_whole_root {
        return None;
    }
    let selectors = segments
        .iter()
        .any(|segment| {
            matches!(
                segment,
                PlaceSegment::Index { .. } | PlaceSegment::FixedIndex { .. }
            )
        })
        .then(|| {
            TargetSelectors::resolve(
                program,
                facts,
                machine,
                state,
                u32::try_from(statement_index).ok()?,
                target,
            )
        })
        .flatten();
    let path = segments
        .iter()
        .map(|segment| match segment {
            PlaceSegment::Field { symbol } => terminal_field_identity(program, *symbol)
                .map(CheckedUnitStructuralPathSegment::Field),
            PlaceSegment::FixedIndex { index } => Some(
                CheckedUnitStructuralPathSegment::FixedIndex(u64::try_from(*index).ok()?),
            ),
            PlaceSegment::Index { expression } => selectors
                .as_ref()
                .and_then(|selectors| selectors.runtime_segment(*expression)),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    // Checked argument sources use the retained structural lane, not the
    // authored parameter list containing scalar and erased formals. Keep the
    // same dense coordinate in the window ledger and both emitted plans.
    Some((u32::try_from(parameter_index).ok()?, path))
}

/// Rebase a place written through a reference-typed local onto the referent
/// storage it borrows, following plain reference copies (`let s = r`),
/// recast chains, and whole-local reassignments, until the root names a
/// parameter or machine rather than a local. Mirrors the checker rule that
/// windows key on the resolved storage place, not the access route.
pub(in crate::execution::terminal_unit) fn resolve_storage_place(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    statement_index: usize,
    root_symbol: SymbolHandle,
    path: &[PlaceSegment],
) -> (PlaceRoot, Vec<PlaceSegment>) {
    let mut root = PlaceRoot::Symbol(root_symbol);
    let mut segments = path.to_vec();
    for _ in 0..16 {
        let PlaceRoot::Symbol(symbol) = root else {
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
        let Some(source) = crate::flow::reference_source_place_through_collection_views(
            program,
            state.symbol,
            statement_index,
            source,
        ) else {
            break;
        };
        let PlaceRoot::Symbol(..) = source.root else {
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

/// Exact places a statement sequence has opened and not yet repaired.
/// Detached values live in the sequence's ordinary structural result table;
/// consuming a value does not discharge the original place's restoration debt.
#[derive(Default)]
pub(super) struct OpenWindows {
    open: BTreeSet<(u32, Vec<CheckedUnitStructuralPathSegment>)>,
}

/// The exact storage place an immutable, non-reference, ownership-carrying
/// local reads whole from an existing place. Such a local has the window
/// route by kind: the sequence admits it and then decides whether the place is
/// exclusive borrowed storage it can move out of. Copy locals never open a
/// window, and a mutable or reference-typed local binds nothing to restore.
pub(in crate::execution::terminal_unit) fn move_out_candidate(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    local: &typed_trees::statement::TableLocalData,
) -> Option<crate::flow::CanonicalPlace> {
    if local.is_mutable
        || crate::checks::type_reference_is_reference(program, local.type_reference)
        || program.type_multiplicity(local.type_reference) == Multiplicity::Unrestricted
    {
        return None;
    }
    crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        local.initial_value,
    )
}

impl OpenWindows {
    /// `let local: T = root.field...;` moving an affine or linear value out
    /// of exclusive borrowed storage. The local binds the moved value at
    /// `binding_ordinal`; a copy value or a place outside a pinned window is
    /// not a move-out and returns None so the ordinary local-data route
    /// plans it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn move_out(
        &mut self,
        program: &TypedTrees,
        facts: &CheckFacts,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
        binders: &[(SymbolHandle, String)],
        statement_index: u32,
        local: &typed_trees::statement::TableLocalData,
        binding_ordinal: u32,
    ) -> Option<(
        CheckedUnitEffectOperationPlan,
        CheckedUnitStructuralResultBindingPlan,
    )> {
        let index = usize::try_from(statement_index).ok()?;
        let place = move_out_candidate(program, state, index, local)?;
        let multiplicity = program.type_multiplicity(local.type_reference);
        let statements = program.statement_table.statements(state.statement_nodes);
        let (position, path) = window_place(
            program,
            facts,
            machine,
            state,
            statements,
            index,
            structural_parameters,
            &place,
            local.initial_value,
        )?;
        let type_identity = shapes.add_type(local.type_reference, binders, &[])?;
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal,
            type_identity: type_identity.clone(),
            multiplicity,
        };
        if !self.open.insert((position, path.clone())) {
            return None;
        }
        Some((
            CheckedUnitEffectOperationPlan::MoveStructuralField {
                result: result.clone(),
                source: CheckedUnitStructuralArgumentPlan {
                    source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: position,
                    },
                    path,
                    type_identity,
                    access: CheckedStructuralAccess::Owned,
                },
            },
            result,
        ))
    }

    /// Store `produced` into an exclusive borrowed field. An existing hole
    /// needs only the store; otherwise first displace the old value into
    /// `displaced_ordinal` and dispose it on the statement's continuation.
    /// Neither route depends on how the owned replacement was produced.
    /// Overwriting a linear field would drop a live obligation and remains
    /// outside this replacement route.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn replace(
        &mut self,
        program: &TypedTrees,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
        statement_index: u32,
        assignment: &typed_trees::statement::TableAssignment,
        produced: &CheckedUnitStructuralResultBindingPlan,
        displaced_ordinal: u32,
        value: CheckedUnitStructuralArgumentPlan,
    ) -> Option<FieldReplacement> {
        self.replace_element(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            statement_index,
            assignment,
            &[],
            produced,
            displaced_ordinal,
            value,
        )
    }

    /// Store `produced` into the element of the fixed array the assignment's
    /// target names at literal `indices`: the same window pair `replace`
    /// builds, over the target's place extended by those indices. A
    /// whole-array literal replaces its array one element at a time this way,
    /// so each element keeps its own exact displacement and disposal.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn replace_element(
        &mut self,
        program: &TypedTrees,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
        statement_index: u32,
        assignment: &typed_trees::statement::TableAssignment,
        indices: &[u64],
        produced: &CheckedUnitStructuralResultBindingPlan,
        displaced_ordinal: u32,
        value: CheckedUnitStructuralArgumentPlan,
    ) -> Option<FieldReplacement> {
        if produced.multiplicity == Multiplicity::Linear {
            return None;
        }
        let index = usize::try_from(statement_index).ok()?;
        let mut destination_place = crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            index,
            assignment.target,
        )?;
        for element in indices {
            destination_place.segments.push(PlaceSegment::FixedIndex {
                index: usize::try_from(*element).ok()?,
            });
        }
        let (resolved_root, resolved_segments) = resolve_storage_place(
            program,
            state,
            program.statement_table.statements(state.statement_nodes),
            index,
            match destination_place.root {
                PlaceRoot::Symbol(symbol) => symbol,
                _ => return None,
            },
            &destination_place.segments,
        );
        let destination_place = CanonicalPlace {
            root: resolved_root,
            segments: resolved_segments,
        };
        let owned_local = match destination_place.root {
            PlaceRoot::Symbol(symbol) => program
                .state_local_declaration(state, symbol)
                .filter(|(ordinal, local)| {
                    *ordinal < index
                        && local.is_mutable
                        && !crate::checks::type_reference_is_reference(
                            program,
                            local.type_reference,
                        )
                })
                .map(|_| symbol),
            _ => None,
        };
        let (source, path, needs_displacement, needs_cleanup) = if let Some(symbol) = owned_local {
            let (_, path) = super::calls::projected_argument_path(
                program,
                state.symbol,
                index,
                &destination_place,
            )?;
            // The permission producer has already selected old-value death.
            // An absent construction slot creates no displaced value. This
            // query follows checked custody, not the initializer's spelling.
            let occupied = facts
                .contextual_cleanup
                .dispositions
                .iter()
                .any(|(_, disposition)| {
                    disposition.machine == machine.symbol
                        && disposition.state == state.symbol
                        && disposition.source
                            == language_semantics::PermissionEventSource::Statement {
                                statement_index: index,
                            }
                        && disposition.root == destination_place.root
                        && facts
                            .flow
                            .ownership
                            .segments
                            .span_or_empty(disposition.path)
                            .starts_with(&destination_place.segments)
                });
            (
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                path,
                false,
                occupied,
            )
        } else {
            let (position, path) = window_place_admitting(
                program,
                facts,
                machine,
                state,
                program.statement_table.statements(state.statement_nodes),
                index,
                structural_parameters,
                &destination_place,
                assignment.target,
                true,
            )?;
            let needs_displacement = !self.open.remove(&(position, path.clone()));
            (
                CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: position,
                },
                path,
                needs_displacement,
                needs_displacement && produced.multiplicity == Multiplicity::Affine,
            )
        };
        let place = CheckedUnitStructuralArgumentPlan {
            source,
            path,
            type_identity: produced.type_identity.clone(),
            access: CheckedStructuralAccess::Owned,
        };
        Some(FieldReplacement {
            move_out: needs_displacement.then(|| {
                CheckedUnitEffectOperationPlan::MoveStructuralField {
                    result: CheckedUnitStructuralResultBindingPlan {
                        statement_index,
                        binding_ordinal: displaced_ordinal,
                        type_identity: produced.type_identity.clone(),
                        multiplicity: produced.multiplicity,
                    },
                    source: place.clone(),
                }
            }),
            cleanup: needs_cleanup.then(|| {
                CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    boundary: checked_trees::CheckedUnitCleanupBoundary::Replacement {
                        statement_index,
                    },
                    // This storage marker is completed from the authoritative
                    // death disposition once the hook catalog exists. Owned
                    // children retire in place; a borrowed replacement has a
                    // separately displaced value and restoration debt.
                    cleanup_actions: vec![checked_trees::CheckedNormalCleanupAction::Discard(
                        CheckedUnitPartialAffineDiscardPlan {
                            source: if needs_displacement {
                                CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                    binding_ordinal: displaced_ordinal,
                                }
                            } else {
                                place.source.clone()
                            },
                            path: if needs_displacement {
                                Vec::new()
                            } else {
                                place.path.clone()
                            },
                            type_identity: produced.type_identity.clone(),
                        },
                    )],
                }
            }),
            store: if owned_local.is_some() && place.path.is_empty() {
                CheckedUnitEffectOperationPlan::RebindStructuralLocal {
                    statement_index,
                    destination: place,
                    value,
                }
            } else {
                CheckedUnitEffectOperationPlan::StoreStructuralField {
                    statement_index,
                    destination: place,
                    value,
                }
            },
        })
    }

    /// Every opened window must be repaired before the body completes; the
    /// checker already rejects an open window at the state's exits, so a
    /// leftover here is a sequencing fault rather than a language decision.
    pub(super) fn is_closed(&self) -> bool {
        self.open.is_empty()
    }
}

/// One store, with displacement and disposal only when its destination was
/// occupied. A vacant destination is repaired without inventing an old value.
pub(super) struct FieldReplacement {
    pub(super) move_out: Option<CheckedUnitEffectOperationPlan>,
    pub(super) store: CheckedUnitEffectOperationPlan,
    pub(super) cleanup: Option<CheckedUnitEffectOperationPlan>,
}

impl FieldReplacement {
    /// Preserve the authored replacement lifecycle independently of the RHS's
    /// producer: displacement if borrowed, selected old-value death, then store.
    pub(super) fn into_operations(self) -> impl Iterator<Item = CheckedUnitEffectOperationPlan> {
        self.move_out
            .into_iter()
            .chain(self.cleanup)
            .chain(std::iter::once(self.store))
    }
}
