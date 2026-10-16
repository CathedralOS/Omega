//! Carry checked provider choices without reconstructing their owner from code.
//!
//! Source custody qualifies whole plans before release. The selected closure
//! remains authoritative for specialization and family aliases; this projection
//! preserves those joins rather than rewriting selected rows. Native admission
//! selects its executable subset without rebuilding this producer's output. Neither a
//! display label nor sharing one realizing machine merges selecting declarations.

use crate::SelectedProviderAdapter;
use effects::provider_plan::{ProviderBinding, ProviderPlan};

/// Transient source-normalized owner beside its exact whole selected plan.
/// External/intrinsic-only plans need no checked selecting declaration.
pub struct NativeProviderPlanDeclaration<'input> {
    pub plan: &'input ProviderPlan,
    pub provider_declaration: Option<String>,
}

pub(crate) fn retain_selected_provider_adapters(
    selected: &effects::SelectedProviderPlanFacts,
    declarations: &[NativeProviderPlanDeclaration<'_>],
) -> Result<Vec<SelectedProviderAdapter>, String> {
    if selected.plans().len() != declarations.len()
        || selected.plans().iter().any(|plan| {
            declarations
                .iter()
                .filter(|declaration| declaration.plan == plan)
                .count()
                != 1
        })
    {
        return Err(
            "native provider declaration roster differs from the complete selected plans".into(),
        );
    }
    project_complete_adapters(selected, |plan| {
        let mut matches = declarations
            .iter()
            .filter(|declaration| declaration.plan == plan);
        let declaration = matches
            .next()
            .ok_or("native selected plan has no declaration custody")?;
        if matches.next().is_some() {
            return Err("native selected plan has competing declaration custody".into());
        }
        declaration
            .provider_declaration
            .clone()
            .filter(|owner| !owner.is_empty())
            .ok_or_else(|| "native checked provider plan has no exact selecting declaration".into())
    })
}

fn project_complete_adapters(
    selected: &effects::SelectedProviderPlanFacts,
    mut declaration: impl FnMut(&ProviderPlan) -> Result<String, String>,
) -> Result<Vec<SelectedProviderAdapter>, String> {
    let mut adapters = Vec::new();
    for plan in selected.plans() {
        let mut owner = None;
        for row in &plan.rows {
            let ProviderBinding::CheckedAdapter {
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
            let owner = match &owner {
                Some(owner) => owner,
                None => owner.insert(declaration(plan)?),
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
                    provider_declaration: owner.clone(),
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
    // A declaring requirement may occur beneath independent inherited calling
    // contracts. Its whole-plan custody remains the disambiguating coordinate;
    // executable scope and installation select exact candidates at real uses.
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
