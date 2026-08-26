use std::collections::{BTreeMap, BTreeSet};

use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationUnitIdentity,
};
use omega_optimization_unit::PsiOptimizationUnit;

use super::{AnalysisProduct, analysis_dependencies, compute_analysis};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisManagerError {
    RevisionMismatch {
        expected: OptimizationUnitIdentity,
        actual: OptimizationUnitIdentity,
    },
    UnsupportedAnalysis(AnalysisKind),
    UndeclaredInvalidation(AnalysisKind),
    WorkerPanicked(AnalysisKind),
}

impl std::fmt::Display for AnalysisManagerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Psi optimizer analysis failure: {self:?}")
    }
}

impl std::error::Error for AnalysisManagerError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisRevisionCommit {
    pub previous: OptimizationUnitIdentity,
    pub current: OptimizationUnitIdentity,
    pub invalidated: Vec<AnalysisKind>,
    pub retained: Vec<AnalysisKind>,
}

/// One-run deterministic analysis cache. There is deliberately no global
/// singleton; each opted-in compilation owns its manager and revision.
#[derive(Debug, Clone)]
pub struct AnalysisManager {
    revision: OptimizationUnitIdentity,
    cache: BTreeMap<AnalysisKind, AnalysisProduct>,
}

impl AnalysisManager {
    pub fn new(unit: &PsiOptimizationUnit) -> Self {
        Self {
            revision: unit.identity,
            cache: BTreeMap::new(),
        }
    }

    pub const fn revision(&self) -> OptimizationUnitIdentity {
        self.revision
    }

    pub fn cached_kinds(&self) -> impl Iterator<Item = AnalysisKind> + '_ {
        self.cache.keys().copied()
    }

    pub fn require(
        &mut self,
        unit: &PsiOptimizationUnit,
        kind: AnalysisKind,
    ) -> Result<&AnalysisProduct, AnalysisManagerError> {
        self.require_revision(unit)?;
        self.compute_with_dependencies(unit, kind)?;
        Ok(&self.cache[&kind])
    }

    /// Resolve a request set in canonical `AnalysisKind::ALL` order. Caller
    /// insertion order cannot influence dependency or output order.
    pub fn require_all(
        &mut self,
        unit: &PsiOptimizationUnit,
        requested: AnalysisSet,
    ) -> Result<Vec<&AnalysisProduct>, AnalysisManagerError> {
        self.require_revision(unit)?;
        for kind in requested.iter() {
            self.compute_with_dependencies(unit, kind)?;
        }
        Ok(requested.iter().map(|kind| &self.cache[&kind]).collect())
    }

    /// Cold independent computation suitable for validation and parallel
    /// scheduling. Results are always returned in canonical kind order.
    pub fn compute_cold_parallel(
        unit: &PsiOptimizationUnit,
        requested: AnalysisSet,
    ) -> Result<Vec<AnalysisProduct>, AnalysisManagerError> {
        let kinds = requested.iter().collect::<Vec<_>>();
        let mut rows = std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(kinds.len());
            for kind in kinds.iter().copied() {
                handles.push((kind, scope.spawn(move || compute_analysis(unit, kind))));
            }
            handles
                .into_iter()
                .map(|(kind, handle)| {
                    handle
                        .join()
                        .map_err(|_| AnalysisManagerError::WorkerPanicked(kind))?
                        .ok_or(AnalysisManagerError::UnsupportedAnalysis(kind))
                })
                .collect::<Result<Vec<_>, _>>()
        })?;
        rows.sort_by_key(AnalysisProduct::kind);
        Ok(rows)
    }

    /// Atomically move to a candidate revision. In pass-validation mode every
    /// supposedly retained cache row is cold-recomputed first; any difference
    /// proves the candidate lied about invalidation and leaves this manager
    /// completely unchanged.
    pub fn commit_revision(
        &mut self,
        unit: &PsiOptimizationUnit,
        declared: AnalysisInvalidationSet,
        validate_retained: bool,
    ) -> Result<AnalysisRevisionCommit, AnalysisManagerError> {
        let invalidated = invalidation_closure(declared);
        if validate_retained {
            for (kind, cached) in self
                .cache
                .iter()
                .filter(|(kind, _)| !invalidated.contains(kind))
            {
                let cold = compute_analysis(unit, *kind)
                    .ok_or(AnalysisManagerError::UnsupportedAnalysis(*kind))?;
                if cold != *cached {
                    return Err(AnalysisManagerError::UndeclaredInvalidation(*kind));
                }
            }
        }
        let previous = self.revision;
        self.cache.retain(|kind, _| !invalidated.contains(kind));
        self.revision = unit.identity;
        Ok(AnalysisRevisionCommit {
            previous,
            current: unit.identity,
            invalidated: invalidated.iter().copied().collect(),
            retained: self.cache.keys().copied().collect(),
        })
    }

    fn require_revision(&self, unit: &PsiOptimizationUnit) -> Result<(), AnalysisManagerError> {
        if unit.identity != self.revision {
            return Err(AnalysisManagerError::RevisionMismatch {
                expected: self.revision,
                actual: unit.identity,
            });
        }
        Ok(())
    }

    fn compute_with_dependencies(
        &mut self,
        unit: &PsiOptimizationUnit,
        kind: AnalysisKind,
    ) -> Result<(), AnalysisManagerError> {
        if self.cache.contains_key(&kind) {
            return Ok(());
        }
        let dependencies =
            analysis_dependencies(kind).ok_or(AnalysisManagerError::UnsupportedAnalysis(kind))?;
        for dependency in dependencies.iter() {
            self.compute_with_dependencies(unit, dependency)?;
        }
        let product =
            compute_analysis(unit, kind).ok_or(AnalysisManagerError::UnsupportedAnalysis(kind))?;
        self.cache.insert(kind, product);
        Ok(())
    }
}

fn invalidation_closure(declared: AnalysisInvalidationSet) -> BTreeSet<AnalysisKind> {
    let mut invalidated = declared.iter().collect::<BTreeSet<_>>();
    loop {
        let previous = invalidated.len();
        for kind in AnalysisKind::ALL {
            let Some(dependencies) = analysis_dependencies(kind) else {
                continue;
            };
            if dependencies
                .iter()
                .any(|dependency| invalidated.contains(&dependency))
            {
                invalidated.insert(kind);
            }
        }
        if invalidated.len() == previous {
            return invalidated;
        }
    }
}
