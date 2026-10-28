//! Resolve inert source requests only after Build has joined Product sources.
//! Semantic admission belongs to Psi; this coordinator publishes its results.

mod additional_declarations;
mod exclusions;

use compilation_products::ComputedBuildConfig;
use diagnostics::Diagnostic;

pub(super) fn admit(
    source: &mut psi_compiler::SourceContinuation,
    computed: &mut ComputedBuildConfig,
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
) -> Result<(), Vec<Diagnostic>> {
    prepare_provider_applications(source, computed)?;
    let typed = source.typed();
    let representations = representation_planning::collect_opaque_representation_selections(
        typed,
        computed.selected_build_machine_symbol,
        &computed.declared_product_selections,
        &computed.product_selection_requests,
    )?;
    let providers =
        build_time_evaluation::provider_declarations::collect_product_provider_selections(
            build_time_evaluation::provider_declarations::ProviderPlanningContext::new(
                typed,
                source::DependencyScope::Product,
            ),
            computed.selected_build_machine_symbol,
            &computed.declared_product_selections,
            &computed.product_selection_requests,
        )?;
    let native = build_time_evaluation::native_bindings::collect(
        typed,
        computed.selected_build_machine_symbol,
        &computed.declared_product_selections,
        &computed.native_binding_requests,
    )?;
    let grants = trust_model::collect_product_root_grants(
        typed,
        computed.selected_build_machine_symbol,
        &computed.declared_product_selections,
        &computed.product_selection_requests,
    )?;
    computed.config.grants = grants;
    computed.config.opaque_representation_selections = representations;
    computed.config.provider_selections = providers;
    computed.config.package_provider_selections = package_inputs
        .map(|inputs| inputs.rejoin_package_provider_selections(typed))
        .transpose()?
        .unwrap_or_default();
    computed.config.native_bindings = native
        .into_iter()
        .map(|selection| {
            let evaluation = computed
                .config
                .native_binding_evaluation
                .clone()
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "native Build selection lost its completed evaluation",
                    )]
                })?;
            Ok(provider_planning::native_bindings::NativeBuildSupply {
                selection,
                evaluation,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    for bundle in package_inputs
        .into_iter()
        .flat_map(|inputs| inputs.product_dependency_native_handoffs())
    {
        let selections = build_time_evaluation::native_bindings::collect_captured(
            typed,
            bundle.native_declarations(),
            bundle.native_intents(),
        )?;
        for selection in selections {
            let evaluation = bundle.native_evaluation().cloned().ok_or_else(|| {
                vec![Diagnostic::error(
                    "native dependency effect lost its completed evaluation",
                )]
            })?;
            computed.config.native_bindings.push(
                provider_planning::native_bindings::NativeBuildSupply {
                    selection,
                    evaluation,
                },
            );
        }
    }
    additional_declarations::admit(typed, computed)?;
    computed.config.behavior_exclusions = exclusions::resolve(
        typed,
        computed.selected_build_machine_symbol,
        &computed.declared_product_selections,
        &computed.product_selection_requests,
        &computed.behavior_exclusion_requests,
    )?;
    Ok(())
}

/// Preparation preserves both the original recipe and each reached closed
/// helper substitution. The immutable source checker supplies all judgments.
fn prepare_provider_applications(
    source: &mut psi_compiler::SourceContinuation,
    computed: &ComputedBuildConfig,
) -> Result<(), Vec<Diagnostic>> {
    use language_semantics::declaration_selection::BuildOperation;
    use typed_trees::product_operand::ProductApplicationDemand;
    let occurrences =
        typed_trees::product_operand::collect_product_selection_occurrences(source.typed());
    let author_for =
        |request: &language_semantics::declaration_selection::ProductDeclarationRequest| {
            let mut owners = occurrences
                .iter()
                .filter(|occurrence| *occurrence.request == *request)
                .map(|occurrence| source.typed().symbols.authored_origin(occurrence.machine));
            let owner = owners.next().ok_or_else(|| {
                vec![
                    Diagnostic::error(
                        "Product provider recipe lost its original authored body occurrence",
                    )
                    .with_source_span(request.source_span),
                ]
            })?;
            if owners.any(|other| other != owner) {
                return Err(vec![
                    Diagnostic::error(
                        "Product provider recipe has multiple original lexical owners",
                    )
                    .with_source_span(request.source_span),
                ]);
            }
            Ok(owner)
        };
    let formations = computed
        .declared_product_selections
        .iter()
        .filter(|request| request.operation == BuildOperation::ProviderSelection)
        .map(|request| Ok((request.clone(), author_for(request)?)))
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    for (request, author) in formations {
        for (index, operand) in request.operands.iter().enumerate() {
            let role = request
                .operation
                .product_operand_role(index)
                .ok_or_else(|| vec![Diagnostic::error("provider recipe lost its operand role")])?;
            source.prepare_product_operand_application(
                operand,
                role,
                author,
                None,
                ProductApplicationDemand::FormationOnly,
            )?;
        }
    }
    for event in computed
        .product_selection_requests
        .iter()
        .filter(|event| event.request.operation == BuildOperation::ProviderSelection)
    {
        let author = typed_trees::product_operand::resolve_build_request_author(
            source.typed(),
            event.selecting_machine.ok_or_else(|| {
                vec![Diagnostic::error(
                    "executed provider recipe lost its original author",
                )]
            })?,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        for (index, operand) in event.request.operands.iter().enumerate() {
            let role = event
                .request
                .operation
                .product_operand_role(index)
                .ok_or_else(|| vec![Diagnostic::error("provider recipe lost its operand role")])?;
            source.prepare_product_operand_application(
                operand,
                role,
                author,
                event.static_environment.as_deref(),
                ProductApplicationDemand::Selected,
            )?;
        }
    }
    Ok(())
}
