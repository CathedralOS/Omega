//! Compare source-selected callable coordinates with stage 03's admitted rows.
//! Only the source preparation reader needs declarations or frozen trees.

use super::declarations::unavailable;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ServiceSchema};
use semantic_vocabulary::{BoundaryMachineId, MachineId, PackageKeyIdentity};
use source_compiler::provider_supply::ClosedLogicalProviderReach;
use terminal_psi::TerminalModule;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct CandidateRow {
    pub requirement: String,
    machine: String,
    package: Option<PackageKeyIdentity>,
    pub candidates: Vec<(BoundaryMachineId, MachineId)>,
}

#[derive(Clone)]
pub(super) struct ProviderChoice {
    pub package: Option<PackageKeyIdentity>,
    pub provider: String,
    pub target: String,
    pub schema: ServiceSchema,
    pub rows: Vec<CandidateRow>,
    pub method_count: usize,
}

impl ProviderChoice {
    pub fn key(
        &self,
    ) -> (
        Option<PackageKeyIdentity>,
        &str,
        Option<PackageKeyIdentity>,
        &str,
    ) {
        (
            self.package,
            &self.provider,
            self.schema.declaration.trait_package_identity,
            self.schema
                .identity_text(self.schema.declaration.trait_name),
        )
    }

    pub fn same_declarations(&self, actual: &Self) -> bool {
        self.key() == actual.key()
            && self.target == actual.target
            && self.schema.declaration == actual.schema.declaration
            && self.rows == actual.rows
            && self.method_count == actual.method_count
    }
}

pub(super) fn from_source(
    closed: &ClosedLogicalProviderReach,
    module: &TerminalModule,
) -> Result<Vec<ProviderChoice>, Vec<Diagnostic>> {
    closed
        .selection()
        .providers()
        .iter()
        .map(|provider| {
            let mut rows = provider
                .rows
                .iter()
                .map(|row| {
                    let candidates = row
                        .candidate_positions
                        .iter()
                        .map(|&position| {
                            module
                                .provider_candidates
                                .get(position)
                                .map(|candidate| (candidate.boundary, candidate.candidate))
                                .ok_or_else(|| {
                                    unavailable("the accepted service requirement contract")
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok::<_, Vec<Diagnostic>>(CandidateRow {
                        requirement: row.requirement.clone(),
                        machine: row.machine.clone(),
                        package: row.machine_package,
                        candidates,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            rows.sort_by(|a, b| a.requirement.cmp(&b.requirement));
            Ok(ProviderChoice {
                package: provider.origin_package,
                provider: module
                    .identities
                    .text(provider.choice.provider_identity)
                    .ok_or_else(|| unavailable("the selected provider's local source declaration"))?
                    .to_owned(),
                target: provider.choice.target.clone(),
                schema: ServiceSchema::from_declaration(provider.schema.surface.clone()),
                method_count: rows.len(),
                rows,
            })
        })
        .collect()
}

pub(super) fn from_selected(
    plans: &[ProviderPlan],
    module: &TerminalModule,
) -> Result<Vec<ProviderChoice>, Vec<Diagnostic>> {
    let authenticated = backend_plan::selected_checked_invocation_rows(plans, module)
        .map_err(|reason| unavailable(&reason))?;
    let mut remaining = authenticated.as_slice();
    let mut selections = Vec::with_capacity(plans.len());
    for plan in plans {
        let count = remaining
            .iter()
            .take_while(|row| std::ptr::eq(row.plan, plan))
            .count();
        let (selected, rest) = remaining.split_at(count);
        remaining = rest;
        let mut rows = Vec::with_capacity(selected.len());
        for selected in selected {
            let row = selected.row;
            let ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } = row.binding
            else {
                return Err(unavailable("the selected provider's checked realization"));
            };
            rows.push(CandidateRow {
                requirement: plan
                    .identity_text(row.declared_requirement_identity)
                    .to_owned(),
                machine: plan.identity_text(machine_identity).to_owned(),
                package: machine_package_identity,
                candidates: selected
                    .candidates
                    .iter()
                    .map(|c| (c.conformance.boundary, c.conformance.candidate))
                    .collect(),
            });
        }
        rows.sort_by(|a, b| a.requirement.cmp(&b.requirement));
        selections.push(ProviderChoice {
            package: plan.origin_package_identity,
            provider: plan.provider_type.clone(),
            target: plan.target.clone(),
            schema: plan.schema.clone(),
            rows,
            method_count: plan.rows.len(),
        });
    }
    Ok(selections)
}

pub(super) fn from_review(
    checked: &products::CheckedCompilation,
) -> Result<Vec<ProviderChoice>, Vec<Diagnostic>> {
    let plans = checked.execution().plans().plans();
    let checked_rows = match checked.execution().package_review() {
        Some(review) => review
            .provider_candidates
            .as_ref()
            .map_err(Clone::clone)?
            .as_slice(),
        None if plans.is_empty() => &[],
        None => return Err(unavailable("the package's target review plan")),
    };
    let mut checked_rows = checked_rows.iter();
    let mut choices = Vec::with_capacity(plans.len());
    for plan in plans {
        let mut rows = Vec::new();
        for row in &plan.rows {
            let ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } = row.binding
            else {
                continue;
            };
            let canonical = plan.identity_text(row.declared_requirement_identity);
            let reviewed = checked_rows
                .next()
                .filter(|reviewed| reviewed.plan == plan.name && reviewed.requirement == canonical)
                .ok_or_else(|| unavailable("one selected row for the exact service method"))?;
            rows.push(CandidateRow {
                requirement: canonical.to_owned(),
                machine: plan.identity_text(machine_identity).to_owned(),
                package: machine_package_identity,
                candidates: reviewed.candidates.clone(),
            });
        }
        rows.sort_by(|a, b| a.requirement.cmp(&b.requirement));
        choices.push(ProviderChoice {
            package: plan.origin_package_identity,
            provider: plan.provider_type.clone(),
            target: plan.target.clone(),
            schema: plan.schema.clone(),
            rows,
            method_count: plan.rows.len(),
        });
    }
    if checked_rows.next().is_some() {
        return Err(unavailable("one selected row for the exact service method"));
    }
    Ok(choices)
}
