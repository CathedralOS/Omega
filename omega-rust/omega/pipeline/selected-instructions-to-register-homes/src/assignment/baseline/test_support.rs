use super::model::{
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterFixedViewCopies,
};

/// One substitutable field of [`StagedOptimizedRegisterHomeCustodyReceipt`](super::StagedOptimizedRegisterHomeCustodyReceipt). The custody matrix
/// substitutes exactly one field per leg so a rejection attributes to that
/// claim alone. Every field is representable in memory; the receipt has no
/// wire form, so no field is canonical-encoding-closed.
#[derive(Debug, Clone, Copy)]
pub enum OptimizedRegisterHomeCustodyFieldForTest {
    Psi,
    Target,
    Entry,
    Optimization,
    Projection,
    Manifest,
    OptimizationUnit,
    FuelSchedule,
    RegisterEnvironment,
    AllocatorAvailability,
    Selected,
    Liveness,
    Ranges,
    Legality,
    Homes,
    PostAllocationManifest,
    FunctionCount,
    AssignmentCount,
}

impl StagedOptimizedRegisterHomes {
    /// Mutate only retained receipt facts; the donor grants no new authority.
    /// Flat fields take fixed alternates that cannot equal any honest value;
    /// `donor` is retained for signature uniformity with nested-receipt
    /// families and is intentionally unused here.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: OptimizedRegisterHomeCustodyFieldForTest,
        _donor: &Self,
    ) {
        match field {
            OptimizedRegisterHomeCustodyFieldForTest::Psi => {
                self.custody.psi.program_fingerprint =
                    terminal_psi::SemanticFingerprint::from_bytes([0xa1; 32]);
            }
            OptimizedRegisterHomeCustodyFieldForTest::Target => {
                self.custody.target = match self.custody.target.architecture {
                    target::Architecture::X86_64 => target::NativeTarget::linux_arm64(),
                    _ => target::NativeTarget::linux_x64(),
                };
            }
            OptimizedRegisterHomeCustodyFieldForTest::Entry => {
                self.custody.entry =
                    semantic_vocabulary::MachineId::new(0x5a15).expect("nonzero machine id");
            }
            OptimizedRegisterHomeCustodyFieldForTest::Optimization => {
                self.custody.optimization =
                    optimization_core::OptimizationIdentityBundleIdentity::from_bytes([0xa2; 32]);
            }
            OptimizedRegisterHomeCustodyFieldForTest::Projection => {
                self.custody.projection =
                    optimization_core::OptimizedAbstractPlanProjectionIdentity::from_bytes(
                        [0xa3; 32],
                    );
            }
            OptimizedRegisterHomeCustodyFieldForTest::Manifest => {
                self.custody.manifest =
                    optimization_core::PrePhysicalOptimizationManifestIdentity::from_bytes(
                        [0xa4; 32],
                    );
            }
            OptimizedRegisterHomeCustodyFieldForTest::OptimizationUnit => {
                self.custody.optimization_unit =
                    optimization_core::OptimizationUnitIdentity::from_bytes([0xa5; 32]);
            }
            OptimizedRegisterHomeCustodyFieldForTest::FuelSchedule => {
                self.custody.fuel_schedule = semantic_vocabulary::FuelScheduleIdentity::new(0x5afe)
                    .expect("nonzero fuel schedule marker");
            }
            OptimizedRegisterHomeCustodyFieldForTest::RegisterEnvironment => {
                self.custody.register_environment =
                    register_model::TargetRegisterEnvironmentIdentity::from_bytes([0xa6; 32]);
            }
            OptimizedRegisterHomeCustodyFieldForTest::AllocatorAvailability => {
                self.custody.allocator_availability =
                    crate::AllocatorAvailabilityIdentity::from_bytes([0xb0; 32]);
            }
            OptimizedRegisterHomeCustodyFieldForTest::Selected => {
                self.custody.selected =
                    selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0xa9; 32]);
            }
            OptimizedRegisterHomeCustodyFieldForTest::Liveness => {
                self.custody.liveness =
                    selected_instructions::LivenessIdentity::from_bytes([0xaa; 32]);
            }
            OptimizedRegisterHomeCustodyFieldForTest::Ranges => {
                self.custody.ranges =
                    selected_instructions::LiveRangeIdentity::from_bytes([0xab; 32]);
            }
            OptimizedRegisterHomeCustodyFieldForTest::Legality => {
                self.custody.legality = crate::AllocationLegalityIdentity::from_bytes([0xb1; 32]);
            }
            OptimizedRegisterHomeCustodyFieldForTest::Homes => {
                self.custody.homes = crate::RegisterHomeIdentity::from_bytes([0xb2; 32]);
            }
            OptimizedRegisterHomeCustodyFieldForTest::PostAllocationManifest => {
                self.custody.post_allocation_manifest =
                    optimization_core::PostAllocationOptimizationManifestIdentity::from_bytes(
                        [0xb3; 32],
                    );
            }
            OptimizedRegisterHomeCustodyFieldForTest::FunctionCount => {
                self.custody.function_count += 1;
            }
            OptimizedRegisterHomeCustodyFieldForTest::AssignmentCount => {
                self.custody.assignment_count += 1;
            }
        }
    }
}

/// One substitutable field of [`StagedOptimizedPostCopyRegisterHomeCustodyReceipt`](super::StagedOptimizedPostCopyRegisterHomeCustodyReceipt). `Source` takes the donor's
/// authentic foreign reanalysis custody receipt; the remaining flat fields
/// take fixed alternates. Every field is representable in memory; the receipt
/// has no wire form, so no field is canonical-encoding-closed.
#[derive(Debug, Clone, Copy)]
pub enum OptimizedPostCopyRegisterHomeCustodyFieldForTest {
    Source,
    Homes,
    PostAllocationManifest,
    FunctionCount,
    AssignmentCount,
}

impl StagedOptimizedRegisterHomesAfterFixedViewCopies {
    /// Mutate only retained receipt facts; the donor's nested source receipt
    /// is authentic foreign evidence and grants no new authority here.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: OptimizedPostCopyRegisterHomeCustodyFieldForTest,
        donor: &Self,
    ) {
        match field {
            OptimizedPostCopyRegisterHomeCustodyFieldForTest::Source => {
                self.custody.source = donor.custody.source;
            }
            OptimizedPostCopyRegisterHomeCustodyFieldForTest::Homes => {
                self.custody.homes = crate::RegisterHomeIdentity::from_bytes([0xb2; 32]);
            }
            OptimizedPostCopyRegisterHomeCustodyFieldForTest::PostAllocationManifest => {
                self.custody.post_allocation_manifest =
                    optimization_core::PostAllocationOptimizationManifestIdentity::from_bytes(
                        [0xb3; 32],
                    );
            }
            OptimizedPostCopyRegisterHomeCustodyFieldForTest::FunctionCount => {
                self.custody.function_count += 1;
            }
            OptimizedPostCopyRegisterHomeCustodyFieldForTest::AssignmentCount => {
                self.custody.assignment_count += 1;
            }
        }
    }
}
