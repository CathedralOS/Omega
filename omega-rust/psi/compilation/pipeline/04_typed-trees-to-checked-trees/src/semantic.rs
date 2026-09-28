//! Semantic facts: what the checked program knows about its own values.
//!
//! This area answers one question in three parts, so its children are the
//! parts rather than three unrelated roots:
//!
//! - [`facts`] states the knowledge itself — the contract, field-domain and
//!   point facts a checked machine publishes for later stages to replay.
//! - [`calls`] names the coordinates a fact is stated about: which call site,
//!   which target parameters, which arguments. Fact construction and every
//!   later consumer ask here instead of re-deriving a call's shape, which is
//!   why this is the area's shared vocabulary rather than a private helper of
//!   `facts`.
//! - [`places`] renders the storage a fact is stated about, built from a call
//!   coordinate and a contract term.
//!
//! The dependency direction inside the area is `places` -> {`calls`,
//! `facts`} and `facts` -> `calls`; nothing here depends on the checking
//! rules that consume it.

pub(crate) mod calls;
pub(crate) mod facts;
pub(crate) mod places;
