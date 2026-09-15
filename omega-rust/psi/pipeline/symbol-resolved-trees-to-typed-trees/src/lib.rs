#![forbid(unsafe_code)]

//! Attaches type and signature meaning to Psi symbol-resolved source trees.
//!
//! Start at `lowerer.rs`: [`lower_symbol_resolved_trees`] owns complete typing.
//! It validates resolved meaning, lowers declarations in dependency order,
//! retains initializer custody, then settles and normalizes typed trees. Its
//! `seeded_continuation` child owns append-only admission and transactional
//! recovery; both routes use the same lowering operations. `declarations`
//! types each declaration form, `expressions` types expressions and
//! statements, and `type_reference` resolves type references and their
//! constraints.

mod declarations;
mod expressions;
mod lowerer;
mod type_reference;

pub use lowerer::seeded_continuation::{
    SeededContinuationError, SeededTypingBase, lower_seeded_extension,
    lower_symbol_resolved_trees_to_seeded_base, retained_typed_base_is_exact_prefix,
};
pub use lowerer::{lower_symbol_resolved_trees, lower_symbol_resolved_trees_owned};
