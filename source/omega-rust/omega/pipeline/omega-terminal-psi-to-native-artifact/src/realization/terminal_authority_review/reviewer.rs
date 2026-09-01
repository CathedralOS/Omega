//! Recursive selected-provider closure expansion and leaf admission.

use std::collections::{BTreeMap, BTreeSet};

use omega_effects::{provider_plan::ProviderBinding, TerminalAuthorityClosureLeaf};
use psi_core::BoundaryMachineId;

use super::context::ReviewContext;

type LeafKey = (
    omega_effects::provider_plan::ServiceSchemaDigest,
    String,
    omega_effects::provider_plan::ProviderPlanDigest,
);

pub(super) struct Reviewer<'a> {
    context: ReviewContext<'a>,
    active_requirements: BTreeSet<String>,
    leaves: BTreeMap<LeafKey, TerminalAuthorityClosureLeaf>,
}

impl<'a> Reviewer<'a> {
    pub(super) fn new(context: ReviewContext<'a>) -> Self {
        Self {
            context,
            active_requirements: BTreeSet::new(),
            leaves: BTreeMap::new(),
        }
    }

    pub(super) fn into_leaves(self) -> Vec<TerminalAuthorityClosureLeaf> {
        self.leaves.into_values().collect()
    }

    pub(super) fn expand_boundary(&mut self, boundary: BoundaryMachineId) -> Result<(), String> {
        let declaration = self
            .context
            .boundaries
            .get(&boundary)
            .ok_or_else(|| format!("reachable boundary {boundary:?} has no declaration"))?;
        let requirement = declaration.identity.as_str();
        let (selected_plan, selected_row) = self.context.selected_requirement(requirement)?;
        match &selected_row.binding {
            ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } => {
                if selected_plan.provider_type.is_empty()
                    || machine_identity.is_empty()
                    || *machine_package_identity != selected_plan.origin_package_identity
                {
                    return Err(format!(
                        "selected checked adapter for `{requirement}` has incomplete or substituted provider custody"
                    ));
                }
                if !self.active_requirements.insert(requirement.to_owned()) {
                    return Err(format!(
                        "selected checked-provider closure contains a cycle through `{requirement}`"
                    ));
                }
                let candidates = self
                    .context
                    .installed_candidates
                    .iter()
                    .filter(|candidate| {
                        candidate.boundary == boundary
                            && candidate.requirement_identity == requirement
                            && candidate.provider_identity == selected_plan.provider_type
                            && candidate.candidate_identity == *machine_identity
                    })
                    .collect::<Vec<_>>();
                let [candidate] = candidates.as_slice() else {
                    self.active_requirements.remove(requirement);
                    return Err(format!(
                        "selected checked adapter for `{requirement}` resolves to {} exact installed candidates",
                        candidates.len()
                    ));
                };
                let nested = self.context.reachable_boundaries(candidate.candidate)?;
                for nested_boundary in nested {
                    self.expand_boundary(nested_boundary)?;
                }
                self.active_requirements.remove(requirement);
            }
            ProviderBinding::CompilerIntrinsic { .. } | ProviderBinding::Import { .. } => {
                let mechanism = self
                    .context
                    .exact_mechanism(boundary, &selected_row.binding)?;
                let exercised = self
                    .context
                    .physical_policy
                    .classify(mechanism)
                    .map_err(|unclassified| {
                        format!(
                            "receiving terminal-authority policy does not classify {:?} required by `{requirement}`",
                            unclassified.mechanism()
                        )
                    })?;
                let schema = selected_plan.schema.identity_digest();
                let permitted = self
                    .context
                    .permission_policy
                    .permission_for(schema, requirement)
                    .map_err(|_| {
                        format!(
                            "receiving terminal-authority permission policy has no exact row for `{requirement}`"
                        )
                    })?;
                let leaf = TerminalAuthorityClosureLeaf::new(
                    schema,
                    requirement.to_owned(),
                    selected_plan.identity_digest(),
                    mechanism,
                    exercised,
                    permitted,
                )
                .map_err(|error| {
                    format!(
                        "terminal mechanism for `{requirement}` exceeds its exact service permission: {error:?}"
                    )
                })?;
                let key = (
                    schema,
                    requirement.to_owned(),
                    selected_plan.identity_digest(),
                );
                if let Some(previous) = self.leaves.insert(key, leaf.clone()) {
                    if previous != leaf {
                        return Err(format!(
                            "selected provider closure substituted the terminal mechanism for `{requirement}`"
                        ));
                    }
                }
            }
            ProviderBinding::StringBackedImportBootstrap { .. } => {
                return Err(format!(
                    "selected requirement `{requirement}` retains a string-backed import with no terminal identity"
                ));
            }
            ProviderBinding::Syscall { .. } => {
                return Err(format!(
                    "selected requirement `{requirement}` uses the unsupported syscall terminal role"
                ));
            }
            ProviderBinding::VtableSlot { .. } => {
                return Err(format!(
                    "selected requirement `{requirement}` uses the unsupported vtable-slot terminal role"
                ));
            }
            ProviderBinding::VtableField { .. } => {
                return Err(format!(
                    "selected requirement `{requirement}` uses the unsupported vtable-field terminal role"
                ));
            }
            ProviderBinding::TableFunction { .. } => {
                return Err(format!(
                    "selected requirement `{requirement}` uses the unsupported table-function terminal role"
                ));
            }
        }
        Ok(())
    }
}
