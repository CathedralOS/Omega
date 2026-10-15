//! Record the closed boundary requirement each concrete provider refines
//! (`checked_trees::CheckedProviderClosedRequirement`).
//!
//! Checking already admitted every provider conformance against the closed
//! application its specialization receipt selects
//! (`validate_machine_top_level_requirement_conformance`). The same join runs
//! once here over the finalized program so lowering's provider catalog reads
//! the selected requirement instead of re-entering the resolution.
//! Named trait maps and direct signature satisfaction use their independently
//! checked exact outer applications and realization tuple receipts; no machine
//! or token requirement is manufactured for either conformance route.

use arena::HandleSpan;
use checked_trees::{CheckedProviderClosedRequirement, CheckedProviderClosedRequirements};
use language_semantics::MachineSupplyMode;
use typed_trees::trait_definition::AppliedTraitScope;

pub(crate) fn record_provider_closed_requirements(
    program: &dyn validation::CheckingInput,
) -> CheckedProviderClosedRequirements {
    // Declaration identity was checked during resolution. These rows join
    // retained applications without rebuilding a second symbol index.
    let mut rows = Vec::new();
    for provider in program.machines().iter().filter(|machine| {
        machine.supply_mode == MachineSupplyMode::CheckedBody
            && program.machine_type_parameters(machine).is_empty()
    }) {
        for conformance in program.machine_trait_conformances(provider) {
            if conformance.external_binding.is_some() {
                continue;
            }
            // A closed provider keeps the original authored satisfaction edge.
            // Rejoin its entire trait application before its local const tuple.
            if let Some(scope) = AppliedTraitScope::from_machine_conformance(
                program.typed_program(),
                provider,
                conformance,
            ) {
                for application in program.closed_requirement_applications() {
                    if conformance.requirement_symbol == application.template
                        && program.requirement_application_matches_scope(
                            application.signature.symbol,
                            &scope,
                        )
                        && program.requirement_realization_matches(
                            application.signature.symbol,
                            provider_template(program, provider.symbol),
                            provider.symbol,
                        )
                    {
                        rows.push((
                            CheckedProviderClosedRequirement {
                                provider: provider.symbol,
                                selecting_declaration: provider.symbol,
                                template: application.template,
                                closed_requirement: application.signature.symbol,
                                receiving_application: application.trait_application_identity,
                                receiving_regions: HandleSpan::empty(),
                            },
                            Vec::new(),
                        ));
                    }
                }
            }
            if conformance.symbol != conformance.requirement_symbol {
                continue;
            }
            let Some(template) = program.machines().iter().find(|candidate| {
                candidate.symbol == conformance.symbol
                    && candidate.supply_mode == MachineSupplyMode::TopLevelRequirement
                    && !program.machine_type_parameters(candidate).is_empty()
            }) else {
                continue;
            };
            if let Ok(closed) =
                validation::resolve_closed_requirement_application(program, provider, template)
            {
                rows.push((
                    CheckedProviderClosedRequirement {
                        provider: provider.symbol,
                        selecting_declaration: provider.symbol,
                        template: template.symbol,
                        closed_requirement: closed.symbol,
                        receiving_application: Default::default(),
                        receiving_regions: HandleSpan::empty(),
                    },
                    Vec::new(),
                ));
            }
        }
        // Named maps retain their authored requirement coordinate. Close it
        // only through a checked realization specialization of the complete
        // tuple, independently of whether any caller uses that coordinate.
        // Record every selecting map rather than collapsing them with `any`:
        // one executable body may belong to multiple independent selections.
        for application in program.closed_requirement_applications() {
            let Some(receiving_scope) = application.trait_scope.scope(program.typed_program())
            else {
                continue;
            };
            for implementation in program.conformances().iter().filter(|implementation| {
                implementation.type_parameters.is_empty()
                    && implementation.rows.iter().any(|row| {
                        row.declaring_trait == application.declaring_trait
                            && row.requirement == application.template
                            && program.requirement_realization_matches(
                                application.signature.symbol,
                                row.realization_machine,
                                provider.symbol,
                            )
                    })
            }) {
                let Some(correspondence) = validation::provider_region_correspondence(
                    program,
                    implementation,
                    &receiving_scope,
                ) else {
                    continue;
                };
                rows.push((
                    CheckedProviderClosedRequirement {
                        provider: provider.symbol,
                        selecting_declaration: implementation.symbol,
                        template: application.template,
                        closed_requirement: application.signature.symbol,
                        receiving_application: application.trait_application_identity,
                        receiving_regions: HandleSpan::empty(),
                    },
                    correspondence.receiving_regions,
                ));
            }
        }
    }
    rows.sort_by_key(|(row, _)| {
        (
            row.provider.arena_index(),
            row.provider.generation(),
            row.selecting_declaration.arena_index(),
            row.selecting_declaration.generation(),
            row.template.arena_index(),
            row.template.generation(),
            row.closed_requirement.arena_index(),
            row.closed_requirement.generation(),
            row.receiving_application,
        )
    });
    rows.dedup();
    let mut recorded = CheckedProviderClosedRequirements::default();
    for (row, receiving_regions) in rows {
        recorded.insert(row, receiving_regions);
    }
    recorded
}

/// The exact specialization receipt, not a generated-name suffix, owns the
/// concrete body's declaration. An ambiguous receipt supplies no template.
fn provider_template(
    program: &dyn validation::CheckingInput,
    provider: symbols::SymbolHandle,
) -> symbols::SymbolHandle {
    let mut receipts = program
        .machine_specializations
        .iter()
        .filter(|receipt| receipt.instance == provider);
    let Some(receipt) = receipts.next() else {
        return provider;
    };
    if receipts.next().is_some() {
        return symbols::SymbolHandle::invalid();
    }
    receipt.template
}
