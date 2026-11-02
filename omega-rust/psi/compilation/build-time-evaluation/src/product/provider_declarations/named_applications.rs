//! Exact named-provider applications before ABI or whole-map selection.
//! Captured members remain joined to their complete source application; a
//! template or diagnostic label never supplies a missing static telescope.

use super::ProviderPlanningContext;
use typed_trees::typed_trees::CapturedMachineApplication;
use typed_trees::{
    TypedTrees,
    trait_definition::{AppliedTraitScope, Conformance},
};

pub struct NamedProviderApplication<'program> {
    pub declaration: &'program Conformance,
    pub scope: AppliedTraitScope<'program>,
    pub capture: arena::Handle<CapturedMachineApplication>,
    pub identity: Option<String>,
}

pub fn prepared_named_provider_applications(
    context: ProviderPlanningContext<'_>,
) -> Vec<NamedProviderApplication<'_>> {
    let program = context.typed;
    let mut applications = Vec::new();
    for declaration in program
        .conformances()
        .iter()
        .filter(|declaration| context.contains(declaration.symbol) && super::eligible(declaration))
    {
        if let Some(scope) = AppliedTraitScope::from_conformance(program, declaration) {
            applications.push(NamedProviderApplication {
                declaration,
                scope,
                capture: arena::Handle::default(),
                identity: None,
            });
        }
    }
    // Capture receipts arise from actual prepared conformance applications.
    // Unexecuted Product recipes never seed captures or executable candidates.
    for (capture_handle, capture) in program.captured_machine_applications.iter() {
        let Some(declaration) = program
            .conformance_by_symbol(capture.conformance_application.declaration)
            .filter(|declaration| context.contains(declaration.symbol))
        else {
            continue;
        };
        let Some(identity) = program
            .portable_conformance_application_identity(
                &capture.conformance_application,
                capture.supplying_application,
            )
            .and_then(|identity| program.type_identity_text(identity))
        else {
            continue;
        };
        if applications.iter().any(|existing| {
            existing.declaration.symbol == declaration.symbol
                && existing.identity.as_ref() == Some(&identity)
        }) {
            continue;
        }
        let Some(scope) = capture.conformance_application.trait_scope.scope(program) else {
            continue;
        };
        applications.push(NamedProviderApplication {
            declaration,
            scope,
            capture: capture_handle,
            identity: Some(identity),
        });
    }
    applications
}

pub fn prepared_named_provider_member(
    program: &TypedTrees,
    application: &NamedProviderApplication<'_>,
    template: symbols::SymbolHandle,
) -> Option<symbols::SymbolHandle> {
    let identity = application.identity.as_deref()?;
    let mut matching = program
        .captured_machine_applications
        .iter()
        .filter(|(_, capture)| {
            capture.template == template
                && capture.conformance_application.declaration == application.declaration.symbol
                && program
                    .portable_conformance_application_identity(
                        &capture.conformance_application,
                        capture.supplying_application,
                    )
                    .and_then(|identity| program.type_identity_text(identity))
                    .as_deref()
                    == Some(identity)
        })
        .map(|(_, capture)| capture.instance);
    let selected = matching.next()?;
    matching.next().is_none().then_some(selected)
}
