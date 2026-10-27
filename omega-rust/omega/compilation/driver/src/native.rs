//! Native product preparation and realization after checked-Psi admission.

use crate::checked::CheckedCompilation;
use diagnostics::Diagnostic;

use compilation_products::NativeProductRequest;

/// Consume the retained Terminal product through the source-free native
/// pipeline. This operation never enters Psi production. The stop and its
/// Build requirements were admitted by
/// `ProductReportPlan::prepare`; nothing is re-admitted here.
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
    let inputs = independent_native_inputs(checked, &terminal)?;
    let mut images = Vec::new();
    let mut calls = Vec::new();
    for (input, product) in &inputs {
        calls.extend(
            input
                .admit_native_calls(terminal.artifact().view())
                .map_err(|reason| vec![Diagnostic::error(reason)])?,
        );
        let exports = native_production::emit_component_exports(
            input,
            native_production::ComponentExportRequest {
                accepted_package_permissions: product.accepted_package_permissions(),
                terminal_authority_policy: &request.terminal_authority_policy,
                receiving_permissions: &request.terminal_authority_permission_policy,
                imports: &[],
            },
        )?;
        images.extend(
            exports
                .into_iter()
                .map(|export| export.emit_shared_image())
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    if images.len() != calls.len() {
        return Err(vec![Diagnostic::error(
            "component export image and import selection rosters differ",
        )]);
    }
    let native_calls = images
        .iter()
        .zip(&calls)
        .map(|(image, call)| {
            let mut plans = checked
                .execution()
                .selected_provider_plan_facts
                .plans()
                .iter()
                .filter(|plan| &plan.identity_digest() == call.plan_digest());
            let plan = plans.next().ok_or_else(|| {
                vec![Diagnostic::error(
                    "component call lost its complete selected provider plan",
                )]
            })?;
            if plans.next().is_some() {
                return Err(vec![Diagnostic::error(
                    "component call has competing complete provider plans",
                )]);
            }
            native_production::ComponentNativeCall::join(
                image,
                call,
                plan,
                checked
                    .execution()
                    .selected_provider_plan_facts
                    .identities(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let native_request = native_production::NativeCompileRequest {
        private_entry_ram: &request.private_entry_ram,
        optimization_selections: post_terminal.selections(),
        terminal_authority_policy: request.terminal_authority_policy,
        accepted_package_terminal_authority_permission_policy,
        terminal_authority_permission_policy: request.terminal_authority_permission_policy,
        image_request: image_emission::ExecutableImageEmissionRequest::direct(
            checked.build().config.settings.subsystem,
        ),
        imports: &[],
        components: &native_calls,
    };
    let mut produced = native_production::compile(terminal, native_request)
        .map_err(|(_, diagnostics)| diagnostics)?;
    drop(native_calls);
    let components = images
        .into_iter()
        .zip(&calls)
        .map(|(image, call)| {
            image
                .into_product(call)
                .map(std::sync::Arc::new)
                .map_err(|reason| vec![Diagnostic::error(reason)])
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !components.is_empty() {
        let runtime = produced.runtime_provisions.take().ok_or_else(|| {
            vec![Diagnostic::error(
                "component native production lost its admitted runtime companions",
            )]
        })?;
        produced.runtime_provisions = Some(
            runtime
                .with_component_products(components)
                .map_err(|reason| vec![Diagnostic::error(reason)])?,
        );
    }
    Ok(produced)
}

/// Borrow one complete production input per explicitly Independent selected
/// plan. A description with no current native product is a refusal here, never
/// a reason to realize the same body inside the receiver.
fn independent_native_inputs<'input>(
    checked: &'input CheckedCompilation,
    terminal: &compilation_products::RetainedTerminalArtifact,
) -> Result<
    Vec<(
        native_production::ComponentNativeInputs<'input>,
        &'input compilation_products::IndependentComponentProduct,
    )>,
    Vec<Diagnostic>,
> {
    let proposal = terminal.native_realization_proposal().ok_or_else(|| {
        vec![Diagnostic::error(
            "native product lost its retained realization proposal",
        )]
    })?;
    let mut result = Vec::new();
    for selected in &checked.execution().selected_provider_provenance {
        if selected
            .selected_by
            .composition_mode()
            .map_err(|reason| vec![Diagnostic::error(reason)])?
            != provider_planning::CompositionMode::Independent
        {
            continue;
        }
        let mut components = checked
            .providers()
            .verified_independent_components
            .iter()
            .filter(|component| component.realizes_selected_plan(&selected.plan).is_ok());
        let component = components.next().ok_or_else(|| {
            vec![Diagnostic::error(
                "independent plan has no exact verified producing component",
            )]
        })?;
        if components.next().is_some() {
            return Err(vec![Diagnostic::error(
                "independent plan has competing producing component verdicts",
            )]);
        }
        let mut products = checked
            .independent_component_products()
            .iter()
            .filter(|product| {
                product.description().expected_subject() == component.subject()
                    && product
                        .retained()
                        .native_realization_proposal()
                        .is_some_and(|candidate| {
                            candidate.target_profile() == proposal.target_profile()
                        })
            });
        let product = products.next().ok_or_else(|| {
            vec![Diagnostic::error(
                "independent plan has no exact retained native component product",
            )]
        })?;
        if products.next().is_some() {
            return Err(vec![Diagnostic::error(
                "independent plan has competing retained component products",
            )]);
        }
        let input = native_production::ComponentNativeInputs::join(
            component,
            product.retained(),
            &selected.plan,
            &proposal.boundary_applications().calling,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        result.push((input, product.as_ref()));
    }
    Ok(result)
}

/// Project and validate the checked package's accepted Terminal permission
/// rows before native proposal production. These are package evidence, not
/// the independently supplied receiver-admission policy.
pub(crate) fn accepted_terminal_authority_permission_policy(
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
