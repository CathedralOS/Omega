//! Optimizer module role: stage group. Control-flow cleanup.
//!
//! A conditional whose condition is a `BooleanConstant` result selects exactly
//! one successor: the fold keeps the selected edge — identity, scalar and
//! structural bindings, and edge-scoped cleanup rows — as an unconditional
//! `Jump`, and every block the untaken arm leaves unreachable is removed with
//! it. A block whose removal would erase a row proof, ownership, or call
//! evidence still names — or would orphan a machine-level structural place
//! declaration — is never removed, so its conditional stays unfolded.
//!
//! Inside a proof-bearing module a rewrite survives only while the
//! reconstructed proof question is unchanged: a fold the question cannot
//! carry verbatim refuses, leaving the complete closure as authored until
//! proof-context transport lets the question move with the rewrite. Ranked
//! machines keep their execution-position evidence, and the retained-value
//! contract matches the sibling rules: proposition, coercion, invariant,
//! suspension, dispatch, call-evidence, and projection carriers pin the exact
//! block-local identities they name. The unsealed sidecars pin theirs too: a
//! recorded source-call or selected-IEEE join keeps its named operation and
//! captured scalar environment reachable.
//!
//! The scan proposes folds; the independent verifier re-derives each removed
//! block's unreachability under the performed folds and checks the exact
//! before/after relation and proof questions.
//!
//! After the per-machine folds, a machine no retained transition reaches —
//! through calls, selected evidence, cleanup actions, or closed reach
//! applications, from the module entry, attached or ranked machines,
//! provider candidates, or any module-level custody or evidence row — is
//! dropped, provided no surviving row still names a block, edge, operation,
//! or value inside it. A machine a surviving row or unsealed sidecar keeps
//! authored is not a retention root, but the machines its own surviving
//! transitions name stay too. The verifier re-derives that machine-level
//! relation from the rewritten module as well.

mod cleanup;
mod machines;
mod retention;
mod validation;

pub(super) use validation::ControlFlowCleanupRewriteError;

use crate::PsiOptimizationStageError;
use lowered_psi::LoweredPsi;
use std::collections::BTreeSet;
use terminal_psi::{DebugSubject, OperationResult};
use validation::validate_control_flow_cleanup;

pub(super) fn cleanup(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
    let mut after = before.clone();
    // Checked-source joins are unsealed sidecars the module-level evidence
    // inventory cannot see: an occurrence keeps its Terminal operation and
    // captured scalar environment live while both representations exist.
    let mut sidecar_operations = BTreeSet::new();
    let mut sidecar_edges = BTreeSet::new();
    let mut sidecar_values = BTreeSet::new();
    for occurrence in &before.source_call_occurrences {
        match occurrence.terminal_site {
            semantic_vocabulary::InvocationSite::Operation(operation) => {
                sidecar_operations.insert(operation);
            }
            semantic_vocabulary::InvocationSite::InvokeNoNormal(edge)
            | semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
                sidecar_edges.insert(edge);
            }
        }
        occurrence.frontier_before_call.visit_value_ids(|value| {
            sidecar_values.insert(value);
        });
    }
    for occurrence in &before.selected_comparison_occurrences {
        sidecar_operations.insert(occurrence.terminal_operation);
    }
    let mut evidence = retention::block_local_evidence(&before.semantic_module);
    evidence.edges.extend(sidecar_edges.iter().copied());
    for machine in &mut after.semantic_module.machines {
        cleanup::cleanup(machine, &evidence, &sidecar_operations, &sidecar_values);
    }
    machines::prune(
        &mut after.semantic_module,
        &before.retained_roots,
        &sidecar_operations,
        &sidecar_edges,
        &sidecar_values,
    );
    validate_control_flow_cleanup(&before.semantic_module, &after.semantic_module)
        .map_err(PsiOptimizationStageError::InvalidControlFlowCleanupRewrite)?;
    if let Some(debug) = after.debug_map.as_mut() {
        let surviving_machines = after
            .semantic_module
            .machines
            .iter()
            .map(|machine| machine.id)
            .collect::<BTreeSet<_>>();
        let surviving_blocks = after
            .semantic_module
            .blocks()
            .map(|block| block.id)
            .collect::<BTreeSet<_>>();
        let surviving_edges = after
            .semantic_module
            .blocks()
            .flat_map(|block| block.terminator.edges())
            .collect::<BTreeSet<_>>();
        let surviving_operations = after
            .semantic_module
            .operations()
            .map(|operation| operation.id)
            .collect::<BTreeSet<_>>();
        let surviving_values = after
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| {
                machine
                    .parameters
                    .iter()
                    .map(|parameter| parameter.id)
                    .chain(machine.result.scalar().map(|result| result.id))
                    .chain(machine.blocks.iter().flat_map(|block| {
                        block.parameters.iter().map(|parameter| parameter.id).chain(
                            block.operations.iter().filter_map(|operation| {
                                match operation.result {
                                    OperationResult::Scalar(result) => Some(result.id),
                                    _ => None,
                                }
                            }),
                        )
                    }))
            })
            .collect::<BTreeSet<_>>();
        debug.sites.retain(|site| match site.subject {
            DebugSubject::Machine(machine) => surviving_machines.contains(&machine),
            DebugSubject::Block(block) => surviving_blocks.contains(&block),
            DebugSubject::Edge(edge) => surviving_edges.contains(&edge),
            DebugSubject::Operation(operation) => surviving_operations.contains(&operation),
            DebugSubject::Value(value) => surviving_values.contains(&value),
            DebugSubject::Claim { machine, .. } => surviving_machines.contains(&machine),
            _ => true,
        });
        debug.semantic = terminal_codec::terminal_psi_identity(&after.semantic_module)
            .map_err(PsiOptimizationStageError::InvalidSemantic)?;
    }
    Ok(after)
}
