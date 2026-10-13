//! Authored build behavior exclusions enforced at product admission.
//!
//! The product's evaluated exclusions and provider selections are applied to
//! borrowed Terminal semantics before optimization. Source attribution is an
//! independent diagnostic companion; no checked tree or lowering is accepted.
//! Entry products and callback thunks use this same admission operation.
//!
//! A violated exclusion and an incomplete evidence account are different
//! rejections: a witnessed prohibited site names possible behavior this
//! composition retains, while an evidence gap says the retained module cannot
//! certify absence at that site. Both fail admission; neither is silently
//! converted into the other.

mod selections;
use diagnostics::Diagnostic;
use effects::{AuthoredBehaviorExclusion, authored_behavior_exclusion_set_in};
use effects::{BehaviorExclusionReport, BehaviorExclusionVerdict, establish_behavior_exclusions};
pub use selections::harvest_behavior_exclusions;
use terminal_psi::TerminalMachineProvenance;

/// Admit the unoptimized composition under the authored product policy.
pub fn verify_entry_behavior_exclusions(
    authored: &[AuthoredBehaviorExclusion],
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    profile: target::TargetProfile,
    module: &terminal_psi::TerminalModule,
    entry: semantic_vocabulary::MachineId,
    provenance: &TerminalMachineProvenance,
) -> Result<(), Vec<Diagnostic>> {
    if authored.is_empty() {
        return Ok(());
    }
    let exclusions = authored_behavior_exclusion_set_in(authored, module);
    if exclusions.is_empty() {
        // Every authored row named a service this composition never
        // declares: nothing in it can invoke that service.
        return Ok(());
    }
    // This is admission of a selected execution, not a rewrite of the
    // portable root or a demand on every retained sibling's private body.
    let report = establish_behavior_exclusions(
        module,
        &[entry],
        &exclusions,
        selected_provider_plans,
        profile.target_name(),
    );
    match report.verdict() {
        BehaviorExclusionVerdict::Satisfied => Ok(()),
        _ => Err(behavior_exclusion_diagnostics(
            authored, module, provenance, &report,
        )),
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
