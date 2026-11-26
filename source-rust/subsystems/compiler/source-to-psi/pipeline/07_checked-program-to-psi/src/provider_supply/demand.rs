//! Finish one product target's source choices while the accepted source owns its declarations.
use super::{ClosedLogicalProviderReach, explicit_build_requirements};
use crate::provider_choice_inputs::NativeBuildSupply;
use crate::provider_choice_inputs::ProviderSelection;
use crate::{AcceptedTerminalSource, SourcePublication};
use diagnostics::Diagnostic;
use semantic_vocabulary::BoundaryMachineId;
use std::sync::{Arc, Weak};

use super::{SourceNativeBinding, SourceProviderDemand};

/// The source producer alone can attach an issued judgment and deployment to these facts.
/// The weak handle checks provenance without retaining trees for use by the receiver.
///
/// A consumer cannot rewrite a checked root roster while retaining its issuer.
///
/// ```compile_fail,E0616
/// use checked_program_to_psi::provider_supply::{SourceProviderDemand, SourceProviderFacts};
/// fn replace_roots(mut demand: SourceProviderFacts<SourceProviderDemand>) {
///     demand.facts = SourceProviderDemand::Unscoped { roots: Vec::new() };
/// }
/// ```
pub struct SourceProviderFacts<T> {
    issuer: Weak<AcceptedTerminalSource>,
    target: Option<String>,
    facts: T,
}

impl<T> SourceProviderFacts<T> {
    fn new(issuer: &Arc<AcceptedTerminalSource>, target: Option<&str>, facts: T) -> Self {
        Self {
            issuer: Arc::downgrade(issuer),
            target: target.map(str::to_owned),
            facts,
        }
    }

    fn require_issuer(
        &self,
        publication: &SourcePublication,
        target: Option<&str>,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.target.as_deref() != target
            || !self
                .issuer
                .ptr_eq(&Arc::downgrade(publication.accepted_owner()?))
        {
            return Err(vec![Diagnostic::error(
                "closed provider demand belongs to another source judgment or deployment",
            )]);
        }
        Ok(())
    }

    pub fn receive(
        self,
        publication: &SourcePublication,
        target: Option<&str>,
    ) -> Result<T, Vec<Diagnostic>> {
        self.require_issuer(publication, target)?;
        Ok(self.facts)
    }
}

impl SourceProviderFacts<SourceProviderDemand> {
    /// Seal these facts to the artifact the same source judgment published for this target.
    pub fn for_published_target(
        self,
        publication: &SourcePublication,
        target: &str,
    ) -> Result<super::PublishedTargetProviders, Vec<Diagnostic>> {
        let demand = self.receive(publication, Some(target))?;
        let artifact = publication.require_published()?;
        Ok(source_provider_publication::issue_target_provider_demand(
            artifact, target, demand,
        ))
    }

    /// A lock-policy request can prepare its source-derived review facts from
    /// the same closed choices that portable publication receives next.
    pub fn logical_provider_reach(
        &self,
        publication: &SourcePublication,
        target: Option<&str>,
    ) -> Result<Option<&ClosedLogicalProviderReach>, Vec<Diagnostic>> {
        self.require_issuer(publication, target)?;
        match &self.facts {
            SourceProviderDemand::Unscoped { .. } => Ok(None),
            SourceProviderDemand::Target { closed, .. } => Ok(Some(closed)),
        }
    }
}

