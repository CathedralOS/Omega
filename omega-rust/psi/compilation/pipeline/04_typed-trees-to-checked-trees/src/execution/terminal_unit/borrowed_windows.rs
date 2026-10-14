//! Detach structural children and reseat their exact storage. A
//! `let local: T = root.field...;` move creates independent owned value custody;
//! an owned store to that exact place closes its hole. Only extraction through
//! exclusive borrowed storage owes restoration at every normal exit. An owned
//! root may remain partially moved under its checked residual disposal plan.
//! The replacement may be the removed value, a
//! construction, or a call result: its custody belongs to the ordinary result
//! bindings, not to the hole. An occupied destination displaces conserved old
//! custody before installation; a freely discardable replacement needs no old-value
//! observation. An already-open hole must not be read or disposed again. The lowered
//! emitter feeds these operations through `BorrowedWindowLedger`, and independent
//! verification reconstructs both restoration debt and the values' custody.
//! An empty path selects the exclusive parameter's whole referent. It shares
//! extraction and repair sequencing with a field; a separate whole-root
//! admission mode would make the value's custody depend on its spelling.
//!
//! A structural assignment captures its destination before the RHS's first
//! producer. The same retained assignment selectors describe construction,
//! call, and already-owned replacement values. Cleanup still selects the
//! original storage's death occurrence; capture changes when coordinates are
//! read, not permission, restoration debt, or the old value's custody.
//! Source admission and this planner share stable spatial comparison. The
//! resulting store retains the opening extraction's result coordinate, so
//! lowering never has to rediscover equality between value-read operands and
//! pre-RHS assignment captures. Mutable selector spelling alone cannot prove
//! agreement with an earlier captured selection.

use validation::CanonicalPlace;

use super::{
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralResultBindingPlan, Multiplicity, StatementNode, SymbolHandle,
};
use crate::execution::terminal_unit::structural_scalar_store::TargetSelectors;
use crate::execution::terminal_unit::types::ShapeCollector;

use checked_trees::CheckFacts;
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::expression::ExpressionHandle;

/// The planning ledger and source admission must agree on the captured value
/// selecting a hole. Re-spelled immutable aliases can denote one index;
/// mutable storage spelling alone cannot identify an earlier selection.
fn same_storage_place(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    left: &CanonicalPlace,
    right: &CanonicalPlace,
) -> bool {
    left.root == right.root
        && crate::checks::storage_path_compatibility(
            program,
            crate::checks::StorageSelectorFacts {
                flow: &facts.flow,
                semantic: &facts.semantic,
            },
            &left.segments,
            &right.segments,
        )
        .1 == checked_trees::CapturedPlaceContainment::Same
}

/// Assignment owns selector evaluation, even when the selector spells a
/// parameter or field that ordinary call arguments can read directly. Its
/// retained operand belongs to the pre-RHS occurrence, not a later snapshot.
fn assignment_path(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    target: ExpressionHandle,
    place: &CanonicalPlace,
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    let selectors =
        TargetSelectors::resolve(program, facts, machine, state, statement_index, target)?;
    super::calls::projected_evaluated_path(
        program,
        machine.symbol,
        state.symbol,
        usize::try_from(statement_index).ok()?,
        place,
        Some(&|expression| selectors.runtime_segment(expression)),
    )
    .map(|(_, path)| path)
}

