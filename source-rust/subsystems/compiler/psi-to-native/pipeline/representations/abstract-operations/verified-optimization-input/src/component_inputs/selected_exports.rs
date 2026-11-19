//! Join a selected native provider plan to exports independently verified from a component.
use component_description::VerifiedComponent;
use effects::provider_plan::{ProviderBinding, ProviderPlan};

pub(super) fn selected_plan_export_positions(
    component: &VerifiedComponent,
    plan: &ProviderPlan,
) -> Result<Vec<usize>, IndependentRealizationMismatch> {
    if plan.provider_type.is_empty() {
        return Err(IndependentRealizationMismatch::EmptyProviderType {
            plan: plan.name.clone(),
        });
    }
    let coverage = plan.validate_against_schema();
    if plan.rows.is_empty() || !coverage.is_empty() {
        return Err(IndependentRealizationMismatch::InvalidPlan {
            plan: plan.name.clone(),
            detail: coverage.join("; "),
        });
    }
    for row in &plan.rows {
        if !matches!(row.binding, ProviderBinding::CheckedAdapter { .. }) {
            return Err(IndependentRealizationMismatch::UncheckedRow {
                requirement_identity: plan.identities.text(row.requirement_identity).to_owned(),
            });
        }
    }
    let rows = backend_plan::selected_provider_candidate_rows(plan, component.module()).map_err(
        |detail| IndependentRealizationMismatch::InvalidPlan {
            plan: plan.name.clone(),
            detail,
        },
    )?;
    let mut exports = Vec::new();
    for projected in rows {
        if projected.candidate_positions.is_empty() {
            return Err(IndependentRealizationMismatch::MissingRealization {
                requirement_identity: plan
                    .identity_text(projected.row.requirement_identity)
                    .to_owned(),
            });
        }
        for position in projected.candidate_positions {
            let candidate = &component.module().provider_candidates[position];
            let requirement = component
                .module()
                .identities
                .text(candidate.requirement_identity)
                .unwrap_or_default();
            if !component.exports_candidate_at(position) {
                return Err(IndependentRealizationMismatch::MissingExport {
                    requirement_identity: requirement.to_owned(),
                });
            }
            exports.push(position);
        }
    }
    let unresolved: Vec<String> = component
        .module()
        .root_service_reach
        .installation_dependencies
        .iter()
        .map(|dependency| {
            component
                .module()
                .identities
                .text(dependency.requirement_identity)
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    if !unresolved.is_empty() {
        return Err(IndependentRealizationMismatch::UnresolvedInstallationRows {
            requirement_identities: unresolved,
        });
    }
    Ok(exports)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum IndependentRealizationMismatch {
    EmptyProviderType { plan: String },
    InvalidPlan { plan: String, detail: String },
    UncheckedRow { requirement_identity: String },
    MissingRealization { requirement_identity: String },
    MissingExport { requirement_identity: String },
    UnresolvedInstallationRows { requirement_identities: Vec<String> },
}

impl std::fmt::Display for IndependentRealizationMismatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyProviderType { plan } => {
                write!(formatter, "selected plan `{plan}` names no provider type")
            }
            Self::InvalidPlan { plan, detail } => {
                write!(
                    formatter,
                    "selected plan `{plan}` is not a valid selection: {detail}"
                )
            }
            Self::UncheckedRow {
                requirement_identity,
            } => write!(
                formatter,
                "requirement `{requirement_identity}` is not realized by a checked adapter"
            ),
            Self::MissingRealization {
                requirement_identity,
            } => write!(
                formatter,
                "verified component realizes no candidate for `{requirement_identity}`"
            ),
            Self::MissingExport {
                requirement_identity,
            } => write!(
                formatter,
                "description exports no realization row for `{requirement_identity}`"
            ),
            Self::UnresolvedInstallationRows {
                requirement_identities,
            } => write!(
                formatter,
                "verified component retains {} unresolved installation-bound requirement row(s) ({}); its service bounds are obligations installation still owes, not resolved reach",
                requirement_identities.len(),
                requirement_identities.join(", ")
            ),
        }
    }
}

impl std::error::Error for IndependentRealizationMismatch {}
