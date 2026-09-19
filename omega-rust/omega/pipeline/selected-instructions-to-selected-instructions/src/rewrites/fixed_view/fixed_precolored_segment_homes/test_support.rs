use super::model::StagedOptimizedFixedPrecoloredSegmentHomes;

/// One substitutable field of [`StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt`](super::StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt).
/// The custody matrix substitutes exactly one field per leg so a rejection
/// attributes to that claim alone. Every field is a nested authentic receipt,
/// so each leg borrows the foreign donor's corresponding receipt rather than
/// fabricating one. Every field is representable in memory; the receipt has
/// no wire form, so no field is canonical-encoding-closed.
#[derive(Debug, Clone, Copy)]
pub enum OptimizedFixedPrecoloredSegmentHomeCustodyFieldForTest {
    Upstream,
    Fixed,
    Requirements,
    Homes,
}

impl StagedOptimizedFixedPrecoloredSegmentHomes {
    /// Mutate only retained receipt facts; the donor's nested receipts are
    /// authentic foreign evidence and grant no new authority here.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: OptimizedFixedPrecoloredSegmentHomeCustodyFieldForTest,
        donor: &Self,
    ) {
        match field {
            OptimizedFixedPrecoloredSegmentHomeCustodyFieldForTest::Upstream => {
                self.custody.upstream = donor.custody.upstream;
            }
            OptimizedFixedPrecoloredSegmentHomeCustodyFieldForTest::Fixed => {
                self.custody.fixed = donor.custody.fixed;
            }
            OptimizedFixedPrecoloredSegmentHomeCustodyFieldForTest::Requirements => {
                self.custody.requirements = donor.custody.requirements;
            }
            OptimizedFixedPrecoloredSegmentHomeCustodyFieldForTest::Homes => {
                self.custody.homes = donor.custody.homes;
            }
        }
    }
}
