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

use checked_trees::CheckedProviderClosedRequirement;
use language_semantics::MachineSupplyMode;
use typed_trees::trait_definition::AppliedTraitScope;

pub(crate) fn record_provider_closed_requirements(
    program: &dyn validation::CheckingInput,
) -> Vec<CheckedProviderClosedRequirement> {
    // Built on the first generic template: most programs name none. An
    // invalid declaration identity was reported when checking resolved the
    // same symbols; the rows below only join what checking admitted.
    let mut symbols = None;
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
            if let Some(definition) = program.trait_definition_by_symbol(conformance.symbol)
                && let Some(scope) = AppliedTraitScope::new(
                    program.typed_program(),
                    definition,
                    program
                        .type_reference_table
                        .type_reference_handles(conformance.arguments),
                )
            {
                for application in program.closed_requirement_applications() {
                    if conformance.requirement_symbol == application.template
                        && program.requirement_application_matches_scope(
                            application.signature.symbol,
                            &scope,
                        )
                        && program
                            .requirement_realization_specialization(
                                application.signature.symbol,
                                provider_template(program, provider.symbol),
                                provider.symbol,
                            )
                            .is_some()
                    {
                        rows.push(CheckedProviderClosedRequirement {
                            provider: provider.symbol,
                            template: application.template,
                            closed_requirement: application.signature.symbol,
                        });
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
            let symbols = symbols.get_or_insert_with(|| {
                validation::TopLevelSymbols::build(program, &mut Vec::new())
            });
            if let Ok(closed) = validation::resolve_closed_requirement_application(
                program, provider, template, symbols,
            ) {
                rows.push(CheckedProviderClosedRequirement {
                    provider: provider.symbol,
                    template: template.symbol,
                    closed_requirement: closed.symbol,
                });
            }
        }
        // Named maps retain their authored requirement coordinate. Close it
        // only through a checked realization specialization of the complete
        // tuple, independently of whether any caller uses that coordinate.
        for application in program.closed_requirement_applications() {
            let admitted = program.conformances().iter().any(|implementation| {
                implementation.type_parameters.is_empty()
                    && implementation.lifetime_parameters.is_empty()
                    && program
                        .trait_definition_by_symbol(implementation.trait_symbol)
                        .and_then(|definition| {
                            AppliedTraitScope::new(
                                program.typed_program(),
                                definition,
                                program
                                    .type_reference_table
                                    .type_reference_handles(implementation.arguments),
                            )
                        })
                        .is_some_and(|scope| {
                            program.requirement_application_matches_scope(
                                application.signature.symbol,
                                &scope,
                            )
                        })
                    && implementation.rows.iter().any(|row| {
                        row.declaring_trait == application.declaring_trait
                            && row.requirement == application.template
                            && program
                                .requirement_realization_specialization(
                                    application.signature.symbol,
                                    row.realization_machine,
                                    provider.symbol,
                                )
                                .is_some()
                    })
            });
            if admitted {
                rows.push(CheckedProviderClosedRequirement {
                    provider: provider.symbol,
                    template: application.template,
                    closed_requirement: application.signature.symbol,
                });
            }
        }
    }
    rows.sort_by_key(|row| {
        (
            row.provider.arena_index(),
            row.provider.generation(),
            row.template.arena_index(),
            row.template.generation(),
            row.closed_requirement.arena_index(),
            row.closed_requirement.generation(),
        )
    });
    rows.dedup_by(|later, earlier| {
        later.provider == earlier.provider
            && later.template == earlier.template
            && later.closed_requirement == earlier.closed_requirement
    });
    rows
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
        return symbols::SymbolHandle::invalid();
    };
    if receipts.next().is_some() {
        return symbols::SymbolHandle::invalid();
    }
    receipt.template
}
