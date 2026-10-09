//! Product policy and reporting inputs for Psi-owned Terminal production.

use artifacts::compile_timings::CompileTimings;
pub use psi_terminal_production::ProducedTerminalArtifact;

/// Product admission reads the actual unoptimized composition, then releases
/// the borrow before optimization. No checked-source or LoweredPsi sidecar is
/// retained in the result.
pub struct TerminalProductionCustody<'a> {
    pub entry_identity: Option<[u8; 32]>,
    pub behavior_exclusions: Option<TerminalBehaviorExclusions<'a>>,
    pub timings: &'a mut CompileTimings,
}

impl<'a> TerminalProductionCustody<'a> {
    pub fn artifact_only(timings: &'a mut CompileTimings) -> Self {
        Self {
            entry_identity: None,
            behavior_exclusions: None,
            timings,
        }
    }
}

pub struct TerminalBehaviorExclusions<'a> {
    authored: &'a [effects::AuthoredBehaviorExclusion],
    providers: &'a effects::SelectedProviderPlanFacts,
}

impl<'a> TerminalBehaviorExclusions<'a> {
    pub fn new(
        authored: &'a [effects::AuthoredBehaviorExclusion],
        providers: &'a effects::SelectedProviderPlanFacts,
    ) -> Option<Self> {
        (!authored.is_empty()).then_some(Self {
            authored,
            providers,
        })
    }
}

impl psi_terminal_production::TerminalCompositionAdmission for TerminalBehaviorExclusions<'_> {
    fn admit(
        &self,
        module: &terminal_psi::TerminalModule,
        provenance: &terminal_psi::TerminalMachineProvenance,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        trust_model::verify_entry_behavior_exclusions(
            self.authored,
            self.providers,
            module,
            provenance,
        )
    }
}
