//! Transition-equation carriers and read-only topology operations.
//!
//! This leaf records named transition edges and answers reachability over an
//! already-built equation set. Equation construction, substitution validation,
//! and fixed-point solving remain in the parent.

use super::place_paths::FramePlaceOrigin;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::state::State;

#[derive(Debug, Clone)]
pub(super) struct CycleFrameEdge {
    pub(super) target: SymbolHandle,
    pub(super) arguments: Vec<ExpressionHandle>,
}

#[derive(Debug)]
pub(super) struct CycleFrameEquation<'program> {
    pub(super) state: &'program State,
    pub(super) locals: Vec<String>,
    pub(super) local_alias_origins: Vec<(String, FramePlaceOrigin)>,
    pub(super) stored: Vec<super::StoredLocalOrigins>,
    pub(super) direct_writes: Vec<String>,
    pub(super) edges: Vec<CycleFrameEdge>,
}

pub(super) fn append_cycle_frame_edge(
    transfer: super::transition_topology::StateTransfer<'_>,
    edges: &mut Vec<CycleFrameEdge>,
) {
    edges.push(CycleFrameEdge {
        target: transfer.target.symbol,
        arguments: transfer.arguments.to_vec(),
    });
}

pub(super) fn transition_state_reaches(
    equations: &[CycleFrameEquation<'_>],
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
