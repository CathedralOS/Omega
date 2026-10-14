//! Native product preparation and realization after checked-Psi admission.

use crate::checked::CheckedCompilation;
use diagnostics::Diagnostic;

use compilation_products::NativeProductRequest;

/// Consume the retained Terminal product through the source-free native
/// pipeline. This operation never enters Psi production. The stop and its
/// Build requirements were admitted by
/// `CheckedCompilation::prepare_product_report`; nothing is re-admitted here.
pub(crate) fn produce_native_product(
    request: NativeProductRequest,
    checked: &CheckedCompilation,
    terminal: compilation_products::RetainedTerminalArtifact,
    selections: &optimization_core::OptimizationSelections,
) -> Result<native_production::RealizedNativeProduct, Vec<Diagnostic>> {
    // Capture package acceptance from the checked source, independently of the
    // proposal being produced. These are NOT receiver permission rows: the
    // separately supplied receiver policy remains optional and unchanged.
    let accepted_package_terminal_authority_permission_policy =
        accepted_terminal_authority_permission_policy(checked)?;
    let post_terminal = selections.project_post_terminal();
    native_production::compile(
        terminal,
        native_production::NativeCompileRequest {
            private_entry_ram: &request.private_entry_ram,
            optimization_selections: post_terminal.selections(),
            terminal_authority_policy: request.terminal_authority_policy,
            accepted_package_terminal_authority_permission_policy,
            terminal_authority_permission_policy: request.terminal_authority_permission_policy,
            image_request: image_emission::ExecutableImageEmissionRequest::direct(
                checked.build().config.settings.subsystem,
            ),
            imports: &[],
        },
    )
    .map_err(|(_, diagnostics)| diagnostics)
}

/// Project and validate the checked package's accepted Terminal permission
/// rows before native proposal production. These are package evidence, not
/// the independently supplied receiver-admission policy.
fn accepted_terminal_authority_permission_policy(
    checked: &CheckedCompilation,
) -> Result<
    terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy,
    Vec<Diagnostic>,
> {
    use terminal_psi_to_abstract_operations::provider_admission::{
        terminal_authority_permission_policy_with_rows,
        terminal_authority_permissions::validate_package_terminal_authority_permission_custody,
    };
    let (permissions, permission_identities) = checked.execution().terminal_authority_permissions();
    validate_package_terminal_authority_permission_custody(
        permissions.iter(),
        &permission_identities,
    )?;
    terminal_authority_permission_policy_with_rows(permissions, permission_identities).map_err(
        |error| {
            vec![Diagnostic::error(format!(
                "checked package terminal-authority permissions are invalid: {error:?}"
            ))]
        },
    )
}
