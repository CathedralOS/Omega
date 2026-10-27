//! Reconstruct no-normal call admission from the current optimized CFG.
//! Catalog validation has authenticated every current node, edge and block;
//! the shared Terminal graph query supplies absence of normal completion.
//! The retained source module or a caller annotation cannot replace this walk.

use crate::OptimizationUnitValidationError;
use abstract_operations::AbstractOperation as O;
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::{BlockId, MachineId};
use std::collections::BTreeMap;
use terminal_semantics::control_graph::{NormalCompletionDisposition, NormalCompletionGraph};

struct CurrentCompletionGraph<'a> {
    functions: &'a BTreeMap<MachineId, &'a PsiOptimizationFunction>,
}

impl NormalCompletionGraph for CurrentCompletionGraph<'_> {
    type Error = &'static str;

    fn entry(&mut self, machine: MachineId) -> Result<BlockId, Self::Error> {
        self.functions
            .get(&machine)
            .map(|function| function.entry)
            .ok_or("no-normal invocation has no exact current callee")
    }

    fn disposition(
        &self,
        machine: MachineId,
        block: BlockId,
    ) -> Result<NormalCompletionDisposition, Self::Error> {
        let block = self
            .functions
            .get(&machine)
            .and_then(|function| {
                function
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == block)
            })
            .ok_or("no-normal invocation has no exact current block")?;
        let node = block
            .nodes
            .last()
            .ok_or("no-normal invocation has no current terminator")?;
        Ok(match &node.operation {
            O::Return { .. } | O::ReturnUnit { .. } | O::ReturnStructural { .. } => {
                NormalCompletionDisposition::Return
            }
            O::Crash { .. } | O::ExternalCompletion { .. } => NormalCompletionDisposition::Stop,
            O::InvokeNoNormal { callee, .. } => NormalCompletionDisposition::Invoke(*callee),
            O::Jump { .. } | O::Conditional { .. } | O::StructuralCase { .. } => {
                NormalCompletionDisposition::Successors(
                    node.successors.iter().map(|edge| edge.target).collect(),
                )
            }
            _ => return Err("no-normal invocation ends in an unclassified current control node"),
        })
    }
}

pub(super) fn validate(
    unit: &PsiOptimizationUnit,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut graph = CurrentCompletionGraph { functions };
    let mut admitted = Vec::new();
    for function in &unit.functions {
        for node in function.block_nodes() {
            let O::InvokeNoNormal { callee, .. } = &node.operation else {
                continue;
            };
            if admitted.contains(callee) {
                continue;
            }
            let invalid = |reason| OptimizationUnitValidationError::InvalidNoNormalInvocation {
                machine: function.machine,
                callee: *callee,
                reason,
            };
            if terminal_semantics::control_graph::normal_completion_possible_in(&mut graph, *callee)
                .map_err(invalid)?
            {
                return Err(invalid("current module callee may complete normally"));
            }
            admitted.push(*callee);
        }
    }
    Ok(())
}
