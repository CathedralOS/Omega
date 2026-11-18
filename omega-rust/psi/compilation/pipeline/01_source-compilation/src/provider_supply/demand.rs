//! Finish one product target's source choices while the accepted source owns its declarations.
use super::{
    ClosedLogicalProviderReach, SourcePackageReviewRequirements, explicit_build_requirements,
};
use crate::provider_choice_inputs::ProviderSelection;
use crate::{AcceptedTerminalSource, SourcePublication};
use diagnostics::Diagnostic;
use provider_choices::NativeBuildSupply;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use std::sync::{Arc, Weak};

pub type SourceNativeBinding = Result<
    (
        BoundaryMachineId,
        evaluation_receipts::NativeBindingMaterialization,
    ),
    String,
>;

/// The source producer alone can attach an issued judgment and deployment to these facts.
/// The weak handle checks provenance without retaining trees for use by the receiver.
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

pub enum SourceProviderDemand {
    Unscoped {
        roots: Vec<MachineId>,
    },
    Target {
        closed: ClosedLogicalProviderReach,
        labels: Vec<(BoundaryMachineId, String)>,
        bindings: Vec<SourceNativeBinding>,
        review: Option<SourcePackageReviewRequirements>,
    },
}

impl SourcePublication {
    pub fn prepare_provider_demand(
        &self,
        target: Option<&str>,
        selections: &[ProviderSelection],
        native_supplies: &[NativeBuildSupply],
        review_package: bool,
    ) -> Result<SourceProviderFacts<SourceProviderDemand>, Vec<Diagnostic>> {
        let accepted = self.accepted_owner()?;
        let module = accepted.module();
        let entry = match target {
            Some(target)
                if !(self.roster()?.entries.is_empty()
                    && module.program_entry_contracts.is_empty()) =>
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
            return Ok(SourceProviderFacts::new(
                accepted,
                None,
                SourceProviderDemand::Unscoped { roots },
            ));
        };
        let closed = accepted.close_logical_provider_reach(deployment, entry, &selected)?;
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
            let mut actuals =
                self.closure()
                    .elaboration
                    .boundaries
                    .iter()
                    .filter(|(boundary, source)| {
                        **source == supply.selection.requirement && reached.contains(boundary)
                    });
            let Some((boundary, _)) = actuals.next() else {
                continue;
            };
            let row = if actuals.next().is_some() {
                Err("Build native binding has competing exact boundary occurrences".to_owned())
            } else {
                crate::evaluation::native_bindings::require_canonical_requirement(
                    &supply.selection,
                    self.source(),
                )
                .map(|()| (boundary, supply.materialization()))
            };
            bindings.push(row);
        }
        bindings
    }
}
