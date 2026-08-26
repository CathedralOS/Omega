use omega_regalloc::{
    TerminalAllocationLegalityError, TerminalLiveRangeError, TerminalLivenessError,
    ValidatedTerminalAllocationLegality, ValidatedTerminalLiveRanges, ValidatedTerminalLiveness,
    analyze_terminal_allocation_legality, analyze_terminal_live_ranges, analyze_terminal_liveness,
    validate_terminal_allocation_legality, validate_terminal_live_ranges,
    validate_terminal_liveness,
};

use crate::{
    OptimizedFixedViewCopyCustodyError, StagedOptimizedFixedViewCopies,
    StagedOptimizedFixedViewCopyCustodyReceipt, validate_optimized_fixed_view_copy_custody,
};

/// Complete mandatory reanalysis of one independently validated transformed
/// selected CFG. No source analysis fact is reused after the rewrite.
#[derive(Debug)]
pub struct StagedOptimizedSelectedReanalysis {
    transformation: StagedOptimizedFixedViewCopies,
    liveness: ValidatedTerminalLiveness,
    ranges: ValidatedTerminalLiveRanges,
    legality: ValidatedTerminalAllocationLegality,
    custody: StagedOptimizedSelectedReanalysisCustodyReceipt,
}

impl StagedOptimizedSelectedReanalysis {
    pub const fn transformation_stage(&self) -> &StagedOptimizedFixedViewCopies {
        &self.transformation
    }
    pub const fn liveness(&self) -> &ValidatedTerminalLiveness {
        &self.liveness
    }
    pub const fn ranges(&self) -> &ValidatedTerminalLiveRanges {
        &self.ranges
    }
    pub const fn legality(&self) -> &ValidatedTerminalAllocationLegality {
        &self.legality
    }
    pub const fn custody(&self) -> StagedOptimizedSelectedReanalysisCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedSelectedReanalysisCustodyReceipt {
    source: StagedOptimizedFixedViewCopyCustodyReceipt,
    transformed_liveness: omega_regalloc::TerminalLivenessIdentity,
    transformed_ranges: omega_regalloc::TerminalLiveRangeIdentity,
    transformed_legality: omega_regalloc::TerminalAllocationLegalityIdentity,
    allocator_availability: omega_regalloc::TerminalAllocatorAvailabilityIdentity,
    function_count: usize,
    virtual_register_count: usize,
    entry_transition_count: usize,
}

impl StagedOptimizedSelectedReanalysisCustodyReceipt {
    pub const fn source(self) -> StagedOptimizedFixedViewCopyCustodyReceipt {
        self.source
    }
    pub const fn transformed_liveness(self) -> omega_regalloc::TerminalLivenessIdentity {
        self.transformed_liveness
    }
    pub const fn transformed_ranges(self) -> omega_regalloc::TerminalLiveRangeIdentity {
        self.transformed_ranges
    }
    pub const fn transformed_legality(self) -> omega_regalloc::TerminalAllocationLegalityIdentity {
        self.transformed_legality
    }
    pub const fn allocator_availability(
        self,
    ) -> omega_regalloc::TerminalAllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }
    pub const fn entry_transition_count(self) -> usize {
        self.entry_transition_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedSelectedReanalysisError {
    UpstreamTransformation(OptimizedFixedViewCopyCustodyError),
    Liveness(TerminalLivenessError),
    LivenessRevalidation(TerminalLivenessError),
    LiveRanges(TerminalLiveRangeError),
    LiveRangeRevalidation(TerminalLiveRangeError),
    AllocationLegality(TerminalAllocationLegalityError),
    AllocationLegalityRevalidation(TerminalAllocationLegalityError),
    RemainingTransitions { count: usize },
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedSelectedReanalysisError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized transformed-selected reanalysis failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedSelectedReanalysisError {}

pub fn stage_optimized_selected_reanalysis(
    transformation: StagedOptimizedFixedViewCopies,
) -> Result<StagedOptimizedSelectedReanalysis, OptimizedSelectedReanalysisError> {
    let source = validate_optimized_fixed_view_copy_custody(
        transformation.source_legality_stage(),
        transformation.copies(),
    )
    .map_err(OptimizedSelectedReanalysisError::UpstreamTransformation)?;
    let copies = transformation.copies();
    let liveness =
        analyze_terminal_liveness(copies).map_err(OptimizedSelectedReanalysisError::Liveness)?;
    let replayed_liveness = validate_terminal_liveness(copies, liveness.plan().clone())
        .map_err(OptimizedSelectedReanalysisError::LivenessRevalidation)?;
    if replayed_liveness.receipt() != liveness.receipt() {
        return Err(OptimizedSelectedReanalysisError::ReceiptMismatch);
    }
    let ranges = analyze_terminal_live_ranges(copies, &liveness)
        .map_err(OptimizedSelectedReanalysisError::LiveRanges)?;
    let replayed_ranges = validate_terminal_live_ranges(copies, &liveness, ranges.plan().clone())
        .map_err(OptimizedSelectedReanalysisError::LiveRangeRevalidation)?;
    if replayed_ranges.receipt() != ranges.receipt() {
        return Err(OptimizedSelectedReanalysisError::ReceiptMismatch);
    }
    let environment = transformation
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let availability = transformation
        .source_legality_stage()
        .allocator_availability();
    let legality = analyze_terminal_allocation_legality(
        &ranges,
        availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedSelectedReanalysisError::AllocationLegality)?;
    let replayed_legality = validate_terminal_allocation_legality(
        &ranges,
        availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        legality.plan().clone(),
    )
    .map_err(OptimizedSelectedReanalysisError::AllocationLegalityRevalidation)?;
    if replayed_legality.receipt() != legality.receipt() {
        return Err(OptimizedSelectedReanalysisError::ReceiptMismatch);
    }
    require_no_transitions(&legality)?;
    let custody = custody_receipt(source, &liveness, &ranges, &legality);
    Ok(StagedOptimizedSelectedReanalysis {
        transformation,
        liveness,
        ranges,
        legality,
        custody,
    })
}

pub fn validate_optimized_selected_reanalysis_custody(
    transformation: &StagedOptimizedFixedViewCopies,
    liveness: &ValidatedTerminalLiveness,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
) -> Result<StagedOptimizedSelectedReanalysisCustodyReceipt, OptimizedSelectedReanalysisError> {
    let source = validate_optimized_fixed_view_copy_custody(
        transformation.source_legality_stage(),
        transformation.copies(),
    )
    .map_err(OptimizedSelectedReanalysisError::UpstreamTransformation)?;
    let replayed_liveness =
        validate_terminal_liveness(transformation.copies(), liveness.plan().clone())
            .map_err(OptimizedSelectedReanalysisError::LivenessRevalidation)?;
    if replayed_liveness.receipt() != liveness.receipt() {
        return Err(OptimizedSelectedReanalysisError::ReceiptMismatch);
    }
    let replayed_ranges =
        validate_terminal_live_ranges(transformation.copies(), liveness, ranges.plan().clone())
            .map_err(OptimizedSelectedReanalysisError::LiveRangeRevalidation)?;
    if replayed_ranges.receipt() != ranges.receipt() {
        return Err(OptimizedSelectedReanalysisError::ReceiptMismatch);
    }
    let environment = transformation
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let availability = transformation
        .source_legality_stage()
        .allocator_availability();
    let replayed_legality = validate_terminal_allocation_legality(
        ranges,
        availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        legality.plan().clone(),
    )
    .map_err(OptimizedSelectedReanalysisError::AllocationLegalityRevalidation)?;
    if replayed_legality.receipt() != legality.receipt() {
        return Err(OptimizedSelectedReanalysisError::ReceiptMismatch);
    }
    require_no_transitions(&replayed_legality)?;
    Ok(custody_receipt(
        source,
        &replayed_liveness,
        &replayed_ranges,
        &replayed_legality,
    ))
}

fn require_no_transitions(
    legality: &ValidatedTerminalAllocationLegality,
) -> Result<(), OptimizedSelectedReanalysisError> {
    let count = legality.receipt().entry_transition_count();
    if count != 0 {
        return Err(OptimizedSelectedReanalysisError::RemainingTransitions { count });
    }
    Ok(())
}

fn custody_receipt(
    source: StagedOptimizedFixedViewCopyCustodyReceipt,
    liveness: &ValidatedTerminalLiveness,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
) -> StagedOptimizedSelectedReanalysisCustodyReceipt {
    StagedOptimizedSelectedReanalysisCustodyReceipt {
        source,
        transformed_liveness: liveness.receipt().identity(),
        transformed_ranges: ranges.receipt().identity(),
        transformed_legality: legality.receipt().identity(),
        allocator_availability: legality.receipt().allocator_availability(),
        function_count: legality.receipt().function_count(),
        virtual_register_count: legality.receipt().virtual_register_count(),
        entry_transition_count: legality.receipt().entry_transition_count(),
    }
}
