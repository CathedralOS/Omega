//! Complete one portable Product judgment before target realization starts.
//!
//! Build supplies source admission and representation choices. Native provider
//! selections are not checking inputs and cannot change this program afterward.

use artifacts::compile_timings::TYPED_TREES_TO_CHECKED_TREES;
use compilation_products::{CheckedProductSource, PreparedProductSource, SettledCheckReuse};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use std::sync::Arc;

pub(crate) fn check_product_source(
    source: PreparedProductSource,
    package_inputs: Option<&PackageCompilationInputs>,
    rollback: &crate::OptimizationRollback,
    reuse: SettledCheckReuse,
    selections: &mut psi_compiler::ProductSelections,
) -> Result<CheckedProductSource, Vec<Diagnostic>> {
    let PreparedProductSource {
        typed,
        pending_pre_checks,
        checked_constant_inputs,
        source_preparation_inputs,
        computed_build_config,
        admission,
        build_sources,
        mut timings,
        retained_build_check,
    } = source;
    let build = &computed_build_config.config;
    let source_constants = psi_compiler::SourceConstantRequest {
        checked_inputs: checked_constant_inputs,
        source_preparation_inputs,
        pending: pending_pre_checks,
        selections: build
            .provider_selections
            .iter()
            .filter_map(|selection| {
                selection
                    .source_operands
                    .clone()
                    .map(|operands| build_time_evaluation::SourceProviderSelection { operands })
            })
            .collect(),
    };
    let representations = representation_planning::rederive_opaque_representation_selections(
        &typed,
        computed_build_config.selected_build_machine_symbol(),
        &build.opaque_representation_selections,
    )?;
    let opaque_properties = representations
        .iter()
        .filter(|selection| {
            selection.copy_disposition()
                == representation_planning::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
        })
        .map(|selection| validation::OpaqueDataPropertyReceipt::copy(selection.opaque()))
        .collect::<Vec<_>>();
    let symbol_sources = typed.symbols.source_map();
    let static_application_demands = build
        .assumed_binding_registrations
        .iter()
        .flat_map(|registration| [&registration.required, &registration.supplied])
        .map(|reference| reference.application_demand())
        .collect::<Vec<_>>();
    let assumed_binding_contract_demands = build
        .assumed_binding_registrations
        .iter()
        .map(
            |registration| typed_trees::static_applications::AssumedBindingContractDemand {
                required: registration.required.application_demand(),
                supplied: registration.supplied.application_demand(),
                selecting_machine: registration.selecting_machine,
                source_span: registration.source_span,
            },
        )
        .collect::<Vec<_>>();
    let source_admission = psi_compiler::SourceAdmission {
        completion: match package_inputs.and_then(|inputs| {
            inputs.accepted_semantic_binding(
                package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32,
            )
        }) {
            Some(binding) => {
                validation::source_completion::SourceCompletionAdmission::AcceptedProcessExit {
                    package: binding.package(),
                    declaration_path: binding.declaration_path(),
                }
            }
            None if package_inputs
                .is_some_and(PackageCompilationInputs::semantic_role_discovery) =>
            {
                validation::source_completion::SourceCompletionAdmission::DiscoverProcessExit
            }
            None => validation::source_completion::SourceCompletionAdmission::None,
        },
        static_application_demands: &static_application_demands,
        assumed_binding_contract_demands: &assumed_binding_contract_demands,
        opaque_properties: &opaque_properties,
        opaque_representations: &representations,
        asm_authority: Some(
            validation::AsmAuthorityAdmission::from_freestanding(build.settings.freestanding)
                .with_grants(
                    build.settings.privileged_services.port_io,
                    build.settings.privileged_services.interrupt_table,
                ),
        ),
        wire_demands: &build.wire_compatibility_demands,
        declarations: package_inputs
            .map(|inputs| inputs as &dyn validation::DeclarationSelectionAuthority),
    };
    let previous: Vec<_> = reuse
        .prior
        .product
        .iter()
        .chain(reuse.prior.sibling_product_checks.iter())
        .collect();
    let outcome = timings
        .record(TYPED_TREES_TO_CHECKED_TREES, || {
            let started = std::time::Instant::now();
            let result = psi_compiler::CheckRequest {
                program: typed.into(),
                constants: source_constants,
                admission: source_admission,
                previous: &previous,
                retain: reuse.retain,
            }
            .check();
            if result.as_ref().is_ok_and(|outcome| outcome.reused) {
                artifacts::compile_timings::counters::record_settled_check_reuse();
            } else {
                artifacts::compile_timings::counters::record_settled_check(started.elapsed());
            }
            result
        })
        .map_err(|mut diagnostics| {
            if let Some(sources) = symbol_sources.as_deref() {
                super::attribute_diagnostics(&mut diagnostics, sources, package_inputs);
            }
            diagnostics
        })?;
    // Product pruning is a Psi operation over all declared roots, not a
    // per-target copy of the checked program. Native reachability chooses the
    // selected entry later, without changing this portable judgment.
    let roots = build.checked_tree_product_roots(rollback);
    let psi_compiler::SelectedCheckedSource {
        original_input,
        program,
        provider_defaults,
        retained,
        selection: product_selection,
    } = selections.select(outcome, roots)?;
    let sources = Arc::new(build_sources.seal(&program, package_inputs)?);
    let accepted_template_classifications =
        trust_model::AcceptedTemplateClassifications::capture(&program);
    Ok(CheckedProductSource {
        original_input,
        program,
        provider_defaults: provider_defaults.into(),
        computed_build_config,
        admission,
        sources,
        timings,
        accepted_template_classifications,
        retained_checks: compilation_products::RetainedCompilationChecks {
            build: retained_build_check,
            product: retained,
            ..Default::default()
        },
        product_selection,
    })
}
