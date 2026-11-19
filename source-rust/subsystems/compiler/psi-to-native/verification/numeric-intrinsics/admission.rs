//! Bind a reference executor's numeric imports to checked provider rows and exact published Psi.
#![forbid(unsafe_code)]
mod catalog;
pub use catalog::catalog_realization;
pub mod selection;
pub mod signature;
use diagnostics::Diagnostic;
pub use provider_correspondence::CompilerIntrinsicPlanProvenance;
use selection::validate_external_provider_owners;
use target_vocabulary::NearestIeeeFloatArithmeticRealization;
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

    /// Borrow the complete selected operation set only for the execution that admitted it.
    pub fn operations_for(
        &self,
        receiving: &terminal_codec::TerminalExecutionIdentity,
    ) -> Result<
        &[(
            semantic_vocabulary::BoundaryMachineId,
            NearestIeeeFloatArithmeticRealization,
        )],
        &'static str,
    > {
        if self.terminal() != receiving {
            return Err("semantic intrinsic admission belongs to another Terminal artifact");
        }
        Ok(&self.operations)
    }

    pub fn realizes(&self, boundary: semantic_vocabulary::BoundaryMachineId) -> bool {
        self.operations.iter().any(|(slot, _)| *slot == boundary)
    }
}

pub fn admit_terminal_intrinsics<'selection>(
    artifact: terminal_codec::TerminalExecutionView<'_>,
    selected: &'selection selected_provider_plans::SelectedProviderPlanFacts,
    provenance: impl ExactSizeIterator<Item = CompilerIntrinsicPlanProvenance<'selection>>,
    demanded: &std::collections::BTreeSet<String>,
) -> Result<AdmittedTerminalIntrinsics, Vec<Diagnostic>> {
    let module = artifact.module();
    validate_external_provider_owners(module, selected)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let mut operations = Vec::new();
    selection::visit_selected_intrinsics(
        selected,
        provenance,
        demanded,
        module,
        |requirement, row, boundary, callable| {
            let plan = &selected.plans()[row.plan_index];
            let identity = provider_correspondence::selected_external_execution(
                &module.interfaces.external_providers,
                &module.identities,
                &callable.source.machine,
                plan,
                row.row,
                row.execution,
                row.external_realization,
            )
            .and_then(|identity| {
                provider_correspondence::external_providers::reconcile_numeric_meaning(
                    boundary.numeric_intrinsic,
                    identity,
                )
                .map_err(str::to_owned)?;
                Ok(identity)
            })
            .map_err(|reason| {
                Diagnostic::error(format!("Terminal boundary `{requirement}` {reason}"))
            })?;
            let refuse = |reason: &str| {
                Diagnostic::error(format!(
                    "selected Terminal intrinsic `{}` {reason}",
                    row.requirement_identity
                ))
            };
            let identity = identity.ok_or_else(|| refuse("has no closed catalog execution"))?;
            if !identity.is_numeric() {
                return Err(refuse("requires separate host or physical admission"));
            }
            let format = boundary
                .numeric_intrinsic
                .and_then(|meaning| meaning.builtin_format())
                .map(Into::into);
            let realization = catalog_realization(identity, format)
                .ok_or_else(|| refuse("has no closed catalog execution"))?;
            if !signature::boundary_contract_matches_selected_schema(
                module,
                boundary,
                plan,
                row.requirement_identity,
            ) || boundary.completion != semantic_vocabulary::BoundaryCompletion::NormalReturn
            {
                return Err(refuse(
                    "differs from its exact selected completion contract",
                ));
            }
            let realization = signature::float_arithmetic_signature(realization, boundary, module)
                .map_err(|reason| refuse(&reason))?;
            if operations.iter().any(|(slot, _)| *slot == boundary.id) {
                return Err(refuse("has competing selected executions"));
            }
            operations.push((boundary.id, realization));
            Ok(())
        },
    )?;
    Ok(AdmittedTerminalIntrinsics {
        terminal: artifact.custody_identity(),
        operations,
    })
}

#[cfg(test)]
mod tests;
