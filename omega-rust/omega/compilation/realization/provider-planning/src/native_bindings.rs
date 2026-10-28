//! Derive native candidates from authenticated, completed Build choices.
//!
//! Source admission supplies exact Product requirements. Evaluation evidence
//! retains the actual Terminal program and measured activation; neither is an
//! executable permission. The native provider path still joins the complete
//! requirement schema, calling policy, target normalization and opaque promise.

mod evaluation;
pub use evaluation::{NativeBuildEvaluation, NativeBuildSupply};

mod plans;
pub use plans::NativeBuildRowOrigin;
pub(crate) use plans::derive;
