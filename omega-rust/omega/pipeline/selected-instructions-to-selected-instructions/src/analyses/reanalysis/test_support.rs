use super::model::StagedOptimizedSelectedReanalysis;

/// One substitutable field of [`StagedOptimizedSelectedReanalysisCustodyReceipt`](super::StagedOptimizedSelectedReanalysisCustodyReceipt). The custody matrix
/// substitutes exactly one field per leg so a rejection attributes to that
/// claim alone. `Source` takes the donor's authentic foreign fixed-view-copy
/// custody receipt; the remaining flat fields take fixed alternates that
/// cannot equal any honest value. Every field is representable in memory;
/// the receipt has no wire form, so no field is canonical-encoding-closed.
#[derive(Debug, Clone, Copy)]
pub enum OptimizedSelectedReanalysisCustodyFieldForTest {
    Source,
    TransformedLiveness,
    TransformedRanges,
    TransformedLegality,
    AllocatorAvailability,
    FunctionCount,
    VirtualRegisterCount,
    EntryTransitionCount,
}

impl StagedOptimizedSelectedReanalysis {
    /// Mutate only retained receipt facts; the donor's nested source receipt
    /// is authentic foreign evidence and grants no new authority here.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: OptimizedSelectedReanalysisCustodyFieldForTest,
        donor: &Self,
    ) {
        match field {
            OptimizedSelectedReanalysisCustodyFieldForTest::Source => {
                self.custody.source = donor.custody.source;
            }
            OptimizedSelectedReanalysisCustodyFieldForTest::TransformedLiveness => {
                self.custody.transformed_liveness =
                    selected_instructions::LivenessIdentity::from_bytes([0xaa; 32]);
            }
            OptimizedSelectedReanalysisCustodyFieldForTest::TransformedRanges => {
                self.custody.transformed_ranges =
                    selected_instructions::LiveRangeIdentity::from_bytes([0xab; 32]);
            }
            OptimizedSelectedReanalysisCustodyFieldForTest::TransformedLegality => {
                self.custody.transformed_legality =
                    crate::AllocationLegalityIdentity::from_bytes([0xb1; 32]);
            }
            OptimizedSelectedReanalysisCustodyFieldForTest::AllocatorAvailability => {
                self.custody.allocator_availability =
                    crate::AllocatorAvailabilityIdentity::from_bytes([0xb0; 32]);
            }
            OptimizedSelectedReanalysisCustodyFieldForTest::FunctionCount => {
                self.custody.function_count += 1;
            }
            OptimizedSelectedReanalysisCustodyFieldForTest::VirtualRegisterCount => {
                self.custody.virtual_register_count += 1;
            }
            OptimizedSelectedReanalysisCustodyFieldForTest::EntryTransitionCount => {
                self.custody.entry_transition_count += 1;
            }
        }
    }
}
