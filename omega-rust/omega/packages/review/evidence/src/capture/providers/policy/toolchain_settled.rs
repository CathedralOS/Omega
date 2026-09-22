//! Validate a retained toolchain-settled plan against the exact identity the
//! toolchain mints for the consumed binding and selected target.

use super::rejected;
use crate::capture::PackageReviewInput;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderPlan;
use provider_planning::{
    ProviderSchemaDeclaration, ProviderSelectionProvenance, SelectedProviderReviewProvenance,
};
use target::TargetProfile;

/// The canonical `FilesystemHost` mint is the only toolchain-settled slot
/// today: replaying the mint against the consumed `FilesystemHostService`
/// binding, the candidate typed program, the selected target, and the
/// retained authored plans reproduces the exact settled identity — schema,
/// rows, and syscall bindings — that custody claims. The retained provenance
/// must then carry the settlement shape: requirement symbols bound to the
/// minted rows, no authored realization machine or target-machine origin, and
/// `UniqueCoveringCandidate` selection.
pub(super) fn validate(
    compilation: &PackageReviewInput<'_>,
    target: TargetProfile,
    retained: &SelectedProviderReviewProvenance,
    authored_plans: &[ProviderPlan],
) -> Result<(), Vec<Diagnostic>> {
    let binding = compilation
        .custody
        .resolved_semantic_bindings()
        .find(|binding| {
            binding.role()
                == package_compilation::AcceptedSemanticBindingRole::FilesystemHostService
        });
    let Some(minted) = build_evaluation::mint_canonical_filesystem_host_plan(
        &compilation.typed,
        binding,
        Some(target.target_name()),
        authored_plans,
    )?
    else {
        return Err(rejected(
            "a retained toolchain-settled plan does not re-mint under this binding, target, or authored coverage",
        ));
    };
    if minted.plan != retained.plan {
        return Err(rejected(
            "a toolchain-settled plan differs from its exact minted identity",
        ));
    }
    let aligned = retained.provider.row_requirements.len() == retained.plan.rows.len()
        && retained.provider.row_realizations.len() == retained.plan.rows.len()
        && retained.provider.row_target_machine_origins.len() == retained.plan.rows.len()
        && retained.row_compiler_intrinsic_executions.len() == retained.plan.rows.len();
    let settlement_shape = matches!(
        retained.provider.schema,
        ProviderSchemaDeclaration::BoundaryTrait(symbol) if symbol == minted.trait_symbol
    ) && retained.provider.provider_type.is_none()
        && retained.provider.row_requirements == minted.requirement_symbols
        && retained
            .provider
            .row_realizations
            .iter()
            .all(|symbol| !symbol.is_valid())
        && retained
            .provider
            .row_target_machine_origins
            .iter()
            .all(Option::is_none)
        && matches!(
            retained.selected_by,
            ProviderSelectionProvenance::UniqueCoveringCandidate
        )
        && retained
            .row_compiler_intrinsic_executions
            .iter()
            .all(Option::is_none);
    if !aligned || !settlement_shape {
        return Err(rejected(
            "a toolchain-settled plan's retained provenance is misaligned or carries authored selection state",
        ));
    }
    Ok(())
}
