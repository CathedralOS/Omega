//! Source custody: lookups that still re-read the authored typed program
//! (`checked.typed`) because stage 04 does not yet record the value lowering
//! needs. Under the 2026-10-04 lowering simplification ruling each value moves
//! into the stage 04 plan and its reader here is deleted; the module goes with
//! the last of them (TASKS.md STAGE05-SOURCE-REREAD-DELETED). Do not add a
//! reader here: record the value in the plan instead.
//!
//! What remains:
//!
//! - `structural_places` answers place questions from the authored source.
//! - Some children still refuse shapes stage 05 cannot lower. Those refusals
//!   stay until the route items admit the shapes; the validators that only
//!   re-confirmed a plan row against its source are gone.

use super::{CheckedTrees, LoweringError, PrimitiveType, unsupported};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};

// Reads: the authored binding, parameter, place or record field a scalar
// read names.
pub(crate) mod structural_places;

// Selected comparisons: a comparison's Terminal meaning, and the authored
// operand of a byte comparison or predicate.
pub(crate) mod comparisons;
pub(crate) mod computation_calls;

// Constructed values: structural view carriers and byte literals.
pub(crate) mod structural;

pub(crate) fn supported_mutable_parameter(primitive: PrimitiveType) -> bool {
    matches!(primitive, PrimitiveType::Bool) || primitive.is_fixed_integer()
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
