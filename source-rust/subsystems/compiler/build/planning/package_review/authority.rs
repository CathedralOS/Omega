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
        CoreVocabulary::FilesystemService(_) => Some(Class::Filesystem),
        CoreVocabulary::ProcessInputService
        | CoreVocabulary::ProcessOutputService
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
) -> Result<Vec<(PackageReviewServiceIdentity, Class)>, Vec<Diagnostic>> {
    let mut classes = Vec::new();
    let source = input.typed();
    let mut applied = std::collections::BTreeMap::new();
    for declaration in source.traits().iter().filter(|d| d.is_boundary) {
        let Some(class) = source.core_vocabulary(declaration.symbol).and_then(critical_class) else { continue };
        if declaration.type_parameters.is_empty() && declaration.lifetime_parameters.is_empty() {
            classes.push((input.unapplied_service(declaration.symbol)?, class));
        } else {
            applied.insert(super::providers::symbol_key(declaration.symbol), class);
        }
    }
    if !applied.is_empty() {
        for root in source.applied_trait_roots() {
            let mut members = Vec::new();
            root.try_visit(source, &mut |scope| {
                if let Some(&class) = applied.get(&super::providers::symbol_key(scope.declaration.symbol)) {
                    members.push((scope.clone(), class));
                }
                Ok(())
            }).map_err(super::declarations::unavailable)?;
            for (scope, class) in members { classes.push((input.effect_identity(&scope)?, class)); }
        }
    }
    for binding in input.sources().accepted_semantic_bindings() {
        let class = match binding.role() {
            Role::ConsoleIo | Role::ProcessExitExitProcessI32 => Class::Process,
            Role::FilesystemHostService => Class::Filesystem,
            _ => continue,
        };
        let (symbol, schema) = checked_service(input, selected, binding)?;
        classes.push((input.effect_identity(&input.schema_effect_scope(symbol, &schema)?)?, class));
    }
    classes.sort();
    classes.dedup();
    Ok(classes)
}
