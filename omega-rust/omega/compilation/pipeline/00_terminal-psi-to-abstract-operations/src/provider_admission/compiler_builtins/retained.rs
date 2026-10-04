//! Retained builtin proposals and their selected-plan custody.
//!
//! These checks bind request data to selected rows. They do not authorize
//! execution: `settle_compiler_builtins` independently rejoins the complete
//! selected plan, Terminal boundary and receiving target catalog.

use super::super::NativeCompilerBuiltinSettlement;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderPlan;

/// One compiler-owned builtin proposed beside a target-neutral Terminal
/// artifact. The local lowerer still has to rejoin this row to the exact
/// selected plan, Terminal demand, and its own target catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCompilerBuiltinProposal {
    requirement_identity: String,
    provider_plan_index: usize,
    execution: target_operations::CompilerBuiltinExecution,
}

impl TerminalCompilerBuiltinProposal {
    pub fn new(
        requirement_identity: String,
        provider_plan_index: usize,
        execution: target_operations::CompilerBuiltinExecution,
    ) -> Result<Self, &'static str> {
        if requirement_identity.is_empty() {
            return Err("Terminal compiler-builtin proposal has an empty requirement identity");
        }
        Ok(Self {
            requirement_identity,
            provider_plan_index,
            execution,
        })
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn provider_plan_index(&self) -> usize {
        self.provider_plan_index
    }

    pub const fn execution(&self) -> target_operations::CompilerBuiltinExecution {
        self.execution
    }
}

/// Validate the retained roster without changing its order. Each requirement
/// must occur once and name exactly one intrinsic row in its indexed plan.
/// Execution identity and target support remain native admission obligations.
pub fn validate_retained_compiler_builtins(
    builtins: &[TerminalCompilerBuiltinProposal],
    selected_plans: &[ProviderPlan],
) -> Result<(), &'static str> {
    let mut requirements = std::collections::BTreeSet::new();
    for builtin in builtins {
        if !requirements.insert(builtin.requirement_identity()) {
            return Err("Terminal native proposal repeats a compiler-builtin requirement");
        }
        let Some(plan) = selected_plans.get(builtin.provider_plan_index()) else {
            return Err("Terminal native proposal names an absent selected provider plan");
        };
        let matching_rows = plan
            .rows
            .iter()
            .filter(|row| {
                row.requirement_identity == builtin.requirement_identity
                    && matches!(
                        row.binding,
                        effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                    )
            })
            .count();
        if matching_rows != 1 {
            return Err(
                "Terminal native proposal compiler builtin does not rejoin one exact selected row",
            );
        }
    }
    Ok(())
}

/// Borrow the exact selected plans for retained re-entry, preserving proposal
/// order and execution identity. The retained product validates roster custody
/// before this projection; provider admission still independently checks every
/// settlement against Terminal demand and the receiving target catalog.
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
                execution: builtin.execution(),
            })
        })
        .collect()
}