/// Owned storage has no borrowed restoration debt. A formal and a
/// local have different physical coordinates, but replacing either retires
/// only the old custody selected by source checking. A whole reference-valued
/// replacement owns its carrier, not the borrowed contents. Its checked result
/// type must equal that carrier's declaration; referent writes still use the
/// loan/window ledger below. Mutability is source admission, not storage
/// identity. A deferred local's first initialization has no old value to
/// displace or borrowed hole to restore.
pub(super) fn owned_assignment_source(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    statement_index: usize,
    place: &CanonicalPlace,
    replacement_type: Option<typed_trees::type_identity::TypeIdentityId>,
) -> Option<CheckedUnitStructuralArgumentSourcePlan> {
    // Owned descriptors do not own storage reached through them. Classify
    // every crossed prefix, including the selected slot when its replacement
    // has the referent's type rather than the descriptor's type.
    for depth in 0..=place.segments.len() {
        let reference = validation::canonical_place_type_reference(
            program,
            state.symbol,
            statement_index,
            &CanonicalPlace {
                root: place.root,
                segments: place.segments[..depth].to_vec(),
            },
        )?;
        if validation::reference_shell(program, reference).is_some()
            && (depth < place.segments.len()
                || replacement_type
                    != Some(validation::stored_structural_type_identity(
                        program,
                        reference,
                        &[],
                        &[],
                    )))
        {
            return None;
        }
    }
    let symbol = place.root.as_symbol()?;
    if let Some((ordinal, local)) = program.state_local_declaration(state, symbol) {
        return (ordinal < statement_index
            && (!crate::checks::type_reference_is_reference(program, local.type_reference)
                || (place.segments.is_empty()
                    && replacement_type
                        == Some(validation::stored_structural_type_identity(
                            program,
                            local.type_reference,
                            &[],
                            &[],
                        )))))
        .then_some(CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol });
    }
    let position =
        typed_trees::signature::state_parameter_position(program.state_parameters(state), symbol)?;
    let (parameter_index, _) = parameters.iter().enumerate().find(|(_, parameter)| {
        parameter.position as usize == position
            && parameter.access == CheckedStructuralAccess::Owned
    })?;
    Some(CheckedUnitStructuralArgumentSourcePlan::Parameter {
        parameter_index: u32::try_from(parameter_index).ok()?,
    })
}

/// Retain the assignment target once, before evaluating any structural RHS.
/// The target is independent of its replacement's producer and of the fixed
/// leaves a whole-array replacement will store beneath it.
pub(super) fn capture_assignment_destination(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    binders: &[(SymbolHandle, String)],
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<CheckedUnitEffectOperationPlan> {
    let ordinal = usize::try_from(statement_index).ok()?;
    let access = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        ordinal,
        assignment.target,
    )?;
    let owned_source = owned_assignment_source(
        program,
        state,
        parameters,
        ordinal,
        &access,
        facts
            .values
            .structural_values
            .root_for_expression(state.symbol, statement_index, assignment.value)
            .map(|value| {
                validation::stored_structural_type_identity(program, value.type_reference, &[], &[])
            }),
    );
    if matches!(
        owned_source,
        Some(CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. })
    ) && access.segments.is_empty()
    {
        // Rebinding a variable evaluates no address or selector. Its prior
        // occurrence may already be moved out; only projected or borrowed
        // storage needs a pre-RHS physical destination capture.
        return None;
    }
    let root = access.root.as_symbol()?;
    let (root, segments) = resolve_storage_place(
        program,
        state,
        program.statement_table.statements(state.statement_nodes),
        ordinal,
        root,
        &access.segments,
    );
    let place = CanonicalPlace { root, segments };
    let (source, mut path) = if let Some(source) = owned_source {
        (
            source,
            assignment_path(
                program,
                facts,
                machine,
                state,
                statement_index,
                assignment.target,
                &place,
            )?,
        )
    } else {
        let selectors = TargetSelectors::resolve(
            program,
            facts,
            machine,
            state,
            statement_index,
            assignment.target,
        )?;
        let place = window_place(
            program,
            state,
            program.statement_table.statements(state.statement_nodes),
            ordinal,
            parameters,
            &access,
            language_semantics::ReferenceAccess::WriteOnly,
            &|expression| selectors.runtime_segment(expression),
        )?;
        (place.source, place.path)
    };
    let declared =
        validation::declared_place_type_raw(program, machine, Some(state), assignment.target)?;
    let stored = validation::assignment_stored_value_type(program, machine, state, assignment)?;
    if !path.is_empty()
        && validation::reference_shell(program, declared).is_some()
        && stored != declared
    {
        path.push(CheckedUnitStructuralPathSegment::Referent);
    }
    let declared = stored;
    // Byte/scalar stores already own their target evaluation in their store
    // operation. This capture belongs to structural replacement sequencing.
    if program.primitive_type_reference(declared).is_some()
        || super::types::byte_sequence_carrier(program, declared, &[]).is_some()
    {
        return None;
    }
    Some(
        CheckedUnitEffectOperationPlan::CaptureAssignmentDestination {
            statement_index,
            destination: CheckedUnitStructuralArgumentPlan {
                source,
                path,
                // Capture the assigned storage, including a pointer when the
                // assignment replaces a reference descriptor. `stored` has
                // already distinguished that from a write to its referent;
                // peeling another reference shell would capture the wrong
                // type even though the destination path still names the slot.
                type_identity: shapes.add_stored_type(declared, binders, &[])?,
                access: CheckedStructuralAccess::Owned,
            },
        },
    )
}