impl SourcePublication {
    pub fn prepare_provider_demand(
        &self,
        target: Option<&str>,
        selections: &[ProviderSelection],
        native_supplies: &[NativeBuildSupply],
        review_package: bool,
        reach_policy: &semantic_vocabulary::dependency_reach::ExecutionReachPolicy,
    ) -> Result<SourceProviderFacts<SourceProviderDemand>, Vec<Diagnostic>> {
        let accepted = self.accepted_owner()?;
        let module = accepted.module();
        let unsafe_services = accepted.unsafe_services()?;
        let imported_calls = if reach_policy.edges.is_empty() {
            Default::default()
        } else {
            accepted.dependency_calls()
        };
        let entry = match target {
            Some(target)
                if !(self.roster()?.entries.is_empty()
                    && module.interfaces.program_entry_contracts.is_empty()) =>
            {
                let contract = module
                    .program_entry_contract(target)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                module.machine(contract.entry).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selected target entry has no exact executable machine",
                    )]
                })?;
                Some(contract.entry)
            }
            _ => None,
        };
        let declared = accepted
            .boundary_requirements_declared()
            .map_err(|error| vec![error])?;
        let selected = explicit_build_requirements(&declared, selections, target);
        let Some(deployment) = target else {
            if !selected.is_empty() {
                return Err(declared
                    .iter()
                    .filter(|row| selected.binary_search(&row.boundary).is_ok())
                    .map(|row| {
                        Diagnostic::error(format!(
                            "boundary requirement `{}` requires an explicit deployment target",
                            row.name
                        ))
                    })
                    .collect());
            }
            let roots = self
                .roster()?
                .entries
                .iter()
                .map(|symbol| {
                    accepted.machine_of(*symbol).ok_or_else(|| {
                        vec![Diagnostic::error(
                            "unscoped source lost its actual invocation root",
                        )]
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            for &root in &roots {
                psi_provider_selections::dependency_reach::check(
                    module,
                    semantic_vocabulary::TargetMachineCallable::Machine(root),
                    &[],
                    None,
                    reach_policy,
                    &unsafe_services,
                    &imported_calls,
                )?;
            }
            return Ok(SourceProviderFacts::new(
                accepted,
                None,
                SourceProviderDemand::Unscoped { roots },
            ));
        };
        let closed = accepted.close_logical_provider_reach(deployment, entry, &selected)?;
        // An installation may export a selected provider with no process entry. Check its
        // executable body as a root even when no ordinary entry calls it.
        let roots = entry
            .into_iter()
            .chain(
                closed
                    .bindings()
                    .iter()
                    .filter(|row| selected.binary_search(&row.boundary).is_ok())
                    .map(|row| row.machine),
            )
            .collect::<std::collections::BTreeSet<_>>();
        for root in roots {
            psi_provider_selections::dependency_reach::check(
                module,
                semantic_vocabulary::TargetMachineCallable::Machine(root),
                closed.bindings(),
                Some(deployment),
                reach_policy,
                    &unsafe_services,
                &imported_calls,
            )?;
        }
        let bindings =
            accepted.reached_native_bindings(deployment, native_supplies, closed.requirements());
        let review = (review_package && !accepted.is_invocation()).then(|| {
            accepted.source_package_review_requirements(
                deployment,
                native_supplies,
                &declared,
                closed.bindings().iter().map(|row| row.machine),
            )
        });
        let mut labels: Vec<_> = declared
            .into_iter()
            .map(|row| (row.boundary, row.name))
            .collect();
        labels.sort_unstable_by_key(|(boundary, _)| *boundary);
        Ok(SourceProviderFacts::new(
            accepted,
            target,
            SourceProviderDemand::Target {
                closed,
                labels,
                bindings,
                review,
            },
        ))
    }

    /// Publish only the named requirements for explicit native-input inspection.
    pub fn prepare_provider_bindings(
        &self,
        target: &str,
        supplies: &[NativeBuildSupply],
        reached: &[BoundaryMachineId],
    ) -> Result<SourceProviderFacts<Vec<SourceNativeBinding>>, Vec<Diagnostic>> {
        let accepted = self.accepted_owner()?;
        let bindings = accepted.reached_native_bindings(target, supplies, reached);
        Ok(SourceProviderFacts::new(accepted, Some(target), bindings))
    }
}

impl AcceptedTerminalSource {
    fn reached_native_bindings(
        &self,
        target: &str,
        supplies: &[NativeBuildSupply],
        reached: &[BoundaryMachineId],
    ) -> Vec<SourceNativeBinding> {
        let mut bindings = Vec::new();
        for supply in supplies
            .iter()
            .filter(|supply| supply.selection.target.canonical_name() == target)
        {
            let mut supplied = Vec::new();
            for (boundary, source) in self
                .closure()
                .elaboration()
                .boundaries
                .iter()
                .filter(|(boundary, _)| reached.contains(boundary))
            {
                let row = match supply.materialization_for(self.source(), *source) {
                    Ok(Some(row)) => row,
                    Ok(None) => continue,
                    Err(reason) => {
                        bindings.push(Err(reason));
                        continue;
                    }
                };
                if supplied.contains(source) {
                    bindings.push(Err(
                        "Build native binding has competing exact boundary occurrences".into(),
                    ));
                    continue;
                }
                supplied.push(*source);
                let callable = self
                    .module()
                    .boundary_machine(boundary)
                    .and_then(|row| self.module().identities.text(row.identity));
                bindings.push(if callable == Some(row.requirement_identity.as_str()) {
                    Ok(row)
                } else {
                    Err("native binding lost its checked requirement identity".into())
                });
            }
        }
        bindings
    }
}
