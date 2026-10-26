//! Settle the executed Product request ledger after the dependency join.
//!
//! Build executes ordinary calls while Product bodies remain unavailable. This
//! owner resolves their inert declaration locators once the admitted Product
//! frontier exists, before this activation's generated extension can contribute
//! names. The ledger contains only committed effects; uncalled helpers and
//! untaken branches cannot acquire grants by a static body scan. Final provider,
//! representation and trust owners still validate their complete contracts.

use build_time_evaluation::provider_declarations::{
    ProviderPlanningContext, ProviderSelectionIdentity, ResolvedProviderSelectionOperands,
    collect_resolved_provider_selections,
};
use compilation_products::{BuildConfig, ServiceAuditRegistration};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildOperation, BuildProductOperand, ExecutedBuildProductRequest,
    ProductDeclarationSelectionRequest, ProductOperandRole,
};
use language_semantics::wire::{WireCompatibilityDemand, WireCompatibilityFact};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::product_operand::{
    resolve_build_request_author, resolve_product_locator_in_frontier,
};

pub fn apply_product_requests(
    typed: &TypedTrees,
    root: Option<SymbolHandle>,
    source_count: usize,
    executed: &[ExecutedBuildProductRequest],
    config: &mut BuildConfig,
) -> Result<(), Vec<Diagnostic>> {
    let roots = executed
        .iter()
        .filter_map(|request| match request {
            ExecutedBuildProductRequest::RootBinding(binding) => Some(binding.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    config.root_bindings = provider_planning::program_entry::collect_root_bindings(typed, &roots)?;
    let mut providers = Vec::new();
    for executed in executed {
        if let ExecutedBuildProductRequest::BehaviorExclusion(exclusion) = executed {
            let row = trust_model::resolve_executed_behavior_exclusion(typed, exclusion)?;
            if !config
                .behavior_exclusions
                .iter()
                .any(|existing| existing == &row)
            {
                config.behavior_exclusions.push(row);
            }
            continue;
        }
        let ExecutedBuildProductRequest::DeclarationSelection(selection) = executed else {
            continue;
        };
        let request = &selection.request;
        let refuse =
            |reason: String| vec![Diagnostic::error(reason).with_source_span(request.source_span)];
        request
            .validate_shape()
            .map_err(|reason| refuse(reason.into()))?;
        let author = resolve_build_request_author(typed, selection.author).map_err(refuse)?;
        let operand = |position| {
            resolve_request_operand(typed, request, position, source_count).map_err(refuse)
        };
        let subject = operand(0)?;
        match request.operation {
            BuildOperation::ProviderSelection => {
                let provider = operand(1)?;
                let identity = |symbol, position| {
                    let BuildProductOperand::Declaration(locator) = &request.operands[position]
                    else {
                        unreachable!("validated declaration position");
                    };
                    ProviderSelectionIdentity {
                        symbol,
                        package: typed.symbols.symbol_package_identity(symbol),
                        canonical_path: typed.symbols.display_path(symbol, "::"),
                        authored_path: locator.path.clone(),
                    }
                };
                providers.push(ResolvedProviderSelectionOperands {
                    subject: identity(subject, 0),
                    provider: identity(provider, 1),
                    composition: selection.composition,
                    selecting_machine: author,
                    source_span: request.source_span,
                });
            }
            BuildOperation::BoundaryAcceptance => {
                let selector = if let Some(definition) = typed.trait_definition_by_symbol(subject) {
                    typed.trait_declaration_path(definition)
                } else {
                    typed
                        .normalized_hermetic_symbol_identity(subject)
                        .map_err(|error| refuse(format!("boundary acceptance identity: {error}")))?
                };
                if !config.grants.iter().any(|grant| grant.selector == selector) {
                    config.grants.push(trust_model::AuthoredRootGrant {
                        selector,
                        selecting_machine: author,
                        source_span: request.source_span,
                    });
                }
            }
            BuildOperation::ServiceExclusion | BuildOperation::ServiceAudit => {
                let definition = typed
                    .trait_definition_by_symbol(subject)
                    .filter(|definition| definition.is_boundary)
                    .ok_or_else(|| {
                        refuse(
                            "Build service operand must name an exact visible boundary trait"
                                .into(),
                        )
                    })?;
                let identity = typed
                    .normalized_hermetic_symbol_identity(subject)
                    .map_err(|error| refuse(format!("Build service identity: {error}")))?;
                let name = definition.name.as_str().to_owned();
                if request.operation == BuildOperation::ServiceExclusion {
                    let exclusion = effects::AuthoredBehaviorExclusion {
                        kind: effects::AuthoredBehaviorExclusionKind::Service { identity, name },
                        selecting_machine: author,
                        source_span: request.source_span,
                    };
                    if !config.behavior_exclusions.iter().any(|existing| {
                        existing.kind == exclusion.kind
                            && existing.source_span == exclusion.source_span
                    }) {
                        config.behavior_exclusions.push(exclusion);
                    }
                } else if !config
                    .service_audits
                    .iter()
                    .any(|existing| existing.identity == identity)
                {
                    config
                        .service_audits
                        .push(ServiceAuditRegistration { identity, name });
                }
            }
            BuildOperation::AssumedBindingContract => {
                let root = root.ok_or_else(|| {
                    refuse("assumed binding contract has no consuming root Build".into())
                })?;
                if typed.symbols.symbol_package_identity(root)
                    != typed.symbols.symbol_package_identity(author)
                {
                    return Err(refuse("assumed binding contract may be authorized only by the consuming root, not a dependency helper".into()));
                }
                config.assumed_bindings.push(
                    build_time_evaluation::assumed_bindings::AssumedBindingContract {
                        required: subject,
                        supplied: operand(1)?,
                        selecting_machine: author,
                        source_span: request.source_span,
                    },
                );
            }
            BuildOperation::RepresentationSelection => {
                let representation =
                    representation_planning::close_resolved_representation_selection(
                        typed,
                        author,
                        subject,
                        operand(1)?,
                        request.source_span,
                    )
                    .map_err(|diagnostic| vec![diagnostic.with_source_span(request.source_span)])?;
                if config
                    .opaque_representation_selections
                    .iter()
                    .any(|existing| existing.opaque() == representation.opaque())
                {
                    return Err(refuse(
                        "build selects an opaque representation more than once".into(),
                    ));
                }
                config.opaque_representation_selections.push(representation);
            }
            BuildOperation::WireCompatibilityRequest => {
                let path = |position| {
                    operand(position).map(|symbol| typed.symbols.display_path(symbol, "::"))
                };
                let mut demand = WireCompatibilityDemand {
                    edge: path(0)?,
                    lineage: path(1)?,
                    local_schema: path(2)?,
                    peer_schema: path(3)?,
                    require_readable: false,
                    require_writable: false,
                    require_unknown_preservation: false,
                    require_canonical: false,
                    require_complete_migration: false,
                };
                for fact in &request.operands[4..] {
                    let BuildProductOperand::WireFact { fact, .. } = fact else {
                        return Err(refuse(
                            "wire demand lost its checked compatibility fact".into(),
                        ));
                    };
                    *match fact {
                        WireCompatibilityFact::Readable => &mut demand.require_readable,
                        WireCompatibilityFact::Writable => &mut demand.require_writable,
                        WireCompatibilityFact::PreserveUnknown => {
                            &mut demand.require_unknown_preservation
                        }
                        WireCompatibilityFact::Canonical => &mut demand.require_canonical,
                        WireCompatibilityFact::CompleteMigration => {
                            &mut demand.require_complete_migration
                        }
                    } = true;
                }
                if config.wire_compatibility_demands.iter().any(|existing| {
                    existing.edge == demand.edge
                        && existing.lineage == demand.lineage
                        && existing.local_schema == demand.local_schema
                        && existing.peer_schema == demand.peer_schema
                }) {
                    return Err(refuse("wire compatibility demand repeats the same edge, lineage and schema coordinates".into()));
                }
                config.wire_compatibility_demands.push(demand);
            }
            _ => {
                return Err(refuse(
                    "executed Product request is not a declaration-recording operation".into(),
                ));
            }
        }
    }
    config.provider_selections = collect_resolved_provider_selections(
        ProviderPlanningContext::new(
            typed.product_declarations(),
            source::DependencyScope::Product,
        ),
        providers,
    )?;
    Ok(())
}

fn resolve_request_operand(
    typed: &TypedTrees,
    request: &ProductDeclarationSelectionRequest,
    position: usize,
    source_count: usize,
) -> Result<SymbolHandle, String> {
    let Some(BuildProductOperand::Declaration(locator)) = request.operands.get(position) else {
        return Err("Build request has no declaration locator at this position".into());
    };
    let role = request
        .operation
        .product_operand_role(position)
        .filter(|role| *role != ProductOperandRole::WireCompatibilityFact)
        .ok_or("Build request has no declaration role at this position")?;
    resolve_product_locator_in_frontier(typed, &locator.path, locator.source_span, role, source_count)
        .ok_or_else(|| format!("Build operand `{}` does not resolve to one visible Product declaration of role {role:?} in the admitted source frontier", locator.path))
}
