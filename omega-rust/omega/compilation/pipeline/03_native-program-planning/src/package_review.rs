//! Settle the numeric operations and native binding descriptions needed by
//! package review. Only the selected execution's own rosters grant runtime
//! supply; a library may expose additional authored declarations for review.
use crate::calling_policy_plans::{CallingProgram, NativeCallingResults};
use crate::target_supply::{NormalizedNativeSupply, RealizedTargetSupply};
use backend_plan::package_review::{
    PackageReviewRequirements, PackageTargetReview, ReviewedNativeBinding, ReviewedNativeBindings,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::BoundaryMachineId;
use terminal_psi::TerminalModule;

pub(crate) struct PreparedReview {
    numeric: Result<Vec<BoundaryMachineId>, Vec<Diagnostic>>,
    bindings: Result<ReviewedNativeBindings, Vec<Diagnostic>>,
}

pub(super) fn prepare(
    program: &CallingProgram,
    target: target::TargetProfile,
    requirements: PackageReviewRequirements,
    defaults: &NativeCallingResults,
) -> PreparedReview {
    let module = program.artifact.semantic_module();
    let bindings = requirements.bindings.and_then(|bindings| {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let mut rows = Vec::with_capacity(bindings.len());
        let mut normalized = Vec::with_capacity(bindings.len());
        for (boundary, materialization) in bindings {
            let callable = module
                .boundary_machine(boundary)
                .and_then(|boundary| boundary.declaration.as_ref())
                .and_then(|declaration| module.identities.text(declaration.source.callable))
                .ok_or_else(|| unavailable("the native binding's complete callable identity"))?;
            let binding = crate::native_bindings::normalize_native_binding(
                &materialization,
                callable,
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
        let calling = crate::calling_policy_plans::native_calling_results(
            target,
            program,
            &normalized,
            Some(defaults),
        )
        .map(|rows| rows.applications);
        Ok(ReviewedNativeBindings {
            identities: identities.build(),
            rows,
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
        execution: &RealizedTargetSupply,
    ) -> PackageTargetReview {
        PackageTargetReview {
            numeric: self
                .numeric
                .and_then(|demand| numeric(module, target, execution, demand)),
            bindings: self.bindings,
        }
    }
}

fn numeric(
    module: &TerminalModule,
    target: target::TargetProfile,
    execution: &RealizedTargetSupply,
    requested: Vec<BoundaryMachineId>,
) -> Result<Vec<backend_plan::DirectBoundaryLeaf>, Vec<Diagnostic>> {
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
        let schemas = backend_plan::compiler_primitive_schemas(module)
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
    Ok(leaves)
}

fn unavailable(what: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "package review cannot retain {what} from the accepted declaration"
    ))]
}
