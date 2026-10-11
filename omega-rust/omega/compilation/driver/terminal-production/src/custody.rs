//! Product policy and reporting inputs for Psi-owned Terminal production.

use artifacts::compile_timings::CompileTimings;
pub use psi_compiler::ProducedTerminalArtifact;

/// Product admission reads the actual unoptimized composition, then releases
/// the borrow before optimization. No checked-source or LoweredPsi sidecar is
/// retained in the result.
pub struct TerminalProductionCustody<'a> {
    pub applications: &'a [TerminalApplicationCustody<'a>],
    pub timings: &'a mut CompileTimings,
}

/// A target's admission and entry correspondence, independent of shared Psi work.
pub struct TerminalApplicationCustody<'a> {
    pub entry_identity: Option<[u8; 32]>,
    pub entry_machine: Option<psi_compiler::TerminalMachineSelection<'a>>,
    pub behavior_exclusions: Option<TerminalBehaviorExclusions<'a>>,
}

impl<'a> TerminalProductionCustody<'a> {
    pub fn artifact_only(timings: &'a mut CompileTimings) -> Self {
        Self {
            applications: &[],
            timings,
        }
    }
}

pub struct TerminalBehaviorExclusions<'a> {
    authored: &'a [effects::AuthoredBehaviorExclusion],
    providers: &'a effects::SelectedProviderPlanFacts,
    profile: target::TargetProfile,
}

impl<'a> TerminalBehaviorExclusions<'a> {
    pub fn new(
        authored: &'a [effects::AuthoredBehaviorExclusion],
        providers: &'a effects::SelectedProviderPlanFacts,
        profile: target::TargetProfile,
    ) -> Option<Self> {
        (!authored.is_empty()).then_some(Self {
            authored,
            providers,
            profile,
        })
    }
}

impl psi_compiler::TerminalCompositionAdmission for TerminalBehaviorExclusions<'_> {
    fn admit(
        &self,
        module: &terminal_psi::TerminalModule,
        entry: semantic_vocabulary::MachineId,
        provenance: &terminal_psi::TerminalMachineProvenance,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        trust_model::verify_entry_behavior_exclusions(
            self.authored,
            self.providers,
            self.profile,
            module,
            entry,
            provenance,
        )
    }
}
