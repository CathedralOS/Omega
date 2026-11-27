//! Settle the numeric operations and native binding descriptions needed by
//! package review. Only the selected execution's own rosters grant runtime
//! supply; a library may expose additional authored declarations for review.
use crate::calling_policy_plans::CallingProgram;
use crate::target_supply::{NormalizedNativeSupply, SelectedProviderRealization};
use diagnostics::Diagnostic;
use build_model::receipts::NativeBindingMaterialization;
use provider_demand::PackageReviewRequirements;
use semantic_vocabulary::BoundaryMachineId;
use terminal_psi::TerminalModule;

pub(crate) struct PreparedReview {
    numeric: Result<Vec<BoundaryMachineId>, Vec<Diagnostic>>,
    bindings: Result<ReviewedNativeBindings, Vec<Diagnostic>>,
}

pub(super) fn prepare(
    program: &CallingProgram,
    requirements: PackageReviewRequirements,
    plan_calling: impl FnOnce(
        &[NormalizedNativeSupply],
    ) -> Result<
        Vec<calling_conventions::BoundaryCallingApplication>,
        Vec<Diagnostic>,
    >,
) -> PreparedReview {
    let module = program.artifact.semantic_module();
    let bindings = requirements.bindings.and_then(|bindings| {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let mut rows = Vec::with_capacity(bindings.applications.len());
        let mut normalized = Vec::with_capacity(bindings.applications.len());
        for (boundary, materialization) in bindings.applications {
            let declaration = module
                .boundary_machine(boundary)
                .and_then(|boundary| boundary.declaration.as_ref())
                .ok_or_else(|| unavailable("the native binding's checked declaration"))?;
            let callable = module
                .identities
                .text(declaration.source.callable)
                .ok_or_else(|| unavailable("the native binding's complete callable identity"))?;
            let binding = crate::native_bindings::normalize_native_binding(
                &materialization,
                callable,
                &declaration.service_declaration,
                &mut identities,
            )
            .map_err(|reason| unavailable(&reason))?;
            normalized.push(NormalizedNativeSupply {
                boundary,
                binding: binding.clone(),
            });
            rows.push(ReviewedNativeBinding {
                boundary,
                binding,
                materialization,
            });
        }
        let unused_generic_imports = bindings
            .unused_generic_imports
            .into_iter()
            .map(|materialization| {
                let binding = crate::native_bindings::normalize_native_binding(
                    &materialization,
                    &materialization.requirement_identity,
                    &materialization.service_declaration,
                    &mut identities,
                )
                .map_err(|reason| unavailable(&reason))?;
                let build_providers::plans::EvaluatedNativeBinding::Import(import) = binding else {
                    return Err(unavailable(
                        "the unused generic import's normalized locator",
                    ));
                };
                Ok((materialization, import))
            })
            .collect::<Result<_, _>>()?;
        let calling = plan_calling(&normalized);
        Ok(ReviewedNativeBindings {
            identities: identities.build(),
            rows,
            unused_generic_imports,
            calling,
        })
    });
    PreparedReview {
        numeric: requirements.numeric,
        bindings,
    }
}

impl PreparedReview {
    pub(super) fn finish(
        self,
        module: &TerminalModule,
        target: target::TargetProfile,
        execution: &SelectedProviderRealization,
    ) -> PackageTargetReview {
        PackageTargetReview {
            numeric: self
                .numeric
                .and_then(|demand| numeric(module, target, execution, demand)),
            bindings: self.bindings,
            provider_candidates: target_provider_admission::selected_checked_invocation_rows(
                execution.plans().plans(),
                module,
            )
            .map(|rows| {
                rows.into_iter()
                    .map(|selected| ReviewedProviderRow {
                        plan: selected.plan.name.clone(),
                        requirement: selected
                            .plan
                            .identity_text(selected.row.declared_requirement_identity)
                            .to_owned(),
                        candidates: selected
                            .candidates
                            .iter()
                            .map(|candidate| {
                                (
                                    candidate.conformance.boundary,
                                    candidate.conformance.candidate,
                                )
                            })
                            .collect(),
                    })
                    .collect()
            })
            .map_err(|reason| unavailable(&reason)),
        }
    }
}

