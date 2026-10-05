//! Retained builtin proposals and their selected-plan custody.
//!
//! Derivation binds each proposal to its selected plan index and Terminal
//! boundary once; `settle_compiler_builtins` reads that binding and admits it
//! through the receiving policy and target catalog.

use super::super::NativeCompilerBuiltinSettlement;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderPlan;

/// One compiler-owned builtin proposed beside a target-neutral Terminal
/// artifact: the demanded requirement, the selected plan holding its one
/// intrinsic row, the Terminal boundary it settles and the native execution.
/// The local lowerer still admits the execution through its target catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCompilerBuiltinProposal {
    requirement_identity: String,
    provider_plan_index: usize,
    boundary: semantic_vocabulary::BoundaryMachineId,
    execution: target_operations::CompilerBuiltinExecution,
}

impl TerminalCompilerBuiltinProposal {
    pub(crate) const fn new(
        requirement_identity: String,
        provider_plan_index: usize,
        boundary: semantic_vocabulary::BoundaryMachineId,
        execution: target_operations::CompilerBuiltinExecution,
    ) -> Self {
        Self {
            requirement_identity,
            provider_plan_index,
            boundary,
            execution,
        }
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn provider_plan_index(&self) -> usize {
        self.provider_plan_index
    }

    pub const fn boundary(&self) -> semantic_vocabulary::BoundaryMachineId {
        self.boundary
    }

    pub const fn execution(&self) -> target_operations::CompilerBuiltinExecution {
        self.execution
    }
}

/// Borrow the exact selected plans for retained re-entry, preserving proposal
/// order, boundary binding and execution identity.
pub fn prepare_retained_compiler_builtins<'request>(
    builtins: &'request [TerminalCompilerBuiltinProposal],
    selected_plans: &'request [ProviderPlan],
) -> Result<Vec<NativeCompilerBuiltinSettlement<'request>>, Vec<Diagnostic>> {
    builtins
        .iter()
        .map(|builtin| {
            let provider_plan = selected_plans
                .get(builtin.provider_plan_index())
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "Terminal native proposal: compiler builtin `{}` names an absent selected provider plan",
                        builtin.requirement_identity(),
                    ))]
                })?;
            Ok(NativeCompilerBuiltinSettlement {
                requirement_identity: builtin.requirement_identity(),
                provider_plan,
                boundary: builtin.boundary(),
                execution: builtin.execution(),
            })
        })
        .collect()
}
