//! A saved caller will check a borrowed result when its callee returns. The
//! successor must allow exactly the same origins for each leaf of that result.

use super::CheckedPrefix;
use terminal_psi::{
    BoundaryLifetimeDependency, DeclaredResultLifetime, ReferenceOriginProjection,
    StructuralReferenceOrigin, TerminalMachineResult,
};

impl CheckedPrefix<'_> {
    pub(super) fn machine_result(
        &self,
        old: &TerminalMachineResult,
        new: &TerminalMachineResult,
    ) -> bool {
        match (old, new) {
            (TerminalMachineResult::Unit, TerminalMachineResult::Unit) => true,
            (TerminalMachineResult::Scalar(a), TerminalMachineResult::Scalar(b)) => {
                self.scalars(std::slice::from_ref(a), std::slice::from_ref(b))
            }
            (TerminalMachineResult::Structural(a), TerminalMachineResult::Structural(b)) => {
                a.place == b.place
                    && a.multiplicity == b.multiplicity
                    && self.structural_type(a.structural_type) == Some(b.structural_type)
                    && self.result_domains(
                        &a.qualifications,
                        &a.projected_qualifications,
                        &b.qualifications,
                        &b.projected_qualifications,
                    )
                    && a.reference_sources.len() == b.reference_sources.len()
                    && a.reference_sources
                        .iter()
                        .zip(&b.reference_sources)
                        .all(|(x, y)| {
                            self.paths(&x.path, &y.path)
                                && x.sources.len() == y.sources.len()
                                && x.sources
                                    .iter()
                                    .zip(&y.sources)
                                    .all(|(l, r)| self.reference_origin(l, r))
                        })
            }
            _ => false,
        }
    }

    fn reference_origin(
        &self,
        old: &StructuralReferenceOrigin,
        new: &StructuralReferenceOrigin,
    ) -> bool {
        let root = match (old, new) {
            (StructuralReferenceOrigin::Parameter(a), StructuralReferenceOrigin::Parameter(b)) => {
                a.place == b.place
            }
            (
                StructuralReferenceOrigin::Invocation {
                    carrier_type: a, ..
                },
                StructuralReferenceOrigin::Invocation {
                    carrier_type: b, ..
                },
            ) => self.structural_type(*a) == Some(*b),
            _ => false,
        };
        root && old.access() == new.access()
            && old.extent() == new.extent()
            && old.path().len() == new.path().len()
            && old
                .path()
                .iter()
                .zip(new.path())
                .all(|(a, b)| match (a, b) {
                    (ReferenceOriginProjection::Element, ReferenceOriginProjection::Element) => {
                        true
                    }
                    (ReferenceOriginProjection::Exact(x), ReferenceOriginProjection::Exact(y)) => {
                        self.paths(std::slice::from_ref(x), std::slice::from_ref(y))
                    }
                    _ => false,
                })
    }

    pub(super) fn result_lifetimes(
        &self,
        old: Option<&[DeclaredResultLifetime]>,
        new: Option<&[DeclaredResultLifetime]>,
    ) -> bool {
        let (old, new) = match (old, new) {
            (Some(a), Some(b)) => (a, b),
            (None, None) => return true,
            _ => return false,
        };
        old.len() == new.len()
            && old.iter().zip(new).all(|(a, b)| {
                a.results.len() == b.results.len()
                    && a.results
                        .iter()
                        .zip(&b.results)
                        .all(|(x, y)| self.paths(x, y))
                    && self.lifetime_inputs(&a.inputs, &b.inputs)
            })
    }
    fn lifetime_inputs(
        &self,
        a: &[BoundaryLifetimeDependency],
        b: &[BoundaryLifetimeDependency],
    ) -> bool {
        a.len() == b.len()
            && a.iter().zip(b).all(|(x, y)| match (x, y) {
                (
                    BoundaryLifetimeDependency::Referent { .. },
                    BoundaryLifetimeDependency::Referent { .. },
                ) => x == y,
                (
                    BoundaryLifetimeDependency::Stored {
                        argument_index: i,
                        access: p,
                        path: u,
                    },
                    BoundaryLifetimeDependency::Stored {
                        argument_index: j,
                        access: q,
                        path: v,
                    },
                ) => i == j && p == q && self.paths(u, v),
                _ => false,
            })
    }

    pub(super) fn boundary_returns(
        &self,
        old: &terminal_psi::BoundaryMachineDeclaration,
        new: &terminal_psi::BoundaryMachineDeclaration,
    ) -> bool {
        use terminal_psi::{
            BoundaryMachineResult as Result, TerminalDynamicReferenceOrigin as Origin,
        };
        let result = match (&old.result, &new.result) {
            (Result::Unit, Result::Unit) => true,
            (Result::Scalar(a), Result::Scalar(b)) => self.same_scalar_storage(Some(*a), Some(*b)),
            (Result::Structural(a), Result::Structural(b)) => {
                self.structural_type(a.structural_type) == Some(b.structural_type)
                    && a.multiplicity == b.multiplicity
                    && self.result_domains(
                        &a.qualifications,
                        &a.projected_qualifications,
                        &b.qualifications,
                        &b.projected_qualifications,
                    )
            }
            _ => false,
        };
        result
            && old.reference_outputs.len() == new.reference_outputs.len()
            && old
                .reference_outputs
                .iter()
                .zip(&new.reference_outputs)
                .all(|(a, b)| {
                    a.destination == b.destination
                        && a.access == b.access
                        && self.paths(&a.path, &b.path)
                        && self.lifetime_inputs(&a.dependencies, &b.dependencies)
                })
            && old.result_reference_regions.len() == new.result_reference_regions.len()
            && old
                .result_reference_regions
                .iter()
                .zip(&new.result_reference_regions)
                .all(|(a, b)| {
                    a.access == b.access
                        && self.paths(&a.path, &b.path)
                        && self.paths(&a.source_path, &b.source_path)
                        && match (&a.origin, &b.origin) {
                            (
                                Origin::Parameter { argument_index: x },
                                Origin::Parameter { argument_index: y },
                            ) => x == y,
                            (
                                Origin::Invocation { carrier_type: x },
                                Origin::Invocation { carrier_type: y },
                            ) => self.structural_type(*x) == Some(*y),
                            _ => false,
                        }
                })
    }

    fn result_domains(
        &self,
        old_domains: &[semantic_vocabulary::StructuralDomainId],
        old_projected: &[terminal_psi::StructuralPathQualification],
        new_domains: &[semantic_vocabulary::StructuralDomainId],
        new_projected: &[terminal_psi::StructuralPathQualification],
    ) -> bool {
        old_domains.len() == new_domains.len()
            && old_domains
                .iter()
                .zip(new_domains)
                .all(|(x, y)| self.structural_domain(*x) == Some(*y))
            && old_projected.len() == new_projected.len()
            && old_projected.iter().zip(new_projected).all(|(x, y)| {
                self.structural_domain(x.domain) == Some(y.domain) && self.paths(&x.path, &y.path)
            })
    }
}
