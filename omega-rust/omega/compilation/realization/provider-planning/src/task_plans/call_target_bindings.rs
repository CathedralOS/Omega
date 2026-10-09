//! Admission-time call-target binding derivation.
//!
//! A sealed partial projection's `UnresolvedCallSite` roster is the worklist
//! provider admission answers: each row names a live call the checker could
//! not resolve — a requirement slot, a machine parameter, a dynamic
//! descriptor, or a target resolved to non-checked supply. The provider's
//! admission-time [`CallTargetAssignment`] names the concrete machine behind
//! such a call. [`derive_call_target_bindings`] turns each addressed site
//! into the [`CallTargetBinding`] `TaskRuntimeAdmission::bind_call_targets`
//! consumes: the binding's `subtree` is the validated frame evidence
//! `task_call_graph` derives for the assigned entry state, and `crossings`
//! is that subtree's canonical suspension-crossing roster — the same
//! derivation a checked call edge would have produced, never a presented
//! literal.
//!
//! An assignment is a claim that the named call invokes the named machine,
//! so the derivation fails closed on claims that cannot carry evidence: an
//! entry symbol resolving to no unique typed state fails, and a callee whose
//! machine is supplied as anything but a checked body — requirement,
//! boundary, admission-claim or external realization — rejects rather than
//! minting a subtree no checker validated. Rows addressing no sealed site
//! mint nothing: a provider's assignment table may span sites of other
//! plans. A site the table never addresses mints no binding and stays
//! unresolved, so the covered plan keeps publishing partial and refusing a
//! lease.

use crate::task_plans::carry_crossings::canonical_suspension_crossing;
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use language_semantics::MachineSupplyMode;
use std::collections::BTreeMap;
use symbols::SymbolHandle;
use target::NativeTarget;

