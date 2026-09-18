//! Checked custody for calls through local named dynamic values.
//!
//! Scalar-result calls remain in this owner. Unit-returning statement calls
//! use the focused `unit` child so they cannot acquire a fabricated result
//! carrier by sharing the scalar path.
//!
//! This module is intentionally independent from Terminal Psi. It consumes
//! typed coordinates once, joins them to checked conformance, contract, value,
//! and service-reach facts, and publishes an all-or-nothing source-handle-free
//! roster for later checked-to-Terminal composition.
//!
//! This file builds the dynamic dispatch plans. `scalar_call_transactions.rs`
//! builds scalar call transactions and descriptor transfers,
//! `forwarded_calls.rs` resolves forwarded dynamic calls, `receivers.rs`
//! locates dynamic receivers, `scalar_call_plans.rs` builds one checked
//! dynamic scalar call and `realization_bodies.rs` checks realization
//! scalar bodies; `join.rs` and `unit.rs` carry joins and Unit calls.

mod forwarded_calls;
mod join;
mod realization_bodies;
mod receivers;
mod scalar_call_plans;
mod scalar_call_transactions;
mod unit;

use super::{
    BTreeMap, CheckFacts, CheckedBoundaryMachinePlan, CheckedStructuralAccess,
    CheckedUnitCallCoordinate, MachineSupplyMode, ServiceReachSummary, StatementNode, SymbolHandle,
    TypedTrees,
};
use crate::execution::terminal_unit::ShapeCollector;
use crate::execution::terminal_unit::dynamic_scalar_calls::scalar_call_transactions::build_checked_dynamic_scalar_call_transaction;
use typed_trees::name::Identifier;

pub(super) fn build_checked_dynamic_dispatch_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
) -> checked_trees::CheckedDynamicDispatchPlans {
    build_checked_dynamic_scalar_call_transaction(program, facts, shapes, boundaries)
        .unwrap_or_default()
}
