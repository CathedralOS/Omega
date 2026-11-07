//! Recursive selected-provider closure expansion and leaf admission.

use std::collections::{BTreeMap, BTreeSet};

use effects::{
    TerminalAuthorityClosureLeaf, TerminalAuthorityClosureLeafOrigin, TerminalMechanismIdentity,
    provider_plan::{ProviderBinding, ProviderPlanDigest, ServiceSchemaDigest},
};
use semantic_vocabulary::BoundaryMachineId;

use super::context::ReviewContext;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum LeafKey {
    SelectedRequirement(
        ServiceSchemaDigest,
        String,
        ProviderPlanDigest,
        TerminalAuthorityClosureLeafOrigin,
    ),
    ProgramRoot(
        semantic_vocabulary::MachineId,
        semantic_vocabulary::OperationId,
    ),
}

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
        // Canonical provider admission already rejects cycles through every
        // installed provider instance; this set
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
        let lifecycle = self
            .context
            .input
            .module()
            .access_lifecycle_plans
            .iter()
            .find(|plan| {
                plan.boundary == boundary
                    && matches!(
                        plan.execution,
                        terminal_psi::TerminalAccessLifecycleExecution::CompilerPrimitive { .. }
                    )
            })
            .map(|plan| {
                effects::CompilerIntrinsicExecutionIdentity::AccessLifecycle(plan.operation)
            });
        let intrinsic = if declaration.declaration.as_ref().map(|row| row.execution)
            == Some(terminal_psi::CheckedBoundaryExecution::EmptyRunRelease)
        {
            Some(effects::CompilerIntrinsicExecutionIdentity::EmptyRunRelease)
        } else {
            lifecycle
        };
        if let Some(intrinsic) = intrinsic {
            // These private, source-authenticated operations consume existing
            // extent custody. Their profile supplier is an ordinary selected
            // call and is reviewed separately. Do not invent a provider/service
            // identity for the primitive, or silently skip a missing settlement.
            let expected = TerminalMechanismIdentity::CompilerIntrinsic(intrinsic);
            if self.context.mechanisms.get(&boundary) != Some(&expected) {
                return Err(format!(
                    "private lifecycle {boundary:?} has no exact admitted execution"
                ));
            }
            let exercised = self
                .context
                .physical_policy
                .classify(expected)
                .map_err(|error| {
                    format!(
                        "private lifecycle {boundary:?} has no physical classification: {:?}",
                        error.mechanism()
                    )
                })?;
            if !exercised.classes().is_empty() {
                return Err(format!(
                    "private lifecycle {boundary:?} unexpectedly exercises host authority"
                ));
            }
            return Ok(());
        }
        let requirement = self
            .context
            .input
            .plan()
            .identities
            .text(declaration.identity)
            .unwrap_or_default();
        let (selected_plan, selected_row, _) = self.context.selected_requirement(requirement)?;
        if let Some(authority) = self.context.component_authority(boundary)? {
            // This witness came from actual producer native admission, not a
            // descriptive receipt reconstructed by the receiver. Every leaf is
            // nevertheless classified and permission-checked again here.
            // Producer-local identity indices resolve in its own retained table.
            for leaf in authority.review().leaves() {
                let effects::TerminalAuthorityClosureCustody::SelectedRequirement {
                    service_schema,
                    requirement_identity,
                    provider_plan,
                } = leaf.custody()
                else {
                    return Err("component direct-root authority requires an explicit receiving subject; it cannot inherit a foreign or boundary permission".into());
                };
                let requirement = authority
                    .review()
                    .identities()
                    .text(requirement_identity)
                    .ok_or("component authority leaf lost its original identity table")?;
                self.admit_leaf(
                    service_schema,
                    requirement,
                    provider_plan,
                    leaf.mechanism(),
                    leaf.origin(),
                )?;
            }
            return Ok(());
        }
        match &selected_row.binding {
            ProviderBinding::CheckedAdapter { .. } => {
                let candidate = self.context.checked_adapter(requirement)?.ok_or_else(|| {
                    format!("checked adapter for `{requirement}` lost its selected role")
                })?;
                let schema = selected_plan.schema_identity_digest();
                let provider_plan = selected_plan.identity_digest();
                let nested = self
                    .context
                    .graph
                    .reachable_authority_edges(candidate.candidate)?;
                for mechanism in nested.checked_operations {
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
                let schema = selected_plan.schema_identity_digest();
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

    pub(super) fn admit_program_root(
        &mut self,
        instruction: super::reachability::ReachableCheckedOperation,
    ) -> Result<(), String> {
        // The current receiving-policy vocabulary names exact selected
        // requirements. A direct root has no such subject; classification
        // alone must never be replayed as a receiving-permission verdict.
        if self.context.permission_policy.is_some() {
            return Err("direct checked instruction has no exact receiving-permission subject in this policy".into());
        }
        let exercised = self
            .context
            .physical_policy
            .classify(instruction.mechanism)
            .map_err(|unclassified| {
                format!(
                    "direct checked instruction has no exact physical classification: {:?}",
                    unclassified.mechanism(),
                )
            })?;
        let declaration = self
            .context
            .input
            .module()
            .services
            .iter()
            .find(|declaration| declaration.id == instruction.service)
            .ok_or_else(|| {
                format!(
                    "direct checked instruction references absent service {:?}",
                    instruction.service
                )
            })?;
        let identity = self
            .context
            .input
            .plan()
            .identities
            .text(declaration.identity)
            .ok_or_else(|| "direct checked instruction's service identity is absent".to_owned())?;
        let (machine, operation) = instruction
            .origin
            .checked_site()
            .ok_or_else(|| "direct checked instruction has no exact source operation".to_owned())?;
        let leaf = TerminalAuthorityClosureLeaf::program_root(
            self.identities.intern(identity),
            instruction.mechanism,
            instruction.origin,
            exercised,
        )
        .map_err(|error| {
            format!("direct checked instruction custody is inconsistent: {error:?}")
        })?;
        let key = LeafKey::ProgramRoot(machine, operation);
        if let Some(previous) = self.leaves.insert(key, leaf.clone())
            && previous != leaf
        {
            return Err(
                "direct checked instruction repeats with a distinct mechanism or service".into(),
            );
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
        let key =
            LeafKey::SelectedRequirement(schema, requirement.to_owned(), provider_plan, origin);
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
