//! Admit selected semantic operations for an existing Terminal executor.
//!
//! This grants no filesystem, device, process or native feature authority.
//! Numeric operations have closed, target-independent reference semantics;
//! their native realization still owes the separate ISA and feature checks.

use super::{TerminalCompilerBuiltinSettlement, float_arithmetic_signature};
use diagnostics::Diagnostic;
use target_operations::{CompilerBuiltinExecution, NearestIeeeFloatArithmeticRealization};

/// Exact selected semantic imports, bound to the artifact that admitted them.
/// Only this owner can issue the carrier; execution cannot invent realizations.
#[derive(Debug, Clone)]
pub struct AdmittedTerminalIntrinsics {
    terminal: terminal_codec::TerminalExecutionIdentity,
    operations: Vec<(
        semantic_vocabulary::BoundaryMachineId,
        NearestIeeeFloatArithmeticRealization,
    )>,
}

impl AdmittedTerminalIntrinsics {
    pub fn terminal(&self) -> &terminal_codec::TerminalExecutionIdentity {
        &self.terminal
    }

    pub fn operations(
        &self,
    ) -> &[(
        semantic_vocabulary::BoundaryMachineId,
        NearestIeeeFloatArithmeticRealization,
    )] {
        &self.operations
    }
}

/// Consume the same selected-row proposals used by native admission. A closed
/// numeric identity has reference semantics; every other mechanism requires
/// its own receiving host/backing admission and is refused here.
pub fn admit_terminal_intrinsics<'selection>(
    artifact: terminal_codec::TerminalExecutionView<'_>,
    selected: &'selection effects::SelectedProviderPlanFacts,
    provenance: impl ExactSizeIterator<Item = super::CompilerIntrinsicPlanProvenance<'selection>>,
    demanded: &std::collections::BTreeSet<String>,
) -> Result<AdmittedTerminalIntrinsics, Vec<Diagnostic>> {
    let module = artifact.module();
    // Derive and admit against this one code view. A separately retained
    // proposal's slot and signature cannot authenticate another code owner.
    let proposals = super::derive_selected_intrinsic_settlement_proposals(
        selected,
        provenance,
        demanded,
        &module.boundary_machines,
        &module.external_providers,
        &module.identities,
    )?;
    let retained = super::prepare_retained_compiler_builtins(&proposals, selected.plans())?;
    let mut operations = Vec::with_capacity(retained.len());
    for proposal in retained {
        let refuse = |reason: &str| {
            vec![Diagnostic::error(format!(
                "selected Terminal intrinsic `{}` {reason}",
                proposal.requirement_identity,
            ))]
        };
        let TerminalCompilerBuiltinSettlement::Execution {
            identity,
            execution,
        } = proposal.settlement
        else {
            return Err(refuse("has no closed catalog execution"));
        };
        let CompilerBuiltinExecution::NearestIeeeFloatArithmetic { operation, format } =
            execution.execution()
        else {
            return Err(refuse("requires separate host or physical admission"));
        };
        if !identity.is_numeric() {
            return Err(refuse("does not retain a numeric semantic identity"));
        }
        let boundary =
            terminal_psi::find_boundary_machine(&module.boundary_machines, proposal.boundary)
                .ok_or_else(|| refuse("names an absent Terminal boundary"))?;
        if !super::boundary_contract_matches_selected_schema(
            boundary,
            proposal.provider_plan,
            proposal.requirement_identity,
        ) || boundary.completion != super::compiler_builtin_completion(execution.execution())
        {
            return Err(refuse(
                "differs from its exact selected completion contract",
            ));
        }
        let realization = float_arithmetic_signature(
            NearestIeeeFloatArithmeticRealization {
                operation,
                format,
                // Reference arithmetic does not issue an x86 instruction or
                // confer a feature admission on later native realization.
                x86_scalar_fma: None,
            },
            boundary,
            &module.structural_types,
        )
        .map_err(|reason| refuse(&reason))?;
        if operations.iter().any(|(slot, _)| *slot == boundary.id) {
            return Err(refuse("has competing selected executions"));
        }
        operations.push((boundary.id, realization));
    }
    Ok(AdmittedTerminalIntrinsics {
        terminal: artifact.custody_identity(),
        operations,
    })
}
