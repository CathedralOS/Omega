//! The product's behavior-exclusion policy for Psi-owned Terminal production.
//!
//! Behavior exclusions are Omega product policy: authored exclusions, the
//! selected provider plan facts and the target profile. Psi's production
//! sequence borrows this policy as an ordinary composition admission and never
//! retains a product type.

use diagnostics::Diagnostic;
use effects::{AuthoredBehaviorExclusion, authored_behavior_exclusion_set_in};
use effects::{BehaviorExclusionReport, BehaviorExclusionVerdict, establish_behavior_exclusions};
use terminal_psi::TerminalMachineProvenance;

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
        let exclusions = authored_behavior_exclusion_set_in(self.authored, module);
        if exclusions.is_empty() {
            // Every authored row named a service this composition never
            // declares: nothing in it can invoke that service.
            return Ok(());
        }
        // Admit this selected execution, not every retained target sibling.
        // Prohibited possible behavior and incomplete evidence both refuse;
        // neither is a claim that a runtime execution was observed.
        let report = establish_behavior_exclusions(
            module,
            &[entry],
            &exclusions,
            self.providers,
            self.profile.target_name(),
        );
        match report.verdict() {
            BehaviorExclusionVerdict::Satisfied => Ok(()),
            _ => Err(behavior_exclusion_diagnostics(
                self.authored,
                module,
                provenance,
                &report,
            )),
        }
    }
}

fn behavior_exclusion_diagnostics(
    authored: &[AuthoredBehaviorExclusion],
    module: &terminal_psi::TerminalModule,
    provenance: &TerminalMachineProvenance,
    report: &BehaviorExclusionReport,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::with_capacity(2 * (report.prohibited.len() + report.gaps.len()) + 1);
    diagnostics.push(Diagnostic::error(format!(
        "{} authored behavior exclusion(s) could not be satisfied",
        authored.len()
    )));
    for prohibited in &report.prohibited {
        // The rejection points at the authored `exclude_*` selection that
        // named the requirement, not an internal Terminal coordinate, and
        // names it as authored rather than by its hermetic identity.
        let row = authored
            .iter()
            .find(|row| row.resolve(module) == Some(prohibited.exclusion));
        let excluded = row
            .and_then(AuthoredBehaviorExclusion::authored_label)
            .unwrap_or_else(|| prohibited.exclusion.describe(module));
        let mut diagnostic = Diagnostic::error(format!(
            "behavior exclusion violated: {excluded} is reachable in {} at {}",
            provenance.label(prohibited.machine),
            prohibited.site.describe(module),
        ));
        if let Some(row) = row {
            diagnostic = diagnostic.with_source_span(row.source_span);
        }
        diagnostics.push(diagnostic);
        // A second row attributes the retained possible behavior to the
        // declaration that contributes it: a possible path, not a witnessed
        // runtime execution.
        if let Some(span) = provenance.source_span(prohibited.machine) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "the selected composition retains {excluded} through {} declared here",
                    provenance.label(prohibited.machine),
                ))
                .with_source_span(span),
            );
        }
    }
    for gap in &report.gaps {
        let mut diagnostic = Diagnostic::error(format!(
            "behavior exclusion evidence is insufficient: {} at {}",
            gap.kind.describe(),
            describe_location(provenance, gap.machine, gap.block, gap.operation),
        ));
        if let Some(first) = authored.first() {
            diagnostic = diagnostic.with_source_span(first.source_span);
        }
        diagnostics.push(diagnostic);
        if let Some(span) = provenance.source_span(gap.machine) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "absence cannot be established for {} declared here",
                    provenance.label(gap.machine),
                ))
                .with_source_span(span),
            );
        }
    }
    diagnostics
}

fn describe_location(
    provenance: &TerminalMachineProvenance,
    machine: semantic_vocabulary::MachineId,
    block: Option<semantic_vocabulary::BlockId>,
    operation: Option<semantic_vocabulary::OperationId>,
) -> String {
    let mut location = provenance.label(machine);
    if let Some(block) = block {
        location.push_str(&format!(", block {block}"));
    }
    if let Some(operation) = operation {
        location.push_str(&format!(", operation {operation}"));
    }
    location
}
