//! Builtins selected for reachable Psi boundaries, before native instruction selection.
use build_providers::plans::{ProviderPlan, ServiceSchema};
use build_providers::selections::SelectedProviderPlanFacts;
use diagnostics::Diagnostic;
use native_intrinsics::CompilerIntrinsicExecutionIdentity;
use provider_correspondence::external_providers::reconcile_numeric_meaning;
use provider_correspondence::{CompilerIntrinsicPlanProvenance, selected_external_execution};
use semantic_vocabulary::{BoundaryMachineId, IeeeFloatFormat};
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedCompilerBuiltin {
    requirement: String,
    plan_index: usize,
    selection: Arc<SelectedProviderPlanFacts>,
    artifact: terminal_psi::TerminalPsiIdentity,
    boundary: BoundaryMachineId,
    // A selected row can lack a closed identity. The receiving callable refuses
    // it when its execution actually reaches this boundary.
    identity: Option<CompilerIntrinsicExecutionIdentity>,
    format: Option<IeeeFloatFormat>,
    satisfied_schema: Option<ServiceSchema>,
}

impl SelectedCompilerBuiltin {
    pub fn select<'a>(
        selected: &'a Arc<SelectedProviderPlanFacts>,
        provenance: impl ExactSizeIterator<Item = CompilerIntrinsicPlanProvenance<'a>>,
        demanded_boundaries: &BTreeSet<String>,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        direct_leaves: &[direct_boundary_bindings::DirectBoundaryLeaf],
    ) -> Result<Vec<Self>, Vec<Diagnostic>> {
        let module = artifact.module();
        let plans = selected.plans();
        let mut builtins = Vec::new();
        provider_correspondence::intrinsic_selection::visit_selected_intrinsics(
            selected,
            provenance,
            demanded_boundaries,
            module,
            |requirement, selected_row, boundary, callable| {
                // The selected direct call already records its declaration and target
                // implementation. Validate that evidence here instead of asking an
                // external provider to supply it again.
                let plan = &plans[selected_row.plan_index];
                let mut direct = direct_leaves.iter().filter(|leaf| &leaf.plan == plan);
                let direct = match (direct.next(), direct.next()) {
                    (leaf, None) => leaf,
                    _ => {
                        return Err(Diagnostic::error(
                            "compiler-builtin proposal has competing direct callable custody",
                        ));
                    }
                };
                let identity = if let Some(leaf) = direct {
                    leaf.validate_bound_calling_custody(module).and_then(|()| {
                        if &leaf.satisfied_schema != selected_row.semantic_schema
                            || leaf.execution
                                != direct_boundary_bindings::DirectBoundaryExecution::Intrinsic(
                                    selected_row.execution,
                                )
                        {
                            return Err("direct callable settlement changes its retained schema or execution".into());
                        }
                        Ok(selected_row.execution)
                    })
                } else {
                    selected_external_execution(
                        &module.interfaces.external_providers,
                        &module.identities,
                        &callable.source.machine,
                        plan,
                        selected_row.row,
                        selected_row.execution,
                        selected_row.external_realization,
                    )
                }
                .map_err(|reason| {
                    Diagnostic::error(format!("Terminal boundary `{requirement}` {reason}"))
                })?;
                reconcile_numeric_meaning(boundary.numeric_intrinsic, identity).map_err(
                    |reason| {
                        Diagnostic::error(format!("Terminal boundary `{requirement}` {reason}"))
                    },
                )?;
                let format = boundary
                    .numeric_intrinsic
                    .and_then(|meaning| meaning.builtin_format())
                    .map(Into::into);
                builtins.push(Self {
                    requirement: selected_row.requirement_identity.to_owned(),
                    plan_index: selected_row.plan_index,
                    selection: Arc::clone(selected),
                    artifact: artifact.identity(),
                    boundary: boundary.id,
                    identity,
                    format,
                    satisfied_schema: (selected_row.semantic_schema != &plan.schema)
                        .then(|| selected_row.semantic_schema.clone()),
                });
                Ok(())
            },
        )?;
        Ok(builtins)
    }

    pub const fn artifact(&self) -> terminal_psi::TerminalPsiIdentity {
        self.artifact
    }

    pub const fn boundary(&self) -> BoundaryMachineId {
        self.boundary
    }

    pub fn requirement(&self) -> &str {
        &self.requirement
    }
    pub const fn identity(&self) -> Option<CompilerIntrinsicExecutionIdentity> {
        self.identity
    }
    pub const fn format(&self) -> Option<IeeeFloatFormat> {
        self.format
    }
    pub fn plan(&self) -> &ProviderPlan {
        &self.selection.plans()[self.plan_index]
    }
    pub fn semantic_schema(&self) -> &ServiceSchema {
        self.satisfied_schema
            .as_ref()
            .unwrap_or(&self.plan().schema)
    }

    /// The ABI digest alone omits the declaring provider and its external realization.
    pub fn belongs_to(&self, selection: &SelectedProviderPlanFacts) -> bool {
        std::ptr::eq(self.selection.as_ref(), selection) || self.selection.as_ref() == selection
    }
}
