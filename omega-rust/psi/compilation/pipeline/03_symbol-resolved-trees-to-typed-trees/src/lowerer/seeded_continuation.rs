//! Transactional append-only typing and exact retained-base admission.

use super::{
    Lowerer, TypingCohort, append_authored_service_reach_rows, close_typed_cohort,
    lower_evidence_forwarding, lower_symbol_resolved_trees, type_resolved_cohort,
};
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeededContinuationError {
    RetainedResolvedRootsChanged,
    CrossPairedResolvedBase,
    AuthoredSelectionPrefixChanged,
    AuthoredSelectionPrefixChangedDuringLowering,
    Lowering(Diagnostic),
}

/// Opaque ownership of one exact resolved/typed base pair. Prepared symbols,
/// semantic domains and service rows join the resolved allocation frontier
/// before either continuation allocates further IDs. Source selection stays exact.
/// `Clone` copies the owned pair for a caller retaining one frontier while the
/// original continues into generated-source extension and checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeededTypingBase {
    resolved: Arc<SymbolResolvedTrees>,
    typed: TypedTrees,
}

impl SeededTypingBase {
    /// Both continuation owners reserve typing's prepared allocations before
    /// resolution resumes. Indexed domains and abstract service rows share ID
    /// spaces with authored declarations, just as generated symbols do.
    pub fn join_prepared_allocations(
        &mut self,
        frontier: syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
    ) -> Result<syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier, Vec<Diagnostic>>
    {
        let frontier = frontier.with_prepared_allocations(
            &self.resolved,
            &self.typed.symbols,
            &self.typed.semantic_domains,
            &self.typed.service_reach_rows,
        )?;
        self.resolved = frontier.shared_trees();
        Ok(frontier)
    }

    /// Complete authored implementations against the exact retained declaration
    /// frontier. This is not a generated-root extension: existing owners gain
    /// bodies, while their interfaces and every prior complete body stay fixed.
    pub fn complete_bodies(
        self,
        source: syntax_trees_to_symbol_resolved_trees::ResolvedBodyCompletion,
    ) -> Result<
        (
            Self,
            syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
        ),
        Vec<Diagnostic>,
    > {
        let source = source
            .rebase_authored_selections_for_typed_completion(
                self.typed.authored_declaration_selections(),
            )
            .map_err(|(_, error)| vec![Diagnostic::error(format!(
                "authored-body completion could not join its retained typed selection ledger: {error:?}"
            ))])?;
        lower_body_completion(source, self).map_err(|error| vec![error])
    }

    /// Admit the resolver's authored-selection suffix against this exact
    /// typed predecessor, then run transactional append-only lowering.
    /// The coordinator cannot choose a different ledger for this join.
    pub fn lower_extension(
        self,
        source: syntax_trees_to_symbol_resolved_trees::SeededSymbolResolvedTrees,
    ) -> Result<TypedTrees, Vec<Diagnostic>> {
        let rebased = source
            .rebase_authored_selections_for_typed_continuation(
                self.typed.authored_declaration_selections(),
            )
            .map_err(|(_, error)| {
                vec![Diagnostic::error(format!(
                    "generated-source authored-selection suffix could not join the retained typed base: {error:?}"
                ))]
            })?;
        lower_seeded_extension(rebased, self).map_err(|(_, error)| match error {
            SeededContinuationError::Lowering(diagnostic) => vec![diagnostic],
            SeededContinuationError::RetainedResolvedRootsChanged => vec![Diagnostic::error(
                "source continuation changed a retained declaration root",
            )],
            error => vec![Diagnostic::error(format!(
                "generated-source continuation violated its retained-base invariant: {error:?}"
            ))],
        })
    }

    /// Reserve typing's prepared allocation suffix for a generated stratum.
    /// Domain instances and abstract service rows must not compete with later
    /// resolver allocations or be reconstructed from their display payloads.
    pub fn resolved_base_for_extension(&mut self) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
        if !self
            .typed
            .symbols
            .is_generated_extension_of(&self.resolved.symbols)
        {
            return Err(vec![Diagnostic::error(
                "generated-source continuation changed retained source custody or appended authored symbols",
            )]);
        }
        if !domain_table_retains(
            &self.resolved.semantic_domains,
            &self.typed.semantic_domains,
        ) || !self
            .typed
            .service_reach_rows
            .starts_with(&self.resolved.service_reach_rows)
        {
            return Err(vec![Diagnostic::error(
                "generated-source continuation changed retained domain or service-row allocations",
            )]);
        }
        if self.typed.symbols != self.resolved.symbols
            || self.typed.semantic_domains != self.resolved.semantic_domains
            || self.typed.service_reach_rows != self.resolved.service_reach_rows
        {
            let prepared = Arc::make_mut(&mut self.resolved);
            prepared.symbols = self.typed.symbols.clone();
            prepared.semantic_domains = self.typed.semantic_domains.clone();
            prepared.service_reach_rows = self.typed.service_reach_rows.clone();
        }
        Ok((*self.resolved).clone())
    }

    pub fn typed(&self) -> &TypedTrees {
        &self.typed
    }

    pub fn typed_mut(&mut self) -> &mut TypedTrees {
        &mut self.typed
    }

    pub fn into_typed(self) -> TypedTrees {
        self.typed
    }
}

