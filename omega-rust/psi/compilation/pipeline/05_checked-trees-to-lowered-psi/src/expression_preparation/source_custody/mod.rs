//! Source custody: lookups that still re-read the authored typed program
//! (`checked.typed`) because stage 04 does not yet record the value lowering
//! needs. Under the 2026-10-04 lowering simplification ruling each value moves
//! into the stage 04 plan and its reader here is deleted; the module goes with
//! the last of them (TASKS.md STAGE05-SOURCE-REREAD-DELETED). Do not add a
//! reader here: record the value in the plan instead.
//!
//! What remains:
//!
//! - `parameter_storage`, `borrowed_parameter_custody`, `primitive_references`,
//!   `structural_places`, `dispatch_subjects` and `view_ranges` answer storage,
//!   borrow, place and dispatch questions from the authored source.
//! - Some children still refuse shapes stage 05 cannot lower. Those refusals
//!   stay until the route items admit the shapes; the validators that only
//!   re-confirmed a plan row against its source are gone.

use super::{CheckedScalarExpressionRole, CheckedTrees, LoweringError, PrimitiveType, unsupported};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::{StatementNode, TransitionGuardNode};

// Reads, storage and borrows: the authored binding, parameter, place or
// record field a scalar read names, the current state's mutable entry
// storage, and primitive local borrows at their checked call occurrence.
pub(crate) mod borrow_occurrences;
mod parameters;
pub(crate) mod primitive_references;
mod storage_reads;
pub(crate) mod structural_places;

// Calls and selected comparisons: computed invocations, the checked flow
// fact behind a Unit scalar call, and the boundary application a selected
// comparison names.
pub(crate) mod comparisons;
pub(crate) mod computation_calls;
pub(crate) mod flow_calls;

// Constructed values: array and case constructors, structural construction
// and dispatch, and the literal or operator meaning of retained operations.
pub(crate) mod array_sources;
pub(crate) mod case_sources;
pub(crate) mod enum_equality;
pub(crate) mod structural;
pub(crate) mod value_correspondence;
pub(crate) mod view_ranges;

// Control: successor rosters, ordered guarded exits, complementary guard
// pairs, and the subject a closed-case dispatch tests.
pub(crate) mod dispatch_subjects;
pub(crate) mod guard_complement;
pub(crate) mod guarded_exits;
pub(crate) mod successors;

pub(crate) use parameters::{borrowed_parameter_custody, parameter_storage};

pub(crate) fn supported_mutable_parameter(primitive: PrimitiveType) -> bool {
    matches!(
        primitive,
        PrimitiveType::Bool
            | PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    )
}

/// The state parameter a `Parameter { parameter_index }` source names. The
/// index is dense over structural parameters only: an `[erased]` binding or a
/// primitive-typed one occupies no position. `missing` is the lane's
/// diagnostic for an index past the last structural parameter.
fn structural_parameter_at<'a>(
    checked: &CheckedTrees,
    parameters: &'a [checked_trees::signature::StateParameter],
    parameter_index: u32,
    missing: &'static str,
) -> Result<&'a checked_trees::signature::StateParameter, LoweringError> {
    parameters
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased()
                && checked
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .nth(parameter_index as usize)
        .ok_or(LoweringError::Unsupported(missing))
}

/// The one authored local of `state` that binds `symbol`, declared before
/// statement `before` with an initializer or authenticated default storage.
/// `missing` is the lane's
/// diagnostic when no local binds `symbol`; `unestablished` covers a second
/// binding, a declaration at or after `before`, and a stale initializer.
fn preceding_local<'a>(
    checked: &'a CheckedTrees,
    state: &checked_trees::state::State,
    symbol: symbols::SymbolHandle,
    before: usize,
    missing: &'static str,
    unestablished: &'static str,
) -> Result<&'a checked_trees::statement::TableLocalData, LoweringError> {
    let mut locals = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .filter_map(|(ordinal, statement)| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some((ordinal, local)),
            _ => None,
        });
    let (ordinal, local) = locals.next().ok_or(LoweringError::Unsupported(missing))?;
    if locals.next().is_some()
        || ordinal >= before
        || if local.initial_value.is_valid() {
            !checked
                .expression_table
                .expression_is_valid(local.initial_value)
        } else {
            crate::emission::call_source_custody::projected_receivers::owned_local_declaration(
                checked, state, before, symbol,
            )
            .is_none()
        }
    {
        return unsupported(unestablished);
    }
    Ok(local)
}

/// The source-role ordinal of the `let` at `statement`: the authored local
/// ordinal its `LocalInitializer` and `CallArgument` roles carry, not the
/// dense binding ordinal its plan assigns.
pub(crate) fn source_local_ordinal(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
) -> Result<u32, LoweringError> {
    let (_, state) = crate::declarations::state_declaration(checked, state)?;
    u32::try_from(
        checked
            .typed
            .immutable_scalar_locals_before(state, statement as usize),
    )
    .map_err(|_| LoweringError::Unsupported("scalar local ordinal exceeds u32"))
}
