#![forbid(unsafe_code)]

//! Self-contained, target-neutral terminal-Psi semantics.
//!
//! The frozen v1 vocabulary contains integer constants and a straight-line
//! chain of explicit jump/return edges; v2 adds Boolean constants, v3 adds
//! explicit wrapping integer addition, v4 adds saturating integer addition,
//! v5 adds wrapping integer subtraction, v6 adds saturating integer
//! subtraction, v7 adds wrapping integer multiplication, v8 adds saturating
//! integer multiplication, v9 adds proof-only structural-place content
//! conservation, v10 adds identity-preserving claim reshuffles, v11 adds
//! stable sum-case structural paths, v12 adds exact authored partition
//! substitutions, v13 adds ordered structural conditional edges, v14 adds
//! independent machine-local entry-claim bindings, v15 adds Boolean logical
//! negation, v16 adds nominal proposition declarations and normalized
//! application identities, v17 adds total Boolean equality, v18 adds total
//! integer equality, v19 adds signedness-aware integer ordering, v20 adds total
//! integer bitwise operations, v21 adds wrapping shifts, v22-v24 add the
//! legacy crash schema, v25 adds integer complement, v26-v27 add widening and
//! the address carrier, v28-v33 add proof-gated exact casts, shifts, addition,
//! subtraction, and multiplication, and current v34 adds proof-gated exact
//! division.
//! This small executable slice
//! exercises values, control, bodyful contracts, and one width-relative
//! arithmetic policy plus Boolean control without pretending that other
//! arithmetic policies have already been specified. Every later operation extends this
//! vocabulary together with its execution transition, generated facts, proof
//! rule, and lowering contract.

mod identity;
mod module;

pub use identity::*;
pub use module::*;
