//! Authored build behavior exclusions enforced at product admission.
//!
//! `builder.exclude_crash` selections arrive on the checked compilation with
//! their declaration spans intact; the contract they select is verified here,
//! against the same unoptimized Terminal composition the artifact will
//! publish, before either product route admits it. The walk replays the
//! selected entry's own lowering rather than the produced artifact because
//! optional optimization must never decide admissibility: an exclusion that
//! only survives after dead-code removal or folding is not established.
//!
//! Callback thunk modules are checked at their own production site, where the
//! unoptimized callback lowering is already in hand. Both paths feed one
//! report-mapping helper so prohibited sites and evidence gaps read the same
//! for every admitted composition.
//!
//! A violated exclusion and an incomplete evidence account are different
//! rejections: a witnessed prohibited site names possible behavior this
//! composition retains, while an evidence gap says the retained module cannot
//! certify absence at that site. Both fail admission; neither is silently
//! converted into the other.

use assembled_syntax_to_checked_compilation::CheckedCompilation;
use build_evaluation::{
    AuthoredBehaviorExclusion, BehaviorExclusionReport, BehaviorExclusionVerdict, EvidenceGapKind,
    ProhibitedSite, authored_behavior_exclusion_set, establish_behavior_exclusions,
};
use diagnostics::Diagnostic;

/// Re-lower the selected program entry without optional optimization and
/// verify the authored exclusions against that exact composition.
pub(crate) fn verify_entry_behavior_exclusions(
    checked: &CheckedCompilation,
    entry_machine_symbol: symbols::SymbolHandle,
) -> Result<(), Vec<Diagnostic>> {
    if checked.behavior_exclusions().is_empty() {
        return Ok(());
    }
    let lowered = checked_trees_to_lowered_psi::lower_machine_by_symbol(
        checked,
        entry_machine_symbol,
    )
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "behavior-exclusion admission could not replay the unoptimized Terminal lowering: {error}"
        ))]
    })?;
    verify_module_behavior_exclusions(checked, &lowered.semantic_module)
}

/// Verify the authored exclusions against one already-unoptimized Terminal
/// module (a selected entry composition or a callback thunk body). The
/// module's own `entry` is the walk's root.
pub(crate) fn verify_module_behavior_exclusions(
    checked: &CheckedCompilation,
    module: &terminal_psi::TerminalModule,
) -> Result<(), Vec<Diagnostic>> {
    let authored = checked.behavior_exclusions();
    if authored.is_empty() {
        return Ok(());
    }
    let exclusions = authored_behavior_exclusion_set(authored);
    let report = establish_behavior_exclusions(
        module,
        &[module.entry],
        &exclusions,
        checked.selected_provider_plans(),
    );
    match report.verdict() {
        BehaviorExclusionVerdict::Satisfied => Ok(()),
        _ => Err(behavior_exclusion_diagnostics(authored, module, &report)),
    }
}

fn behavior_exclusion_diagnostics(
    authored: &[AuthoredBehaviorExclusion],
    module: &terminal_psi::TerminalModule,
    report: &BehaviorExclusionReport,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::with_capacity(report.prohibited.len() + report.gaps.len() + 1);
    diagnostics.push(Diagnostic::error(format!(
        "{} authored behavior exclusion(s) could not be satisfied",
        authored.len()
    )));
    for prohibited in &report.prohibited {
        let mut diagnostic = Diagnostic::error(format!(
            "behavior exclusion violated: {} is reachable in Terminal machine {} at {}",
            describe_exclusion(prohibited.exclusion),
            prohibited.machine,
            describe_site(module, &prohibited.site),
        ));
        // The rejection points at the authored `exclude_*` selection that
        // named the requirement, not an internal Terminal coordinate.
        if let Some(row) = authored
            .iter()
            .find(|row| row.exclusion() == prohibited.exclusion)
        {
            diagnostic = diagnostic.with_source_span(row.source_span);
        }
        diagnostics.push(diagnostic);
    }
    for gap in &report.gaps {
        let mut diagnostic = Diagnostic::error(format!(
            "behavior exclusion evidence is insufficient: {} at {}",
            describe_gap(&gap.kind),
            describe_location(gap.machine, gap.block, gap.operation),
        ));
        if let Some(first) = authored.first() {
            diagnostic = diagnostic.with_source_span(first.source_span);
        }
        diagnostics.push(diagnostic);
    }
    diagnostics
}

fn describe_exclusion(exclusion: build_evaluation::BehaviorExclusion) -> String {
    match exclusion {
        build_evaluation::BehaviorExclusion::CrashCause(cause) => {
            format!("crash cause {cause:?}")
        }
        build_evaluation::BehaviorExclusion::Service(service) => {
            format!("service {service}")
        }
    }
}

fn describe_site(module: &terminal_psi::TerminalModule, site: &ProhibitedSite) -> String {
    match site {
        ProhibitedSite::CrashTerminator { block } => {
            format!("crash terminator in block {block}")
        }
        ProhibitedSite::BoundaryCall {
            block,
            operation,
            boundary,
        } => {
            let identity = module
                .boundary_machines
                .iter()
                .find(|declaration| declaration.id == *boundary)
                .map(|declaration| declaration.identity.as_str())
                .unwrap_or("<undeclared boundary>");
            format!("call to boundary `{identity}` (block {block}, operation {operation})")
        }
        ProhibitedSite::PortWrite { block, operation } => {
            format!("port write (block {block}, operation {operation})")
        }
    }
}

fn describe_gap(kind: &EvidenceGapKind) -> String {
    match kind {
        EvidenceGapKind::DynamicCall => {
            "a dynamic call has no bounded retained realization".to_owned()
        }
        EvidenceGapKind::UnknownCallee(callee) => {
            format!("call target machine {callee} is not retained in the module")
        }
        EvidenceGapKind::UnknownBoundary(boundary) => {
            format!("boundary {boundary} has no retained declaration")
        }
        EvidenceGapKind::UnknownEntry => {
            "the selected entry is not retained in the module".to_owned()
        }
    }
}

fn describe_location(
    machine: semantic_vocabulary::MachineId,
    block: Option<semantic_vocabulary::BlockId>,
    operation: Option<semantic_vocabulary::OperationId>,
) -> String {
    let mut location = format!("machine {machine}");
    if let Some(block) = block {
        location.push_str(&format!(", block {block}"));
    }
    if let Some(operation) = operation {
        location.push_str(&format!(", operation {operation}"));
    }
    location
}
