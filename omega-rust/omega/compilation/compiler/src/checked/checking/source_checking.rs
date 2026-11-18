// Prepare the actual frozen typed owner, judge its complete structural closure,
// and retain that same owner for product consumers. No old judgment or checked
// tree projection participates in producing the application.

use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use products::{CheckedProductSource, CompileFailure, PreparedProductSource};
use std::sync::Arc;

pub(crate) fn check_product_source(
    source: PreparedProductSource,
    package_inputs: Option<&PackageCompilationInputs>,
    calling_targets: &[target::TargetProfile],
    rollback: &crate::OptimizationRollback,
    inspection: Option<&str>,
    frontier: Option<&mut Option<super::typed_frontier::TypedFrontier>>,
) -> Result<CheckedProductSource, CompileFailure> {
    let build_accounting = source.build_accounting.clone();
    let symbol_sources = source.typed.symbols.source_map();
    let mut accounting = source.source_completion_accounting.clone();
    let result = (|| -> Result<CheckedProductSource, Vec<Diagnostic>> {
        let PreparedProductSource {
            build_accounting: _,
            source_completion_accounting: _,
            typed,
            computed_build_config,
            admission,
            build_sources,
            mut timings,
        } = source;
        let build = &computed_build_config.config;
        let targets = package_compilation::source_imports::source_targets(package_inputs);
        let calling_targets = calling_targets
            .iter()
            .map(|target| target.target_name())
            .collect::<Vec<_>>();
        let source_compiler::product_source::CompletedProductSource {
            program,
            source_completion_accounting,
            terminal_publication,
        } = source_compiler::product_source::complete_product_source(
            typed,
            source_compiler::product_source::ProductSourceInputs {
                targets: &targets,
                calling_targets: &calling_targets,
                subject: package_inputs.map(PackageCompilationInputs::root),
                root_bindings: &build.root_bindings,
                provider_selections: &build.provider_selections,
                default_provider_selections: &build.default_provider_selections,
                native_bindings: &build.native_bindings,
                representations: source_compiler::ProductRepresentationInput {
                    activation: computed_build_config.selected_build_machine_symbol,
                    declared: &computed_build_config.declared_product_selections,
                    executed: &computed_build_config.product_selection_requests,
                    retained: &build.opaque_representation_selections,
                },
                layout_selection_authority: package_inputs.map(|inputs| {
                    Arc::new(inputs.clone())
                        as Arc<dyn source_compiler::evaluation::BuildTimeSelectionAuthority>
                }),
                asm_authority: Some(
                    language_core::inline_assembly::AsmAuthorityAdmission::from_freestanding(
                        build.settings.freestanding,
                    )
                    .with_grants(
                        build.settings.privileged_services.port_io,
                        build.settings.privileged_services.interrupt_table,
                    ),
                ),
                wire_demands: &build.wire_compatibility_demands,
                optimizations: rollback
                    .settle(&build.settings.optimizations)
                    .effective()
                    .project_psi()
                    .selections()
                    .clone(),
            },
            &build_accounting,
            &mut accounting,
            inspection,
            frontier,
            &mut crate::observations::PsiStageTimings::new(&mut timings),
        )?;
        let sources = Arc::new(build_sources.seal(&program, package_inputs)?);
        let provider_inventory = Arc::new(
            source_compiler::source_state::provider_declarations::PreparedProviderInventory::new(
                Arc::clone(&program),
                sources.exact_toolchain_sources(),
            ),
        );
        Ok(CheckedProductSource {
            optimizations: rollback
                .settle(&computed_build_config.config.settings.optimizations)
                .effective()
                .clone(),
            source_completion_accounting,
            program,
            provider_inventory,
            terminal_publication,
            computed_build_config,
            admission,
            sources,
            timings,
        })
    })();
    let mut spent = build_accounting;
    spent.append(accounting);
    result.map_err(|mut diagnostics| {
        if let Some(sources) = symbol_sources.as_deref() {
            super::attribute_diagnostics(&mut diagnostics, sources, package_inputs);
        }
        CompileFailure::source_completion(diagnostics, spent)
    })
}
