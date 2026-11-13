// A retained target publication catalog contains only already-formed choices.
// Source-based candidate preparation/admission has been removed.
use diagnostics::Diagnostic;
use provider_selection::{
    CanonicalProviderDeclaration, PreparedTargetChoices, TargetProviderChoice,
};
use std::sync::Arc;
type ApplicationRoots = Result<Option<Vec<CanonicalProviderDeclaration>>, Vec<Diagnostic>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPublicationCatalog {
    base_targets: Vec<String>,
    deployed: bool,
    rows: Vec<PublicationRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PublicationRow {
    target: String,
    roots: ApplicationRoots,
    choices: Result<PreparedTargetChoices, Vec<Diagnostic>>,
}

impl ProviderPublicationCatalog {
    pub fn prepare(
        &self,
        requested_scopes: &[String],
        selected: Option<&Arc<provider_selection::SelectedTargetProviderPublication>>,
        assumptions: &[effects::AssumedBindingContractSelection],
        components: &[Arc<component_description::VerifiedComponent>],
    ) -> provider_selection::PreparedTargetProviderPublication {
        let mut targets = self.base_targets.clone();
        targets.extend(requested_scopes.iter().cloned());
        targets.sort();
        targets.dedup();
        let targets = targets
            .into_iter()
            .map(|target| {
                let choice = match self.rows.iter().find(|row| row.target == target) {
                    Some(row) => {
                        if let Some(selected) = selected
                            && selected.profile.target_name() == target
                        {
                            row.roots.clone().map(|application_roots| {
                                TargetProviderChoice::Selected {
                                    selected: Arc::clone(selected),
                                    application_roots,
                                }
                            })
                        } else {
                            row.choices.clone().map(|mut choices| {
                                choices.independent_components = components.to_vec();
                                TargetProviderChoice::Prepared(choices)
                            })
                        }
                    }
                    None => Err(vec![Diagnostic::error(format!(
                        "provider proposal has no receiving target catalog for `{target}`"
                    ))]),
                };
                (target, choice)
            })
            .collect();
        provider_selection::PreparedTargetProviderPublication::new(
            targets,
            assumptions.to_vec(),
            self.deployed,
        )
    }
}