/// Restoration debt and the selected access route are separate coordinates.
/// Canonical storage identifies the hole even through forwarding; the emitted
/// operand must still use the retained reference loan whose child suspends that
/// storage's original parameter. Replacing the route by the debt root loses
/// authority or a narrowed view's captured offset.
pub(in crate::execution::terminal_unit) struct WindowPlace {
    storage: CanonicalPlace,
    source: CheckedUnitStructuralArgumentSourcePlan,
    path: Vec<CheckedUnitStructuralPathSegment>,
    restoration_required: bool,
}

/// The exact field chain beneath an exclusive structural loan.
/// An empty path pins the whole referent, not its loan descriptor; a fixed or
/// retained runtime element step selects the element the window pins. Below
/// a `&mut [T]` loan that element is one the view selects in storage it
/// does not own; lowering bounds it by the view's runtime length.
///
/// The place is first keyed on the resolved *storage* it names — the same
/// rebase the checker performs: a route through a reference-typed local
/// (`let r = &mut self; r.f`) names the same hole as the owner path
/// (`self.f`), so both must land on one window. The access's evaluation owner supplies
/// runtime coordinates: value reads consume checked scalar operands, while
/// assignment targets consume their retained pre-RHS selector slots.
pub(in crate::execution::terminal_unit) fn window_place(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    statement_index: usize,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    place: &CanonicalPlace,
    required_access: language_semantics::ReferenceAccess,
    selector: &dyn Fn(ExpressionHandle) -> Option<CheckedUnitStructuralPathSegment>,
) -> Option<WindowPlace> {
    let root_symbol = place.root.as_symbol()?;
    let (root, segments) = resolve_storage_place(
        program,
        state,
        statements,
        statement_index,
        root_symbol,
        &place.segments,
    );
    // Storage is the checker's restoration subject, not an emitted operand.
    // In particular its range segments must survive: a local subslice owns no
    // elements, and rebasing its descriptor to a parameter would lose the
    // captured offset and bypass its immediate loan lineage.
    let storage = CanonicalPlace { root, segments };
    let local = program
        .state_local_declaration(state, root_symbol)
        .filter(|(ordinal, _)| *ordinal < statement_index);
    let (source, path, restoration_required) = if let Some((_, local)) = local {
        let borrowed = crate::checks::type_reference_is_reference(program, local.type_reference);
        if borrowed
            && !matches!(
                (
                    required_access,
                    validation::reference_shell(program, local.type_reference)?.access
                ),
                (_, language_semantics::ReferenceAccess::Mutable)
                    | (
                        language_semantics::ReferenceAccess::WriteOnly,
                        language_semantics::ReferenceAccess::WriteOnly
                    )
            )
        {
            return None;
        }
        if !borrowed && place.segments.is_empty() {
            return None;
        }
        (
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: root_symbol,
            },
            place.segments.as_slice(),
            borrowed,
        )
    } else {
        let symbol = root.as_symbol()?;
        let position = typed_trees::signature::state_parameter_position(
            program.state_parameters(state),
            symbol,
        )?;
        let position = u32::try_from(position).ok()?;
        let (parameter_index, parameter) = structural_parameters
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.position == position)?;
        if !matches!(
            parameter.access,
            CheckedStructuralAccess::Owned | CheckedStructuralAccess::MutableBorrow
        ) && !(required_access == language_semantics::ReferenceAccess::WriteOnly
            && parameter.access == CheckedStructuralAccess::WriteOnlyBorrow)
            || (parameter.access == CheckedStructuralAccess::MutableBorrow
                && parameter.multiplicity == Multiplicity::Linear)
            || (parameter.access == CheckedStructuralAccess::Owned && storage.segments.is_empty())
        {
            return None;
        }
        // The declaration's memberships describe the root and its children;
        // they do not revoke its exclusive storage authority. Window emission
        // retains the exact selected memberships and rejects any replacement
        // that would leave a qualified ancestor or the hole unestablished.
        (
            CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(parameter_index).ok()?,
            },
            storage.segments.as_slice(),
            parameter.access != CheckedStructuralAccess::Owned,
        )
    };
    let (machine, _) = validation::find_state(program, state.symbol)?;
    let path_root = match source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            PlaceRoot::Symbol(symbol)
        }
        _ => storage.root,
    };
    let (_, path) = super::calls::projected_evaluated_path(
        program,
        machine.symbol,
        state.symbol,
        statement_index,
        &CanonicalPlace {
            root: path_root,
            segments: path.to_vec(),
        },
        Some(selector),
    )?;
    Some(WindowPlace {
        storage,
        source,
        path,
        restoration_required,
    })
}

