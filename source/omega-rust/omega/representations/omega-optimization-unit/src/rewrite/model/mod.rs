//! Optimizer module role: stage group. Semantic vocabulary for immutable Psi rewrite plans, witnesses, identities, and candidate contracts.
//!
//! Foundations define source locations, provenance, and substitutions. Scalar
//! evaluation and SCCP own constant evidence; CFG and scalar plans name exact
//! mutations; contracts bind those plans into independently validated candidates.

mod cfg_rewrite_plans;
mod contracts;
mod foundations;
mod scalar_evaluation;
mod scalar_rewrite_plans;
mod sccp;

pub use cfg_rewrite_plans::*;
pub use contracts::*;
pub use foundations::*;
pub use scalar_evaluation::*;
pub use scalar_rewrite_plans::*;
pub use sccp::*;
