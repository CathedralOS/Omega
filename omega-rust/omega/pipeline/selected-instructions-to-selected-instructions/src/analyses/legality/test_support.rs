use super::model::StagedOptimizedAllocationLegality;

/// One substitutable field of [`StagedOptimizedAllocationLegalityCustodyReceipt`](super::StagedOptimizedAllocationLegalityCustodyReceipt). The custody matrix
/// substitutes exactly one field per leg so a rejection attributes to that
/// claim alone. Every field is representable in memory; the receipt has no
/// wire form, so no field is canonical-encoding-closed. The independent
/// checker is `validate_optimized_allocation_legality_custody`: it recomputes
/// the receipt from the staged legality evidence and rejects the
/// substitution.
#[derive(Debug, Clone, Copy)]
pub enum OptimizedAllocationLegalityCustodyFieldForTest {
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
    FunctionCount,
    VirtualRegisterCount,
    PointCount,
    CandidateCount,
    EntryTransitionCount,
}

impl StagedOptimizedAllocationLegality {
    /// Mutate only retained receipt facts; the donor grants no new authority.
    /// Flat fields take fixed alternates that cannot equal any honest value;
    /// `donor` is retained for signature uniformity with nested-receipt
    /// families and is intentionally unused here.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: OptimizedAllocationLegalityCustodyFieldForTest,
        _donor: &Self,
    ) {
        match field {
            OptimizedAllocationLegalityCustodyFieldForTest::Psi => {
                self.custody.psi.program_fingerprint =
                    terminal_psi::SemanticFingerprint::from_bytes([0xa1; 32]);
            }
            OptimizedAllocationLegalityCustodyFieldForTest::Target => {
                self.custody.target = match self.custody.target.architecture {
                    target::Architecture::X86_64 => target::NativeTarget::linux_arm64(),
                    _ => target::NativeTarget::linux_x64(),
                };
            }
            OptimizedAllocationLegalityCustodyFieldForTest::Entry => {
                self.custody.entry =
                    semantic_vocabulary::MachineId::new(0x5a14).expect("nonzero machine id");
            }
            OptimizedAllocationLegalityCustodyFieldForTest::Optimization => {
                self.custody.optimization =
                    optimization_core::OptimizationIdentityBundleIdentity::from_bytes([0xa2; 32]);
            }
            OptimizedAllocationLegalityCustodyFieldForTest::Projection => {
                self.custody.projection =
                    optimization_core::OptimizedAbstractPlanProjectionIdentity::from_bytes(
                        [0xa3; 32],
                    );
            }
            OptimizedAllocationLegalityCustodyFieldForTest::Manifest => {
                self.custody.manifest =
                    optimization_core::PrePhysicalOptimizationManifestIdentity::from_bytes(
                        [0xa4; 32],
                    );
            }
            OptimizedAllocationLegalityCustodyFieldForTest::OptimizationUnit => {
                self.custody.optimization_unit =
                    optimization_core::OptimizationUnitIdentity::from_bytes([0xa5; 32]);
            }
            OptimizedAllocationLegalityCustodyFieldForTest::FuelSchedule => {
                self.custody.fuel_schedule = semantic_vocabulary::FuelScheduleIdentity::new(0x5afe)
                    .expect("nonzero fuel schedule marker");
            }
            OptimizedAllocationLegalityCustodyFieldForTest::RegisterEnvironment => {
                self.custody.register_environment =
                    register_model::TargetRegisterEnvironmentIdentity::from_bytes([0xa6; 32]);
            }
            OptimizedAllocationLegalityCustodyFieldForTest::AllocatorAvailability => {
                self.custody.allocator_availability =
                    crate::AllocatorAvailabilityIdentity::from_bytes([0xb0; 32]);
            }
            OptimizedAllocationLegalityCustodyFieldForTest::Selected => {
                self.custody.selected =
                    selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0xa9; 32]);
            }
            OptimizedAllocationLegalityCustodyFieldForTest::Liveness => {
                self.custody.liveness =
                    selected_instructions::LivenessIdentity::from_bytes([0xaa; 32]);
            }
            OptimizedAllocationLegalityCustodyFieldForTest::Ranges => {
                self.custody.ranges =
                    selected_instructions::LiveRangeIdentity::from_bytes([0xab; 32]);
            }
            OptimizedAllocationLegalityCustodyFieldForTest::Legality => {
                self.custody.legality = crate::AllocationLegalityIdentity::from_bytes([0xb1; 32]);
            }
            OptimizedAllocationLegalityCustodyFieldForTest::FunctionCount => {
                self.custody.function_count += 1;
            }
            OptimizedAllocationLegalityCustodyFieldForTest::VirtualRegisterCount => {
                self.custody.virtual_register_count += 1;
            }
            OptimizedAllocationLegalityCustodyFieldForTest::PointCount => {
                self.custody.point_count += 1;
            }
            OptimizedAllocationLegalityCustodyFieldForTest::CandidateCount => {
                self.custody.candidate_count += 1;
            }
            OptimizedAllocationLegalityCustodyFieldForTest::EntryTransitionCount => {
                self.custody.entry_transition_count += 1;
            }
        }
    }
}
