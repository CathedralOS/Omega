use super::model::StagedOptimizedLiveRanges;

/// One substitutable field of [`StagedOptimizedLiveRangeCustodyReceipt`](super::StagedOptimizedLiveRangeCustodyReceipt). The custody matrix
/// substitutes exactly one field per leg so a rejection attributes to that
/// claim alone. Every field is representable in memory; the receipt has no
/// wire form, so no field is canonical-encoding-closed.
#[derive(Debug, Clone, Copy)]
pub enum OptimizedLiveRangeCustodyFieldForTest {
    Psi,
    Target,
    Entry,
    Optimization,
    Projection,
    Manifest,
    OptimizationUnit,
    FuelSchedule,
    RegisterEnvironment,
    Selected,
    Liveness,
    Ranges,
    FunctionCount,
    BlockCount,
    VirtualRegisterCount,
    VirtualOccurrenceCount,
    FixedConstraintCount,
    VirtualFragmentCount,
    ArchitecturalUnitCount,
    ArchitecturalActionCount,
    ArchitecturalFragmentCount,
    VirtualEdgeConnectorCount,
    ArchitecturalEdgeConnectorCount,
    InterferenceCount,
}

impl StagedOptimizedLiveRanges {
    /// Mutate only retained receipt facts; the donor grants no new authority.
    /// Flat fields take fixed alternates that cannot equal any honest value;
    /// `donor` is retained for signature uniformity with nested-receipt
    /// families and is intentionally unused here.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: OptimizedLiveRangeCustodyFieldForTest,
        _donor: &Self,
    ) {
        match field {
            OptimizedLiveRangeCustodyFieldForTest::Psi => {
                self.custody.psi.program_fingerprint =
                    terminal_psi::SemanticFingerprint::from_bytes([0xa1; 32]);
            }
            OptimizedLiveRangeCustodyFieldForTest::Target => {
                self.custody.target = match self.custody.target.architecture {
                    target::Architecture::X86_64 => target::NativeTarget::linux_arm64(),
                    _ => target::NativeTarget::linux_x64(),
                };
            }
            OptimizedLiveRangeCustodyFieldForTest::Entry => {
                self.custody.entry =
                    semantic_vocabulary::MachineId::new(0x5a13).expect("nonzero machine id");
            }
            OptimizedLiveRangeCustodyFieldForTest::Optimization => {
                self.custody.optimization =
                    optimization_core::OptimizationIdentityBundleIdentity::from_bytes([0xa2; 32]);
            }
            OptimizedLiveRangeCustodyFieldForTest::Projection => {
                self.custody.projection =
                    optimization_core::OptimizedAbstractPlanProjectionIdentity::from_bytes(
                        [0xa3; 32],
                    );
            }
            OptimizedLiveRangeCustodyFieldForTest::Manifest => {
                self.custody.manifest =
                    optimization_core::PrePhysicalOptimizationManifestIdentity::from_bytes(
                        [0xa4; 32],
                    );
            }
            OptimizedLiveRangeCustodyFieldForTest::OptimizationUnit => {
                self.custody.optimization_unit =
                    optimization_core::OptimizationUnitIdentity::from_bytes([0xa5; 32]);
            }
            OptimizedLiveRangeCustodyFieldForTest::FuelSchedule => {
                self.custody.fuel_schedule = semantic_vocabulary::FuelScheduleIdentity::new(0x5afe)
                    .expect("nonzero fuel schedule marker");
            }
            OptimizedLiveRangeCustodyFieldForTest::RegisterEnvironment => {
                self.custody.register_environment =
                    register_model::TargetRegisterEnvironmentIdentity::from_bytes([0xa6; 32]);
            }
            OptimizedLiveRangeCustodyFieldForTest::Selected => {
                self.custody.selected =
                    selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0xa9; 32]);
            }
            OptimizedLiveRangeCustodyFieldForTest::Liveness => {
                self.custody.liveness =
                    selected_instructions::LivenessIdentity::from_bytes([0xaa; 32]);
            }
            OptimizedLiveRangeCustodyFieldForTest::Ranges => {
                self.custody.ranges =
                    selected_instructions::LiveRangeIdentity::from_bytes([0xab; 32]);
            }
            OptimizedLiveRangeCustodyFieldForTest::FunctionCount => {
                self.custody.function_count += 1;
            }
            OptimizedLiveRangeCustodyFieldForTest::BlockCount => {
                self.custody.block_count += 1;
            }
            OptimizedLiveRangeCustodyFieldForTest::VirtualRegisterCount => {
                self.custody.virtual_register_count += 1;
            }
            OptimizedLiveRangeCustodyFieldForTest::VirtualOccurrenceCount => {
                self.custody.virtual_occurrence_count += 1;
            }
            OptimizedLiveRangeCustodyFieldForTest::FixedConstraintCount => {
                self.custody.fixed_constraint_count += 1;
            }
            OptimizedLiveRangeCustodyFieldForTest::VirtualFragmentCount => {
                self.custody.virtual_fragment_count += 1;
            }
            OptimizedLiveRangeCustodyFieldForTest::ArchitecturalUnitCount => {
                self.custody.architectural_unit_count += 1;
            }
            OptimizedLiveRangeCustodyFieldForTest::ArchitecturalActionCount => {
                self.custody.architectural_action_count += 1;
            }
            OptimizedLiveRangeCustodyFieldForTest::ArchitecturalFragmentCount => {
                self.custody.architectural_fragment_count += 1;
            }
            OptimizedLiveRangeCustodyFieldForTest::VirtualEdgeConnectorCount => {
                self.custody.virtual_edge_connector_count += 1;
            }
            OptimizedLiveRangeCustodyFieldForTest::ArchitecturalEdgeConnectorCount => {
                self.custody.architectural_edge_connector_count += 1;
            }
            OptimizedLiveRangeCustodyFieldForTest::InterferenceCount => {
                self.custody.interference_count += 1;
            }
        }
    }
}
