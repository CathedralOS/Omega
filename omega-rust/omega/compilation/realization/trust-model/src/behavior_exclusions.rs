//! Authored build behavior exclusions enforced at product admission.
//!
//! `Build::exclude_crash` selections arrive on the checked compilation with
//! their declaration spans intact; the contract they select is verified here,
//! against the same unoptimized Terminal composition the artifact will
//! publish, before either product route admits it. The walk replays the
//! selected entry's own lowering rather than the produced artifact because
//! optional optimization must never decide admissibility: an exclusion that
//! only survives after dead-code removal or folding is not established.
//!
//! Entry products and callback thunks use the same admission operation with
//! their own retained unoptimized lowering and authored entry symbol.
//!
//! A violated exclusion and an incomplete evidence account are different
//! rejections: a witnessed prohibited site names possible behavior this
//! composition retains, while an evidence gap says the retained module cannot
//! certify absence at that site. Both fail admission; neither is silently
//! converted into the other.

use checked_trees::CheckedTrees;

mod selections;
use checked_trees_to_lowered_psi::LoweredMachineProvenance;
use diagnostics::Diagnostic;
use effects::{AuthoredBehaviorExclusion, authored_behavior_exclusion_set_in};
use effects::{
    BehaviorExclusionReport, BehaviorExclusionVerdict, establish_behavior_exclusions_with_owners,
};
pub use selections::harvest_behavior_exclusions;

/// Verify the authored exclusions against the selected program entry's
/// unoptimized composition: the lowered module Terminal production retained
/// before its selected optimization ran, the same lowering the artifact was
/// published from. `unoptimized` is `None` only when production was not
/// asked to retain it. With authored exclusions that is a production-request
/// fault, not an admission; without exclusions no retained copy is needed.
pub fn verify_entry_behavior_exclusions(
    checked: &CheckedTrees,
    authored: &[AuthoredBehaviorExclusion],
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    unoptimized: Option<&lowered_psi::LoweredPsi>,
    entry_machine_symbol: symbols::SymbolHandle,
) -> Result<(), Vec<Diagnostic>> {
    if authored.is_empty() {
        return Ok(());
    }
    let lowered = unoptimized.ok_or_else(|| {
        vec![Diagnostic::error(
            "behavior-exclusion admission needs the unoptimized Terminal lowering retained by production",
        )]
    })?;
    let module = &lowered.semantic_module;
    let trait_identity = |symbol| effects::boundary_trait_identity(checked, symbol);
    let exclusions = authored_behavior_exclusion_set_in(authored, module, &trait_identity);
    if exclusions.is_empty() {
        // Every authored row named a service this composition never
        // declares: nothing in it can invoke that service.
        return Ok(());
    }
    let report = establish_behavior_exclusions_with_owners(
        module,
        &[module.entry],
        &exclusions,
        selected_provider_plans,
        &effects::BoundaryServiceOwners::from_declared_traits(checked, module),
    );
    match report.verdict() {
        BehaviorExclusionVerdict::Satisfied => Ok(()),
        _ => {
            let provenance =
                LoweredMachineProvenance::from_lowering(checked, lowered, entry_machine_symbol);
            Err(behavior_exclusion_diagnostics(
                authored,
                module,
                &provenance,
                &trait_identity,
                &report,
            ))
        }
    }
}

fn behavior_exclusion_diagnostics(
    authored: &[AuthoredBehaviorExclusion],
    module: &terminal_psi::TerminalModule,
    provenance: &LoweredMachineProvenance,
    trait_identity: &dyn Fn(symbols::SymbolHandle) -> Option<String>,
    report: &BehaviorExclusionReport,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::with_capacity(2 * (report.prohibited.len() + report.gaps.len()) + 1);
    diagnostics.push(Diagnostic::error(format!(
        "{} authored behavior exclusion(s) could not be satisfied",
        authored.len()
    )));
    for prohibited in &report.prohibited {
        let mut diagnostic = Diagnostic::error(format!(
            "behavior exclusion violated: {} is reachable in {} at {}",
            prohibited.exclusion.describe(module),
            provenance.label(prohibited.machine),
            prohibited.site.describe(module),
        ));
        // The rejection points at the authored `exclude_*` selection that
        // named the requirement, not an internal Terminal coordinate.
        if let Some(row) = authored
            .iter()
            .find(|row| row.resolve(module, trait_identity) == Some(prohibited.exclusion))
        {
            diagnostic = diagnostic.with_source_span(row.source_span);
        }
        diagnostics.push(diagnostic);
        // A second row attributes the retained possible behavior to the
        // declaration that contributes it: a possible path, not a witnessed
        // runtime execution.
        if let Some(span) = provenance.source_span(prohibited.machine) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "the selected composition retains {} through {} declared here",
                    prohibited.exclusion.describe(module),
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
    provenance: &LoweredMachineProvenance,
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
