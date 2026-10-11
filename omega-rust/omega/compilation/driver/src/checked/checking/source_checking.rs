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
) -> Result<CheckedProductSource, Vec<Diagnostic>> {
    let PreparedProductSource {
        mut typed,
        pending_pre_checks,
        computed_build_config,
        mut admission,
        build_sources,
        mut timings,
        retained_build_check,
    } = source;
    // A deferred application still needs selected runtime meaning. Its portable
    // recipe must cross Psi before realization can close it; folding against one
    // target here would make the shared source judgment target-dependent again.
    if !pending_pre_checks.is_empty() {
        return Err(vec![Diagnostic::error(
            "unimplemented: portable provider-dependent constant recipes; target realization cannot fold selected values into checked source",
        )]);
    }
    provider_planning::prepare_source_completion(&mut typed, package_inputs)?;
    admission.provider_defaults = provider_planning::target_machines::retain_provider_defaults(
        provider_planning::ProviderPlanningContext::new(&typed, source::DependencyScope::Product),
    )?;
    let build = &computed_build_config.config;
    let representations = representation_planning::rederive_opaque_representation_selections(
        &typed,
        computed_build_config.selected_build_machine_symbol,
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
    let inferred_synchronous_invocations = validation::infer_synchronous_invocations(&typed);
    let symbol_sources = typed.symbols.source_map();
    let preparation = typed_trees_to_typed_trees::PreparationRequest::settled()
        .with_opaque_property_receipts(&opaque_properties)
        .with_asm_authority(
            validation::AsmAuthorityAdmission::from_freestanding(build.settings.freestanding)
                .with_grants(
                    build.settings.privileged_services.port_io,
                    build.settings.privileged_services.interrupt_table,
                ),
        )
        .with_wire_compatibility_demands(&build.wire_compatibility_demands);
    let checking = typed_trees_to_checked_trees::CheckingRequest::new().with_declaration_authority(
        package_inputs.map(|inputs| inputs as &dyn validation::DeclarationSelectionAuthority),
    );
    let outcome = timings
        .record(TYPED_TREES_TO_CHECKED_TREES, || {
            let started = std::time::Instant::now();
            let admission = preparation.admission();
            let prior = reuse
                .prior
                .product
                .iter()
                .chain(reuse.prior.sibling_product_checks.iter())
                .find(|prior| prior.judges_preparation_input(&typed, &admission));
            let result = if let Some(prior) = prior {
                typed_trees_to_checked_trees::reuse_retained_check(prior, &checking, reuse.retain)
            } else {
                let retained_input = reuse.retain.then(|| (Arc::new(typed.clone()), admission));
                typed_trees_to_typed_trees::prepare_typed_trees(typed, &preparation).and_then(
                    |(typed, preparation)| {
                        typed_trees_to_checked_trees::lower_typed_trees_reusing(
                            typed,
                            preparation,
                            retained_input,
                            &checking,
                            reuse.prior.product.as_ref(),
                            reuse.retain,
                        )
                    },
                )
            };
            if result.as_ref().is_ok_and(|outcome| outcome.took_prior) {
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
    let (program, product_selection) = typed_trees_to_checked_trees::select_checked_tree_product(
        Arc::new(outcome.checked),
        roots,
    )?;
    let sources = Arc::new(build_sources.seal(&program, package_inputs)?);
    let accepted_template_classifications =
        trust_model::AcceptedTemplateClassifications::capture(&program);
    Ok(CheckedProductSource {
        program,
        inferred_synchronous_invocations,
        computed_build_config,
        admission,
        sources,
        timings,
        accepted_template_classifications,
        retained_checks: compilation_products::RetainedCompilationChecks {
            build: retained_build_check,
            product: outcome.retained,
            ..Default::default()
        },
        product_selection,
    })
}
