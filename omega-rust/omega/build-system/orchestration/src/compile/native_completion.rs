/*

Complete post-Terminal native products and scalar constant demands (`complete_product`, `complete_products`).

By the time a product reaches this module, the source compiler has already fixed
the Terminal Psi artifact, package authority policy, independent-component joins,
and report settlement inside a `PendingNativeProduct`:
1. Native preparation lowers the Terminal Psi module and surfaces any
   compile-time scalar constant demands.
2. We evaluate each scalar demand through
   `build_settings::scalar_completion` (charging the settlement's
   `source_completion_accounting_mut` budget) and feed the result back into the
   demand slot so neither `compiler` nor `native_compiler` embeds an interpreter
   callback.
3. For `NativeImageDemand::Emit`, the native backend emits the executable image
   and attaches its runtime provisions to the settlement. For
   `NativeImageDemand::RuntimeAdmissionOnly` (`omega run --engine terminal`),
   runtime admission is validated without emitting machine code before settling
   the report.
4. Any failure wraps the diagnostics together with the attempted evaluation
   accounting (`CompilationProductError::ProductionAttempt`) so resource-bound
   (`Incomplete`) refusals are preserved.

Non-native `CompilationOutput` variants (`Report`, `Inspection`, etc.) are
already complete and pass through `complete_product` and `complete_products`
unchanged. When multiple native targets are present, `complete_products` drains
them across bounded compiler-stack workers via `queue::execute`.

*/

mod queue;

use compilation_products::PendingNativeProduct;
use compiler::{CompilationOutput, CompilationProductError};
use diagnostics::Diagnostic;

pub fn complete_product(
    product: CompilationOutput,
) -> Result<CompilationOutput, CompilationProductError> {
    let CompilationOutput::Native(product) = product else {
        return Ok(product);
    };
    compiler::checked::run_on_compile_thread(|| complete_native(*product))
}

pub(crate) fn complete_products<Owner: Send>(
    products: Vec<(Owner, Result<CompilationOutput, CompilationProductError>)>,
) -> Vec<(Owner, Result<CompilationOutput, CompilationProductError>)> {
    let count = products.len();
    let mut completed = Vec::new();
    let mut pending = Vec::new();
    for (position, (owner, product)) in products.into_iter().enumerate() {
        match product {
            Ok(CompilationOutput::Native(product)) => {
                pending.push((position, move || (owner, complete_native(*product))))
            }
            result => completed.push((position, (owner, result))),
        }
    }
    completed.extend(queue::execute(pending));
    completed.sort_unstable_by_key(|(position, _)| *position);
    assert_eq!(completed.len(), count, "native completion lost a product");
    assert!(completed.iter().map(|(position, _)| *position).eq(0..count));
    completed.into_iter().map(|(_, product)| product).collect()
}

enum NativeStop {
    Image(native_compiler::RealizedNativeProduct),
    RuntimeAdmission(Option<compilation_products::TerminalRuntimeProvisions>),
}

pub(crate) fn complete_native(
    product: PendingNativeProduct,
) -> Result<CompilationOutput, CompilationProductError> {
    let PendingNativeProduct {
        terminal,
        retained_native_inputs: _,
        components,
        accepted_package_permissions,
        request,
        rollback,
        subsystem,
        mut settlement,
        tests,
    } = product;
    // Registered tests run before the application can succeed. A failing or
    // incomplete test refuses the application with its exact identity.
    super::test_execution::run_tests(tests)?;
    let image = request.image;
    let produced = {
        let selected = components
            .as_ref()
            .map_err(|reason| vec![Diagnostic::error(reason.clone())])?;
        let mut components = Vec::with_capacity(selected.len());
        for component in selected {
            components.push(native_compiler::ComponentCompilation {
                input: component
                    .input()
                    .map_err(|reason| vec![Diagnostic::error(reason)])?,
                plan: component.plan(),
                receiving_identities: component.receiving_identities(),
                accepted_package_permissions: component.accepted_package_permissions(),
            });
        }
        let post_terminal = rollback.effective().project_post_terminal();
        let request = native_compiler::NativeCompileRequest {
            installation_assumptions: &request.installation_assumptions,
            private_entry_ram: &request.private_entry_ram,
            optimization_selections: post_terminal.selections(),
            terminal_authority_policy: request.terminal_authority_policy,
            accepted_package_terminal_authority_permission_policy: accepted_package_permissions,
            terminal_authority_permission_policy: request.terminal_authority_permission_policy,
            image_request: image_emission::ExecutableImageEmissionRequest::direct(subsystem),
            imports: &[],
            components: &components,
        };
        let mut prepared =
            native_compiler::prepare(&terminal, request).map_err(|(_, diagnostics)| diagnostics)?;
        for completion in prepared.scalar_completions() {
            let completed = build_settings::scalar_completion::complete(
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
        match image {
            compilation_products::NativeImageDemand::Emit => native_compiler::compile(prepared)
                .map(NativeStop::Image)
                .map_err(|(_, diagnostics)| diagnostics),
            compilation_products::NativeImageDemand::RuntimeAdmissionOnly => {
                native_compiler::admit_runtime(prepared)
                    .map(NativeStop::RuntimeAdmission)
                    .map_err(|(_, diagnostics)| diagnostics)
            }
        }
    };
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
    let report =
        (|| -> Result<_, CompilationProductError> {
            match produced {
                NativeStop::Image(produced) => Ok(settlement
                    .finish_native(produced.artifact, rollback.into_receipt())?
                    .with_runtime_provisions(produced.runtime_provisions)
                    .map_err(|message| vec![Diagnostic::error(message)])?),
                NativeStop::RuntimeAdmission(provisions) => Ok(settlement
                    .finish_terminal_runtime(terminal, provisions, rollback.into_receipt())?),
            }
        })()
        .map_err(|error| CompilationProductError::ProductionAttempt {
            diagnostics: error.into_diagnostics(),
            accounting,
        })?;
    Ok(CompilationOutput::Report(report))
}
