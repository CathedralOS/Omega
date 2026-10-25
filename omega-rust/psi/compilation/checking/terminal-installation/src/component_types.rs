//! Exact logical coordinates for one independently checked carrier graph.
//!
//! This presentation changes artifact-local keys, not storage ownership or the
//! selected native representation. Construction shares the complete carrier
//! comparison used by membership theories; callers cannot fabricate a map.

use semantic_vocabulary::{StructuralCaseId, StructuralTypeId};
use std::collections::BTreeMap;
use terminal_psi::{StructuralPathSegment, TerminalIdentityIndex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentTypePresentation {
    source: StructuralTypeId,
    destination: StructuralTypeId,
    types: BTreeMap<StructuralTypeId, StructuralTypeId>,
    fields: BTreeMap<TerminalIdentityIndex, TerminalIdentityIndex>,
    cases: BTreeMap<StructuralCaseId, StructuralCaseId>,
}

impl ComponentTypePresentation {
    pub(super) fn new(
        source: StructuralTypeId,
        destination: StructuralTypeId,
        types: BTreeMap<StructuralTypeId, StructuralTypeId>,
        fields: BTreeMap<TerminalIdentityIndex, TerminalIdentityIndex>,
        cases: BTreeMap<StructuralCaseId, StructuralCaseId>,
    ) -> Self {
        Self {
            source,
            destination,
            types,
            fields,
            cases,
        }
    }

    pub const fn source(&self) -> StructuralTypeId {
        self.source
    }
    pub const fn destination(&self) -> StructuralTypeId {
        self.destination
    }

    pub fn present_type(&self, source: StructuralTypeId) -> Option<StructuralTypeId> {
        self.types.get(&source).copied()
    }

    pub fn present_case(&self, source: StructuralCaseId) -> Option<StructuralCaseId> {
        self.cases.get(&source).copied()
    }

    pub fn present_qualification_path(
        &self,
        path: &mut [StructuralPathSegment],
    ) -> Result<(), &'static str> {
        for step in path {
            if !matches!(step, StructuralPathSegment::AllElements) {
                self.present_path(std::slice::from_mut(step))?;
            }
        }
        Ok(())
    }

    /// Only already resolved logical paths participate. Reference/re-view and
    /// runtime selectors retain separate backing/loan requirements.
    pub fn present_path(&self, path: &mut [StructuralPathSegment]) -> Result<(), &'static str> {
        for step in path {
            match step {
                StructuralPathSegment::Field(field) => {
                    *field = *self
                        .fields
                        .get(field)
                        .ok_or("component path has no matched field")?
                }
                StructuralPathSegment::Case(case) => {
                    *case = *self
                        .cases
                        .get(case)
                        .ok_or("component path has no matched case")?
                }
                StructuralPathSegment::FixedIndex(_) => {}
                _ => return Err("component path retains unresolved backing or selector"),
            }
        }
        Ok(())
    }

    /// Recover original owner coordinates for address/identity observations.
    /// Numeric equality of unrelated artifact-local indices is insufficient.
    pub fn original_path(&self, path: &mut [StructuralPathSegment]) -> Result<(), &'static str> {
        fn unique_source<Coordinate: Copy + Ord>(
            rows: &BTreeMap<Coordinate, Coordinate>,
            destination: Coordinate,
        ) -> Option<Coordinate> {
            let mut matching = rows
                .iter()
                .filter_map(|(source, candidate)| (*candidate == destination).then_some(*source));
            let source = matching.next()?;
            matching.next().is_none().then_some(source)
        }
        for step in path {
            match step {
                StructuralPathSegment::Field(field) => {
                    *field = unique_source(&self.fields, *field)
                        .ok_or("component field has no unique original coordinate")?
                }
                StructuralPathSegment::Case(case) => {
                    *case = unique_source(&self.cases, *case)
                        .ok_or("component case has no unique original coordinate")?
                }
                StructuralPathSegment::FixedIndex(_) => {}
                _ => return Err("component origin retains unresolved backing or selector"),
            }
        }
        Ok(())
    }
}
