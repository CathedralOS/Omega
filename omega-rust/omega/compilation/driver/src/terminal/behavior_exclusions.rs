//! The product's behavior-exclusion policy for Psi-owned Terminal production.
//!
//! Behavior exclusions are Omega product policy: authored exclusions, the
//! selected provider plan facts and the target profile. Psi's production
//! sequence borrows this policy as an ordinary composition admission and never
//! retains a product type.

use diagnostics::Diagnostic;

/// Product admission reads the actual unoptimized composition, then releases
/// the borrow before optimization. No checked-source or LoweredPsi sidecar is
/// retained in the result.
///
/// A product caller lends this policy as the `admission` of the
/// [`psi_compiler::TerminalApplicationCustody`] row it submits to
/// [`psi_compiler::TerminalProductionRequest::produce`]; the caller owns the policy value for
/// as long as production borrows it.
pub(super) struct TerminalBehaviorExclusions<'a> {
    authored: &'a [effects::AuthoredBehaviorExclusion],
    providers: &'a effects::SelectedProviderPlanFacts,
    profile: target::TargetProfile,
}

impl<'a> TerminalBehaviorExclusions<'a> {
    pub(super) fn new(
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
    ) -> Result<(), Vec<Diagnostic>> {
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
