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

/// The canonical `FilesystemHost` and `TimeHost` mints are the only
/// toolchain-settled slots today: replaying each mint against the consumed
/// service binding, the candidate typed program, the selected target, and the
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
    let mut minted = Vec::new();
    for binding in compilation.custody.resolved_semantic_bindings() {
        let mint = match binding.role() {
            package_compilation::AcceptedSemanticBindingRole::FilesystemHostService => {
                build_evaluation::mint_canonical_filesystem_host_plan(
                    &compilation.typed,
                    Some(binding),
                    Some(target.target_name()),
                    authored_plans,
                )?
                .map(|mint| (mint.plan, mint.trait_symbol, mint.requirement_symbols))
            }
            package_compilation::AcceptedSemanticBindingRole::TimeHostService => {
                build_evaluation::mint_canonical_time_host_plan(
                    &compilation.typed,
                    Some(binding),
                    Some(target.target_name()),
                    authored_plans,
                )?
                .map(|mint| (mint.plan, mint.trait_symbol, mint.requirement_symbols))
            }
            _ => None,
        };
        if let Some(mint) = mint {
            minted.push(mint);
        }
    }
    let Some((minted_plan, minted_trait_symbol, minted_requirement_symbols)) = minted
        .into_iter()
        .find(|(plan, _, _)| *plan == retained.plan)
    else {
        return Err(rejected(
            "a retained toolchain-settled plan does not re-mint under this binding, target, or authored coverage",
        ));
    };
    if minted_plan != retained.plan {
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
        ProviderSchemaDeclaration::BoundaryTrait(symbol) if symbol == minted_trait_symbol
    ) && retained.provider.provider_type.is_none()
        && retained.provider.row_requirements == minted_requirement_symbols
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
