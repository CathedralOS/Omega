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

use crate::checked::CheckedCompilation;
use build_evaluation::{AuthoredBehaviorExclusion, authored_behavior_exclusion_set_in};
use checked_trees_to_lowered_psi::LoweredMachineProvenance;
use diagnostics::Diagnostic;
use effects::{
    BehaviorExclusionReport, BehaviorExclusionVerdict, EvidenceGapKind, ProhibitedSite,
    establish_behavior_exclusions_with_owners,
};
use std::collections::BTreeMap;

/// Verify the authored exclusions against the selected program entry's
/// unoptimized composition: the lowered module Terminal production retained
/// before its selected optimization ran, the same lowering the artifact was
/// published from. `unoptimized` is `None` only when production was not
/// asked to retain it. With authored exclusions that is a production-request
/// fault, not an admission; without exclusions no retained copy is needed.
pub(crate) fn verify_entry_behavior_exclusions(
    checked: &CheckedCompilation,
    unoptimized: Option<&lowered_psi::LoweredPsi>,
    entry_machine_symbol: symbols::SymbolHandle,
) -> Result<(), Vec<Diagnostic>> {
    if checked.behavior_exclusions().is_empty() {
        return Ok(());
    }
    let lowered = unoptimized.ok_or_else(|| {
        vec![Diagnostic::error(
            "behavior-exclusion admission needs the unoptimized Terminal lowering retained by production",
        )]
    })?;
    let module = &lowered.semantic_module;
    let authored = checked.behavior_exclusions();
    let trait_identity = boundary_trait_identity(checked);
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
        checked.selected_provider_plans(),
        &boundary_service_owners(checked, module),
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

/// Join each boundary declaration of `module` to the service its trait
/// owns: the boundary's canonical identity is the trait requirement's
/// normalized overload identity, and the owning service identity is the
/// boundary trait's declared name.
pub(crate) fn boundary_service_owners(
    checked: &CheckedCompilation,
    module: &terminal_psi::TerminalModule,
) -> effects::BoundaryServiceOwners {
    let mut owning_service: BTreeMap<String, String> = BTreeMap::new();
    for definition in checked
        .typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
    {
        for signature in checked.typed.trait_machine_signatures(definition) {
            owning_service.insert(
                checked
                    .typed
                    .normalized_trait_requirement_overload_identity(definition, signature)
                    .identity(),
                definition.name.as_str().to_owned(),
            );
        }
    }
    effects::BoundaryServiceOwners::from_module(module, &|boundary_identity| {
        owning_service.get(boundary_identity).cloned()
    })
}

/// Map a checked boundary-trait symbol to the identity Terminal service
/// declarations carry for it: the trait's declared name.
pub(crate) fn boundary_trait_identity(
    checked: &CheckedCompilation,
) -> impl Fn(symbols::SymbolHandle) -> Option<String> + '_ {
    move |symbol| {
        checked
            .typed
            .traits()
            .iter()
            .find(|definition| definition.symbol == symbol && definition.is_boundary)
            .map(|definition| definition.name.as_str().to_owned())
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
            describe_exclusion(module, prohibited.exclusion),
            provenance.label(prohibited.machine),
            describe_site(module, &prohibited.site),
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
                    describe_exclusion(module, prohibited.exclusion),
                    provenance.label(prohibited.machine),
                ))
                .with_source_span(span),
            );
        }
    }
    for gap in &report.gaps {
        let mut diagnostic = Diagnostic::error(format!(
            "behavior exclusion evidence is insufficient: {} at {}",
            describe_gap(&gap.kind),
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

fn describe_exclusion(
    module: &terminal_psi::TerminalModule,
    exclusion: effects::BehaviorExclusion,
) -> String {
    match exclusion {
        effects::BehaviorExclusion::CrashCause(cause) => {
            format!("crash cause {cause:?}")
        }
        effects::BehaviorExclusion::Service(service) => {
            match module
                .services
                .iter()
                .find(|declaration| declaration.id == service)
            {
                Some(declaration) => format!("service `{}`", declaration.identity),
                None => format!("service {service}"),
            }
        }
        // A physical-class selection is a demand on the mechanism closure:
        // this walk never yields a prohibited site for it, but the arm keeps
        // the description total if one is ever named.
        effects::BehaviorExclusion::PhysicalAuthorityClass(class) => {
            format!("physical authority class {class:?}")
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
        ProhibitedSite::DynamicCall { block, operation } => {
            format!("dynamic call (block {block}, operation {operation})")
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