/// Run the ordinary complete resolved-to-typed lowering while retaining the
/// exact resolved predecessor inside an opaque continuation carrier.
pub fn lower_symbol_resolved_trees_to_seeded_base(
    resolved: impl Into<Arc<SymbolResolvedTrees>>,
) -> Result<SeededTypingBase, Diagnostic> {
    let resolved = resolved.into();
    // Complete typing freshens callable binders. Their signatures retain the
    // generated symbols, so keep that suffix with the typed arenas; the next
    // resolver extension admits it through resolved_base_for_extension.
    let typed = lower_symbol_resolved_trees(&resolved)?;
    Ok(SeededTypingBase { resolved, typed })
}

/// Append an ordinarily resolved declaration cohort to an exact retained typed
/// base. Mutation occurs on a clone, so every error returns the input base
/// byte-for-byte; no caller is licensed to reconstruct a second frontend.
pub fn lower_seeded_extension(
    source: syntax_trees_to_symbol_resolved_trees::RebasedSeededSymbolResolvedTrees,
    retained: SeededTypingBase,
) -> Result<TypedTrees, (SeededTypingBase, SeededContinuationError)> {
    let (source, resolved_base) = source.into_typing_continuation_parts();
    if resolved_base != *retained.resolved {
        return Err((retained, SeededContinuationError::CrossPairedResolvedBase));
    }
    // This owned pair is the output of typing, not an independently supplied
    // trust claim. Typing elaborates selections and service combinations;
    // requiring equality with resolution revalidates the stage's own output
    // and incorrectly forbids those ordinary elaborations.
    if !resolved_root_prefix_is_retained(&source, &resolved_base) {
        return Err((
            retained,
            SeededContinuationError::RetainedResolvedRootsChanged,
        ));
    }
    let destination_ledger = retained.typed.authored_declaration_selections();
    if !source
        .authored_declaration_selections()
        .as_slice()
        .starts_with(destination_ledger.as_slice())
    {
        return Err((
            retained,
            SeededContinuationError::AuthoredSelectionPrefixChanged,
        ));
    }

    if let Err(error) = super::validate_resolved_semantics(&source) {
        return Err((retained, SeededContinuationError::Lowering(error)));
    }
    let resolved_ledger = source.authored_declaration_selections().clone();
    let mut candidate = retained.typed.clone();
    if let Err(error) = join_resolved_semantics(&source, &resolved_base, &mut candidate) {
        return Err((retained, SeededContinuationError::Lowering(error)));
    }
    let mut lowerer = Lowerer {
        typed_trees: candidate,
        source_trees: &source,
        equality_scope: None,
        type_reference_exposure:
            Some(language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation),
    };
    let cohort = TypingCohort::continuing(&resolved_base, &retained.typed);
    if let Err(error) = type_resolved_cohort(&mut lowerer, &cohort) {
        return Err((retained, SeededContinuationError::Lowering(error)));
    }
    for forwarding in source
        .evidence_forwardings
        .iter()
        .skip(resolved_base.evidence_forwardings.len())
    {
        lowerer
            .typed_trees
            .evidence_forwardings
            .push(lower_evidence_forwarding(
                forwarding,
                &lowerer.typed_trees.proof_output_calls,
            ));
    }
    if let Err(error) = close_typed_cohort(&mut lowerer, &cohort) {
        return Err((retained, SeededContinuationError::Lowering(error)));
    }
    if !lowerer
        .typed_trees
        .authored_declaration_selections()
        .as_slice()
        .starts_with(resolved_ledger.as_slice())
    {
        return Err((
            retained,
            SeededContinuationError::AuthoredSelectionPrefixChangedDuringLowering,
        ));
    }
    // The ordinary typing operations own this continuation's mutations.
    // Replaying a whole-tree storage comparison here is not a trust boundary:
    // generated declarations legitimately extend retained owners' child spans.
    // Retained preservation belongs to those operations and their fixtures.
    Ok(lowerer.typed_trees)
}

