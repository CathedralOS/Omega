//! The acceptance vocabulary the admissibility views share: the
//! `AcceptanceView` trait (`view`), the seven `AcceptanceDimension`s
//! (`dimension`), one dimension's `AcceptanceCheck` with its verdict and
//! provenance (`check`), the whole-row `AcceptanceSummary` and
//! `AcceptanceVerdict` (`summary`), and the borrowed wrapper structs for a
//! state, statement, call, exit, operator use or state operation
//! (`wrappers`).

mod check;
mod dimension;
mod summary;
mod view;
mod wrappers;

pub use check::{AcceptanceCheck, AcceptanceCheckProvenance, AcceptanceCheckVerdict};
pub use dimension::AcceptanceDimension;
pub use summary::{AcceptanceSummary, AcceptanceVerdict};
pub use view::AcceptanceView;
pub use wrappers::{
    CallAcceptance, ExitAcceptance, OperatorAcceptance, StateAcceptance, StateOperationAcceptance,
    StateOperationAcceptanceKind, StatementAcceptance,
};
