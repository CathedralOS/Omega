//! Acceptance views over checked facts: for a state and each of its
//! statements, calls, exits and operator uses, how much evidence checking
//! retained on each acceptance dimension.
//!
//! These are read-only views, not stored records: each wrapper borrows
//! `CheckFacts` and the row it describes, which is a flow state, statement,
//! call or exit fact or a contract operator use from the proof facts.
//! `CheckedTrees::state_acceptance` (in `state`) finds a state's
//! `FlowStateFact` and returns its `StateAcceptance`, whose `operations`
//! yields a `StateOperationAcceptance` for each statement, call, exit and
//! operator use. Each wrapper implements `AcceptanceView::summary`, which
//! counts the evidence behind its row on the seven `AcceptanceDimension`s
//! (borrow, proof, service reach, suspension, blocking, boundaries,
//! termination) and builds the summary with `AcceptanceSummary::accepted`.
//! The only callers are tests in `typed-trees-to-checked-trees`
//! (`src/tests/admissibility.rs` and `src/tests/borrow/certificates/`).
//!
//! `types` defines the view trait, the dimensions, the per-dimension check,
//! the summary and the wrapper structs; `state`, `statement`, `call`,
//! `exit`, `operator` and `operation` implement each wrapper's summary; and
//! `evidence_counts` holds the counting helpers they share.

mod call;
mod evidence_counts;
mod exit;
mod operation;
mod operator;
mod state;
mod statement;
mod types;

pub use types::{
    AcceptanceCheck, AcceptanceCheckProvenance, AcceptanceCheckVerdict, AcceptanceDimension,
    AcceptanceSummary, AcceptanceVerdict, AcceptanceView, CallAcceptance, ExitAcceptance,
    OperatorAcceptance, StateAcceptance, StateOperationAcceptance, StateOperationAcceptanceKind,
    StatementAcceptance,
};