/// Authored completion shares ordinary body lowering, but admits only the
/// selected owners' implementation deltas. Generated continuation keeps its
/// independent, stricter append-only admission above.
fn lower_body_completion(
    source: syntax_trees_to_symbol_resolved_trees::RebasedResolvedBodyCompletion,
    retained: SeededTypingBase,
) -> Result<
    (
        SeededTypingBase,
        syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
    ),
    Diagnostic,
> {
    let (frontier, predecessor, selected) = source.into_typing_completion_parts();
    let source = frontier.shared_trees();
    if predecessor != retained.resolved {
        return Err(Diagnostic::error(
            "authored-body completion does not match the exact retained resolved predecessor",
        ));
    }
    if retained.typed.symbols != predecessor.symbols {
        return Err(Diagnostic::error(
            "authored-body completion found changed typed predecessor symbols",
        ));
    }
    if !source
        .authored_declaration_selections()
        .as_slice()
        .starts_with(retained.typed.authored_declaration_selections().as_slice())
    {
        return Err(Diagnostic::error(
            "authored-body completion did not retain the typed selection ledger prefix",
        ));
    }
    if !resolved_completion_retains_declarations(&predecessor, &source, &selected) {
        return Err(Diagnostic::error(
            "authored-body completion changed retained resolved declaration or unselected body storage",
        ));
    }
    if !resolved_normalized_prefix_is_retained(&predecessor, &source) {
        return Err(Diagnostic::error(
            "authored-body completion changed retained normalized statement, transition target or type storage",
        ));
    }
    super::validate_resolved_semantics(&source)?;
    let mut candidate = retained.typed.clone();
    join_resolved_semantics(&source, &predecessor, &mut candidate)?;
    let mut lowerer = Lowerer {
        typed_trees: candidate,
        source_trees: &source,
        equality_scope: None,
        type_reference_exposure: Some(
            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
        ),
    };
    let cohort = TypingCohort::continuing(&predecessor, &retained.typed).completing(&selected);
    type_resolved_cohort(&mut lowerer, &cohort)?;
    for forwarding in source
        .evidence_forwardings
        .iter()
        .skip(predecessor.evidence_forwardings.len())
    {
        lowerer
            .typed_trees
            .evidence_forwardings
            .push(lower_evidence_forwarding(
                forwarding,
                &lowerer.typed_trees.proof_output_calls,
            ));
    }
    close_typed_cohort(&mut lowerer, &cohort)?;
    // Retained-storage preservation is carried by these append-only operations,
    // not by a second comparison of their output: the supplied completion data
    // was admitted against the exact resolved predecessor above, and
    // omega-rust/pipeline.md's verification placement forbids a stage replaying
    // what the same compile just produced. A producer that replaces retained
    // storage instead of appending to it is a bug for the corpus and the
    // fixtures to find.
    let typed = lowerer.typed_trees;
    Ok((
        SeededTypingBase {
            resolved: source,
            typed,
        },
        frontier,
    ))
}

fn resolved_normalized_prefix_is_retained(
    base: &SymbolResolvedTrees,
    candidate: &SymbolResolvedTrees,
) -> bool {
    let old_statements = &base.tables.bodies.statements;
    let new_statements = &candidate.tables.bodies.statements;
    let old_types = &base.tables.types.references;
    let new_types = &candidate.tables.types.references;
    new_statements.statement_count() >= old_statements.statement_count()
        && new_statements.transition_target_count() >= old_statements.transition_target_count()
        && new_types.type_reference_count() >= old_types.type_reference_count()
        && (1..=old_statements.statement_count()).all(|ordinal| {
            let Ok(ordinal) = u32::try_from(ordinal) else {
                return false;
            };
            let handle =
                symbol_resolved_trees::statement::StatementHandle::from_arena_index(ordinal);
            old_statements.statement(handle) == new_statements.statement(handle)
        })
        && (1..=old_statements.transition_target_count()).all(|ordinal| {
            let Ok(ordinal) = u32::try_from(ordinal) else {
                return false;
            };
            let handle =
                symbol_resolved_trees::statement::TransitionTargetHandle::from_arena_index(ordinal);
            old_statements.transition_target(handle) == new_statements.transition_target(handle)
        })
        && (1..=old_types.type_reference_count()).all(|ordinal| {
            let Ok(ordinal) = u32::try_from(ordinal) else {
                return false;
            };
            let handle =
                symbol_resolved_trees::types::TypeReferenceHandle::from_arena_index(ordinal);
            old_types.type_reference(handle) == new_types.type_reference(handle)
        })
}

