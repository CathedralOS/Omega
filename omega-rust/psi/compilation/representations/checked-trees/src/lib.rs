//! Checked source trees and their independently established facts.
//!
//! Begin at [`checked_trees`]; validation adds facts to typed trees without
//! replacing them with a second copy of the same source vocabulary.

pub mod checked_trees;
pub use checked_trees::facts::ConstantPredicateObligation;
pub use checked_trees::facts::{
    IntegerRangeDependencies, IntegerRangeModel, IntegerRangeObligation, IntegerRangeRecipe,
};
pub use checked_trees::*;
pub(crate) use optimization::digest_encoder;

// Downstream realization reads typed vocabulary through checked custody:
// `CheckedTrees` carries the typed program, so its contained tree surface is
// reachable here rather than through a second direct representation edge.
pub use typed_trees;

#[cfg(test)]
mod source_guards;

pub use checked_trees::facts::{CheckedScalarReturnOrigin, CheckedScalarReturnQualification};
pub use checked_trees::{CheckedCleanupCaseCondition, CheckedCleanupControlPlans};

/// The identity request a consumer builds when it asks the typed program
/// behind a checked carrier for a binder-aware type identity.
pub use typed_trees::type_identity::TypeIdentityRequest;
