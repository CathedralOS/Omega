use crate::builtins::SelectedCompilerBuiltin;
use std::collections::BTreeSet;

/// Provider rows selected for the reachable boundaries of one native entry.
pub struct ScopedProviderBindings {
    pub external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    pub demands: BTreeSet<String>,
    pub compiler_builtins: Vec<SelectedCompilerBuiltin>,
    pub boundary_application_coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderAdapter {
    pub provider_declaration: String, // Selecting declaration, independent of the realizing callable.
    pub provider_plan_digest: semantic_vocabulary::provider_identity::ProviderPlanDigest, // Full originating plan identity; report fingerprints cannot select it.
    pub selected_requirement_identity: String, // Original row before specialization or target-family aliasing.
    pub requirement_identity: String,
    pub provider_identity: String,
    pub machine_identity: String,
}

/// Adapter rows derived from one selected provider closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderAdapters {
    selection: build_providers::selections::SelectedProviderClosureDigest,
    rows: std::sync::Arc<[SelectedProviderAdapter]>,
}

impl SelectedProviderAdapters {
    pub fn from_selected_plans(
        selected: &build_providers::selections::SelectedProviderPlanFacts,
    ) -> Result<Self, String> {
        Ok(Self {
            rows: retain_selected_provider_adapters(selected)?.into(),
            selection: selected.identity_digest(),
        })
    }

    /// An independent requirement enters its installed supplier through an import.
    /// Keep the complete selection identity while excluding its local body from reach.
    pub fn without_imports(
        mut self,
        imported: &[semantic_vocabulary::provider_identity::ProviderPlanDigest],
    ) -> Self {
        if !imported.is_empty() {
            self.rows = self
                .rows
                .iter()
                .filter(|row| !imported.contains(&row.provider_plan_digest))
                .cloned()
                .collect();
        }
        self
    }

    pub fn require_selection(
        &self,
        selected: &build_providers::selections::SelectedProviderPlanFacts,
    ) -> Result<(), &'static str> {
        if self.selection != selected.identity_digest() {
            return Err("physical adapter rows belong to another provider selection");
        }
        Ok(())
    }

    pub(crate) const fn selection_digest(
        &self,
    ) -> build_providers::selections::SelectedProviderClosureDigest {
        self.selection
    }

    pub fn rows(&self) -> &[SelectedProviderAdapter] {
        &self.rows
    }
}

pub fn retain_selected_provider_adapters(
    selected: &build_providers::selections::SelectedProviderPlanFacts,
) -> Result<Vec<SelectedProviderAdapter>, String> {
    let mut adapters = Vec::new();
    for plan in selected.plans() {
        let mut owner = None;
        for row in &plan.rows {
            let build_providers::plans::ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } = &row.binding
            else {
                continue;
            };
            if plan.provider_type.is_empty()
                || plan.identities.text(row.requirement_identity).is_empty()
                || plan.identity_text(*machine_identity).is_empty()
                || *machine_package_identity != plan.origin_package_identity
            {
                return Err(format!(
                    "selected ProviderPlan `{}` has incomplete checked-adapter custody",
                    plan.name
                ));
            }
            let owner = match owner {
                Some(owner) => owner,
                None => {
                    let declared = plan
                        .provider_declaration
                        .as_deref()
                        .filter(|owner| !owner.is_empty())
                        .ok_or("native checked provider plan has no exact selecting declaration")?;
                    owner = Some(declared);
                    declared
                }
            };
            for requirement in selected.selected_row_requirement_identities(
                plan.report_fingerprint(),
                plan.identities.text(row.requirement_identity),
            ) {
                let machine = selected
                    .specialized_requirement_row(requirement)
                    .map_or_else(
                        || plan.identity_text(*machine_identity),
                        |specialization| {
                            selected
                                .identities()
                                .text(specialization.machine_identity)
                                .unwrap_or_default()
                        },
                    );
                if machine.is_empty() {
                    return Err("selected checked realization has an empty machine identity".into());
                }
                adapters.push(SelectedProviderAdapter {
                    provider_declaration: owner.to_owned(),
                    provider_plan_digest: plan.identity_digest(),
                    selected_requirement_identity: plan
                        .identities
                        .text(row.requirement_identity)
                        .to_owned(),
                    requirement_identity: requirement.to_owned(),
                    provider_identity: plan.provider_type.clone(),
                    machine_identity: machine.to_owned(),
                });
            }
        }
    }
    adapters.sort_by(|left, right| {
        left.requirement_identity
            .cmp(&right.requirement_identity)
            .then_with(|| {
                left.provider_plan_digest
                    .as_bytes()
                    .cmp(right.provider_plan_digest.as_bytes())
            })
    });
    if let Some(duplicate) = adapters.windows(2).find(|rows| {
        rows[0].requirement_identity == rows[1].requirement_identity
            && rows[0].provider_plan_digest == rows[1].provider_plan_digest
    }) {
        return Err(format!(
            "one selected whole provider plan projects repeated checked adapters for exact requirement `{}`",
            duplicate[0].requirement_identity
        ));
    }
    Ok(adapters)
}
