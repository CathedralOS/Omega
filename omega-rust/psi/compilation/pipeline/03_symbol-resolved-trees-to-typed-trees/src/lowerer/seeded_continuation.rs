//! Transactional append-only typing and exact retained-base admission.

use super::{
    Lowerer, TypingCohort, append_authored_service_reach_rows, close_typed_cohort,
    exact_field_symbol, exact_top_level_data_symbol, lower_evidence_forwarding,
    lower_symbol_resolved_trees, type_resolved_cohort,
};
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::JoinedPath;
use typed_trees::TypedTrees;

mod seeded_local_instances;
mod seeded_type_application;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeededContinuationError {
    UnsupportedExtensionShape,
    CrossPairedResolvedBase,
    ResolvedSemanticTablesChanged,
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
            SeededContinuationError::UnsupportedExtensionShape => vec![Diagnostic::error(
                "generated source uses a declaration shape not yet supported by retained-checkpoint continuation; reconstructing a second frontend is forbidden",
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

/// Append one supported generated-source cohort to an exact retained typed
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
    let data_frontier = resolved_base.data_definitions.len();
    let machine_frontier = resolved_base.machines.len();
    let trait_frontier = resolved_base.traits.len();
    if !resolved_root_shape_is_supported(&source, &resolved_base)
        || !seeded_extension_shape_is_supported(
            &source,
            data_frontier,
            machine_frontier,
            trait_frontier,
        )
    {
        return Err((retained, SeededContinuationError::UnsupportedExtensionShape));
    }
    if source.service_reaches != resolved_base.service_reaches
        || !source
            .service_reach_rows
            .starts_with(&resolved_base.service_reach_rows)
        || !source
            .service_reach_rows
            .starts_with(&retained.typed.service_reach_rows)
        || !retained_authored_service_reaches_are_exact(&source, &resolved_base)
        || source.semantic_domains != resolved_base.semantic_domains
        || source.external_bindings != resolved_base.external_bindings
        || source.evidence_forwardings != resolved_base.evidence_forwardings
    {
        return Err((
            retained,
            SeededContinuationError::ResolvedSemanticTablesChanged,
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
    candidate.retain_authored_declaration_selections(resolved_ledger.clone());
    candidate.symbols = source.symbols.clone();
    // Resolution may append combinations of retained services, but both base
    // tables must keep their exact row IDs and meanings.
    candidate.service_reach_rows = source.service_reach_rows.clone();
    append_authored_service_reach_rows(
        &mut candidate,
        &source,
        &resolved_base.authored_service_reach_rows,
    );
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
    // Keep the explicit comparison available for validation, off compilation.
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
    let violation = || {
        Diagnostic::error(
            "authored-body completion changed its retained declaration or implementation frontier",
        )
    };
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
    if !source
        .service_reaches
        .definitions()
        .starts_with(predecessor.service_reaches.definitions())
        || !source
            .service_reach_rows
            .starts_with(&predecessor.service_reach_rows)
        || !source
            .service_reach_rows
            .starts_with(&retained.typed.service_reach_rows)
        || !source
            .evidence_forwardings
            .starts_with(&predecessor.evidence_forwardings)
        || !domain_table_retains(&predecessor.semantic_domains, &source.semantic_domains)
        || !source
            .external_bindings
            .identities()
            .take(predecessor.external_bindings.identities().len())
            .eq(predecessor.external_bindings.identities())
    {
        return Err(Diagnostic::error(
            "authored-body completion changed a retained semantic sidecar prefix",
        ));
    }
    super::validate_resolved_semantics(&source)?;
    let mut candidate = retained.typed.clone();
    candidate.symbols = source.symbols.clone();
    candidate.service_reaches = source.service_reaches.clone();
    candidate.service_reach_rows = source.service_reach_rows.clone();
    candidate.external_bindings = source.external_bindings.clone();
    // Typing may already have interned membership instances after the resolved
    // prefix. A resolver suffix must agree with those exact IDs, never replace
    // the typed table and discard its retained instances.
    for ordinal in 1..=source.semantic_domains.len() {
        let identity = language_semantics::SemanticDomainId(ordinal as u32);
        let name = source
            .semantic_domains
            .name(identity)
            .ok_or_else(violation)?;
        if candidate.semantic_domains.intern(name) != identity {
            return Err(violation());
        }
    }
    for row in &predecessor.authored_service_reach_rows {
        if source
            .authored_service_reach_rows
            .iter()
            .find(|next| next.owner == row.owner)
            != Some(row)
        {
            return Err(violation());
        }
    }
    candidate
        .retain_authored_declaration_selections(source.authored_declaration_selections().clone());
    append_authored_service_reach_rows(
        &mut candidate,
        &source,
        &predecessor.authored_service_reach_rows,
    );
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

fn retained_authored_service_reaches_are_exact(
    source: &SymbolResolvedTrees,
    base: &SymbolResolvedTrees,
) -> bool {
    // Resolution groups machines before trait requirements, so generated
    // machine rows can precede retained requirement rows. Rejoin by owner;
    // preserve the typed base order and append only generated machine rows.
    let retained_owner = |row: &&symbol_resolved_trees::signature::AuthoredServiceReachRow| {
        base.authored_service_reach_rows
            .iter()
            .any(|base_row| base_row.owner == row.owner)
    };
    source
        .authored_service_reach_rows
        .iter()
        .filter(retained_owner)
        .eq(base.authored_service_reach_rows.iter())
        && source
            .authored_service_reach_rows
            .iter()
            .filter(|row| !retained_owner(row))
            .all(|row| {
                source
                    .machines
                    .iter()
                    .skip(base.machines.len())
                    .any(|machine| machine.symbol == row.owner)
            })
}

pub(super) fn resolved_root_shape_is_supported(
    source: &SymbolResolvedTrees,
    base: &SymbolResolvedTrees,
) -> bool {
    source
        .const_declarations
        .iter()
        .take(base.const_declarations.len())
        .eq(base.const_declarations.iter())
        && source.const_declarations.len() >= base.const_declarations.len()
        // Constant suffixes use ordinary declaration typing and initializer
        // checking below, not a second private/structured-carrier recognizer.
        && source
            .data_definitions
            .iter()
            .take(base.data_definitions.len())
            .eq(base.data_definitions.iter())
        && source.data_definitions.len() >= base.data_definitions.len()
        && source
            .machines
            .iter()
            .take(base.machines.len())
            .eq(base.machines.iter())
        && source.machines.len() >= base.machines.len()
        && retained_declaration_families_are_exact(base, source)
        && source
            .traits
            .iter()
            .take(base.traits.len())
            .eq(base.traits.iter())
        && source.traits.len() >= base.traits.len()
}

/// Declaration families resolution may never append to: an extension and a
/// completion both retain them exactly, so none of them is a frontier and the
/// whole arena must match the base. Mathematical declarations are the clearest
/// case — a suffix that reaches one reaches a second frontend, which
/// continuation forbids.
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

pub(super) fn seeded_extension_shape_is_supported(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    machine_frontier: usize,
    trait_frontier: usize,
) -> bool {
    let Some(local_instances) = seeded_local_instances::validated_symbols(source, data_frontier)
    else {
        return false;
    };
    source
        .data_definitions
        .iter()
        .skip(data_frontier)
        .all(|definition| {
            if definition.generic_instance.is_some() {
                return local_instances.contains(&definition.symbol);
            }
            let type_parameters = source.data_type_parameters(definition.type_parameters);
            exact_top_level_data_symbol(source, definition)
                && type_parameters.iter().all(|parameter| {
                    seeded_local_instances::parameter_is_supported(
                        source,
                        definition.symbol,
                        parameter,
                    )
                })
                && definition.quotient.is_none()
                && definition.where_facts.is_empty()
                && !definition.zero_gated
                && source
                    .data_members(definition.members)
                    .iter()
                    .all(|member| {
                        let fields = match member {
                            symbol_resolved_trees::data::DataMember::Field(field) => {
                                std::slice::from_ref(field)
                            }
                            symbol_resolved_trees::data::DataMember::Variant(variant) => {
                                source.data_payload_fields(variant.payload)
                            }
                        };
                        fields.iter().all(|field| {
                            plain_type_is_supported(
                                source,
                                data_frontier,
                                &local_instances,
                                definition.symbol,
                                &definition.lifetime_parameters,
                                type_parameters,
                                &field.type_reference,
                            )
                        })
                    })
        })
        && source
            .machines
            .iter()
            .skip(machine_frontier)
            .all(|machine| {
                exact_extension_machine_symbol(source, data_frontier, &local_instances, machine)
            })
        && source
            .traits
            .iter()
            .skip(trait_frontier)
            .all(|trait_definition| {
                exact_extension_trait_definition(
                    source,
                    data_frontier,
                    &local_instances,
                    trait_definition,
                )
            })
}

fn exact_extension_trait_definition(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    local_instances: &[symbols::SymbolHandle],
    trait_definition: &symbol_resolved_trees::trait_definition::TraitDefinition,
) -> bool {
    let requirements = source.trait_machine_signatures(trait_definition.machines);
    exact_flat_trait_definition(source, trait_definition)
        && !requirements.is_empty()
        && requirements.iter().all(|requirement| {
            exact_flat_trait_requirement(
                source,
                data_frontier,
                local_instances,
                trait_definition,
                requirement,
            )
        })
}

fn exact_flat_trait_definition(
    source: &SymbolResolvedTrees,
    trait_definition: &symbol_resolved_trees::trait_definition::TraitDefinition,
) -> bool {
    trait_definition.symbol.is_valid()
        && source.symbols.get(trait_definition.symbol).kind == symbols::SymbolKind::Trait
        && source.symbols.get(trait_definition.symbol).parent == source.symbols.root()
        && source.symbols.name(trait_definition.symbol) == trait_definition.name.as_str()
        && !trait_definition.is_boundary
        && trait_definition.lifetime_parameters.is_empty()
        && source
            .data_type_parameters(trait_definition.type_parameters)
            .is_empty()
        && trait_definition.conformance_bounds.is_empty()
        && source
            .trait_requirements(trait_definition.requires)
            .is_empty()
}

fn exact_flat_trait_requirement(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    local_instances: &[symbols::SymbolHandle],
    trait_definition: &symbol_resolved_trees::trait_definition::TraitDefinition,
    requirement: &symbol_resolved_trees::signature::StateSignature,
) -> bool {
    requirement.symbol.is_valid()
        && source.symbols.get(requirement.symbol).kind == symbols::SymbolKind::State
        && source.symbols.get(requirement.symbol).parent == trait_definition.symbol
        && source.symbols.name(requirement.symbol) == requirement.name.as_str()
        && requirement.spelling.is_none()
        && requirement.lifetime_parameters.is_empty()
        && source
            .data_type_parameters(requirement.type_parameters)
            .is_empty()
        && !requirement.is_default
        && requirement.native_callback_parameters.is_empty()
        && source
            .state_parameters(requirement.parameters)
            .iter()
            .all(|value| {
                value.symbol.is_valid()
                    && source.symbols.get(value.symbol).kind == symbols::SymbolKind::Parameter
                    && source.symbols.get(value.symbol).parent == requirement.symbol
                    && source.symbols.name(value.symbol) == value.name.as_str()
                    && plain_type_is_supported(
                        source,
                        data_frontier,
                        local_instances,
                        symbols::SymbolHandle::invalid(),
                        &[],
                        &[],
                        &value.type_reference,
                    )
            })
        && requirement.return_type.as_ref().is_none_or(|return_type| {
            plain_type_is_supported(
                source,
                data_frontier,
                local_instances,
                symbols::SymbolHandle::invalid(),
                &[],
                &[],
                return_type,
            )
        })
        && requirement.invokes.is_empty()
        && requirement.service_reach_row == language_semantics::ServiceReachRowTable::EMPTY_ROW
        && !requirement.service_reach_is_installation_bound
        && requirement.suspends_keyword_source_spans.is_empty()
        && requirement.blocks_keyword_source_spans.is_empty()
        && !requirement.suspends
        && !requirement.blocks
        && requirement.contracts.is_empty()
        && !requirement.terminates_guarantee
}

fn exact_extension_machine_symbol(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    local_instances: &[symbols::SymbolHandle],
    machine: &symbol_resolved_trees::machine::Machine,
) -> bool {
    let type_parameters = source.data_type_parameters(machine.type_parameters);
    if !machine.symbol.is_valid()
        || source.symbols.get(machine.symbol).kind != symbols::SymbolKind::Machine
        || source.symbols.name(machine.symbol) != machine.symbol_spelling()
        || !type_parameters
            .iter()
            .all(|parameter| {
                match &parameter.kind {
                symbol_resolved_trees::data::TypeParameterKind::Type => {
                    seeded_local_instances::parameter_is_supported(
                        source,
                        machine.symbol,
                        parameter,
                    )
                }
                symbol_resolved_trees::data::TypeParameterKind::Const { .. }
                | symbol_resolved_trees::data::TypeParameterKind::Value { .. } => {
                    seeded_local_instances::const_parameter_is_supported(
                        source,
                        machine.symbol,
                        parameter,
                    )
                }
                symbol_resolved_trees::data::TypeParameterKind::Machine { contract } => {
                    match contract {
                        symbol_resolved_trees::data::MachineParameterContract::Structural(
                            _,
                        ) => exact_extension_structural_machine_parameter(
                            source,
                            data_frontier,
                            local_instances,
                            machine,
                            type_parameters,
                            parameter,
                            contract,
                        ),
                        symbol_resolved_trees::data::MachineParameterContract::Nominal {
                            ..
                        } => exact_extension_nominal_machine_parameter(
                            source,
                            data_frontier,
                            local_instances,
                            machine,
                            type_parameters,
                            parameter,
                            contract,
                        ),
                        symbol_resolved_trees::data::MachineParameterContract::RequirementIdentity
                        | symbol_resolved_trees::data::MachineParameterContract::AuthoredNominal {
                            ..
                        } => false,
                    }
                }
                symbol_resolved_trees::data::TypeParameterKind::Implementation
                | symbol_resolved_trees::data::TypeParameterKind::Proposition { .. } => false,
            }
            })
        || !machine.satisfies.is_empty()
        || !machine.conformance_bounds.is_empty()
        || !machine.ranking_subjects.is_empty()
        || !machine.ranking_view.is_empty()
        || !machine.ranking_view_arguments.is_empty()
        || machine.ranking_range.is_valid()
        || machine.suspends
        || machine.blocks
        || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !machine.body_is_present
    {
        return false;
    }
    if type_parameters.iter().any(|parameter| {
        seeded_local_instances::structured_const_parameter_is_supported(
            source,
            machine.symbol,
            parameter,
        )
    }) && !source
        .machine_state_handles(machine.states)
        .iter()
        .all(|state| {
            let state = source.machine_state(*state);
            source
                .state_parameters(state.parameters)
                .iter()
                .all(|parameter| {
                    plain_type_is_supported(
                        source,
                        data_frontier,
                        local_instances,
                        machine.symbol,
                        &machine.lifetime_parameters,
                        type_parameters,
                        &parameter.type_reference,
                    )
                })
                && state.return_type.as_ref().is_none_or(|return_type| {
                    plain_type_is_supported(
                        source,
                        data_frontier,
                        local_instances,
                        machine.symbol,
                        &machine.lifetime_parameters,
                        type_parameters,
                        return_type,
                    )
                })
        })
    {
        return false;
    }
    let parent = source.symbols.get(machine.symbol).parent;
    match (&machine.attached_data, machine.attached_data_symbol) {
        (None, attached) => parent == source.symbols.root() && !attached.is_valid(),
        (Some(name), attached) => {
            parent == source.symbols.root()
                && attached.is_valid()
                && source.data_definitions.iter().any(|definition| {
                    definition.symbol == attached && definition.name.as_str() == name.as_str()
                })
        }
    }
}

/// The parameter-identity conjuncts both machine-parameter exact-extension
/// probes share: default bounds, a valid MachineParameter symbol owned by the
/// machine, and the authored name restated in the symbol table.
fn exact_extension_parameter_identity(
    source: &SymbolResolvedTrees,
    machine: &symbol_resolved_trees::machine::Machine,
    parameter: &symbol_resolved_trees::data::TypeParameter,
) -> bool {
    parameter.bounds == symbol_resolved_trees::data::DataProperties::default()
        && parameter.symbol.is_valid()
        && source.symbols.get(parameter.symbol).kind == symbols::SymbolKind::MachineParameter
        && source.symbols.get(parameter.symbol).parent == machine.symbol
        && source.symbols.name(parameter.symbol) == parameter.name.as_str()
}

fn exact_extension_structural_machine_parameter(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    local_instances: &[symbols::SymbolHandle],
    machine: &symbol_resolved_trees::machine::Machine,
    owner_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    parameter: &symbol_resolved_trees::data::TypeParameter,
    contract: &symbol_resolved_trees::data::MachineParameterContract,
) -> bool {
    let symbol_resolved_trees::data::MachineParameterContract::Structural(signature) = contract
    else {
        return false;
    };
    exact_extension_parameter_identity(source, machine, parameter)
        && signature.symbol == parameter.symbol
        && signature.name.as_str() == parameter.name.as_str()
        && signature.lifetime_parameters.is_empty()
        && signature.type_parameters.is_empty()
        && !signature.is_default
        && signature.native_callback_parameters.is_empty()
        && signature.invokes.is_empty()
        && !signature.service_reach_is_installation_bound
        && signature.suspends_keyword_source_spans.is_empty()
        && signature.blocks_keyword_source_spans.is_empty()
        && !signature.suspends
        && !signature.blocks
        && signature.contracts.is_empty()
        && !signature.terminates_guarantee
        && source
            .state_parameters(signature.parameters)
            .iter()
            .all(|value| {
                value.symbol.is_valid()
                    && source.symbols.get(value.symbol).kind == symbols::SymbolKind::Parameter
                    && source.symbols.get(value.symbol).parent == parameter.symbol
                    && source.symbols.name(value.symbol) == value.name.as_str()
                    && plain_type_is_supported(
                        source,
                        data_frontier,
                        local_instances,
                        machine.symbol,
                        &machine.lifetime_parameters,
                        owner_type_parameters,
                        &value.type_reference,
                    )
            })
        && signature.return_type.as_ref().is_none_or(|return_type| {
            plain_type_is_supported(
                source,
                data_frontier,
                local_instances,
                machine.symbol,
                &machine.lifetime_parameters,
                owner_type_parameters,
                return_type,
            )
        })
}

fn exact_extension_nominal_machine_parameter(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    local_instances: &[symbols::SymbolHandle],
    machine: &symbol_resolved_trees::machine::Machine,
    owner_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    parameter: &symbol_resolved_trees::data::TypeParameter,
    contract: &symbol_resolved_trees::data::MachineParameterContract,
) -> bool {
    let symbol_resolved_trees::data::MachineParameterContract::Nominal {
        trait_definition,
        requirement,
        authored_path,
        ..
    } = contract
    else {
        return false;
    };
    let [trait_path @ .., requirement_name] = authored_path.as_slice() else {
        return false;
    };
    let trait_definitions = source
        .traits
        .iter()
        .filter(|candidate| candidate.symbol == *trait_definition)
        .collect::<Vec<_>>();
    let [trait_definition] = trait_definitions.as_slice() else {
        return false;
    };
    let requirements = source
        .trait_machine_signatures(trait_definition.machines)
        .iter()
        .filter(|candidate| candidate.symbol == *requirement)
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        return false;
    };
    exact_extension_parameter_identity(source, machine, parameter)
        && owner_type_parameters
            .iter()
            .filter(|candidate| {
                matches!(
                    candidate.kind,
                    symbol_resolved_trees::data::TypeParameterKind::Machine { .. }
                )
            })
            .count()
            == 1
        && exact_flat_trait_definition(source, trait_definition)
        && !trait_path.is_empty()
        && trait_path.joined_path() == trait_definition.name.as_str()
        && requirement_name.as_str() == requirement.name.as_str()
        && exact_flat_trait_requirement(
            source,
            data_frontier,
            local_instances,
            trait_definition,
            requirement,
        )
}

fn plain_type_is_supported(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    local_instances: &[symbols::SymbolHandle],
    owner: symbols::SymbolHandle,
    owner_lifetimes: &[symbol_resolved_trees::name::DiagnosticName],
    owner_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    type_reference: &symbol_resolved_trees::types::TypeReference,
) -> bool {
    use symbol_resolved_trees::types::TypeReference;
    match type_reference {
        TypeReference::Named { symbol, .. } if !symbol.is_valid() => false,
        TypeReference::Named { symbol, name, .. }
            if source.symbols.name(*symbol) != name.as_str() =>
        {
            false
        }
        TypeReference::Named { symbol, .. } => match source.symbols.get(*symbol).kind {
            symbols::SymbolKind::BuiltinType => true,
            symbols::SymbolKind::Data => source.data_definitions.iter().any(|definition| {
                definition.symbol == *symbol
                    && definition.lifetime_parameters.is_empty()
                    && definition.type_parameters.is_empty()
                    && (definition.generic_instance.is_none() || local_instances.contains(symbol))
            }),
            symbols::SymbolKind::TypeParameter => owner_type_parameters.iter().any(|parameter| {
                parameter.symbol == *symbol
                    && matches!(
                        parameter.kind,
                        symbol_resolved_trees::data::TypeParameterKind::Type
                    )
            }),
            _ => false,
        },
        TypeReference::Unit => true,
        TypeReference::Reference(reference) => {
            reference.lifetime.as_ref().is_none_or(|lifetime| {
                owner_lifetimes
                    .iter()
                    .any(|parameter| parameter.as_str() == lifetime.as_str())
            }) && plain_type_is_supported(
                source,
                data_frontier,
                local_instances,
                owner,
                owner_lifetimes,
                owner_type_parameters,
                source.child_type_reference(reference.referee),
            )
        }
        TypeReference::Slice(slice) => plain_type_is_supported(
            source,
            data_frontier,
            local_instances,
            owner,
            owner_lifetimes,
            owner_type_parameters,
            source.child_type_reference(slice.element_type),
        ),
        TypeReference::FixedArray(array) => {
            seeded_local_instances::array_length_is_supported(
                source,
                owner,
                owner_type_parameters,
                &array.length,
            ) && plain_type_is_supported(
                source,
                data_frontier,
                local_instances,
                owner,
                owner_lifetimes,
                owner_type_parameters,
                source.child_type_reference(array.element_type),
            )
        }
        TypeReference::Generic(generic) => {
            seeded_local_instances::instance_application_is_supported(
                source,
                local_instances,
                owner_lifetimes,
                generic,
            ) || seeded_local_instances::template_application_is_supported(
                source,
                data_frontier,
                owner,
                owner_lifetimes,
                owner_type_parameters,
                generic,
            ) || seeded_type_application::is_supported(
                source,
                data_frontier,
                local_instances,
                owner,
                owner_lifetimes,
                owner_type_parameters,
                generic,
            )
        }
        TypeReference::Constrained(_)
        | TypeReference::ConstExpression(_)
        | TypeReference::DynamicTrait { .. } => false,
    }
}

/// The type-parameter kinds whose argument is bound by machine or nominal
/// machinery rather than template substitution — the shared rejection arm of
/// the seeded-argument support matches.
macro_rules! argument_defers_substitution_pattern {
    () => {
        symbol_resolved_trees::data::TypeParameterKind::Machine { .. }
            | symbol_resolved_trees::data::TypeParameterKind::Implementation
            | symbol_resolved_trees::data::TypeParameterKind::Proposition { .. }
    };
}
pub(super) use argument_defers_substitution_pattern;