fn domain_table_retains(
    base: &language_semantics::SemanticDomainTable,
    candidate: &language_semantics::SemanticDomainTable,
) -> bool {
    candidate.len() >= base.len()
        && (1..=base.len()).all(|ordinal| {
            let identity = language_semantics::SemanticDomainId(ordinal as u32);
            candidate.name(identity) == base.name(identity)
        })
}

fn resolved_completion_retains_declarations(
    base: &SymbolResolvedTrees,
    candidate: &SymbolResolvedTrees,
    selected: &[symbols::SymbolHandle],
) -> bool {
    if candidate.machines.len() < base.machines.len()
        || selected
            .iter()
            .enumerate()
            .any(|(ordinal, symbol)| selected[..ordinal].contains(symbol))
        || selected.iter().any(|symbol| {
            !base
                .machines
                .iter()
                .any(|machine| machine.symbol == *symbol)
        })
    {
        return false;
    }
    for (old, new) in base.machines.iter().zip(candidate.machines.iter()) {
        if !selected.contains(&old.symbol) {
            if old != new {
                return false;
            }
            if base
                .machine_state_handles(old.states)
                .iter()
                .any(|state| base.machine_state(*state) != candidate.machine_state(*state))
                || base.machine_owned_data(old.owned_data)
                    != candidate.machine_owned_data(new.owned_data)
                || base.machine_trait_conformances(old.satisfies)
                    != candidate.machine_trait_conformances(new.satisfies)
            {
                return false;
            }
            continue;
        }
        if old.body_status != symbol_resolved_trees::machine::MachineBodyStatus::Deferred
            || new.body_status != symbol_resolved_trees::machine::MachineBodyStatus::Complete
        {
            return false;
        }
        let mut header = new.clone();
        header.body_status = old.body_status;
        header.termination_plan.implementation_witness =
            old.termination_plan.implementation_witness.clone();
        header.ranking_subjects = old.ranking_subjects;
        header.ranking_view = old.ranking_view;
        header.ranking_view_arguments = old.ranking_view_arguments;
        header.ranking_range = old.ranking_range;
        header.states = old.states;
        header.owned_data = old.owned_data;
        header.satisfies = old.satisfies;
        if header != *old {
            return false;
        }
        let old_states = base.machine_state_handles(old.states);
        let new_states = candidate.machine_state_handles(new.states);
        if !new_states.starts_with(old_states) {
            return false;
        }
        for &state in old_states {
            let old_state = base.machine_state(state);
            let mut new_state = candidate.machine_state(state).clone();
            new_state.statements = old_state.statements;
            new_state.statement_nodes = old_state.statement_nodes;
            if new_state != *old_state {
                return false;
            }
        }
        let old_owned = base.machine_owned_data(old.owned_data);
        let new_owned = candidate.machine_owned_data(new.owned_data);
        if old_owned.len() != new_owned.len() {
            return false;
        }
        for (old, new) in old_owned.iter().zip(new_owned) {
            let mut header = new.clone();
            header.initial_value = old.initial_value;
            if header != *old {
                return false;
            }
        }
        let old_conformances = base.machine_trait_conformances(old.satisfies);
        let new_conformances = candidate.machine_trait_conformances(new.satisfies);
        if old_conformances.len() != new_conformances.len() {
            return false;
        }
        for (old, new) in old_conformances.iter().zip(new_conformances) {
            let mut header = new.clone();
            header.via_expression = old.via_expression;
            if header != *old {
                return false;
            }
        }
    }
    // Non-machine roots can append through ordinary body-driven synthesis,
    // but no retained declaration is replaced. These roots do not own deferred
    // authored implementations in this transition.
    candidate.const_declarations.len() >= base.const_declarations.len()
        && candidate
            .const_declarations
            .iter()
            .take(base.const_declarations.len())
            .eq(base.const_declarations.iter())
        && candidate.data_definitions.len() >= base.data_definitions.len()
        && candidate
            .data_definitions
            .iter()
            .take(base.data_definitions.len())
            .eq(base.data_definitions.iter())
        && candidate.traits.len() >= base.traits.len()
        && candidate
            .traits
            .iter()
            .take(base.traits.len())
            .eq(base.traits.iter())
        && retained_declaration_families_are_exact(base, candidate)
}