/// Rebase a place written through a reference-typed local onto the referent
/// storage it borrows, following plain reference copies (`let s = r`),
/// recast chains, and whole-local reassignments, until the root names a
/// parameter or machine rather than a local. Mirrors the checker rule that
/// windows key on the resolved storage place, not the access route.
pub(in crate::execution::terminal_unit) fn resolve_storage_place(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    statement_index: usize,
    root_symbol: SymbolHandle,
    path: &[PlaceSegment],
) -> (PlaceRoot, Vec<PlaceSegment>) {
    let mut root = PlaceRoot::Symbol(root_symbol);
    let mut segments = path.to_vec();
    for _ in 0..16 {
        let Some(symbol) = root.as_symbol() else {
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
    // Invocation scratch, keyed by the same canonical storage subject the
    // source checker uses. A dense parameter coordinate cannot name local
    // loans, and executable selector roles change at each statement.
    open: Vec<OpenWindow>,
}

struct OpenWindow {
    storage: CanonicalPlace,
    opening_binding: u32,
    restoration_required: bool,
}

/// The exact storage place an immutable, non-reference, ownership-carrying
/// local reads whole from an existing place. Such a local has the window
/// route by kind: the sequence admits it and then decides whether the place is
/// exclusive borrowed storage it can move out of. Copy locals never open a
/// window, and a mutable or reference-typed local binds nothing to restore.
pub(in crate::execution::terminal_unit) fn move_out_candidate(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    local: &typed_trees::statement::TableLocalData,
) -> Option<validation::CanonicalPlace> {
    if local.is_mutable
        || crate::checks::type_reference_is_reference(program, local.type_reference)
        || program.type_multiplicity(local.type_reference) == Multiplicity::Unrestricted
    {
        return None;
    }
    validation::canonical_place_from_expression_in_state(
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
        program: &dyn ::validation::CheckingInput,
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
        let place = window_place(
            program,
            state,
            statements,
            index,
            structural_parameters,
            &place,
            language_semantics::ReferenceAccess::Mutable,
            &|expression| {
                super::calls::value_index_segment(
                    program,
                    &facts.operators,
                    state,
                    index,
                    expression,
                )
            },
        )?;
        let type_identity = shapes.add_type(local.type_reference, binders, &[])?;
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal,
            type_identity: type_identity.clone(),
            multiplicity,
            local: crate::execution::terminal_unit::initialized_locals::initialized_local(
                program, machine, state, local,
            ),
        };
        self.bind_extraction(program, facts, place, result, local.initial_value)
    }

    /// Materialize an owned argument by vacating its borrowed storage. This is
    /// the same move as a named extraction, at the argument's actual evaluation
    /// point. Owned homes keep their ordinary call transfer; only an exclusive
    /// borrowed referent needs a separate restoration debt.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::execution::terminal_unit) fn move_call_operand(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
        statement_index: u32,
        expression: ExpressionHandle,
        type_reference: typed_trees::types::TypeReferenceHandle,
        binding_ordinal: u32,
    ) -> Option<(
        CheckedUnitEffectOperationPlan,
        CheckedUnitStructuralResultBindingPlan,
    )> {
        let ordinal = usize::try_from(statement_index).ok()?;
        let place = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            ordinal,
            expression,
        )?;
        let place = window_place(
            program,
            state,
            program.statement_table.statements(state.statement_nodes),
            ordinal,
            structural_parameters,
            &place,
            language_semantics::ReferenceAccess::Mutable,
            &|selector| {
                super::calls::value_index_segment(
                    program,
                    &facts.operators,
                    state,
                    ordinal,
                    selector,
                )
            },
        )?;
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal,
            type_identity: shapes.add_type(
                type_reference,
                &super::types::machine_binders(program, machine),
                &[],
            )?,
            multiplicity: program.type_multiplicity(type_reference),
            local: Default::default(),
        };
        // An owned call operand already transfers through its checked argument
        // schedule. Only extracting through a loan needs an extra detached
        // result and the original referent's restoration debt here.
        if !place.restoration_required {
            return None;
        }
        self.bind_extraction(program, facts, place, result, expression)
    }

    fn bind_extraction(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        place: WindowPlace,
        result: CheckedUnitStructuralResultBindingPlan,
        expression: ExpressionHandle,
    ) -> Option<(
        CheckedUnitEffectOperationPlan,
        CheckedUnitStructuralResultBindingPlan,
    )> {
        let type_identity = result.type_identity.clone();
        let binding_ordinal = result.binding_ordinal;
        if self
            .open
            .iter()
            .any(|open| same_storage_place(program, facts, &open.storage, &place.storage))
        {
            return None;
        }
        self.open.push(OpenWindow {
            storage: place.storage,
            opening_binding: binding_ordinal,
            restoration_required: place.restoration_required,
        });
        Some((
            CheckedUnitEffectOperationPlan::MoveStructuralField {
                result: result.clone(),
                expression,
                source: CheckedUnitStructuralArgumentPlan {
                    source: place.source,
                    path: place.path,
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
    /// A linear replacement may repair a vacancy, but cannot overwrite a
    /// live obligation. Decide that from the exact place's checked custody,
    /// not from the replacement's multiplicity alone.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn replace(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &super::types::ShapeCollector<'_>,
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
            shapes,
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
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &super::types::ShapeCollector<'_>,
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
        let index = usize::try_from(statement_index).ok()?;
        let mut destination_place = validation::canonical_place_from_expression_in_state(
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
        let access_place = destination_place.clone();
        let owned_source = owned_assignment_source(
            program,
            state,
            structural_parameters,
            index,
            &access_place,
            Some(produced.type_identity),
        );
        // Replacing a local's carrier does not write its prior referent. Keep
        // that binding coordinate for both old-value disposition and install.
        let (resolved_root, resolved_segments) = if owned_source.is_some() {
            (access_place.root, access_place.segments.clone())
        } else {
            resolve_storage_place(
                program,
                state,
                program.statement_table.statements(state.statement_nodes),
                index,
                match destination_place.root {
                    PlaceRoot::Symbol(symbol) => symbol,
                    _ => return None,
                },
                &destination_place.segments,
            )
        };
        let destination_place = CanonicalPlace {
            root: resolved_root,
            segments: resolved_segments,
        };
        let owned_rebind = owned_source.is_some();
        let (source, mut path, opening_binding, needs_displacement, needs_cleanup) =
            if let Some(source) = owned_source {
                let path = assignment_path(
                    program,
                    facts,
                    machine,
                    state,
                    statement_index,
                    assignment.target,
                    &destination_place,
                )?;
                let occupied = slot_occupied(facts, machine, state, index, &destination_place)
                    || validation::reference_result_custody::replaced_reference_carrier(
                        program,
                        facts,
                        machine.symbol,
                        state,
                        destination_place.root,
                        statement_index,
                        &path,
                    )
                    .is_some();
                let opening = self
                    .open
                    .iter()
                    .position(|open| {
                        same_storage_place(program, facts, &open.storage, &destination_place)
                    })
                    .map(|position| self.open.remove(position).opening_binding);
                (source, path, opening, false, occupied)
            } else {
                let selectors = TargetSelectors::resolve(
                    program,
                    facts,
                    machine,
                    state,
                    statement_index,
                    assignment.target,
                )?;
                let place = window_place(
                    program,
                    state,
                    program.statement_table.statements(state.statement_nodes),
                    index,
                    structural_parameters,
                    &access_place,
                    language_semantics::ReferenceAccess::WriteOnly,
                    &|expression| selectors.runtime_segment(expression),
                )?;
                let opening_binding = match self.open.iter().position(|open| {
                    same_storage_place(program, facts, &open.storage, &place.storage)
                }) {
                    Some(position) => Some(self.open.remove(position).opening_binding),
                    None => None,
                };
                // Intact replacement needs no observation when the exact
                // source type grants no-code disposal. Affine incoming values
                // still transfer once; nominal cleanup and conserved contents
                // retain the ordinary displacement and cleanup route.
                let freely_discardable = produced.multiplicity == Multiplicity::Unrestricted
                    || produced.multiplicity == Multiplicity::Affine
                        && shapes
                            .types
                            .get(&produced.type_identity)
                            .is_some_and(|declaration| {
                                declaration.no_code_affine_cleanup || declaration.descriptor_cleanup
                            });
                let needs_displacement = opening_binding.is_none() && !freely_discardable;
                (
                    place.source,
                    place.path,
                    opening_binding.or(needs_displacement.then_some(displaced_ordinal)),
                    needs_displacement,
                    needs_displacement && produced.multiplicity == Multiplicity::Affine,
                )
            };
        let declared = validation::canonical_place_type_reference(
            program,
            state.symbol,
            index,
            &access_place,
        )?;
        if !path.is_empty()
            && validation::reference_shell(program, declared).is_some()
            && validation::stored_structural_type_identity(program, declared, &[], &[])
                != produced.type_identity
        {
            path.push(CheckedUnitStructuralPathSegment::Referent);
        }
        // An owned old value dies through its checked contextual disposition,
        // including an owner-authorized linear hook. A borrowed displacement
        // still needs a separate exact linear restoration contract.
        if produced.multiplicity == Multiplicity::Linear && needs_displacement {
            return None;
        }
        let place = CheckedUnitStructuralArgumentPlan {
            source,
            path,
            type_identity: produced.type_identity.clone(),
            access: CheckedStructuralAccess::Owned,
        };
        let old_value = if needs_displacement {
            CheckedUnitStructuralArgumentPlan::structural_result(
                displaced_ordinal,
                produced.type_identity.clone(),
                CheckedStructuralAccess::Owned,
            )
        } else {
            place.clone()
        };
        // The store names the window by its authored root; the move-out and
        // cleanup keep the ledger's dense spelling of the same place.
        let root = match place.source {
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                checked_trees::CheckedStoreRoot::Parameter {
                    position: structural_parameters
                        .get(parameter_index as usize)?
                        .position,
                }
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                checked_trees::CheckedStoreRoot::Local { symbol }
            }
            _ => return None,
        };
        let type_identity = produced.type_identity.clone();
        let leaf = if owned_rebind && place.path.is_empty() {
            checked_trees::CheckedStoreLeaf::OwnedRebind { type_identity }
        } else {
            checked_trees::CheckedStoreLeaf::StructuralStore {
                type_identity,
                opening_binding,
            }
        };
        Some(FieldReplacement {
            move_out: needs_displacement.then(|| {
                CheckedUnitEffectOperationPlan::MoveStructuralField {
                    result: CheckedUnitStructuralResultBindingPlan {
                        statement_index,
                        binding_ordinal: displaced_ordinal,
                        type_identity: produced.type_identity.clone(),
                        multiplicity: produced.multiplicity,
                        local: Default::default(),
                    },
                    source: place.clone(),
                    expression: assignment.target,
                }
            }),
            cleanup: needs_cleanup.then(|| {
                CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    selected_regions: Vec::new(),
                    boundary: checked_trees::CheckedUnitCleanupBoundary::Replacement {
                        statement_index,
                        destination: place.clone(),
                        old_value: old_value.clone(),
                        receiver_path: indices
                            .iter()
                            .copied()
                            .map(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex)
                            .collect(),
                    },
                    // This storage marker is completed from the authoritative
                    // death disposition once the hook catalog exists. Owned
                    // children retire in place; a borrowed replacement has a
                    // separately displaced value and restoration debt.
                    cleanup_actions: vec![checked_trees::CheckedNormalCleanupAction::Discard(
                        CheckedUnitPartialAffineDiscardPlan {
                            source: old_value.source,
                            path: old_value.path,
                            type_identity: produced.type_identity.clone(),
                            case_conditions: Default::default(),
                        },
                    )],
                }
            }),
            store: CheckedUnitEffectOperationPlan::new_store(
                statement_index,
                root,
                place.path,
                leaf,
                checked_trees::CheckedStoreValue::Place(value),
            ),
        })
    }

    /// Borrowed holes must be repaired before the body completes. Owned
    /// projected moves may leave partial custody; the source checker owns
    /// their residual disposal. A borrowed
    /// leftover is a sequencing fault rather than a language decision.
    pub(super) fn is_closed(&self) -> bool {
        self.open.iter().all(|open| !open.restoration_required)
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

/// Whether the permission producer selected an old-value death for the store
/// at `statement_index` over `destination`: the slot holds a value the store
/// replaces. An absent construction slot (a declared local's field before its
/// first store) creates no displaced value and owes no disposal. This follows
/// checked custody, not the initializer's spelling.
fn slot_occupied(
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    destination: &CanonicalPlace,
) -> bool {
    facts
        .contextual_cleanup
        .dispositions
        .iter()
        .any(|(_, disposition)| {
            disposition.machine == machine.symbol
                && disposition.state == state.symbol
                && disposition.source
                    == language_semantics::PermissionEventSource::Statement { statement_index }
                && disposition.root == destination.root
                && facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(disposition.path)
                    .starts_with(&destination.segments)
        })
}
