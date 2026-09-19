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
    ProhibitedSite, authored_behavior_exclusion_set_in, establish_behavior_exclusions_with_owners,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::MachineId;
use std::collections::BTreeMap;

/// Checked provenance for the Terminal machines of one replayed closure: the
/// authored machine name and declaration symbol, so a rejection can name
/// the machine that retains the prohibited site instead of a Terminal
/// coordinate alone. Names come from the entry symbol, the lowering's
/// source-call joins, and the provider candidate catalog's checked
/// identities; a machine none of those cover is attributed through the
/// nearest named caller on the walk from the entry, and reported by its
/// Terminal identity when no caller is named either.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct MachineProvenance {
    machines: BTreeMap<MachineId, (String, symbols::SymbolHandle)>,
    /// First caller reached from the entry on the static call graph.
    callers: BTreeMap<MachineId, MachineId>,
}

impl MachineProvenance {
    /// Join the replayed module's call operations to the checked machines
    /// they target. Each ephemeral source-call occurrence names the Terminal
    /// call operation and the checked callee symbol; the operation's callee
    /// is the Terminal machine that symbol lowered to. The entry is named by
    /// its selected symbol.
    pub(crate) fn from_lowering(
        checked: &CheckedCompilation,
        lowered: &lowered_psi::LoweredPsi,
        entry_machine_symbol: symbols::SymbolHandle,
    ) -> Self {
        let module = &lowered.semantic_module;
        let mut callee_by_operation = BTreeMap::new();
        for machine in &module.machines {
            for block in &machine.blocks {
                for operation in &block.operations {
                    if let Some(callee) = call_callee(&operation.kind) {
                        callee_by_operation.insert(operation.id, callee);
                    }
                }
            }
        }
        let mut provenance = Self::default();
        provenance.name(checked, module.entry, entry_machine_symbol);
        for occurrence in &lowered.source_call_occurrences {
            if let Some(callee) = callee_by_operation.get(&occurrence.terminal_operation) {
                provenance.name(checked, *callee, occurrence.source_target);
            }
        }
        // Provider candidate bodies are reached through boundary dispatch,
        // not a source call; the catalog retains each candidate's canonical
        // checked identity, which rejoins its declaration.
        if !module.provider_candidates.is_empty() {
            let identities: BTreeMap<String, symbols::SymbolHandle> = checked
                .typed
                .machines()
                .iter()
                .filter_map(|machine| {
                    checked
                        .typed
                        .normalized_machine_overload_identity(machine)
                        .map(|identity| (identity.identity(), machine.symbol))
                })
                .collect();
            for candidate in &module.provider_candidates {
                if let Some(symbol) = identities.get(&candidate.candidate_identity) {
                    provenance.name(checked, candidate.candidate, *symbol);
                }
            }
        }
        // Static call parents from the entry, so an unnamed helper can be
        // attributed through the named machine that reaches it.
        let mut pending = std::collections::VecDeque::from([module.entry]);
        let mut visited = std::collections::BTreeSet::from([module.entry]);
        while let Some(current) = pending.pop_front() {
            let Some(machine) = module.machines.iter().find(|machine| machine.id == current) else {
                continue;
            };
            for block in &machine.blocks {
                for operation in &block.operations {
                    if let Some(callee) = call_callee(&operation.kind)
                        && visited.insert(callee)
                    {
                        provenance.callers.insert(callee, current);
                        pending.push_back(callee);
                    }
                    // A boundary call reaches its candidate bodies through
                    // dispatch, the same edge the absence walk follows.
                    if let terminal_psi::OperationKind::BoundaryCall { boundary, .. } =
                        &operation.kind
                    {
                        for candidate in module
                            .provider_candidates
                            .iter()
                            .filter(|candidate| candidate.boundary == *boundary)
                        {
                            if visited.insert(candidate.candidate) {
                                provenance.callers.insert(candidate.candidate, current);
                                pending.push_back(candidate.candidate);
                            }
                        }
                    }
                }
            }
        }
        provenance
    }

    /// The nearest machine on the walk from the entry that has a checked
    /// name: the machine itself when named, otherwise its first named caller.
    fn named_ancestor(
        &self,
        machine: MachineId,
    ) -> Option<(MachineId, &(String, symbols::SymbolHandle))> {
        let mut current = machine;
        let mut steps = 0_usize;
        loop {
            if let Some(named) = self.machines.get(&current) {
                return Some((current, named));
            }
            current = *self.callers.get(&current)?;
            steps += 1;
            if steps > self.callers.len() {
                return None;
            }
        }
    }

    fn name(
        &mut self,
        checked: &CheckedCompilation,
        machine: MachineId,
        symbol: symbols::SymbolHandle,
    ) {
        if self.machines.contains_key(&machine) {
            return;
        }
        let Some(declaration) = checked
            .typed
            .machines()
            .iter()
            .find(|candidate| candidate.symbol == symbol)
        else {
            return;
        };
        // Checked machine names are already owner-qualified (`Type::name`).
        let name = declaration.name.as_str().to_owned();
        self.machines.insert(machine, (name, symbol));
    }

    /// `machine \`name\` (Terminal machine N)` when the machine is joined to
    /// checked provenance; `Terminal machine N, reached through machine
    /// \`caller\` (Terminal machine M)` when only a caller is; otherwise
    /// `Terminal machine N`.
    fn label(&self, machine: MachineId) -> String {
        match self.named_ancestor(machine) {
            Some((named, (name, _))) if named == machine => {
                format!("machine `{name}` (Terminal machine {machine})")
            }
            Some((named, (name, _))) => format!(
                "Terminal machine {machine}, reached through machine `{name}` (Terminal machine {named})"
            ),
            None => format!("Terminal machine {machine}"),
        }
    }

