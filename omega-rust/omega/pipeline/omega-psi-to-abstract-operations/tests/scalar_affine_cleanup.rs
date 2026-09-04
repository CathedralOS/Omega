//! Scalar-affine cleanup integration tests by lowering and custody boundary.
//!
//! Exact behavior descends through payloadless-case fencing, jump cleanup,
//! proof-bearing scalar cleanup, and structural-return custody. Typed fixture
//! identities remain in one explicit support leaf.

#[path = "scalar_affine_cleanup/jump_cleanup.rs"]
mod jump_cleanup;
#[path = "scalar_affine_cleanup/payloadless_case.rs"]
mod payloadless_case;
#[path = "scalar_affine_cleanup/scalar_cleanup_proofs.rs"]
mod scalar_cleanup_proofs;
#[path = "scalar_affine_cleanup/structural_return.rs"]
mod structural_return;
#[path = "scalar_affine_cleanup/support.rs"]
mod support;
