use super::model::StagedOptimizedSelectedInstructions;

/// One substitutable field of [`StagedOptimizedSelectionCustodyReceipt`](super::StagedOptimizedSelectionCustodyReceipt). The custody matrix
/// substitutes exactly one field per leg so a rejection attributes to that
/// claim alone. Every field is representable in memory; the receipt has no
/// wire form, so no field is canonical-encoding-closed. The independent
/// checker is `validate_optimized_selection_custody`: it recomputes the
/// receipt from the staged selection and rejects the substitution.
#[derive(Debug, Clone, Copy)]
pub enum OptimizedSelectionCustodyFieldForTest {
    Psi,
    Target,
    Entry,
    Optimization,
    Projection,
    Manifest,
    OptimizationUnit,
    FuelSchedule,
    RegisterEnvironment,
    Legalized,
    LegalizationValidator,
    Selected,
    FunctionCount,
}

impl StagedOptimizedSelectedInstructions {
    /// Mutate only retained receipt facts; the donor grants no new authority.
    /// Flat fields take fixed alternates that cannot equal any honest value;
    /// `donor` is retained for signature uniformity with nested-receipt
    /// families and is intentionally unused here.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: OptimizedSelectionCustodyFieldForTest,
        _donor: &Self,
    ) {
        match field {
            OptimizedSelectionCustodyFieldForTest::Psi => {
                self.custody.psi.program_fingerprint =
                    terminal_psi::SemanticFingerprint::from_bytes([0xa1; 32]);
            }
            OptimizedSelectionCustodyFieldForTest::Target => {
                self.custody.target = match self.custody.target.architecture {
                    target::Architecture::X86_64 => target::NativeTarget::linux_arm64(),
                    _ => target::NativeTarget::linux_x64(),
                };
            }
            OptimizedSelectionCustodyFieldForTest::Entry => {
                self.custody.entry =
                    semantic_vocabulary::MachineId::new(0x5a11).expect("nonzero machine id");
            }
            OptimizedSelectionCustodyFieldForTest::Optimization => {
                self.custody.optimization =
                    optimization_core::OptimizationIdentityBundleIdentity::from_bytes([0xa2; 32]);
            }
            OptimizedSelectionCustodyFieldForTest::Projection => {
                self.custody.projection =
                    optimization_core::OptimizedAbstractPlanProjectionIdentity::from_bytes(
                        [0xa3; 32],
                    );
            }
            OptimizedSelectionCustodyFieldForTest::Manifest => {
                self.custody.manifest =
                    optimization_core::PrePhysicalOptimizationManifestIdentity::from_bytes(
                        [0xa4; 32],
                    );
            }
            OptimizedSelectionCustodyFieldForTest::OptimizationUnit => {
                self.custody.optimization_unit =
                    optimization_core::OptimizationUnitIdentity::from_bytes([0xa5; 32]);
            }
            OptimizedSelectionCustodyFieldForTest::FuelSchedule => {
                self.custody.fuel_schedule = semantic_vocabulary::FuelScheduleIdentity::new(0x5afe)
                    .expect("nonzero fuel schedule marker");
            }
            OptimizedSelectionCustodyFieldForTest::RegisterEnvironment => {
                self.custody.register_environment =
                    register_model::TargetRegisterEnvironmentIdentity::from_bytes([0xa6; 32]);
            }
            OptimizedSelectionCustodyFieldForTest::Legalized => {
                self.custody.legalized =
                    legalized_operations::LegalizedOperationPlanIdentity::from_bytes([0xa7; 32]);
            }
            OptimizedSelectionCustodyFieldForTest::LegalizationValidator => {
                self.custody.legalization_validator =
                    optimization_core::OptimizationValidatorIdentity::from_bytes([0xa8; 32]);
            }
            OptimizedSelectionCustodyFieldForTest::Selected => {
                self.custody.selected =
                    selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0xa9; 32]);
            }
            OptimizedSelectionCustodyFieldForTest::FunctionCount => {
                self.custody.function_count += 1;
            }
        }
    }
}
