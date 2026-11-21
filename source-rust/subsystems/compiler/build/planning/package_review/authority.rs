//! Highlight the critical services of the selected Product, including capability-only traits.
use super::{input::PolicySource, semantic_bindings::checked_service};
use diagnostics::Diagnostic;
use language_core::core_vocabulary::CoreVocabulary;
use provider_acceptance::AcceptedSemanticBindingRole as Role;
use review_evidence::record::{
    PackageReviewDangerousAuthorityClass as Class, PackageReviewNominalIdentity,
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
) -> Result<Vec<(PackageReviewNominalIdentity, Class)>, Vec<Diagnostic>> {
    let mut classes = Vec::new();
    for declaration in input.typed().traits().iter().filter(|d| d.is_boundary) {
        if let Some(class) = input
            .typed()
            .core_vocabulary(declaration.symbol)
            .and_then(critical_class)
        {
            classes.push((input.nominal(declaration.symbol)?, class));
        }
    }
    for binding in input.sources().accepted_semantic_bindings() {
        let class = match binding.role() {
            Role::ConsoleIo | Role::ProcessExitExitProcessI32 => Class::Process,
            Role::FilesystemHostService => Class::Filesystem,
            _ => continue,
        };
        let (symbol, _) = checked_service(input, selected, binding)?;
        classes.push((input.nominal(symbol)?, class));
    }
    classes.sort();
    classes.dedup();
    Ok(classes)
}
