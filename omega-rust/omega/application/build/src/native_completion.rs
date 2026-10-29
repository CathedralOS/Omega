//! Complete source-free native products under the application's evaluator sponsor.
//!
//! Compiler admission has already fixed the Terminal product, policy, component
//! joins and report settlement. This owner drives the native facade and completes
//! its scalar demand; neither the source compiler nor native stages run an
//! interpreter callback. Every failure retains the attempted evaluation ledger.

use compilation_products::PendingNativeProduct;
use compiler::{CompilationOutput, CompilationProductError};
use diagnostics::Diagnostic;

/// Complete the application-owned work of a compiler product. Non-native stops
/// are already complete and never acquire native obligations or execution.
pub fn complete_product(
    product: CompilationOutput,
) -> Result<CompilationOutput, CompilationProductError> {
    let CompilationOutput::Native(product) = product else {
        return Ok(product);
    };
    // Direct product consumers need the same stack as the enclosing batch
    // entrances. Nested completion reuses their existing compiler worker.
    compiler::checked::run_on_compile_thread(|| complete_native(*product))
}

fn complete_native(
    product: PendingNativeProduct,
) -> Result<CompilationOutput, CompilationProductError> {
    let PendingNativeProduct {
        terminal,
        components,
        accepted_package_permissions,
        request,
        rollback,
        subsystem,
        mut settlement,
    } = product;
    let produced = phase_profile::scope("06.0.e_produce_native_product", || {
        let selected = components
            .as_ref()
            .map_err(|reason| vec![Diagnostic::error(reason.clone())])?;
        let mut components = Vec::with_capacity(selected.len());
        for component in selected {
            components.push(native_production::ComponentCompilation {
                input: component
                    .input()
                    .map_err(|reason| vec![Diagnostic::error(reason)])?,
                plan: component.plan(),
                receiving_identities: component.receiving_identities(),
                accepted_package_permissions: component.accepted_package_permissions(),
            });
        }
        let post_terminal = rollback.effective().project_post_terminal();
        let request = native_production::NativeCompileRequest {
            private_entry_ram: &request.private_entry_ram,
            optimization_selections: post_terminal.selections(),
            terminal_authority_policy: request.terminal_authority_policy,
            accepted_package_terminal_authority_permission_policy: accepted_package_permissions,
            terminal_authority_permission_policy: request.terminal_authority_permission_policy,
            image_request: image_emission::ExecutableImageEmissionRequest::direct(subsystem),
            imports: &[],
            components: &components,
        };
        let mut prepared = native_production::prepare(&terminal, request)
            .map_err(|(_, diagnostics)| diagnostics)?;
        for completion in prepared.scalar_completions() {
            let completed = build_evaluation::scalar_completion::complete(
                completion.artifact(),
                completion.target(),
                completion.demand(),
                completion.provider_proposals(),
                settlement.source_completion_accounting_mut(),
            )
            .map_err(|reason| vec![Diagnostic::error(format!("constant checking: {reason}"))])?;
            completion
                .complete(completed)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        }
        native_production::compile(prepared).map_err(|(_, diagnostics)| diagnostics)
    });
    let produced = match produced {
        Ok(produced) => produced,
        Err(diagnostics) => {
            return Err(CompilationProductError::ProductionAttempt {
                diagnostics,
                accounting: std::mem::take(settlement.source_completion_accounting_mut()),
            });
        }
    };
    let accounting = settlement.source_completion_accounting_mut().clone();
    let report = phase_profile::scope(
        "06.0.f_settlement_finish_native",
        || -> Result<_, CompilationProductError> {
            Ok(settlement
                .finish_native(produced.artifact, rollback.into_receipt())?
                .with_runtime_provisions(produced.runtime_provisions)
                .map_err(|message| vec![Diagnostic::error(message)])?)
        },
    )
    .map_err(|error| CompilationProductError::ProductionAttempt {
        diagnostics: error.into_diagnostics(),
        accounting,
    })?;
    Ok(CompilationOutput::Report(report))
}
