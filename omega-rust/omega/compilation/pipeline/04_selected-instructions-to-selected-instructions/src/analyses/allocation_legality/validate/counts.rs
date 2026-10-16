//! Report counts for one admitted legality function; logical widths stay checked.

use crate::AllocationLegalityError;
use register_homes::FunctionAllocationLegality;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FunctionCounts {
    pub(super) virtual_register_count: usize,
    pub(super) point_count: usize,
    pub(super) candidate_count: usize,
    pub(super) early_clobber_point_count: usize,
    pub(super) early_clobber_candidate_count: usize,
    pub(super) entry_transition_count: usize,
}

impl FunctionCounts {
    pub(super) fn new(
        function: &FunctionAllocationLegality,
        function_index: usize,
    ) -> Result<Self, AllocationLegalityError> {
        let overflow = || AllocationLegalityError::PointOverflow {
            function: function_index,
        };
        let mut point_count = 0usize;
        let mut candidate_count = 0usize;
        for span in function
            .virtual_registers
            .iter()
            .flat_map(|register| &register.spans)
        {
            let width = usize::try_from(span.end.0 - span.start.0).map_err(|_| overflow())?;
            point_count = point_count.checked_add(width).ok_or_else(overflow)?;
            let candidates = width
                .checked_mul(span.candidates.len())
                .ok_or_else(overflow)?;
            candidate_count = candidate_count
                .checked_add(candidates)
                .ok_or_else(overflow)?;
        }
        Ok(Self {
            virtual_register_count: function.virtual_registers.len(),
            point_count,
            candidate_count,
            early_clobber_point_count: function
                .virtual_registers
                .iter()
                .map(|register| register.early_clobber_points.len())
                .sum(),
            early_clobber_candidate_count: function
                .virtual_registers
                .iter()
                .flat_map(|register| &register.early_clobber_points)
                .map(|point| point.candidates.len())
                .sum(),
            entry_transition_count: function
                .virtual_registers
                .iter()
                .map(|register| register.entry_transitions.len())
                .sum(),
        })
    }
}
