//! Checked source trees and their independently established facts.
//!
//! Begin at [`checked_trees`]; validation adds facts to typed trees without
//! replacing them with a second copy of the same source vocabulary.

pub mod checked_trees;

pub use checked_trees::*;
/// The identity request a consumer builds when it asks the typed program
/// behind a checked carrier for a binder-aware type identity.
pub use typed_trees::type_identity::TypeIdentityRequest;