/// Join the resolved cohort's shared semantic IDs before typing its declarations.
/// Both continuation routes preserve the same interner prefixes. In particular,
/// typing may have interned domain applications of its own; retain those IDs
/// while adding the resolver's suffix rather than replacing the typed table.
fn join_resolved_semantics(
    source: &SymbolResolvedTrees,
    base: &SymbolResolvedTrees,
    candidate: &mut TypedTrees,
) -> Result<(), Diagnostic> {
    if !source
        .service_reaches
        .definitions()
        .starts_with(base.service_reaches.definitions())
        || !source
            .service_reaches
            .definitions()
            .starts_with(candidate.service_reaches.definitions())
    {
        return Err(Diagnostic::error(
            "source continuation changed retained service definitions",
        ));
    }
    if !source
        .service_reach_rows
        .starts_with(&base.service_reach_rows)
        || !source
            .service_reach_rows
            .starts_with(&candidate.service_reach_rows)
    {
        return Err(Diagnostic::error(
            "source continuation changed retained service-row identities",
        ));
    }
    if !domain_table_retains(&base.semantic_domains, &source.semantic_domains) {
        return Err(Diagnostic::error(
            "source continuation changed retained semantic domains",
        ));
    }
    if !source
        .external_bindings
        .identities()
        .take(base.external_bindings.identities().len())
        .eq(base.external_bindings.identities())
        || !source
            .external_bindings
            .identities()
            .take(candidate.external_bindings.identities().len())
            .eq(candidate.external_bindings.identities())
    {
        return Err(Diagnostic::error(
            "source continuation changed retained external-binding identities",
        ));
    }
    if !source
        .evidence_forwardings
        .starts_with(&base.evidence_forwardings)
    {
        return Err(Diagnostic::error(
            "source continuation changed retained evidence forwardings",
        ));
    }
    for row in &base.authored_service_reach_rows {
        if source
            .authored_service_reach_rows
            .iter()
            .find(|next| next.owner == row.owner)
            != Some(row)
        {
            return Err(Diagnostic::error(
                "source continuation changed a retained authored service ceiling",
            ));
        }
    }
    for ordinal in 1..=source.semantic_domains.len() {
        let identity = language_semantics::SemanticDomainId(ordinal as u32);
        let Some(name) = source.semantic_domains.name(identity) else {
            return Err(Diagnostic::error(
                "source continuation lost a semantic domain identity",
            ));
        };
        if candidate.semantic_domains.intern(name) != identity {
            return Err(Diagnostic::error(
                "source continuation conflicts with a prepared semantic domain identity",
            ));
        }
    }
    candidate.symbols = source.symbols.clone();
    candidate.service_reaches = source.service_reaches.clone();
    candidate.service_reach_rows = source.service_reach_rows.clone();
    candidate.external_bindings = source.external_bindings.clone();
    candidate
        .retain_authored_declaration_selections(source.authored_declaration_selections().clone());
    append_authored_service_reach_rows(candidate, source, &base.authored_service_reach_rows);
    Ok(())
}

/// A new declaration cohort may use every ordinary language form. Its roots
/// retain the exact prior declarations; ordinary resolution and typing own the
/// meaning of the suffix instead of a second declaration-shape recognizer.
fn resolved_root_prefix_is_retained(
    source: &SymbolResolvedTrees,
    base: &SymbolResolvedTrees,
) -> bool {
    macro_rules! retains {
        ($($family:ident),* $(,)?) => {
            $(source.$family.iter().take(base.$family.len()).eq(base.$family.iter()))&&*
        };
    }
    retains!(
        const_declarations,
        data_definitions,
        domain_definitions,
        machines,
        measures,
        operators,
        propositions,
        mathematical_definitions,
        traits,
        conformances,
        wire_schemas,
    )
}

/// Completing a retained machine body can synthesize data, traits and attached
/// machines, but does not author new top-level proof or declaration families.
/// A declaration extension uses the independent full-family prefix rule above.
fn retained_declaration_families_are_exact(
    base: &SymbolResolvedTrees,
    candidate: &SymbolResolvedTrees,
) -> bool {
    candidate.domain_definitions == base.domain_definitions
        && candidate.measures == base.measures
        && candidate.operators == base.operators
        && candidate.propositions == base.propositions
        && candidate.mathematical_definitions == base.mathematical_definitions
        && candidate.conformances == base.conformances
        && candidate.wire_schemas == base.wire_schemas
}
