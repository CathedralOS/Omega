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
use crate::task_plans::stack_graphs::{exact_frame_target, task_call_graph};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use language_semantics::MachineSupplyMode;
use std::collections::BTreeMap;
use symbols::SymbolHandle;
use target::NativeTarget;
use task_plans::{CallTargetBinding, TaskStackFrameId, ValidatedActivationPlan};

/// The provider's admission-time resolution of one sealed unresolved call
/// site: which concrete machine the call actually invokes.
///
/// The coordinate — `frame`, `state`, `statement_index`, `call_ordinal` —
/// names the exact sealed `UnresolvedCallSite` the assignment answers, the
/// same coordinate `CallTargetBinding` carries. `callee_entry` is the
/// entry-state symbol of the concrete machine the provider resolves the call
/// to; it resolves to its owning machine through the same exact-target rule
/// a checked call's target symbol follows, and that machine must be supplied
/// as a checked body — a binding's evidence is the callee's validated
/// call-graph subtree, which only a checked body can produce.
#[derive(Debug, Clone)]
pub struct CallTargetAssignment {
    /// Frame owning the unresolved call the provider resolves.
    pub frame: TaskStackFrameId,
    /// Name of the reachable state inside `frame` whose flow row owns the
    /// call — the frame spans several states of one machine, so the
    /// statement/call index pair is only exact alongside it.
    pub state: String,
    /// Exact call coordinate inside the state's checked flow row.
    pub statement_index: usize,
    pub call_ordinal: usize,
    /// Entry-state symbol of the concrete machine the provider binds the
    /// call to.
    pub callee_entry: SymbolHandle,
}

/// Derive the [`CallTargetBinding`] rows an admission-time assignment table
/// presents against a served activation plan's sealed unresolved-call
/// roster.
///
/// Each roster row the table addresses produces one binding whose `subtree`
/// and `crossings` come from the real `task_call_graph` of the assigned
/// callee — the identical derivation a checked call edge would have produced
/// had the checker resolved the target itself — so covering merges the same
/// validated evidence graph resolution would have bound. Bindings emit in
/// the roster's canonical order, so the result is stable regardless of
/// assignment-table order.
///
/// The table is a claim, not evidence. A coordinate naming no sealed site
/// mints nothing — the row may address another plan's roster — and leaves
/// the plan unchanged. A row whose callee fails to resolve to a checked-body
/// machine state rejects: the site stays uncovered because no honest binding
/// exists for it, not because the derivation silently thinned the set. Two
/// rows answering one site coordinate reject as contradictory claims.
pub fn derive_call_target_bindings(
    program: &CheckedTrees,
    target: NativeTarget,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    layouts: &layout::LayoutPlan,
    plan: &ValidatedActivationPlan,
    assignments: &[CallTargetAssignment],
) -> Result<Vec<CallTargetBinding>, Vec<Diagnostic>> {
    let Some(projection) = plan.wcsu_stack_projection() else {
        return Err(vec![Diagnostic::error(
            "call target binding derivation requires the activation plan's sealed \
             whole-call-graph WCSU evidence",
        )]);
    };
    let mut assigned = BTreeMap::new();
    for assignment in assignments {
        let coordinate = (
            assignment.frame,
            assignment.state.as_str(),
            assignment.statement_index,
            assignment.call_ordinal,
        );
        if assigned
            .insert(coordinate, assignment.callee_entry)
            .is_some()
        {
            return Err(vec![Diagnostic::error(format!(
                "provider assignment resolves call site at frame 0x{:016x} state `{}` \
                 statement {} call {} more than once",
                assignment.frame.normalized_identity(),
                assignment.state,
                assignment.statement_index,
                assignment.call_ordinal,
            ))]);
        }
    }
    let mut bindings = Vec::new();
    for site in projection.unresolved_calls() {
        let coordinate = (
            site.frame,
            site.state.as_str(),
            site.statement_index,
            site.call_ordinal,
        );
        let Some(&callee_entry) = assigned.get(&coordinate) else {
            continue;
        };
        let (machine, entry) = exact_frame_target(program, callee_entry)?;
        if machine.supply_mode != MachineSupplyMode::CheckedBody {
            return Err(vec![Diagnostic::error(format!(
                "provider assignment for call site at frame 0x{:016x} state `{}` statement {} \
                 call {} binds `{}`, supplied as {:?} rather than a checked body",
                site.frame.normalized_identity(),
                site.state,
                site.statement_index,
                site.call_ordinal,
                machine.name,
                machine.supply_mode,
            ))]);
        }
        let graph = task_call_graph(
            program,
            target,
            opaque_representation_selections,
            layouts,
            machine,
            entry,
        )?;
        let crossings = graph
            .crossings
            .iter()
            .map(|crossing| canonical_suspension_crossing(program, crossing))
            .collect::<Result<Vec<_>, _>>()?;
        bindings.push(CallTargetBinding {
            frame: site.frame,
            state: site.state.clone(),
            statement_index: site.statement_index,
            call_ordinal: site.call_ordinal,
            callee: graph.root,
            subtree: graph.frames,
            crossings,
        });
    }
    Ok(bindings)
}
