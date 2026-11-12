/*
We select exact provider and target-machine members from Psi's retained
declaration inventory without mutating the shared source program.

Psi owns product-family formation and retains every authored target body. Only
`Build` source selects an execution target before resolution; neither route
carries a parallel inventory joined back by names or source spans.
`AuthoredTargetProviders` closes the target/default/candidate relationship
before portable publication so `PreparedProductProviders` and
`TargetProviderPreparationRequest` can attach Calling realizations later.
*/

/*
A family implemented only by other targets keeps its declaration as an inert
sibling with its callers so the call resolves and Psi checking accepts every
body whether or not a realization reaches it (`wiki/spec/language/machines.md`,
target-scoped families). `reject_reached_inert_sibling_calls` refuses calls the
realized product actually reaches starting from the selected `ProgramEntry`
root across explicit calls, named transitions, and selected provider rows.
It never treats a portable family head as an executable body and never enters
an inert body, so callers scoped to a sibling's own target go inert with it.
*/

use crate::{DerivedProviderPlan, ProviderSelection, SettledTargetMachineDeclarations};
use diagnostics::Diagnostic;
use language_semantics::direct_calls::DirectCallDeclarations;
use symbols::SymbolHandle;
use target::TargetProfile;

mod prepared_providers;
pub use prepared_providers::{
    PreparedProductProviders, ProductProviderPreparation, ProviderPublicationCatalog,
};
mod provider_publication;
pub(crate) use provider_publication::TargetProviderPreparationRequest;
pub use provider_publication::{PreparedTargetProviderPublication, provider_selection_roots};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthoredTargetProviders {
    target: Option<target::TargetProfile>,
    defaults: Vec<ProviderSelection>,
    candidates: Vec<DerivedProviderPlan>,
}

impl AuthoredTargetProviders {
    pub(crate) fn unevaluated(
        services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
        body_declarations: &DirectCallDeclarations<SymbolHandle>,
        defaults: &[ProviderSelection],
        target: target::TargetProfile,
    ) -> Result<Self, Vec<Diagnostic>> {
        let candidates =
            crate::derive_satisfies_plans(crate::ProviderPlanDerivation::unevaluated(
                services,
                body_declarations,
                &[],
                Some(target.target_name()),
            ))?;
        Ok(Self {
            target: Some(target),
            defaults: defaults.to_vec(),
            candidates,
        })
    }

    pub(crate) fn target(&self) -> Option<target::TargetProfile> {
        self.target
    }

    pub(crate) fn derive(
        services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
        body_declarations: &DirectCallDeclarations<SymbolHandle>,
        bindings: &crate::evaluated_via_bindings::EvaluatedViaBindingTable,
        target: Option<target::TargetProfile>,
        declarations: crate::SettledTargetMachineDeclarations,
        native_bindings: &[crate::DerivedProviderPlan],
    ) -> Result<Self, Vec<Diagnostic>> {
        let candidates = crate::derive_satisfies_plans(crate::ProviderPlanDerivation::evaluated(
            services,
            body_declarations,
            &[],
            target.map(target::TargetProfile::target_name),
            bindings,
            &declarations.origins,
            native_bindings,
        ))?;
        Ok(Self {
            target,
            defaults: declarations.provider_defaults,
            candidates,
        })
    }

    pub(crate) fn defaults(&self) -> &[ProviderSelection] {
        &self.defaults
    }

    pub(crate) fn candidates(&self) -> &[DerivedProviderPlan] {
        &self.candidates
    }

    pub(crate) fn into_candidates(self) -> Vec<DerivedProviderPlan> {
        self.candidates
    }
}

pub fn reject_reached_inert_sibling_calls(
    declarations: &DirectCallDeclarations<SymbolHandle>,
    target: TargetProfile,
    entry: Option<symbols::SymbolHandle>,
    selected_providers: &[crate::SelectedProviderReviewProvenance],
) -> Result<(), Vec<Diagnostic>> {
    let provider_rows = selected_providers
        .iter()
        .flat_map(|selected| {
            selected
                .provider
                .row_requirements
                .iter()
                .copied()
                .zip(&selected.provider.row_realizations)
                .filter_map(|(required, owner)| owner.authored().map(|machine| (required, machine)))
        })
        .collect::<Vec<_>>();
    provider_selection::reject_reached_inert_calls(
        declarations,
        target.target_name(),
        entry.as_ref(),
        &provider_rows,
    )
}

pub(crate) fn realized_row_machine<'program>(
    context: crate::ProviderPlanningContext<'program>,
    named: symbols::SymbolHandle,
    selected_target: Option<&str>,
) -> Option<&'program checked_trees::typed_trees::machine::Machine> {
    let typed = context.typed;
    let machine = typed.machine_by_symbol(named)?;
    let realization = if machine.target_family_interface {
        let target = selected_target
            .filter(|target| !target.is_empty())
            .unwrap_or_else(|| TargetProfile::host().target_name());
        typed.target_family_member(named, target)?
    } else {
        machine
    };
    realizes_machine(context, realization, selected_target).then_some(realization)
}

/// Whether `machine` is a body this scope and target actually realize: in
/// scope, not a target-family interface, and authored for `target` (or for no
/// target in a Build program).
pub fn realizes_machine(
    context: crate::ProviderPlanningContext<'_>,
    machine: &checked_trees::typed_trees::machine::Machine,
    target: Option<&str>,
) -> bool {
    if machine.target_family_interface || !context.contains(machine.symbol) {
        return false;
    }
    if context.scope == source::DependencyScope::Build {
        return machine.target.is_none();
    }
    let target = target
        .filter(|target| !target.is_empty())
        .unwrap_or_else(|| TargetProfile::host().target_name());
    context
        .typed
        .source_target_context(machine)
        .is_none_or(|authored| authored == target)
}

pub fn settle_provider_defaults(
    origins: &[crate::SelectedTargetMachineOrigin],
    retained: &[crate::TargetProviderDefaults],
    target: TargetProfile,
) -> SettledTargetMachineDeclarations {
    let matches_target = |name: &str| {
        TargetProfile::from_canonical_target_name(name).is_ok_and(|resolved| resolved == target)
    };
    let provider_defaults = retained
        .iter()
        .filter(|row| row.target.as_ref().is_none_or(|name| matches_target(name)))
        .flat_map(|row| row.selections.iter().cloned())
        .collect();
    let origins = origins
        .iter()
        .filter(|origin| matches_target(&origin.target))
        .cloned()
        .collect();
    SettledTargetMachineDeclarations {
        provider_defaults,
        origins,
    }
}
