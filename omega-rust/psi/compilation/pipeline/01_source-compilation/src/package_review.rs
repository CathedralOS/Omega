//! Identify numeric declarations a checked package exposes through its entry
//! or its selected provider bodies. Without an exact indirect-call closure,
//! review must cover every declared numeric operation in that package.

use semantic_vocabulary::{BoundaryMachineId, MachineId, TargetMachineCallable};
use std::collections::BTreeMap;
use std::sync::Mutex;

type NumericRequirements = Result<Vec<BoundaryMachineId>, &'static str>;

/// One immutable answer per entry and selected provider-body set of this
/// judgment. Deployments selecting the same bodies share the same source fact.
#[derive(Debug, Default)]
pub(crate) struct NumericReviewInventory {
    by_roots: Mutex<BTreeMap<Vec<MachineId>, NumericRequirements>>,
}

impl crate::AcceptedTerminalSource {
    fn package_review_roots(
        &self,
        provider_bodies: impl IntoIterator<Item = MachineId>,
    ) -> Vec<MachineId> {
        let mut roots: Vec<_> = self.entry().into_iter().chain(provider_bodies).collect();
        roots.sort_unstable();
        roots.dedup();
        roots
    }

    pub fn publish_package_review_numeric_requirements(
        &self,
        provider_bodies: impl IntoIterator<Item = MachineId>,
    ) -> Result<Vec<BoundaryMachineId>, &'static str> {
        let roots = self.package_review_roots(provider_bodies);
        let mut published = self
            .package_review_numeric
            .by_roots
            .lock()
            .map_err(|_| "the package's checked numeric review inventory")?;
        published
            .entry(roots)
            .or_insert_with_key(|roots| self.compute_numeric_requirements(roots))
            .clone()
    }

    /// Return the source fact issued during provider publication. Package
    /// inspection can verify the target rows without running closure analysis.
    pub fn retained_package_review_numeric_requirements(
        &self,
        provider_bodies: impl IntoIterator<Item = MachineId>,
    ) -> Result<Vec<BoundaryMachineId>, &'static str> {
        let roots = self.package_review_roots(provider_bodies);
        if roots.is_empty() {
            return Ok(Vec::new());
        }
        let published = self
            .package_review_numeric
            .by_roots
            .lock()
            .map_err(|_| "the package's checked numeric review inventory")?;
        published
            .get(&roots)
            .ok_or("the package's checked numeric review inventory")?
            .clone()
    }

    fn compute_numeric_requirements(&self, roots: &[MachineId]) -> NumericRequirements {
        let mut numeric: Vec<_> = self
            .module()
            .boundary_machines
            .iter()
            .filter(|boundary| {
                boundary.declaration.as_ref().is_some_and(|declaration| {
                    declaration.execution
                        == terminal_psi::CheckedBoundaryExecution::NumericPrimitive
                })
            })
            .map(|boundary| boundary.id)
            .collect();
        if numeric.is_empty() || roots.is_empty() {
            return Ok(Vec::new());
        }
        numeric.sort_unstable();
        let elaboration = &self.closure().elaboration;
        let Some(machines) = elaboration.call_closure(roots.iter().copied()) else {
            return Ok(numeric);
        };
        let mut demanded = Vec::new();
        for machine in machines {
            let tables = elaboration
                .tables
                .machines
                .get(machine)
                .ok_or("the reached machine's checked call inventory")?;
            for call in tables.invocations.values() {
                if let terminal_check_tables::edges::Callee::Declared(
                    TargetMachineCallable::Boundary(boundary),
                ) = call.callee
                    && numeric.binary_search(&boundary).is_ok()
                {
                    demanded.push(boundary);
                }
            }
        }
        demanded.sort_unstable();
        demanded.dedup();
        Ok(demanded)
    }
}
