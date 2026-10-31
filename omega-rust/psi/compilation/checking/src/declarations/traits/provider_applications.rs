//! Derive exact public-application/provider associations from retained typed rows.
//!
//! These rows select no Build provider and certify no body or effect. Checking
//! readers must still judge every associated body. The checked facts producer
//! publishes the same associations only after ordinary conformance admission.

use language_semantics::MachineSupplyMode;
use symbols::SymbolHandle;
use typed_trees::trait_definition::AppliedTraitScope;
use typed_trees::type_identity::{LifetimeParameterIdentity, NormalizedTypeIdentity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderApplicationIdentity {
    pub provider: SymbolHandle,
    pub selecting_declaration: SymbolHandle,
    pub template: SymbolHandle,
    pub closed_requirement: SymbolHandle,
    pub receiving_application: NormalizedTypeIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderApplicationAssociation {
    pub identity: ProviderApplicationIdentity,
    pub receiving_regions: Vec<LifetimeParameterIdentity>,
}

/// The complete derived roster for this immutable checking input. This is raw
/// declaration correspondence, never a checked receipt or body summary. A bare
/// mutable typed program has no memo; context changes reset this association.
pub fn provider_application_associations(
    program: &dyn crate::CheckingInput,
) -> std::sync::Arc<Vec<ProviderApplicationAssociation>> {
    let derive = || std::sync::Arc::new(derive_provider_application_associations(program));
    match crate::frozen_program::frozen_program_memos(program) {
        Some(memos) => memos.provider_applications.get_or_init(derive).clone(),
        None => derive(),
    }
}

/// Complete candidate coverage for a closed public invocation. Missing source
/// suppliers remain unavailable; an association subset cannot stand for all
/// implementations. A fully fixed, provably different outer scope is unrelated.
pub(crate) fn closed_requirement_provider_associations(
    program: &dyn crate::CheckingInput,
    requirement: SymbolHandle,
) -> Option<Vec<ProviderApplicationAssociation>> {
    let application = program.closed_requirement_application(requirement)?;
    let all = provider_application_associations(program);
    let rows = all
        .iter()
        .filter(|row| row.identity.closed_requirement == requirement)
        .cloned()
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return None;
    }
    for implementation in program.conformances() {
        let declares_row = implementation.rows.iter().any(|row| {
            row.declaring_trait == application.declaring_trait
                && row.requirement == application.template
        });
        // A partial map of this same trait is a missing alternative, not
        // permission to infer coverage from the healthy maps alone.
        if !trait_reaches(
            program,
            implementation.trait_symbol,
            application.declaring_trait,
        )? {
            if declares_row {
                return None;
            }
            continue;
        }
        if rows
            .iter()
            .any(|row| row.identity.selecting_declaration == implementation.symbol)
        {
            continue;
        }
        if !program
            .conformance_type_parameters(implementation)
            .is_empty()
            || !implementation.lifetime_parameters.is_empty()
        {
            return None;
        }
        let scope = AppliedTraitScope::from_conformance(program.typed_program(), implementation)?;
        let scope = scope.declaring_scope(program.typed_program(), application.declaring_trait)?;
        if scope.application_identity(program.typed_program())?
            == application.trait_application_identity
        {
            return None;
        }
    }
    for machine in program.machines() {
        if program
            .machine_specializations
            .iter()
            .any(|row| row.instance == machine.symbol)
            || program
                .captured_machine_applications
                .iter()
                .any(|(_, row)| row.instance == machine.symbol)
            || machine.generic_data_template.is_valid()
        {
            continue;
        }
        for conformance in program.machine_trait_conformances(machine) {
            let Some(typed_trees::machine::SatisfiedDeclaration::Trait {
                requirement: declared,
                ..
            }) = typed_trees::machine::resolve_satisfied_declaration(program, machine, conformance)
            else {
                continue;
            };
            if declared.symbol != application.template {
                continue;
            }
            let mut covered = false;
            for row in &rows {
                if row.identity.selecting_declaration == row.identity.provider
                    && program
                        .machine_invokes_declaration(row.identity.provider, machine.symbol)
                        .ok()?
                {
                    covered = true;
                }
            }
            if covered {
                continue;
            }
            if machine.has_specialization_parameters() || !machine.lifetime_parameters.is_empty() {
                return None;
            }
            let scope = AppliedTraitScope::from_machine_conformance(
                program.typed_program(),
                machine,
                conformance,
            )?;
            let scope =
                scope.declaring_scope(program.typed_program(), application.declaring_trait)?;
            if scope.application_identity(program.typed_program())?
                == application.trait_application_identity
            {
                return None;
            }
        }
    }
    Some(rows)
}

/// Scope projection can be absent or ambiguous. Only this complete nominal
/// walk can establish that a map is unrelated; unknown declarations refuse.
fn trait_reaches(
    program: &dyn crate::CheckingInput,
    root: SymbolHandle,
    wanted: SymbolHandle,
) -> Option<bool> {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(symbol) = pending.pop() {
        if symbol == wanted {
            return Some(true);
        }
        if visited.contains(&symbol) {
            continue;
        }
        let declaration = program.trait_definition_by_symbol(symbol)?;
        visited.push(symbol);
        pending.extend(
            program
                .trait_requirements(declaration)
                .iter()
                .chain(declaration.refines.iter())
                .map(|parent| parent.symbol),
        );
    }
    Some(false)
}

fn derive_provider_application_associations(
    program: &dyn crate::CheckingInput,
) -> Vec<ProviderApplicationAssociation> {
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
                            ProviderApplicationIdentity {
                                provider: provider.symbol,
                                selecting_declaration: provider.symbol,
                                template: application.template,
                                closed_requirement: application.signature.symbol,
                                receiving_application: application.trait_application_identity,
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
                crate::resolve_closed_requirement_application(program, provider, template)
            {
                rows.push((
                    ProviderApplicationIdentity {
                        provider: provider.symbol,
                        selecting_declaration: provider.symbol,
                        template: template.symbol,
                        closed_requirement: closed.symbol,
                        receiving_application: Default::default(),
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
                implementation.rows.iter().any(|row| {
                    row.declaring_trait == application.declaring_trait
                        && row.requirement == application.template
                        && program.requirement_realization_matches(
                            application.signature.symbol,
                            row.realization_machine,
                            provider.symbol,
                        )
                })
            }) {
                let captures = implementation
                    .rows
                    .iter()
                    .filter(|row| {
                        row.requirement == application.template
                            && row.declaring_trait == application.declaring_trait
                    })
                    .filter_map(|row| {
                        program.typed_program().implementation_capture(
                            provider.symbol,
                            row.realization_machine,
                            implementation.symbol,
                        )
                    })
                    .collect::<Vec<_>>();
                let correspondence = match captures.as_slice() {
                    [] => crate::provider_region_correspondence(
                        program,
                        implementation,
                        &receiving_scope,
                    ),
                    [capture] => crate::closed_provider_region_correspondence(
                        program,
                        implementation,
                        &receiving_scope,
                        capture,
                    ),
                    _ => None,
                };
                let Some(correspondence) = correspondence else {
                    continue;
                };
                rows.push((
                    ProviderApplicationIdentity {
                        provider: provider.symbol,
                        selecting_declaration: implementation.symbol,
                        template: application.template,
                        closed_requirement: application.signature.symbol,
                        receiving_application: application.trait_application_identity,
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
    rows.into_iter()
        .map(
            |(identity, receiving_regions)| ProviderApplicationAssociation {
                identity,
                receiving_regions,
            },
        )
        .collect()
}

/// The exact specialization receipt, not a generated-name suffix, owns the
/// concrete body's declaration. An ambiguous receipt supplies no template.
fn provider_template(
    program: &dyn crate::CheckingInput,
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
