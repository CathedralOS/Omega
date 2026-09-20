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

/// One window body admitted by `borrowed_window_shape`: a leading run of
/// move-out locals followed by whole-field restore stores.
pub(super) struct BorrowedWindowShape {
    /// Leading LocalData statements consumed by move-out rows; the
    /// call-statement splice starts after them.
    pub(super) local_count: usize,
    /// Window operations in authored statement order.
    pub(super) operations: Vec<CheckedUnitEffectOperationPlan>,
}

/// The exact field chain beneath an exclusive borrowed structural parameter.
/// Whole-root moves and index/referent segments open no pinned window.
fn window_place(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    place: &CanonicalPlace,
) -> Option<(u32, Vec<CheckedUnitStructuralPathSegment>)> {
    let PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    let position = program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == root)?;
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
    if place.segments.is_empty() {
        return None;
    }
    let path = place
        .segments
        .iter()
        .map(|segment| match segment {
            PlaceSegment::Field { symbol } => terminal_field_identity(program, *symbol)
                .map(CheckedUnitStructuralPathSegment::Field),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some((position, path))
}

/// Admit the borrowed-window statement shape: move-out locals then restore
/// stores, each store closing the exact window an earlier move opened. Any
/// other statement returns None so the ordinary families still adjudicate the
/// body.
pub(super) fn borrowed_window_shape(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
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
    let mut next_binding = 0_u32;
    for (index, statement) in statements.iter().enumerate() {
        let statement_index = u32::try_from(index).ok()?;
        match statement {
            StatementNode::LocalData(local) => {
                let place = crate::flow::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    index,
                    local.initial_value,
                )?;
                let (position, path) = window_place(program, state, structural_parameters, &place)?;
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
                let (position, path) =
                    window_place(program, state, structural_parameters, &destination_place)?;
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
    })
}
