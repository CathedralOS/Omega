//! Borrow checked receiving coordinates beneath one unchanged whole provider.
//!
//! A declaration pattern and its caller application are not equal region keys.
//! Source conformance checking already admitted the map; its retained facts
//! qualify each receiving requirement. Selection consumes those facts rather
//! than erasing regions or rebuilding the map's schema with caller operands.

use crate::ProviderPlanningContext;
use checked_trees::typed_trees::trait_definition::AppliedTraitScope;
use checked_trees::{CheckedProviderClosedRequirement, CheckedTrees};
use effects::provider_plan::{ProviderPlan, ServiceSchema};

/// Invocation-local view of checked rows, not a new IR or admission grant.
pub struct ProviderReceiverApplication<'checked> {
    pub declaration_view: usize,
    pub requirements: Vec<&'checked CheckedProviderClosedRequirement>,
}

/// Join a receiver to exactly one declared view and its complete checked
/// application roster. Absence means this plan does not supply that receiver.
pub fn provider_receiver_application<'checked>(
    checked: &'checked CheckedTrees,
    plan: &ProviderPlan,
    receiver: &AppliedTraitScope<'_>,
    schema: &ServiceSchema,
) -> Option<ProviderReceiverApplication<'checked>> {
    let typed = &checked.typed;
    let context = ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let implementation = super::named_provider(context, plan);
    if let Some(declaration_view) = plan.schema.receiver_application_index(schema)
        && implementation.is_none_or(|map| map.lifetime_parameters.is_empty())
    {
        return Some(ProviderReceiverApplication {
            declaration_view,
            requirements: Vec::new(),
        });
    }
    let implementation = implementation?;
    let pattern = AppliedTraitScope::from_conformance(typed, implementation)?
        .declaring_scope(typed, receiver.declaration.symbol)?;
    let arguments = typed
        .type_reference_table
        .type_reference_handles(implementation.arguments);
    let pattern_schema =
        crate::service_schema::from_typed_application(context, &[], pattern, arguments)?;
    let declaration_view = plan.schema.receiver_application_index(&pattern_schema)?;
    let mut demands = Vec::new();
    receiver.visit(typed, &mut |declaring| {
        if declaring.declaration.is_boundary {
            for signature in typed.trait_machine_signatures(declaring.declaration) {
                demands.push((signature.symbol, declaring.application_identity(typed)));
            }
        }
    });
    let mut requirements = Vec::new();
    let mut receiving_regions = None;
    for (template, application_identity) in demands {
        let application_identity = application_identity?;
        let applications = typed
            .closed_requirement_applications()
            .filter(|application| {
                application.template == template
                    && application.trait_application_identity == application_identity
            })
            .collect::<Vec<_>>();
        if applications.is_empty() {
            return None;
        }
        let mut authored = implementation
            .rows
            .iter()
            .filter(|row| row.requirement == template);
        let authored = authored.next().filter(|_| authored.next().is_none())?;
        for application in applications {
            let mut matching = checked
                .facts
                .provider_closed_requirements
                .iter()
                .filter(|row| {
                    row.selecting_declaration == implementation.symbol
                        && row.template == template
                        && row.closed_requirement == application.signature.symbol
                        && row.receiving_application == application_identity
                        && typed.requirement_realization_matches(
                            row.closed_requirement,
                            authored.realization_machine,
                            row.provider,
                        )
                });
            let row = matching.next().filter(|_| matching.next().is_none())?;
            let regions = checked
                .facts
                .provider_closed_requirements
                .receiving_regions(row)?;
            if regions.len() != implementation.lifetime_parameters.len()
                || receiving_regions.is_some_and(|existing| existing != regions)
            {
                return None;
            }
            receiving_regions = Some(regions);
            if !requirements.contains(&row) {
                requirements.push(row);
            }
        }
    }
    (!requirements.is_empty()).then_some(ProviderReceiverApplication {
        declaration_view,
        requirements,
    })
}
