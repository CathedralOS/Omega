//! Builtins selected for reachable Psi boundaries, before native instruction selection.
use diagnostics::Diagnostic;
use native_intrinsics::CompilerIntrinsicExecutionIdentity;
use provider_correspondence::external_providers::reconcile_numeric_meaning;
use provider_correspondence::{CompilerIntrinsicPlanProvenance, selected_external_execution};
use provider_plans::{ProviderPlan, ProviderPlanDigest, ServiceSchema};
use semantic_vocabulary::{BoundaryMachineId, IeeeFloatFormat};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedCompilerBuiltin {
    requirement: String,
    plan_index: usize,
    plan_digest: ProviderPlanDigest,
    boundary: BoundaryMachineId,
    // A selected row can lack a closed identity. The receiving callable refuses
    // it when its execution actually reaches this boundary.
    identity: Option<CompilerIntrinsicExecutionIdentity>,
    format: Option<IeeeFloatFormat>,
    satisfied_schema: Option<ServiceSchema>,
}

impl SelectedCompilerBuiltin {
    pub fn select<'a>(
        selected: &'a selected_provider_plans::SelectedProviderPlanFacts,
        provenance: impl ExactSizeIterator<Item = CompilerIntrinsicPlanProvenance<'a>>,
        demanded_boundaries: &BTreeSet<String>,
        module: &terminal_psi::TerminalModule,
        direct_leaves: &[direct_boundary_bindings::DirectBoundaryLeaf],
    ) -> Result<Vec<Self>, Vec<Diagnostic>> {
        let plans = selected.plans();
        let mut evidence = Vec::new();
        provider_correspondence::intrinsic_selection::visit_selected_intrinsics(
            selected,
            provenance,
            demanded_boundaries,
            module,
            |requirement, selected_row, boundary, callable| {
                // The selected direct call already records its declaration and target
                // implementation. Validate that evidence here instead of asking an
                // external provider to supply it again.
                let mut direct = direct_leaves
                    .iter()
                    .filter(|leaf| leaf.plan == plans[selected_row.plan_index]);
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
                            return Err(
                            "direct callable settlement changes its retained schema or execution"
                                .into(),
                        );
                        }
                        Ok(selected_row.execution)
                    })
                } else {
                    selected_external_execution(
                        &module.interfaces.external_providers,
                        &module.identities,
                        &callable.source.machine,
                        &plans[selected_row.plan_index],
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
                let plan = &plans[selected_row.plan_index];
                evidence.push(Self {
                    requirement: selected_row.requirement_identity.to_owned(),
                    plan_index: selected_row.plan_index,
                    plan_digest: plan.identity_digest(),
                    boundary: boundary.id,
                    identity,
                    format,
                    satisfied_schema: (selected_row.semantic_schema != &plan.schema)
                        .then(|| selected_row.semantic_schema.clone()),
                });
                Ok(())
            },
        )?;
        Ok(evidence)
    }

    pub const fn boundary(&self) -> BoundaryMachineId {
        self.boundary
    }

    pub fn bind_plan<'a>(
        &'a self,
        plans: &'a [ProviderPlan],
    ) -> Result<BuiltinProvider<'a>, Diagnostic> {
        let plan = plans.get(self.plan_index).ok_or_else(|| Diagnostic::error(format!(
            "Terminal native proposal: compiler builtin `{}` names an absent selected provider plan",
            self.requirement
        )))?;
        if plan.identity_digest() != self.plan_digest {
            return Err(Diagnostic::error(format!(
                "compiler builtin `{}` belongs to a different selected provider plan",
                self.requirement
            )));
        }
        Ok(BuiltinProvider {
            selection: self,
            plan,
        })
    }
}

/// An intrinsic row joined to the same plan and semantic contract used at selection.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinProvider<'a> {
    selection: &'a SelectedCompilerBuiltin,
    plan: &'a ProviderPlan,
}

impl<'a> BuiltinProvider<'a> {
    pub fn requirement(self) -> &'a str {
        &self.selection.requirement
    }
    pub const fn boundary(self) -> BoundaryMachineId {
        self.selection.boundary
    }
    pub const fn identity(self) -> Option<CompilerIntrinsicExecutionIdentity> {
        self.selection.identity
    }
    pub const fn format(self) -> Option<IeeeFloatFormat> {
        self.selection.format
    }
    pub const fn plan(self) -> &'a ProviderPlan {
        self.plan
    }
    pub fn semantic_schema(self) -> &'a ServiceSchema {
        self.selection
            .satisfied_schema
            .as_ref()
            .unwrap_or(&self.plan.schema)
    }
}