fn numeric(
    module: &TerminalModule,
    target: target::TargetProfile,
    execution: &SelectedProviderRealization,
    requested: Vec<BoundaryMachineId>,
) -> Result<Vec<ReviewedNumericBinding>, Vec<Diagnostic>> {
    let mut retained: Vec<_> = execution
        .direct_boundary_leaves()
        .iter()
        .filter(|leaf| leaf.target_realization.is_some())
        .collect();
    retained.sort_unstable_by(|left, right| left.callable_identity.cmp(&right.callable_identity));
    let mut leaves = Vec::with_capacity(requested.len());
    let mut additional = Vec::new();
    for id in requested {
        let callable = module
            .boundary_machine(id)
            .and_then(|boundary| module.identities.text(boundary.identity))
            .ok_or_else(|| unavailable("the accepted numeric callable coordinate"))?;
        match retained.binary_search_by(|leaf| leaf.callable_identity.as_str().cmp(callable)) {
            Ok(index) => leaves.push((*retained[index]).clone()),
            Err(_) => additional.push(id),
        }
    }
    if !additional.is_empty() {
        let schemas = direct_boundary_bindings::compiler_primitive_schemas(module)
            .map_err(|reason| unavailable(&reason))?;
        leaves.extend(crate::direct_compiler_primitive_leaves(
            module,
            &schemas,
            target.target_name(),
            additional,
        )?);
    }
    leaves.sort_unstable_by(|left, right| left.callable_identity.cmp(&right.callable_identity));
    if leaves
        .windows(2)
        .any(|pair| pair[0].callable_identity == pair[1].callable_identity)
    {
        return Err(unavailable("one exact numeric target realization"));
    }
    leaves
        .into_iter()
        .map(|leaf| {
            ReviewedNumericBinding::from_leaf(module, target, leaf)
                .map_err(|reason| unavailable(&reason))
        })
        .collect()
}

fn unavailable(what: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "package review cannot retain {what} from the accepted declaration"
    ))]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageTargetReview {
    /// Includes declarations reached through selected provider implementations.
    pub numeric: Result<Vec<ReviewedNumericBinding>, Vec<Diagnostic>>,
    pub bindings: Result<ReviewedNativeBindings, Vec<Diagnostic>>,
    /// Checked adapter candidates, in retained plan and binding order.
    pub provider_candidates: Result<Vec<ReviewedProviderRow>, Vec<Diagnostic>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedProviderRow {
    pub plan: String,
    pub requirement: String,
    pub candidates: Vec<(BoundaryMachineId, semantic_vocabulary::MachineId)>,
}

/// A numeric target leaf checked against the exact canonical declaration it implements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedNumericBinding {
    boundary: BoundaryMachineId,
    leaf: direct_boundary_bindings::DirectBoundaryLeaf,
}

impl ReviewedNumericBinding {
    fn from_leaf(
        module: &terminal_psi::TerminalModule,
        target: target::TargetProfile,
        leaf: direct_boundary_bindings::DirectBoundaryLeaf,
    ) -> Result<Self, String> {
        let declaration = leaf.bound_calling_declaration(module)?;
        if leaf.plan.target != target.target_name()
            || declaration.declaration.as_ref().is_none_or(|source| {
                source.execution != terminal_psi::CheckedBoundaryExecution::NumericPrimitive
            })
        {
            return Err("the numeric declaration's target realization".into());
        }
        Ok(Self {
            boundary: declaration.id,
            leaf,
        })
    }

    pub const fn boundary(&self) -> BoundaryMachineId {
        self.boundary
    }

    pub const fn leaf(&self) -> &direct_boundary_bindings::DirectBoundaryLeaf {
        &self.leaf
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedNativeBindings {
    pub identities: terminal_psi::TerminalIdentityTable,
    /// In the same order as the Build supplied the bindings for this target.
    pub rows: Vec<ReviewedNativeBinding>,
    pub unused_generic_imports: Vec<(
        NativeBindingMaterialization,
        build_providers::plans::EvaluatedForeignImport,
    )>,
    pub calling: Result<Vec<calling_conventions::BoundaryCallingApplication>, Vec<Diagnostic>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedNativeBinding {
    pub boundary: BoundaryMachineId,
    pub materialization: NativeBindingMaterialization,
    pub binding: build_providers::plans::EvaluatedNativeBinding,
}
