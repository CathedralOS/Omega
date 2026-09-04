//! Transition-equation carriers and read-only topology operations.
//!
//! This leaf records named transition edges and answers reachability over an
//! already-built equation set. Equation construction, permutation validation,
//! and fixed-point solving remain in the parent.

use super::place_paths::FramePlaceOrigin;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{TransitionTargetHandle, TransitionTargetNode};

#[derive(Debug, Clone)]
pub(super) struct PermutedCycleFrameEdge {
    pub(super) target: SymbolHandle,
    pub(super) arguments: Vec<ExpressionHandle>,
}

#[derive(Debug)]
pub(super) struct PermutedCycleFrameEquation<'program> {
    pub(super) state: &'program State,
    pub(super) locals: Vec<String>,
    pub(super) local_alias_origins: Vec<(String, FramePlaceOrigin)>,
    pub(super) direct_writes: Vec<String>,
    pub(super) edges: Vec<PermutedCycleFrameEdge>,
}

pub(super) fn append_permuted_cycle_frame_edge(
    program: &TypedTrees,
    machine: &Machine,
    source: &State,
    target: TransitionTargetHandle,
    edges: &mut Vec<PermutedCycleFrameEdge>,
) -> Option<()> {
    if !target.is_valid() {
        return Some(());
    }
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Terminal
        | TransitionTargetNode::Value(_)
        | TransitionTargetNode::SelfTarget => Some(()),
        TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            let target = program
                .machine_states(machine)
                .iter()
                .find(|candidate| candidate.symbol == path.symbol)
                .or_else(|| {
                    let members = program.statement_table.name_path_members(path.members);
                    matches!(members, [member] if member.as_str() == "self").then_some(source)
                })?;
            edges.push(PermutedCycleFrameEdge {
                target: target.symbol,
                arguments: program
                    .statement_table
                    .expression_handles(*arguments)
                    .to_vec(),
            });
            Some(())
        }
    }
}

pub(super) fn transition_state_reaches(
    equations: &[PermutedCycleFrameEquation<'_>],
    start: SymbolHandle,
    sought: SymbolHandle,
) -> bool {
    let mut pending = vec![start];
    let mut visited = Vec::new();
    while let Some(symbol) = pending.pop() {
        if symbol == sought {
            return true;
        }
        if visited.contains(&symbol) {
            continue;
        }
        visited.push(symbol);
        let Some(equation) = equations
            .iter()
            .find(|equation| equation.state.symbol == symbol)
        else {
            return false;
        };
        pending.extend(equation.edges.iter().map(|edge| edge.target));
    }
    false
}
