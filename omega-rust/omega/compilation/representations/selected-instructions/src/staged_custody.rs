//! The custody core every staged selected-program receipt carries.
//!
//! Instruction selection mints it, and selected optimization and register
//! allocation copy it whole into their own receipts, so it is shared
//! vocabulary of those stages rather than any one stage's output.

/// The nine pipeline-custody identities every staged optimized receipt
/// re-proves — terminal psi, target, entry, optimization bundle, projection,
/// manifest, optimization unit, fuel schedule, and the validated register
/// environment. Downstream stages embed this record rather than repeating
/// its fields, so a custody mint copies `upstream.core()` whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedCustodyCore {
    /// Terminal Psi identity of the staged upstream result.
    pub psi: terminal_psi::TerminalPsiIdentity,
    /// Native target of the staged upstream result.
    pub target: target::NativeTarget,
    /// Entry machine the staged result lowers.
    pub entry: semantic_vocabulary::MachineId,
    /// Optimization identity bundle of the staged result.
    pub optimization: optimization_core::OptimizationIdentityBundleIdentity,
    /// Optimized-plan projection identity of the staged result.
    pub projection: optimization_core::OptimizedAbstractPlanProjectionIdentity,
    /// Pre-physical manifest identity of the staged result.
    pub manifest: optimization_core::PrePhysicalOptimizationManifestIdentity,
    /// Optimization-unit identity of the staged result.
    pub optimization_unit: optimization_core::OptimizationUnitIdentity,
    /// Fuel-schedule identity of the staged result.
    pub fuel_schedule: semantic_vocabulary::FuelScheduleIdentity,
    /// Validated target-register-environment identity of the staged result.
    pub register_environment: register_model::TargetRegisterEnvironmentIdentity,
}

/// Minted by `staging_custody_core_accessors!`: the identity getters every
/// staged receipt shares, projecting out of the embedded
/// [`StagedOptimizedCustodyCore`].
#[macro_export]
macro_rules! staging_custody_core_accessors {
    () => {
        /// Terminal Psi identity of the staged upstream result.
        pub const fn psi(self) -> TerminalPsiIdentity {
            self.core.psi
        }
        /// Native target of the staged upstream result.
        pub const fn target(self) -> target::NativeTarget {
            self.core.target
        }
        /// Entry machine the staged result lowers.
        pub const fn entry(self) -> MachineId {
            self.core.entry
        }
        /// Optimization identity bundle of the staged result.
        pub const fn optimization(self) -> OptimizationIdentityBundleIdentity {
            self.core.optimization
        }
        /// Optimized-plan projection identity of the staged result.
        pub const fn projection(self) -> OptimizedAbstractPlanProjectionIdentity {
            self.core.projection
        }
        /// Pre-physical manifest identity of the staged result.
        pub const fn manifest(self) -> PrePhysicalOptimizationManifestIdentity {
            self.core.manifest
        }
        /// Optimization-unit identity of the staged result.
        pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
            self.core.optimization_unit
        }
        /// Fuel-schedule identity of the staged result.
        pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
            self.core.fuel_schedule
        }
        /// Validated target-register-environment identity of the staged result.
        pub const fn register_environment(
            self,
        ) -> register_model::TargetRegisterEnvironmentIdentity {
            self.core.register_environment
        }
        /// The nine re-proven upstream identities, as one record.
        pub const fn core(self) -> $crate::StagedOptimizedCustodyCore {
            self.core
        }
    };
}
