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
        computed_build_config,
        admission,
        build_sources,
        mut timings,
        retained_build_check,
    } = source;
    // A deferred application still needs selected runtime meaning. The shared
    // judgment checks its symbolic recipe; each target closure resumes this
    // continuation under its own selected rows after settlement.
    let constant_recipe_continuations = pending_pre_checks;
    let constant_recipe_source =
        (!constant_recipe_continuations.is_empty()).then(|| Arc::new(typed.clone()));
    let build = &computed_build_config.config;
    let mut entry_contexts = Vec::new();
    for root in build.program_roots() {
        let mut entries = vec![root];
        for family in &typed.target_machine_families {
            if family.canonical == root
                || family.members.iter().any(|member| member.machine == root)
            {
                entries.extend(family.members.iter().map(|member| member.machine));
            }
        }
        for entry_symbol in entries {
            let Some(entry) = typed.machine_by_symbol(entry_symbol) else {
                continue;
            };
            if !entry.supplies_checked_body() {
                continue;
            }
            let Some(state) = typed.machine_states(entry).first() else {
                continue;
            };
            if let Some(context) =
                provider_planning::program_entry::provisioned_entry_context(&typed, entry, state)
                    .map_err(|diagnostic| vec![diagnostic])?
            {
                let origin = checked_trees::typed_trees::machine::EntryContextOrigin {
                    entry: entry_symbol,
                    context_parameter: context.parameter.symbol,
                };
                if !entry_contexts.contains(&origin) {
                    entry_contexts.push(origin);
                }
            }
        }
    }
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
    let symbol_sources = typed.symbols.source_map();
    let source_admission = psi_compiler::SourceAdmission {
        entry_contexts: &entry_contexts,
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
        .record_result(TYPED_TREES_TO_CHECKED_TREES, || {
            let started = std::time::Instant::now();
            let result = psi_compiler::CheckRequest {
                program: typed,
                admission: source_admission,
                previous: &previous,
                retain: reuse.retain,
            }
            .check();
            if !result.as_ref().is_ok_and(|outcome| outcome.reused) {
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
        program,
        provider_defaults,
        retained,
        selection: product_selection,
    } = selections.select(outcome, roots)?;
    let sources = Arc::new(build_sources.seal(&program, package_inputs)?);
    let accepted_template_classifications =
        trust_model::AcceptedTemplateClassifications::capture(&program);
    Ok(CheckedProductSource {
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
        constant_recipe_continuations,
        constant_recipe_source,
    })
}
