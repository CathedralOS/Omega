//! Ordinary move-out/restore statements admitted as borrowed-window
//! operations. A body of `let local: T = root.field...;` moves followed by
//! `root.field... = move local` repairs maps onto the checked
//! MoveStructuralField/StoreStructuralField rows the lowered emitter feeds
//! through `BorrowedWindowLedger`.

use std::collections::BTreeMap;

use super::{
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralResultBindingPlan, Multiplicity,
    StatementNode, SymbolHandle, TypedTrees,
};
use crate::execution::terminal_unit::ShapeCollector;
use crate::execution::terminal_unit::types::terminal_field_identity;
use crate::flow::CanonicalPlace;
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// One window body admitted by `borrowed_window_shape`: a leading run of
/// move-out locals followed by whole-field restore stores.
pub(super) struct BorrowedWindowShape {
    /// Leading LocalData statements consumed by move-out rows; the
    /// call-statement splice starts after them.
    pub(super) local_count: usize,
    /// Window operations in authored statement order.
    pub(super) operations: Vec<CheckedUnitEffectOperationPlan>,
    /// Reference-typed alias declarations the shape consumed without an
    /// operation — they rebase onto their referent storage rather than open
    /// a window of their own.
    pub(super) reference_locals: usize,
}

/// The exact field chain beneath an exclusive borrowed structural parameter.
/// Whole-root moves and index/referent segments open no pinned window.
///
/// The place is first keyed on the resolved *storage* it names — the same
/// rebase the checker performs: a route through a reference-typed local
/// (`let r = &mut self; r.f`) names the same hole as the owner path
/// (`self.f`), so both must land on one window.
fn window_place(
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
fn resolve_storage_place(
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

/// Admit the borrowed-window statement shape: move-out locals then restore
/// stores, each store closing the exact window an earlier move opened. Any
/// other statement returns None so the ordinary families still adjudicate the
/// body.
pub(super) fn borrowed_window_shape(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    statements: &[StatementNode],
    binders: &[(SymbolHandle, String)],
) -> Option<BorrowedWindowShape> {
    if statements.is_empty() {
        return None;
    }
    let local_count = statements
        .iter()
        .take_while(|statement| matches!(statement, StatementNode::LocalData(_)))
        .count();
    if local_count == 0 || local_count == statements.len() {
        return None;
    }
    let mut operations = Vec::new();
    // Each open window's exact place, so a restore names the same hole the
    // move opened and every move sees one repair.
    let mut open: BTreeMap<(u32, Vec<CheckedUnitStructuralPathSegment>), u32> = BTreeMap::new();
    // Bound move-out locals by symbol until their consuming store retires the
    // binding exactly once.
    let mut locals: Vec<(SymbolHandle, (u32, String))> = Vec::new();
    let mut reference_locals = 0_usize;
    let mut next_binding = 0_u32;
    for (index, statement) in statements.iter().enumerate() {
        let statement_index = u32::try_from(index).ok()?;
        match statement {
            StatementNode::LocalData(local) => {
                // A reference-typed local is an alias declaration, not a
                // move-out: it carries no window and binds no result.
                if type_reference_is_reference(program, local.type_reference) {
                    reference_locals += 1;
                    continue;
                }
                let place = crate::flow::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    index,
                    local.initial_value,
                )?;
                let (position, path) = window_place(
                    program,
                    machine,
                    state,
                    statements,
                    index,
                    structural_parameters,
                    &place,
                )?;
                let multiplicity = program.type_multiplicity(local.type_reference);
                if multiplicity == Multiplicity::Unrestricted {
                    return None;
                }
                let type_identity = shapes.add_type(local.type_reference, binders, &[])?;
                let result = CheckedUnitStructuralResultBindingPlan {
                    statement_index,
                    binding_ordinal: next_binding,
                    type_identity: type_identity.clone(),
                    multiplicity,
                };
                open.insert((position, path.clone()), next_binding);
                locals.push((local.symbol, (next_binding, type_identity.clone())));
                next_binding = next_binding.checked_add(1)?;
                operations.push(CheckedUnitEffectOperationPlan::MoveStructuralField {
                    result,
                    source: CheckedUnitStructuralArgumentPlan {
                        source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                            parameter_index: position,
                        },
                        path,
                        type_identity,
                        access: CheckedStructuralAccess::Owned,
                    },
                });
            }
            StatementNode::Assignment(assignment) => {
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
                let (binding_ordinal, value_type) = locals
                    .iter()
                    .position(|(symbol, _)| *symbol == value_root)
                    .map(|bound| locals.remove(bound).1)?;
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
                open.remove(&(position, path.clone()))?;
                operations.push(CheckedUnitEffectOperationPlan::StoreStructuralField {
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
                });
            }
            _ => return None,
        }
    }
    if !open.is_empty() {
        return None;
    }
    Some(BorrowedWindowShape {
        local_count,
        operations,
        reference_locals,
    })
}
