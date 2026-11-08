// We enforce authored behavior exclusions (`TerminalBehaviorExclusions`) over
// the canonical Terminal Psi module and its retained semantic operations.
// Behavior exclusions are Omega product policy: Psi's production sequence never
// executes them, and we verify each root's entry, callbacks, and physical
// adapter in order against the closed provider specialization facts without
// reopening source trees.

use diagnostics::Diagnostic;
use effects::{AuthoredBehaviorExclusion, authored_behavior_exclusion_set_in};
use effects::{BehaviorExclusionReport, BehaviorExclusionVerdict, establish_behavior_exclusions};
use terminal_psi::TerminalMachineProvenance;

pub(super) struct TerminalBehaviorExclusions<'a> {
    authored: &'a [effects::AuthoredBehaviorExclusion],
    providers: &'a effects::SelectedProviderPlanFacts,
    native_sources:
        &'a Result<std::sync::Arc<backend_plan::NativeProviderSources>, Vec<Diagnostic>>,
    profile: target::TargetProfile,
}

impl<'a> TerminalBehaviorExclusions<'a> {
    pub(super) fn new(
        authored: &'a [effects::AuthoredBehaviorExclusion],
        providers: &'a effects::SelectedProviderPlanFacts,
        native_sources: &'a Result<
            std::sync::Arc<backend_plan::NativeProviderSources>,
            Vec<Diagnostic>,
        >,
        profile: target::TargetProfile,
    ) -> Option<Self> {
        (!authored.is_empty()).then_some(Self {
            authored,
            providers,
            native_sources,
            profile,
        })
    }
}

impl TerminalBehaviorExclusions<'_> {
    pub(super) fn admit(
        &self,
        module: &terminal_psi::TerminalModule,
        roots: &mut dyn Iterator<
            Item = Result<psi_compiler::TerminalAdmissionRoot, Vec<Diagnostic>>,
        >,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(first) = roots.next() else {
            return Ok(());
        };
        let first = first?;
        let exclusions = authored_behavior_exclusion_set_in(self.authored, module);
        if exclusions.is_empty() {
            for root in roots {
                root?;
            }
            return Ok(());
        }
        let sources = self.native_sources.as_ref().map_err(Clone::clone)?;
        let providers = backend_plan::close_checked_provider_specializations(
            self.providers.clone(),
            &sources.declarations,
            module,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        for root in std::iter::once(Ok(first)).chain(roots) {
            let root = root?;
            let report = establish_behavior_exclusions(
                module,
                &[root.machine],
                &exclusions,
                &providers,
                self.profile.target_name(),
            );
            if report.verdict() != BehaviorExclusionVerdict::Satisfied {
                return Err(behavior_exclusion_diagnostics(
                    self.authored,
                    module,
                    &root.provenance,
                    &report,
                ));
            }
        }
        Ok(())
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
        let row = authored.iter().find(|row| {
            effects::resolve_authored_behavior_exclusion(row, module) == Some(prohibited.exclusion)
        });
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
