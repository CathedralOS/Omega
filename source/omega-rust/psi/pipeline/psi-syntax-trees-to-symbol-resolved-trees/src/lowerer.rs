use crate::item::lower_item;
use psi_diagnostics::Diagnostic;
use psi_source::SourceMap;
use psi_symbol_resolved_trees::{
    AuthoredDeclarationSelections, AuthoredSelectionExtensionFrontier,
    AuthoredSelectionExtensionRebaseError, SymbolResolvedTrees,
};
use psi_syntax_trees::SyntaxTrees;
use std::sync::Arc;

use psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
use psi_symbol_resolved_trees::expression::ExpressionHandle;

#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingAuthoredProofMembership {
    pub(crate) fact: psi_arena::Handle<psi_symbol_resolved_trees::domain::ProofFact>,
    pub(crate) exposure: AuthoredDeclarationSelectionExposure,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingAuthoredExpression {
    pub(crate) expression: ExpressionHandle,
    pub(crate) exposure: AuthoredDeclarationSelectionExposure,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingConstDeclaration {
    pub(crate) semantic_name: String,
    pub(crate) source_span: psi_source::SourceSpan,
    pub(crate) is_public: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingConstSelection {
    pub(crate) expression: ExpressionHandle,
    pub(crate) source_span: psi_source::SourceSpan,
    pub(crate) declaration_ordinal: usize,
    pub(crate) exposure: AuthoredDeclarationSelectionExposure,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingOutcomeSpecificContract {
    pub(crate) contract: psi_arena::Handle<psi_symbol_resolved_trees::signature::SignatureContract>,
    pub(crate) result_data_name: String,
    pub(crate) result_data_source_span: psi_source::SourceSpan,
    pub(crate) result_case_name: String,
}

pub fn lower_syntax_trees(
    syntax_trees: &SyntaxTrees,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    lower_syntax_trees_with_optional_sources(syntax_trees, None, Vec::new())
}

pub fn lower_syntax_trees_with_sources(
    syntax_trees: &SyntaxTrees,
    sources: Arc<SourceMap>,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    lower_syntax_trees_with_optional_sources(syntax_trees, Some(sources), Vec::new())
}

pub fn lower_syntax_trees_with_sources_and_top_level_bindings(
    syntax_trees: &SyntaxTrees,
    sources: Arc<SourceMap>,
    bindings: Vec<psi_symbols::SourceScopedTopLevelBinding>,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    lower_syntax_trees_with_optional_sources(syntax_trees, Some(sources), bindings)
}

/// Append one already-parsed later-stratum syntax forest to an exact retained
/// symbol-resolved base. Existing arenas and symbol tables are consumed and
/// extended in place; no source bytes are read and neither forest is parsed
/// again.
pub fn lower_syntax_extension_against_resolved_base(
    base: SymbolResolvedTrees,
    extension_syntax: &SyntaxTrees,
    sources: Arc<SourceMap>,
    additional_source_scoped_top_level_bindings: Vec<psi_symbols::SourceScopedTopLevelBinding>,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    lower_syntax_extension_with_authored_selection_frontier(
        base,
        extension_syntax,
        sources,
        additional_source_scoped_top_level_bindings,
    )
    .map(SeededSymbolResolvedTrees::into_unrebased_trees)
}

/// Resolve one syntax extension while retaining the exact append frontier of
/// every authored-selection occurrence store.
///
/// The returned carrier is readable, but its trees can enter a later seeded
/// phase only by transactionally rebasing the extension suffix against that
/// phase's exact retained authored-selection ledger.
pub fn lower_syntax_extension_with_authored_selection_frontier(
    base: SymbolResolvedTrees,
    extension_syntax: &SyntaxTrees,
    sources: Arc<SourceMap>,
    additional_source_scoped_top_level_bindings: Vec<psi_symbols::SourceScopedTopLevelBinding>,
) -> Result<SeededSymbolResolvedTrees, Vec<Diagnostic>> {
    let retained_sources = base.symbols.source_files().collect::<Vec<_>>();
    if retained_sources.len() > sources.len()
        || !retained_sources
            .iter()
            .copied()
            .eq(sources.files().take(retained_sources.len()))
    {
        return Err(vec![Diagnostic::error(
            "seeded symbol resolution source map does not retain the exact base frontier",
        )]);
    }
    let authored_selection_frontier = base.authored_selection_extension_frontier();
    let roots = RootWatermarks::capture(&base);
    let retained_service_reaches = base.service_reaches.clone();
    let retained_service_reach_rows = base.service_reach_rows.clone();
    let mut syntax_trees = extension_syntax.clone();
    crate::trait_defaults::synthesize_trait_defaults(&mut syntax_trees)?;
    let mut lowerer = Lowerer::new(Some(sources), additional_source_scoped_top_level_bindings);
    lowerer.seed_resolved_base(base);

    for item in syntax_trees.root_items() {
        lower_item(&mut lowerer, &syntax_trees, item).map_err(|diagnostic| vec![diagnostic])?;
    }
    for selection in &mut lowerer.pending_const_selections {
        selection.declaration_ordinal = selection
            .declaration_ordinal
            .checked_add(roots.const_declarations)
            .expect("seeded const declaration ordinal overflow");
    }

    lowerer
        .finish_with(FinishMode::Seeded {
            roots,
            retained_service_reaches,
            retained_service_reach_rows,
        })
        .map(|trees| SeededSymbolResolvedTrees {
            trees,
            authored_selection_frontier,
        })
}

#[derive(Debug, PartialEq, Eq)]
pub struct SeededSymbolResolvedTrees {
    trees: SymbolResolvedTrees,
    authored_selection_frontier: AuthoredSelectionExtensionFrontier,
}

impl SeededSymbolResolvedTrees {
    pub fn trees(&self) -> &SymbolResolvedTrees {
        &self.trees
    }

    pub fn rebase_authored_selections(
        self,
        destination_base: &AuthoredDeclarationSelections,
    ) -> Result<SymbolResolvedTrees, (Self, AuthoredSelectionExtensionRebaseError)> {
        // This is a representation join, not an authority boundary. The
        // seeded typed continuation must supply the ledger owned by its exact
        // retained base rather than accepting one from compilation input.
        match self
            .trees
            .rebase_authored_selection_extension(self.authored_selection_frontier, destination_base)
        {
            Ok(trees) => Ok(trees),
            Err((trees, error)) => Err((Self { trees, ..self }, error)),
        }
    }

    fn into_unrebased_trees(self) -> SymbolResolvedTrees {
        self.trees
    }
}

fn lower_syntax_trees_with_optional_sources(
    syntax_trees: &SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Vec<psi_symbols::SourceScopedTopLevelBinding>,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    let mut syntax_trees = syntax_trees.clone();
    crate::trait_defaults::synthesize_trait_defaults(&mut syntax_trees)?;
    let mut lowerer = Lowerer::new(sources, source_scoped_top_level_bindings);

    for item in syntax_trees.root_items() {
        lower_item(&mut lowerer, &syntax_trees, item).map_err(|diagnostic| vec![diagnostic])?;
    }

    lowerer.finish()
}

pub(crate) struct Lowerer {
    pub(crate) symbol_resolved_trees: SymbolResolvedTrees,
    /// Authored machine `reaches` clauses retained until symbol assignment
    /// binds every member occurrence to its exact boundary-trait identity.
    pub(crate) pending_machine_service_reaches:
        Vec<crate::service_reaches::PendingAuthoredServiceReach>,
    pub(crate) pending_signature_service_reaches: Vec<PendingSignatureServiceReach>,
    /// Authored expressions whose exact declaration selections are collected
    /// after symbol assignment. Expressions absent from this list are either
    /// compiler-generated or belong to a source surface whose public/private
    /// disposition has not yet been classified by its owning declaration.
    pub(crate) pending_authored_expressions: Vec<PendingAuthoredExpression>,
    pub(crate) pending_authored_proof_memberships: Vec<PendingAuthoredProofMembership>,
    /// Const values disappear during lowering, but their declaration identity
    /// must remain available to package-selection admission.
    pub(crate) pending_const_declarations: Vec<PendingConstDeclaration>,
    pub(crate) pending_const_selections: Vec<PendingConstSelection>,
    /// Outcome paths are validated against the declared result sum during
    /// lowering, then stamped with exact declaration symbols after the shared
    /// symbol-assignment pass has minted those handles.
    pub(crate) pending_outcome_specific_contracts: Vec<PendingOutcomeSpecificContract>,
    pub(crate) current_authored_expression_exposure: Option<AuthoredDeclarationSelectionExposure>,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Vec<psi_symbols::SourceScopedTopLevelBinding>,
    /// Per-lowering counter that mints unique names for synthetic `let`
    /// temporaries hoisted out of operand-position indexed reads (see
    /// `statement::hoist_indexed_operands`). `__hoist_` prefixed so the
    /// generated names cannot collide with source identifiers.
    hoist_counter: u32,
    /// Names of the CURRENT state's parameters declared as a shared reference
    /// to a NAMED type (`table: &EfiSystemTable`). A member read through one
    /// (`table.con_out`) must dereference the pointer slot; the flat fold reads
    /// frame garbage (the entry-ref-param face). The guard/operand hoists use
    /// this to materialize such reads into `let` temps, which lower through the
    /// boot-verified pointee path. Overwritten at each state's lowering.
    pub(crate) reference_struct_parameters: Vec<String>,
    /// ALL of the current state's parameter names -- the computed-index hoist
    /// gate uses this to tell a typeable bare-Name operand (a param, whose
    /// declared type the typed layer resolves via `parameter_type`) from a
    /// LOCAL (untypeable at the hoist-temp layer; hoisting would mint a Unit
    /// temp and a confusing layout error). Overwritten at each state.
    pub(crate) current_state_parameter_names: Vec<String>,
    /// Whether the machine currently being lowered is a BOUNDARY machine. Only
    /// boundary params are REAL pointer slots (the entry hand-off vouches
    /// them); a non-boundary `&Struct` param is a call-site ALIAS slot sharing
    /// the caller's storage, and materializing a pointee deref through one
    /// loads a non-pointer as an address (segfault -- probed 2026-07-04). The
    /// ref-param hoist is scoped to boundary machines.
    pub(crate) current_machine_is_boundary: bool,
    /// Machine contract evidence names are a distinct erased namespace. They
    /// classify bare-name assignments before ordinary value resolution.
    pub(crate) current_machine_root_index: Option<usize>,
    pub(crate) current_machine_name: Option<String>,
    pub(crate) current_state_name: Option<String>,
    pub(crate) current_evidence_term_names: Vec<String>,
    /// Maps a match SUBJECT syntax expression handle to the name of the single
    /// hoisted temp for it. All arms of one enum-variant match share the same
    /// syntax subject handle (the parser reuses it across arms), so the first
    /// arm mints `let __hoist_N = <subject>` and the siblings reuse the name --
    /// keeping ONE shared subject so match exhaustiveness still groups the arms
    /// (`statement::hoist_membership_match_subject`). Keyed by the subject
    /// syntax handle's arena index (`Handle` is not `Hash`).
    match_subject_temps: std::collections::HashMap<u32, String>,
    /// The CURRENT state's parameters (name + resolved type) -- the
    /// guarded-arm value-call rewrite copies parameter records into its
    /// synthesized continuation state. Overwritten at each state.
    pub(crate) current_state_parameters: Vec<(
        String,
        psi_symbol_resolved_trees::types::TypeReference,
        bool,
    )>,
    /// The CURRENT state's explicit `self` parameter, retained so an
    /// arm-selected synthesized continuation can carry the same receiver.
    pub(crate) current_state_self_parameter:
        Option<psi_symbol_resolved_trees::signature::StateParameter>,
    /// Explicitly typed locals declared so far in the CURRENT state. A
    /// guarded arm continuation may carry them across its generated edge.
    pub(crate) current_state_locals: Vec<(
        String,
        psi_symbol_resolved_trees::types::TypeReference,
        bool,
    )>,
    /// The CURRENT state's declared return type -- the synthesized
    /// continuation state returns the same type. Overwritten at each state.
    pub(crate) current_state_return_type: Option<psi_symbol_resolved_trees::types::TypeReference>,
    /// Continuation states synthesized by the guarded-arm value-call rewrite
    /// (`cond -> (call(a, b))` becomes `cond -> __arm_k_N(a, b)` plus a
    /// state whose Always terminal hoists the call). Drained by the machine
    /// lowering after the authored states.
    pub(crate) pending_synthesized_states: Vec<SynthesizedArmState>,
    /// Continuation states that evaluate guarded named-target call arguments
    /// only after their arm is selected.
    pub(crate) pending_synthesized_transition_argument_states:
        Vec<SynthesizedTransitionArgumentState>,
    arm_state_counter: u32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RootWatermarks {
    pub(crate) const_declarations: usize,
    pub(crate) data_definitions: usize,
    pub(crate) domain_definitions: usize,
    pub(crate) machines: usize,
    pub(crate) operators: usize,
    pub(crate) propositions: usize,
    pub(crate) traits: usize,
    pub(crate) conformances: usize,
    pub(crate) wire_schemas: usize,
}

impl RootWatermarks {
    fn capture(program: &SymbolResolvedTrees) -> Self {
        Self {
            const_declarations: program.const_declarations.len(),
            data_definitions: program.data_definitions.len(),
            domain_definitions: program.domain_definitions.len(),
            machines: program.machines.len(),
            operators: program.operators.len(),
            propositions: program.propositions.len(),
            traits: program.traits.len(),
            conformances: program.conformances.len(),
            wire_schemas: program.wire_schemas.len(),
        }
    }
}

enum FinishMode {
    Complete,
    Seeded {
        roots: RootWatermarks,
        retained_service_reaches: psi_language_semantics::ServiceReachTable,
        retained_service_reach_rows: psi_language_semantics::ServiceReachRowTable,
    },
}

/// One continuation state the guarded-arm value-call rewrite synthesizes.
pub(crate) struct SynthesizedArmState {
    pub(crate) name: String,
    pub(crate) parameters: Vec<(String, psi_symbol_resolved_trees::types::TypeReference)>,
    pub(crate) return_type: psi_symbol_resolved_trees::types::TypeReference,
    /// The original call expression -- its Name arguments resolve against
    /// the synthesized state's SAME-named parameters.
    pub(crate) call: psi_symbol_resolved_trees::expression::ExpressionHandle,
}

/// One arm-selected continuation that materializes direct value-call target
/// arguments, then performs the original named transition.
pub(crate) struct SynthesizedTransitionArgumentState {
    pub(crate) name: String,
    pub(crate) self_parameter: Option<psi_symbol_resolved_trees::signature::StateParameter>,
    pub(crate) parameters: Vec<(
        String,
        psi_symbol_resolved_trees::types::TypeReference,
        bool,
    )>,
    pub(crate) return_type: Option<psi_symbol_resolved_trees::types::TypeReference>,
    pub(crate) target: psi_symbol_resolved_trees::statement::NamedTransitionTarget,
    pub(crate) calls: Vec<psi_symbol_resolved_trees::expression::ExpressionHandle>,
}

impl Lowerer {
    fn new(
        sources: Option<Arc<SourceMap>>,
        source_scoped_top_level_bindings: Vec<psi_symbols::SourceScopedTopLevelBinding>,
    ) -> Self {
        Self {
            symbol_resolved_trees: SymbolResolvedTrees::default(),
            pending_machine_service_reaches: Vec::new(),
            pending_signature_service_reaches: Vec::new(),
            pending_authored_expressions: Vec::new(),
            pending_authored_proof_memberships: Vec::new(),
            pending_const_declarations: Vec::new(),
            pending_const_selections: Vec::new(),
            pending_outcome_specific_contracts: Vec::new(),
            current_authored_expression_exposure: None,
            sources,
            source_scoped_top_level_bindings,
            hoist_counter: 0,
            reference_struct_parameters: Vec::new(),
            current_state_parameter_names: Vec::new(),
            current_machine_is_boundary: false,
            current_machine_root_index: None,
            current_machine_name: None,
            current_state_name: None,
            current_evidence_term_names: Vec::new(),
            match_subject_temps: std::collections::HashMap::new(),
            current_state_parameters: Vec::new(),
            current_state_self_parameter: None,
            current_state_locals: Vec::new(),
            current_state_return_type: None,
            pending_synthesized_states: Vec::new(),
            pending_synthesized_transition_argument_states: Vec::new(),
            arm_state_counter: 0,
        }
    }

    fn seed_resolved_base(&mut self, base: SymbolResolvedTrees) {
        self.pending_const_declarations = base
            .const_declarations
            .iter()
            .map(|declaration| PendingConstDeclaration {
                semantic_name: base.symbols.name(declaration.symbol).to_owned(),
                source_span: base
                    .symbols
                    .symbol_source_span(declaration.symbol)
                    .unwrap_or_default(),
                is_public: declaration.is_public,
            })
            .collect();
        self.pending_machine_service_reaches = base
            .machines
            .iter()
            .map(|machine| pending_service_reach_for(&base, machine.symbol))
            .collect();
        for definition in &base.traits {
            for offset in 0..definition.machines.len() {
                let handle = handle_at_offset(definition.machines, offset);
                let signature = base
                    .tables
                    .declarations
                    .trait_machine_signatures
                    .get(handle);
                let pending = pending_service_reach_for(&base, signature.symbol);
                self.pending_signature_service_reaches
                    .push(PendingSignatureServiceReach {
                        location: PendingSignatureLocation::Trait(handle),
                        owner: PendingSignatureOwner::Trait(definition.name.clone()),
                        keyword_source_spans: pending.keyword_source_spans,
                        authored: pending.authored,
                    });
            }
        }
        for (handle, parameter) in base.tables.declarations.data_type_parameters.iter() {
            let psi_symbol_resolved_trees::data::TypeParameterKind::Machine { contract } =
                &parameter.kind
            else {
                continue;
            };
            let Some(signature) = contract.structural() else {
                continue;
            };
            let pending = pending_service_reach_for(&base, signature.symbol);
            self.pending_signature_service_reaches
                .push(PendingSignatureServiceReach {
                    location: PendingSignatureLocation::MachineParameter(handle),
                    owner: PendingSignatureOwner::Requirement(parameter.name.clone()),
                    keyword_source_spans: pending.keyword_source_spans,
                    authored: pending.authored,
                });
        }
        self.symbol_resolved_trees = base;
    }

    pub(crate) fn source_reference_can_see_declaration(
        &self,
        reference: psi_source::SourceSpan,
        declaration: psi_source::SourceSpan,
    ) -> bool {
        self.sources
            .as_deref()
            .is_none_or(|sources| sources.reference_can_see_declaration(reference, declaration))
    }

    pub(crate) fn source_resolution_strata_separate(
        &self,
        left: psi_source::SourceSpan,
        right: psi_source::SourceSpan,
    ) -> bool {
        self.sources
            .as_deref()
            .is_some_and(|sources| sources.resolution_strata_separate(left, right))
    }

    pub(crate) fn next_arm_state_name(&mut self) -> String {
        let name = format!("__arm_k_{}", self.arm_state_counter);
        self.arm_state_counter += 1;
        name
    }

    pub(crate) fn with_authored_expression_exposure<T>(
        &mut self,
        exposure: AuthoredDeclarationSelectionExposure,
        operation: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        let previous = self.current_authored_expression_exposure.replace(exposure);
        let result = operation(self);
        self.current_authored_expression_exposure = previous;
        result
    }

    /// The shared hoist-temp name for a match subject syntax handle, if the first arm already
    /// minted one; otherwise records `name` as the temp for later arms and returns None.
    pub(crate) fn match_subject_temp(&mut self, subject_arena_index: u32) -> Option<String> {
        self.match_subject_temps.get(&subject_arena_index).cloned()
    }

    pub(crate) fn record_match_subject_temp(&mut self, subject_arena_index: u32, name: String) {
        self.match_subject_temps.insert(subject_arena_index, name);
    }

    pub(crate) fn next_hoist_name(&mut self) -> String {
        let name = format!("__hoist_{}", self.hoist_counter);
        self.hoist_counter += 1;
        name
    }

    pub(crate) fn finish(self) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
        self.finish_with(FinishMode::Complete)
    }

    fn finish_with(
        mut self,
        finish_mode: FinishMode,
    ) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
        crate::domain_operator_homes::normalize_domain_operator_homes(
            &mut self.symbol_resolved_trees,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        match &finish_mode {
            FinishMode::Complete => crate::symbols::assign_symbols(
                &mut self.symbol_resolved_trees,
                self.sources,
                self.source_scoped_top_level_bindings,
                &self.pending_const_declarations,
            )?,
            FinishMode::Seeded { roots, .. } => {
                let sources = self.sources.ok_or_else(|| {
                    vec![Diagnostic::error(
                        "seeded symbol resolution requires retained source custody",
                    )]
                })?;
                crate::symbols::assign_symbols_against_resolved_base(
                    &mut self.symbol_resolved_trees,
                    sources,
                    self.source_scoped_top_level_bindings,
                    *roots,
                    &self.pending_const_declarations,
                )?;
            }
        }
        crate::state::finalize_outcome_specific_contract_symbols(
            &mut self.symbol_resolved_trees,
            &self.pending_outcome_specific_contracts,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        crate::constant::finalize_const_declarations(
            &mut self.symbol_resolved_trees,
            &self.pending_const_declarations,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        crate::constant::finalize_const_selections(
            &mut self.symbol_resolved_trees,
            &self.pending_const_selections,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        crate::authored_selections::finalize_authored_expression_selections(
            &mut self.symbol_resolved_trees,
            &self.pending_authored_expressions,
            &self.pending_authored_proof_memberships,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let compatibility =
            crate::signature_free_requirements::validate_signature_free_requirement_compatibility(
                &self.symbol_resolved_trees,
            );
        if !compatibility.is_empty() {
            return Err(compatibility);
        }
        crate::machine_parameter_requirements::normalize_nominal_machine_parameter_requirements(
            &mut self.symbol_resolved_trees,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        crate::machine_parameter_requirements::normalize_trait_machine_requirement_arguments(
            &mut self.symbol_resolved_trees,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        bind_evidence_forwarding_owners(&mut self.symbol_resolved_trees);
        assert_eq!(
            self.symbol_resolved_trees.machines.len(),
            self.pending_machine_service_reaches.len(),
            "each initially resolved root machine has one pending authored service row"
        );
        let pending_machine_service_reaches = self
            .symbol_resolved_trees
            .machines
            .iter()
            .zip(&self.pending_machine_service_reaches)
            .map(|(machine, reaches)| (machine.symbol, reaches.clone()))
            .collect::<Vec<_>>();
        let pending_signature_service_reaches = self
            .pending_signature_service_reaches
            .iter()
            .map(|pending| {
                let symbol = match pending.location {
                    PendingSignatureLocation::Trait(handle) => {
                        self.symbol_resolved_trees
                            .tables
                            .declarations
                            .trait_machine_signatures
                            .get(handle)
                            .symbol
                    }
                    PendingSignatureLocation::MachineParameter(handle) => {
                        self.symbol_resolved_trees
                            .tables
                            .declarations
                            .data_type_parameters
                            .get(handle)
                            .symbol
                    }
                };
                crate::service_reaches::PendingSignatureServiceReach {
                    symbol,
                    owner: pending.owner.clone(),
                    keyword_source_spans: pending.keyword_source_spans.clone(),
                    authored: pending.authored.clone(),
                }
            })
            .collect::<Vec<_>>();
        crate::conformance_blocks::normalize_closed_conformance_blocks(
            &mut self.symbol_resolved_trees,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        crate::authored_selections::finalize_conformance_reference_selections(
            &mut self.symbol_resolved_trees,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        crate::domain_establishment::normalize_domain_establishment_routes(
            &mut self.symbol_resolved_trees,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        match finish_mode {
            FinishMode::Complete => crate::service_reaches::normalize_service_reaches(
                &mut self.symbol_resolved_trees,
                &pending_machine_service_reaches,
                &pending_signature_service_reaches,
            ),
            FinishMode::Seeded {
                retained_service_reaches,
                retained_service_reach_rows,
                ..
            } => crate::service_reaches::normalize_service_reaches_with_retained_tables(
                &mut self.symbol_resolved_trees,
                &pending_machine_service_reaches,
                &pending_signature_service_reaches,
                retained_service_reaches,
                retained_service_reach_rows,
            ),
        }
        .map_err(|diagnostic| vec![diagnostic])?;
        self.symbol_resolved_trees.rebuild_tables();
        crate::conformance_blocks::route_inline_member_calls(&mut self.symbol_resolved_trees);
        let SymbolResolvedTrees {
            roots,
            tables,
            symbols,
            service_reaches,
            service_reach_rows,
            authored_service_reach_rows,
            semantic_domains,
            external_bindings,
            evidence_forwardings,
        } = self.symbol_resolved_trees;

        let mut trees = SymbolResolvedTrees::with_roots(roots, tables, symbols);
        // The interned semantic rows/domains built during lowering survive
        // the rebuild.
        trees.service_reaches = service_reaches;
        trees.service_reach_rows = service_reach_rows;
        trees.authored_service_reach_rows = authored_service_reach_rows;
        trees.semantic_domains = semantic_domains;
        trees.external_bindings = external_bindings;
        trees.evidence_forwardings = evidence_forwardings;
        Ok(trees)
    }
}

fn bind_evidence_forwarding_owners(program: &mut SymbolResolvedTrees) {
    let mut owners = Vec::new();
    let mut incoming_evidence_names = Vec::new();
    for (machine_root_index, machine) in program.machines.iter().enumerate() {
        incoming_evidence_names.extend(program.machine_contracts(machine).iter().filter_map(
            |contract| {
                (contract.kind
                    == psi_symbol_resolved_trees::signature::SignatureContractKind::Requires)
                    .then_some(contract.binding.as_ref())
                    .flatten()
                    .map(|binding| (machine.symbol, binding.as_str().to_owned()))
            },
        ));
        for state_handle in program.machine_state_handles(machine.states) {
            let state = program.machine_state(*state_handle);
            owners.push((
                machine_root_index,
                state.name.as_str().to_owned(),
                machine.symbol,
                state.symbol,
            ));
        }
    }
    let subjectless_conformances = program
        .conformances
        .iter()
        .filter_map(|conformance| {
            (matches!(
                conformance.subject,
                psi_symbol_resolved_trees::trait_definition::ConformanceSubject::Subjectless
            ))
            .then(|| {
                conformance
                    .alias
                    .as_ref()
                    .map(|alias| (alias.as_str().to_owned(), conformance.symbol))
            })
            .flatten()
        })
        .collect::<Vec<_>>();
    for forwarding in &mut program.evidence_forwardings {
        if let Some((_, _, machine_symbol, state_symbol)) =
            owners
                .iter()
                .find(|(machine_root_index, state_name, _, _)| {
                    *machine_root_index == forwarding.machine_root_index
                        && state_name == forwarding.state_name.as_str()
                })
        {
            forwarding.machine_symbol = *machine_symbol;
            forwarding.state_symbol = *state_symbol;
        }
        if !incoming_evidence_names.iter().any(|(machine, name)| {
            *machine == forwarding.machine_symbol && name == forwarding.source.as_str()
        }) {
            forwarding.source_conformance =
                subjectless_conformances.iter().find_map(|(alias, symbol)| {
                    (alias == forwarding.source.as_str()).then_some(*symbol)
                });
        }
    }
}

fn pending_service_reach_for(
    program: &SymbolResolvedTrees,
    owner: psi_symbols::SymbolHandle,
) -> crate::service_reaches::PendingAuthoredServiceReach {
    let Some(row) = program
        .authored_service_reach_rows
        .iter()
        .find(|row| row.owner == owner)
    else {
        return crate::service_reaches::PendingAuthoredServiceReach {
            keyword_source_spans: Vec::new(),
            authored: Vec::new(),
        };
    };
    crate::service_reaches::PendingAuthoredServiceReach {
        keyword_source_spans: row.keyword_source_spans.clone(),
        authored: row
            .targets
            .iter()
            .map(|target| {
                psi_symbol_resolved_trees::name::DiagnosticName::new(
                    program.symbols.name(target.service),
                    target.source_span,
                )
            })
            .collect(),
    }
}

fn handle_at_offset<T>(span: psi_arena::HandleSpan<T>, offset: usize) -> psi_arena::Handle<T> {
    psi_arena::Handle::from_parts(
        span.start()
            .arena_index()
            .checked_add(u32::try_from(offset).expect("handle-span offset fits u32"))
            .expect("handle-span offset overflow"),
        span.start().generation(),
    )
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PendingSignatureLocation {
    Trait(psi_arena::Handle<psi_symbol_resolved_trees::signature::StateSignature>),
    MachineParameter(psi_arena::Handle<psi_symbol_resolved_trees::data::TypeParameter>),
}

#[derive(Debug, Clone)]
pub(crate) enum PendingSignatureOwner {
    Trait(psi_symbol_resolved_trees::name::DiagnosticName),
    Requirement(psi_symbol_resolved_trees::name::DiagnosticName),
}

#[derive(Debug, Clone)]
pub(crate) struct PendingSignatureServiceReach {
    pub(crate) location: PendingSignatureLocation,
    pub(crate) owner: PendingSignatureOwner,
    pub(crate) keyword_source_spans: Vec<psi_source::SourceSpan>,
    pub(crate) authored: Vec<psi_symbol_resolved_trees::name::DiagnosticName>,
}

#[cfg(test)]
mod tests;
