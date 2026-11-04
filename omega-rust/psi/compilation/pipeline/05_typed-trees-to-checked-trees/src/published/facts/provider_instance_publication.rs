//! Freeze provider obligations and portable receiving coordinates at checking.
//!
//! A later executable closure may retain only some bodies. Its candidate list
//! cannot establish which closed instances the selected declaration still owes.
//! Unpublishable descriptions retain their reason until that interface is used.

use checked_trees::{
    CheckedProviderClosedRequirement, CheckedProviderInstanceDeclaration,
    CheckedProviderInstancePublication, CheckedProviderInstanceRequirement,
    CheckedProviderReceivingApplication, CheckedProviderReceivingDescription, CheckedTrees,
};
use symbols::SymbolHandle;
use typed_trees::type_identity::CallableIdentityId;

mod candidates;

pub(crate) fn retain(checked: &mut CheckedTrees) {
    let typed = &checked.typed;
    let mut publication = CheckedProviderInstancePublication::default();
    for association in checked.facts.provider_closed_requirements.iter() {
        let description = receiving(checked, &mut publication, association);
        let receiving = publication
            .receivers
            .insert(CheckedProviderReceivingApplication {
                association: *association,
                description,
            });
        if typed
            .trait_machine_signature(association.template)
            .is_none()
            || typed
                .closed_requirement_application(association.closed_requirement)
                .is_none()
        {
            continue;
        }
        let selecting_provider = match candidates::provider_declaration(typed, association) {
            Ok(Some(provider)) => Ok(provider),
            Ok(None) => continue,
            Err(reason) => Err(reason),
        };
        publication
            .requirements
            .insert(CheckedProviderInstanceRequirement {
                template: association.template,
                boundary: association.closed_requirement,
                provider: association.provider,
                declaration: selecting_provider
                    .and_then(|provider| named(checked, association, receiving, provider)),
            });
    }
    // Preserve the existing requirement-only obligations while the separate
    // top-level-requirement migration removes their remaining issuers.
    for template in typed.machines().iter().filter(|machine| {
        machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
            && !typed.machine_type_parameters(machine).is_empty()
    }) {
        for instance in typed
            .machine_specializations
            .iter()
            .filter(|receipt| receipt.template == template.symbol)
            .filter_map(|receipt| typed.machine(receipt.instance))
            .filter(|instance| {
                instance.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
            })
        {
            let requirement = typed.machine_callable_identity(instance);
            if !requirement.is_valid() {
                continue;
            }
            let template_identity = typed.machine_callable_identity(template);
            let declaration = if template_identity.is_valid() {
                Ok(CheckedProviderInstanceDeclaration {
                    template_identity,
                    package: typed.symbols.symbol_package_identity(template.symbol),
                    selecting_declaration: SymbolHandle::default(),
                    source_candidate_identity: CallableIdentityId::default(),
                    requirement_identity: requirement,
                    candidate_identity: CallableIdentityId::default(),
                    receiving: arena::Handle::default(),
                })
            } else {
                Err("generic provider requirement has no template callable identity".to_owned())
            };
            publication
                .requirements
                .insert(CheckedProviderInstanceRequirement {
                    template: template.symbol,
                    boundary: instance.symbol,
                    provider: SymbolHandle::default(),
                    declaration,
                });
        }
    }
    candidates::retain(checked, &mut publication);
    checked.facts.provider_instance_publication = publication;
}

fn named(
    checked: &CheckedTrees,
    association: &CheckedProviderClosedRequirement,
    receiving: arena::Handle<CheckedProviderReceivingApplication>,
    selecting_provider: SymbolHandle,
) -> Result<CheckedProviderInstanceDeclaration, String> {
    let typed = &checked.typed;
    let (owner, signature) = typed
        .trait_machine_signature(association.template)
        .ok_or("provider obligation lost its original signature")?;
    let application = typed
        .closed_requirement_application(association.closed_requirement)
        .ok_or("provider obligation lost its closed requirement application")?;
    let declaring = typed
        .trait_definition_by_symbol(application.declaring_trait)
        .ok_or("provider obligation lost its declaring trait")?;
    let requirement = typed.trait_requirement_callable_identity(declaring, application.signature);
    if !requirement.is_valid() {
        return Err("provider obligation has no closed requirement identity".into());
    }
    let provider = typed
        .machine(association.provider)
        .ok_or("provider obligation lost its checked realization")?;
    let mut receipts = typed
        .machine_specializations
        .iter()
        .filter(|receipt| receipt.instance == association.provider);
    let template = receipts
        .next()
        .map_or(association.provider, |receipt| receipt.template);
    if receipts.next().is_some() {
        return Err("provider callable has more than one specialization receipt".into());
    }
    let template = typed
        .machine(template)
        .ok_or("provider obligation lost its immediate template")?;
    let source_candidate_identity = typed.machine_callable_identity(template);
    if !source_candidate_identity.is_valid() {
        return Err("provider obligation has no realization template identity".into());
    }
    let candidate_identity = typed.machine_callable_identity(provider);
    if !candidate_identity.is_valid() {
        return Err("provider obligation has no closed realization identity".into());
    }
    Ok(CheckedProviderInstanceDeclaration {
        template_identity: typed.trait_requirement_callable_identity(owner, signature),
        package: typed.symbols.symbol_package_identity(association.template),
        selecting_declaration: selecting_provider,
        source_candidate_identity,
        requirement_identity: requirement,
        candidate_identity,
        receiving,
    })
}

fn receiving(
    checked: &CheckedTrees,
    publication: &mut CheckedProviderInstancePublication,
    association: &CheckedProviderClosedRequirement,
) -> Result<CheckedProviderReceivingDescription, String> {
    let typed = &checked.typed;
    let Some(application) = typed.closed_requirement_application(association.closed_requirement)
    else {
        return Ok(CheckedProviderReceivingDescription {
            application: String::new(),
            regions: arena::HandleSpan::empty(),
        });
    };
    if association.template != application.template
        || association.receiving_application != application.trait_application_identity
    {
        return Err("checked provider obligation has a different receiving application".into());
    }
    let regions = checked
        .facts
        .provider_closed_requirements
        .receiving_regions(association)
        .ok_or("provider candidate lost its checked receiving region span")?;
    let expected_regions = typed
        .conformance_by_symbol(association.selecting_declaration)
        .map_or(0, |implementation| implementation.lifetime_parameters.len());
    if regions.len() != expected_regions {
        return Err("provider candidate has an incomplete checked receiving region tuple".into());
    }
    let application = typed
        .type_identity_text(application.portable_trait_application_identity)
        .filter(|text| !text.is_empty())
        .ok_or("provider receiving application has no portable identity")?;
    let regions = regions
        .iter()
        .map(|region| {
            typed_trees::type_identity::portable_lifetime_parameter_identity(typed, *region)
                .ok_or_else(|| {
                    "provider receiving region has no portable declaration coordinate".to_owned()
                })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(CheckedProviderReceivingDescription {
        application,
        regions: publication.regions.insert_many(regions),
    })
}
