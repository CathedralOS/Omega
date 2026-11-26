//! Highlight the critical services of the selected Product, including capability-only traits.
use super::{input::PolicySource, semantic_bindings::checked_service};
use diagnostics::Diagnostic;
use language_core::core_vocabulary::CoreVocabulary;
use provider_acceptance::AcceptedSemanticBindingRole as Role;
use review_evidence::record::{
    PackageReviewDangerousAuthorityClass as Class, PackageReviewServiceIdentity,
};

/// Core vocabulary lookup authenticates the toolchain declaration's full source bytes.
fn critical_class(vocabulary: CoreVocabulary) -> Option<Class> {
    match vocabulary {
        CoreVocabulary::NetworkService => Some(Class::Network),
        CoreVocabulary::FilesystemService(_) => Some(Class::Filesystem),
        CoreVocabulary::StandardInputService
        | CoreVocabulary::StandardOutputService
        | CoreVocabulary::ProcessExitService => Some(Class::Process),
        CoreVocabulary::MachineControlService => Some(Class::MachineControl),
        CoreVocabulary::PortIoService => Some(Class::PortIo),
        CoreVocabulary::InterruptControlService => Some(Class::InterruptControl),
        CoreVocabulary::InterruptEntryService => Some(Class::InterruptEntry),
        CoreVocabulary::RootMemoryAccessService => Some(Class::RootMemory),
        _ => None,
    }
}

pub(super) fn critical_services(
    input: &PolicySource<'_>,
    selected: &compiler::SelectedProviderResult,
    policy: &review_evidence::record::PackagePolicyBaseline,
) -> Result<Vec<(PackageReviewServiceIdentity, Class)>, Vec<Diagnostic>> {
    let mut classes = Vec::new();
    let source = input.typed();
    let mut classified_declarations = std::collections::BTreeMap::new();
    for declaration in source.traits().iter().filter(|d| d.is_boundary) {
        if let Some(class) = source
            .core_vocabulary(declaration.symbol)
            .and_then(critical_class)
        {
            classified_declarations.insert(input.nominal(declaration.symbol)?, class);
        }
    }
    for service in policy
        .callables
        .callables
        .iter()
        .flat_map(|row| row.exposed_services())
    {
        if let Some(&class) = classified_declarations.get(&service.declaration) {
            classes.push((service.clone(), class));
        }
    }
    // Console binds a physical provider. Its requirements declare the core
    // standard-stream services classified above.
    for binding in input.sources().accepted_semantic_bindings() {
        let class = match binding.role() {
            Role::ProcessExitExitProcessI32 => Class::Process,
            Role::FilesystemHostService => Class::Filesystem,
            _ => continue,
        };
        let (symbol, schema) = checked_service(input, selected, binding)?;
        classes.push((
            input.effect_identity(&input.schema_effect_scope(symbol, &schema)?)?,
            class,
        ));
    }
    classes.sort();
    classes.dedup();
    Ok(classes)
}
