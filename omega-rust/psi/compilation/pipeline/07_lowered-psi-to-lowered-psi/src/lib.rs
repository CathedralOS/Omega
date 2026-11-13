/*
We execute the selected target-neutral Psi optimization passes over `LoweredPsi`
and seal the validated `PsiOptimizationStageResult` and
`PsiOptimizationExecutionRecord` required by canonical Terminal publication
(`run_psi_optimization`).
*/

/*
Our optimizer dispatches selected passes in canonical order in
`psi_optimization.rs`, backed by control-flow cleanup and proof/ranking
identity retention in `control_flow_and_retention.rs` and scalar and value
rewrites (`sparse_conditional_constant_propagation`, `copy_propagation`,
`global_value_numbering`, and `dead_scalar_elimination`) in
`scalar_and_value_passes.rs`. Only `run_psi_optimization` can construct
`PsiOptimizationStageResult`, ensuring every published Terminal artifact
records its input and output semantic identities.
*/

/*
Stage 07 (`lowered-psi-to-lowered-psi`) consumes `LoweredPsi` and produces
`PsiOptimizationStageResult` for `lowered-psi-to-terminal-psi` (Stage 08).
*/

#![forbid(unsafe_code)]

mod control_flow_and_retention;
mod psi_optimization;
mod scalar_and_value_passes;

pub(crate) use control_flow_and_retention::{control_flow_cleanup, retained_identities};
pub(crate) use scalar_and_value_passes::{
    copy_propagation, dead_scalar_elimination, global_value_numbering,
    sparse_conditional_constant_propagation,
};

pub(crate) mod edge_arguments {
    use semantic_vocabulary::{BlockId, ValueId};
    use std::collections::BTreeMap;
    use terminal_psi::{TerminalMachine, Terminator};

    pub(crate) fn drop_removed_positions(
        arguments: &mut Vec<ValueId>,
        removed: Option<&Vec<usize>>,
    ) {
        let Some(removed) = removed else { return };
        let mut position = 0;
        arguments.retain(|_| {
            let retained = !removed.contains(&position);
            position += 1;
            retained
        });
    }

    pub(crate) fn drop_removed_positions_across_edges(
        machine: &mut TerminalMachine,
        removed_positions: &BTreeMap<BlockId, Vec<usize>>,
    ) {
        for block in &mut machine.blocks {
            match &mut block.terminator {
                Terminator::Jump {
                    target, arguments, ..
                } => drop_removed_positions(arguments, removed_positions.get(target)),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for edge in [when_true, when_false] {
                        drop_removed_positions(
                            &mut edge.arguments,
                            removed_positions.get(&edge.target),
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

pub(crate) mod optimization_error {
    use terminal_codec::{CodecError, DebugMapError, PsiOptimizationExecutionRecordError};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum PsiOptimizationStageError {
        InvalidSemantic(CodecError),
        InvalidDebugMap(DebugMapError),
        InvalidExecutionRecord(PsiOptimizationExecutionRecordError),
    }

    impl std::fmt::Display for PsiOptimizationStageError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::InvalidSemantic(error) => {
                    write!(formatter, "invalid optimization semantics: {error}")
                }
                Self::InvalidDebugMap(error) => {
                    write!(formatter, "invalid optimization debug map: {error}")
                }
                Self::InvalidExecutionRecord(error) => {
                    write!(formatter, "invalid optimization execution record: {error}")
                }
            }
        }
    }

    impl std::error::Error for PsiOptimizationStageError {}
}

pub use optimization_error::PsiOptimizationStageError;
pub use psi_optimization::{PsiOptimizationStageResult, run_psi_optimization};
pub use terminal_codec::{PsiOptimizationExecutionIdentity, PsiOptimizationExecutionRecord};
