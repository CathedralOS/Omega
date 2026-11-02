//! The shared scalar predicate judgment, used both by source proof contexts
//! and retained constant obligations when a selected target closes their leaves.
pub(in crate::checks::contracts) use checked_trees::checked_trees::facts::scalar_predicates::{
    ScalarValue, closed_boolean_value, evaluate, evaluate_for_atoms, has_builtin_operators,
};