    /// A diagnostic spanned at the checked declaration of `machine`, when
    /// the machine is joined to one that retains authored source custody.
    fn declared_here(
        &self,
        checked: &CheckedCompilation,
        machine: MachineId,
        message: String,
    ) -> Option<Diagnostic> {
        let (_, (_, symbol)) = self.named_ancestor(machine)?;
        let span = checked.typed.symbols.symbol_source_span(*symbol)?;
        Some(Diagnostic::error(message).with_source_span(span))
    }
}

fn call_callee(kind: &terminal_psi::OperationKind) -> Option<MachineId> {
    match kind {
        terminal_psi::OperationKind::Call { callee, .. }
        | terminal_psi::OperationKind::CallUnit { callee, .. }
        | terminal_psi::OperationKind::CallStructuralScalar { callee, .. }
        | terminal_psi::OperationKind::CallStructural { callee, .. }
        | terminal_psi::OperationKind::CallStructuralWithScalarArguments { callee, .. } => {
            Some(*callee)
        }
        _ => None,
    }
}

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
    let provenance = MachineProvenance::from_lowering(checked, &lowered, entry_machine_symbol);
    verify_module_behavior_exclusions(checked, &lowered.semantic_module, &provenance)
}

/// Verify the authored exclusions against one already-unoptimized Terminal
/// module (a selected entry composition or a callback thunk body). The
/// module's own `entry` is the walk's root. `provenance` names the module's
/// machines where the lowering joined them to checked declarations; a
/// callback thunk body carries none.
pub(crate) fn verify_module_behavior_exclusions(
    checked: &CheckedCompilation,
    module: &terminal_psi::TerminalModule,
    provenance: &MachineProvenance,
) -> Result<(), Vec<Diagnostic>> {
    let authored = checked.behavior_exclusions();
    if authored.is_empty() {
        return Ok(());
    }
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
        _ => Err(behavior_exclusion_diagnostics(
            checked,
            authored,
            module,
            provenance,
            &trait_identity,
            &report,
        )),
    }
}

/// Join each boundary declaration of `module` to the service its trait
/// owns: the boundary's canonical identity is the trait requirement's
/// normalized overload identity, and the owning service identity is the
/// boundary trait's declared name.
pub(crate) fn boundary_service_owners(
    checked: &CheckedCompilation,
    module: &terminal_psi::TerminalModule,
) -> build_evaluation::BoundaryServiceOwners {
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
    build_evaluation::BoundaryServiceOwners::from_module(module, &|boundary_identity| {
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
    checked: &CheckedCompilation,
    authored: &[AuthoredBehaviorExclusion],
    module: &terminal_psi::TerminalModule,
    provenance: &MachineProvenance,
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
        if let Some(declared) = provenance.declared_here(
            checked,
            prohibited.machine,
            format!(
                "the selected composition retains {} through {} declared here",
                describe_exclusion(module, prohibited.exclusion),
                provenance.label(prohibited.machine),
            ),
        ) {
            diagnostics.push(declared);
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
        if let Some(declared) = provenance.declared_here(
            checked,
            gap.machine,
            format!(
                "absence cannot be established for {} declared here",
                provenance.label(gap.machine),
            ),
        ) {
            diagnostics.push(declared);
        }
    }
    diagnostics
}

fn describe_exclusion(
    module: &terminal_psi::TerminalModule,
    exclusion: build_evaluation::BehaviorExclusion,
) -> String {
    match exclusion {
        build_evaluation::BehaviorExclusion::CrashCause(cause) => {
            format!("crash cause {cause:?}")
        }
        build_evaluation::BehaviorExclusion::Service(service) => {
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
        build_evaluation::BehaviorExclusion::PhysicalAuthorityClass(class) => {
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
    provenance: &MachineProvenance,
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

#[cfg(test)]
mod tests {
    use super::MachineProvenance;
    use semantic_vocabulary::MachineId;

    fn machine(raw: u64) -> MachineId {
        MachineId::new(raw).expect("nonzero machine identity")
    }

    #[test]
    fn labels_name_joined_machines_and_keep_terminal_identities_otherwise() {
        let mut provenance = MachineProvenance::default();
        provenance.machines.insert(
            machine(4),
            ("verify".to_owned(), symbols::SymbolHandle::invalid()),
        );
        assert_eq!(
            provenance.label(machine(4)),
            format!("machine `verify` (Terminal machine {})", machine(4))
        );
        assert_eq!(
            provenance.label(machine(9)),
            format!("Terminal machine {}", machine(9))
        );
        // A helper reached only through a named caller is attributed to it.
        provenance.callers.insert(machine(7), machine(4));
        assert_eq!(
            provenance.label(machine(7)),
            format!(
                "Terminal machine {}, reached through machine `verify` (Terminal machine {})",
                machine(7),
                machine(4)
            )
        );
        // A caller cycle without any name terminates without a label.
        provenance.callers.insert(machine(11), machine(12));
        provenance.callers.insert(machine(12), machine(11));
        assert_eq!(
            provenance.label(machine(11)),
            format!("Terminal machine {}", machine(11))
        );
        assert_eq!(
            super::describe_location(&provenance, machine(9), None, None),
            format!("Terminal machine {}", machine(9))
        );
    }
}
