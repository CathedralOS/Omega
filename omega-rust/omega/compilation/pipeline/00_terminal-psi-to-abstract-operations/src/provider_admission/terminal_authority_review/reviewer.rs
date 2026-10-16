//! Recursive selected-provider closure expansion and leaf admission.

use std::collections::{BTreeMap, BTreeSet};

use effects::{
    TerminalAuthorityClosureLeaf, TerminalAuthorityClosureLeafOrigin, TerminalMechanismIdentity,
    provider_plan::{ProviderBinding, ProviderPlanDigest, ServiceSchemaDigest},
};
use semantic_vocabulary::BoundaryMachineId;

use super::context::ReviewContext;

type LeafKey = (
    effects::provider_plan::ServiceSchemaDigest,
    String,
    effects::provider_plan::ProviderPlanDigest,
    TerminalAuthorityClosureLeafOrigin,
);

pub(super) struct Reviewer<'a> {
    context: ReviewContext<'a>,
    expanded: BTreeSet<BoundaryMachineId>,
    leaves: BTreeMap<LeafKey, TerminalAuthorityClosureLeaf>,
    identities: terminal_psi::TerminalIdentityTableBuilder,
}

impl<'a> Reviewer<'a> {
    pub(super) fn new(context: ReviewContext<'a>) -> Self {
        Self {
            // Provider-side requirement text can be absent from the plan's
            // table, so leaves mint into a review-local copy of it rather
            // than resolving through `index_of`.
            identities: context.input.plan().identities.clone().into_builder(),
            context,
            expanded: BTreeSet::new(),
            leaves: BTreeMap::new(),
        }
    }

    pub(super) fn into_leaves(
        self,
    ) -> (
        Vec<TerminalAuthorityClosureLeaf>,
        terminal_psi::TerminalIdentityTable,
    ) {
        (self.leaves.into_values().collect(), self.identities.build())
    }

    pub(super) fn expand_boundary(&mut self, boundary: BoundaryMachineId) -> Result<(), String> {
        // Provider planning's selected synchronous-invocation cycle check
        // already rejects a cycle through the selected providers; this set
        // only bounds the recursion so a repeated boundary is expanded once
        // rather than re-checked.
        if !self.expanded.insert(boundary) {
            return Ok(());
        }
        let declaration = self
            .context
            .boundaries
            .get(&boundary)
            .ok_or_else(|| format!("reachable boundary {boundary:?} has no declaration"))?;
        let requirement = self
            .context
            .input
            .plan()
            .identities
            .text(declaration.identity)
            .unwrap_or_default();
        let (selected_plan, selected_row, specialization) =
            self.context.selected_requirement(requirement)?;
        match &selected_row.binding {
            ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } => {
                if selected_plan.provider_type.is_empty()
                    || !machine_identity.is_valid()
                    || *machine_package_identity != selected_plan.origin_package_identity
                {
                    return Err(format!(
                        "selected checked adapter for `{requirement}` has incomplete or substituted provider custody"
                    ));
                }
                // Recover the exact installed row, without treating an absent
                // catalog or projection as successful installation. The trust
                // boundary reports missing custody rather than panicking.
                // The installed candidate names the specialized provider
                // instance for a closed requirement; the selected row's
                // binding retains the authored generic provider template.
                let realization_identity = specialization
                    .and_then(|row| {
                        self.context
                            .selected
                            .identities()
                            .text(row.machine_identity)
                    })
                    .unwrap_or(selected_plan.identity_text(*machine_identity));
                let candidate = self
                    .context
                    .installed_candidates
                    .iter()
                    .find(|candidate| {
                        candidate.boundary == boundary
                            && self.context.input.plan().identities.text(candidate.requirement_identity) == Some(requirement)
                            && self.context.input.plan().identities.text(candidate.provider_identity) == Some(selected_plan.provider_type.as_str())
                            && self.context.input.plan().identities.text(candidate.candidate_identity) == Some(realization_identity)
                    })
                    .ok_or_else(|| {
                        format!(
                            "reachable requirement `{requirement}` has no admitted installation for selected provider `{}` adapter `{realization_identity}`",
                            selected_plan.provider_type
                        )
                    })?;
                let schema = selected_plan.schema.identity_digest();
                let provider_plan = selected_plan.identity_digest();
                let nested = self
                    .context
                    .graph
                    .reachable_authority_edges(candidate.candidate)?;
                for mechanism in nested.checked_physical {
                    self.admit_leaf(
                        schema,
                        requirement,
                        provider_plan,
                        mechanism.mechanism,
                        mechanism.origin,
                    )?;
                }
                for nested_boundary in nested.boundaries {
                    self.expand_boundary(nested_boundary)?;
                }
            }
            ProviderBinding::CompilerIntrinsic { .. }
            | ProviderBinding::Import { .. }
            | ProviderBinding::Syscall { .. }
            | ProviderBinding::VtableField { .. }
            | ProviderBinding::TableFunction { .. } => {
                let mechanism = self
                    .context
                    .exact_mechanism(boundary, &selected_row.binding)?;
                let schema = selected_plan.schema.identity_digest();
                self.admit_leaf(
                    schema,
                    selected_plan
                        .identities
                        .text(selected_row.requirement_identity),
                    selected_plan.identity_digest(),
                    mechanism,
                    TerminalAuthorityClosureLeafOrigin::SelectedBoundary,
                )?;
            }
            ProviderBinding::VtableSlot { .. } => {
                return Err(format!(
                    "selected requirement `{requirement}` uses the unsupported vtable-slot terminal role"
                ));
            }
        }
        Ok(())
    }

    fn admit_leaf(
        &mut self,
        schema: ServiceSchemaDigest,
        requirement: &str,
        provider_plan: ProviderPlanDigest,
        mechanism: TerminalMechanismIdentity,
        origin: TerminalAuthorityClosureLeafOrigin,
    ) -> Result<(), String> {
        let exercised = match self.context.physical_policy.classify(mechanism) {
            Ok(exercised) => Some(exercised),
            Err(unclassified) if self.context.permission_policy.is_some() => {
                return Err(format!(
                    "receiving terminal-authority policy does not classify {:?} required by `{requirement}`",
                    unclassified.mechanism()
                ));
            }
            Err(_) => None,
        };
        // The receiver-admission axis is consulted only when a receiving
        // permission policy was explicitly supplied. Without one the leaf
        // retains its mechanism and any available classification with no
        // adjudicated permission; the leaf cannot claim admission later.
        let permitted = self
            .context
            .permission_policy
            .map(|policy| policy.permission_for(schema, requirement))
            .transpose()
            .map_err(|_| {
                format!(
                    "receiving terminal-authority permission policy has no exact row for `{requirement}`"
                )
            })?;
        let leaf = TerminalAuthorityClosureLeaf::new(
            schema,
            self.identities.intern(requirement),
            provider_plan,
            mechanism,
            origin,
            exercised,
            permitted,
        )
        .map_err(|error| {
            format!(
                "terminal mechanism for `{requirement}` exceeds its exact service permission: {error:?}"
            )
        })?;
        let key = (schema, requirement.to_owned(), provider_plan, origin);
        if let Some(previous) = self.leaves.insert(key, leaf.clone())
            && previous != leaf
        {
            return Err(format!(
                "selected provider closure repeats `{requirement}` with a distinct terminal mechanism"
            ));
        }
        Ok(())
    }
}
