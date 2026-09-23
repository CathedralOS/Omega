//! Ordinary move-out/restore statements as borrowed-window operations. A
//! `let local: T = root.field...;` move opens a window and a later
//! `root.field... = move local` repair closes it; the statement sequence
//! emits one MoveStructuralField/StoreStructuralField row per statement, and
//! the lowered emitter feeds them through `BorrowedWindowLedger`. Any other
//! statement between the two is the sequence's ordinary business.

use std::collections::BTreeMap;

use super::{
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralResultBindingPlan, Multiplicity,
    StatementNode, SymbolHandle, TypedTrees,
};
use crate::execution::terminal_unit::types::ShapeCollector;

use crate::execution::terminal_unit::types::terminal_field_identity;
use crate::flow::CanonicalPlace;
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// The exact field chain beneath an exclusive borrowed structural parameter.
/// Whole-root moves and index/referent segments open no pinned window.
///
/// The place is first keyed on the resolved *storage* it names — the same
/// rebase the checker performs: a route through a reference-typed local
/// (`let r = &mut self; r.f`) names the same hole as the owner path
/// (`self.f`), so both must land on one window.
pub(in crate::execution::terminal_unit) fn window_place(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    statement_index: usize,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    place: &CanonicalPlace,
) -> Option<(u32, Vec<CheckedUnitStructuralPathSegment>)> {
    let PlaceRoot::Symbol(root_symbol) = place.root else {
        return None;
    };
    let (root, segments) = resolve_storage_place(
        program,
        machine,
        state,
        statements,
        statement_index,
        root_symbol,
        &place.segments,
    );
    let position = if root == PlaceRoot::Symbol(machine.symbol) {
        program
            .state_parameters(state)
            .iter()
            .position(|parameter| parameter.is_self)
    } else {
        let PlaceRoot::Symbol(symbol) = root else {
            return None;
        };
        program
            .state_parameters(state)
            .iter()
            .position(|parameter| parameter.symbol == symbol)
    }?;
    let position = u32::try_from(position).ok()?;
    let parameter = structural_parameters
        .iter()
        .find(|parameter| parameter.position == position)?;
    if parameter.access != CheckedStructuralAccess::MutableBorrow
        || parameter.multiplicity == Multiplicity::Linear
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
    {
        return None;
    }
    if segments.is_empty() {
        return None;
    }
    let path = segments
        .iter()
        .map(|segment| match segment {
            PlaceSegment::Field { symbol } => terminal_field_identity(program, *symbol)
                .map(CheckedUnitStructuralPathSegment::Field),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some((position, path))
}

/// Rebase a place written through a reference-typed local onto the referent
/// storage it borrows, following plain reference copies (`let s = r`),
/// recast chains, and whole-local reassignments, until the root names a
/// parameter or machine rather than a local. Mirrors the checker rule that
/// windows key on the resolved storage place, not the access route.
pub(in crate::execution::terminal_unit) fn resolve_storage_place(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    statement_index: usize,
    root_symbol: SymbolHandle,
    path: &[PlaceSegment],
) -> (PlaceRoot, Vec<PlaceSegment>) {
    let mut root =
        crate::flow::normalized_event_place_root(program, PlaceRoot::Symbol(root_symbol));
    let mut segments = path.to_vec();
    for _ in 0..16 {
        let PlaceRoot::Symbol(symbol) = root else {
            break;
        };
        let Some(source) =
            local_reference_source(program, state, statements, statement_index, symbol)
        else {
            break;
        };
        let Some(source) = reference_source_place(program, state.symbol, statement_index, source)
        else {
            break;
        };
        let PlaceRoot::Symbol(..) = source.root else {
            break;
        };
        let mut rebased = source.segments;
        rebased.extend_from_slice(&segments);
        root = crate::flow::normalized_event_place_root(program, source.root);
        let mut rebased_place = CanonicalPlace {
            root,
            segments: rebased,
        };
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
fn local_reference_source(
    program: &TypedTrees,
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
    if !type_reference_is_reference(program, declaration.type_reference) {
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
                (target.root == PlaceRoot::Symbol(symbol) && target.segments.is_empty())
                    .then_some(assignment.value)
            }
            _ => None,
        })
}

/// The referent place a reference initializer captures: explicit borrows and
/// whole-place recasts unwrap to their target, and a plain place expression
/// names the place the new local aliases.
fn reference_source_place(
    program: &TypedTrees,
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

/// Whether the local's declared type is a reference, following constrained
/// base types.
fn type_reference_is_reference(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_is_reference(program, *base_type)
        }
        _ => false,
    }
}

/// The windows a statement sequence has opened and not yet repaired, with the
/// move-out locals whose consuming store retires each binding exactly once.
#[derive(Default)]
pub(super) struct OpenWindows {
    open: BTreeMap<(u32, Vec<CheckedUnitStructuralPathSegment>), u32>,
    locals: Vec<(SymbolHandle, (u32, String))>,
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
        || type_reference_is_reference(program, local.type_reference)
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
            machine,
            state,
            statements,
            index,
            structural_parameters,
            &place,
        )?;
        let type_identity = shapes.add_type(local.type_reference, binders, &[])?;
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal,
            type_identity: type_identity.clone(),
            multiplicity,
        };
        self.open.insert((position, path.clone()), binding_ordinal);
        self.locals
            .push((local.symbol, (binding_ordinal, type_identity.clone())));
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

    /// `root.field... = move local` restoring a moved-out value to the exact
    /// place its move opened. Returns None when the value is not one of this
    /// sequence's move-out locals or the destination names no open window.
    pub(super) fn restore(
        &mut self,
        program: &TypedTrees,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
        statement_index: u32,
        assignment: &typed_trees::statement::TableAssignment,
    ) -> Option<CheckedUnitEffectOperationPlan> {
        let statements = program.statement_table.statements(state.statement_nodes);
        let index = usize::try_from(statement_index).ok()?;
        let value_place = crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            index,
            assignment.value,
        )?;
        let PlaceRoot::Symbol(value_root) = value_place.root else {
            return None;
        };
        if !value_place.segments.is_empty() {
            return None;
        }
        let bound = self
            .locals
            .iter()
            .position(|(symbol, _)| *symbol == value_root)?;
        let destination_place = crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            index,
            assignment.target,
        )?;
        let (position, path) = window_place(
            program,
            machine,
            state,
            statements,
            index,
            structural_parameters,
            &destination_place,
        )?;
        self.open.remove(&(position, path.clone()))?;
        let (binding_ordinal, value_type) = self.locals.remove(bound).1;
        Some(CheckedUnitEffectOperationPlan::StoreStructuralField {
            statement_index,
            destination: CheckedUnitStructuralArgumentPlan {
                source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: position,
                },
                path,
                type_identity: value_type.clone(),
                access: CheckedStructuralAccess::Owned,
            },
            value: CheckedUnitStructuralArgumentPlan {
                source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal,
                },
                path: Vec::new(),
                type_identity: value_type,
                access: CheckedStructuralAccess::Owned,
            },
        })
    }

    /// Every opened window must be repaired before the body completes; the
    /// checker already rejects an open window at the state's exits, so a
    /// leftover here is a sequencing fault rather than a language decision.
    pub(super) fn is_closed(&self) -> bool {
        self.open.is_empty() && self.locals.is_empty()
    }
}
