//! Whole-value images use the installed target's geometry, never the host's.
//! This adapter shares the reference-layout algorithm with native admission;
//! neither byte extents nor a case tag establish ownership of their contents.

use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::engine::execution::TerminalExecution;
use semantic_vocabulary::{ScalarApplicationId, StructuralTypeId, TargetCarrierLayout};
use terminal_psi::{StructuralPathSegment, TargetLayoutInput};
use terminal_semantics::{ReferenceStorageGeometry, ReferenceStorageRegion, StorageGeometryError};

#[derive(Clone, Copy)]
pub(super) struct ImageGeometry<'execution, 'program> {
    execution: &'execution TerminalExecution<'program>,
    carriers: Option<TargetCarrierLayout>,
}

impl<'execution, 'program> ImageGeometry<'execution, 'program> {
    pub(super) fn new(
        execution: &'execution TerminalExecution<'program>,
        target: Option<&TargetLayoutInput>,
    ) -> Result<Self, TerminalInterpretError> {
        let carriers = target
            .map(|target| {
                Ok::<_, TerminalInterpretError>(TargetCarrierLayout {
                    pointer_byte_size: target.pointer_size,
                    pointer_byte_alignment: target.pointer_alignment,
                    descriptor_byte_size: target
                        .pointer_size
                        .checked_mul(2)
                        .ok_or_else(malformed)?,
                    descriptor_byte_alignment: target.pointer_alignment,
                    case_tag_byte_size: target.case_tag_size,
                    case_tag_byte_alignment: target.case_tag_alignment,
                })
            })
            .transpose()?;
        Ok(Self {
            execution,
            carriers,
        })
    }

    pub(super) fn tag_width(&self) -> Result<usize, TerminalInterpretError> {
        self.carriers
            .and_then(|layout| usize::try_from(layout.case_tag_byte_size).ok())
            .filter(|width| (1..=8).contains(width))
            .ok_or(TerminalInterpretError::ValueImageGeometry(
                StorageGeometryError::MissingCarrierLayout,
            ))
    }

    pub(super) fn layout(
        &self,
        root: StructuralTypeId,
    ) -> Result<(u64, u64), TerminalInterpretError> {
        let types = self.execution.structural_types();
        ReferenceStorageGeometry::new(types.values(), unresolved)
            .with_target_carriers(self.carriers)
            .layout(root)
            .map_err(TerminalInterpretError::ValueImageGeometry)
    }

    pub(super) fn sum_layout(
        &self,
        root: StructuralTypeId,
    ) -> Result<layout_plans::ConventionalSumGeometry, TerminalInterpretError> {
        let types = self.execution.structural_types();
        ReferenceStorageGeometry::new(types.values(), unresolved)
            .with_target_carriers(self.carriers)
            .sum_layout(root)
            .map_err(TerminalInterpretError::ValueImageGeometry)
    }

    pub(super) fn region(
        &self,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<ReferenceStorageRegion> {
        let types = self.execution.structural_types();
        ReferenceStorageGeometry::new(types.values(), unresolved)
            .with_target_carriers(self.carriers)
            .region(root, path)
    }
}

fn unresolved(application: ScalarApplicationId) -> Result<u64, StorageGeometryError> {
    Err(StorageGeometryError::UnresolvedExtent(application))
}
