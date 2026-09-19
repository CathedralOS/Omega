//! The lowerer's working state.
//!
//! `Lowerer` carries the carrier under construction plus the pending sidecars
//! each translator leaves for a later phase: authored expressions awaiting
//! selection, constant declarations and selections, service-reach names,
//! outcome-specific contracts, synthesized continuation states. `resolution`
//! drives it; nothing here decides phase order. Seeding fills the same
//! sidecars from a retained base so an extension resolves against it.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
use source::SourceMap;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::ExpressionHandle;

#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingAuthoredProofMembership {
    pub(crate) fact: arena::Handle<symbol_resolved_trees::domain::ProofFact>,
    pub(crate) exposure: AuthoredDeclarationSelectionExposure,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingAuthoredExpression {
    pub(crate) expression: ExpressionHandle,
    pub(crate) exposure: AuthoredDeclarationSelectionExposure,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingConstDeclaration {
    pub(crate) scope: syntax_trees::identifier::Identifier,
    pub(crate) semantic_name: String,
    pub(crate) source_span: source::SourceSpan,
    pub(crate) is_public: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingConstSelection {
    pub(crate) expression: ExpressionHandle,
    pub(crate) source_span: source::SourceSpan,
    pub(crate) declaration_ordinal: usize,
    pub(crate) exposure: AuthoredDeclarationSelectionExposure,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingConstArgumentSelection {
    pub(crate) origin: syntax_trees::types::ConstArgumentOrigin,
    pub(crate) exposure: AuthoredDeclarationSelectionExposure,
}

/// A use-site constant occurrence and its actual retained generic argument slot.
/// This private link survives symbol allocation; encoded value labels never
/// reconstruct which nominal binder received the selected declaration.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingConstArgumentSlot {
    pub(crate) selection: usize,
    pub(crate) arguments: arena::HandleSpan<symbol_resolved_trees::types::TypeReference>,
    pub(crate) ordinal: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingOutcomeSpecificContract {
    pub(crate) contract: arena::Handle<symbol_resolved_trees::signature::SignatureContract>,
    pub(crate) result_data_name: String,
    pub(crate) result_data_source_span: source::SourceSpan,
    pub(crate) result_case_name: String,
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ConstResolutionMode {
    #[default]
    Complete,
    ArgumentSelection,
    InitializerSelection,
}
pub(crate) struct Lowerer {
    pub(crate) const_resolution_mode: ConstResolutionMode,
    pub(crate) constant_selection:
        Option<crate::preparation::generic_data::constant_selection::ConstantSelection<'static>>,
    pub(crate) namespace_declarations: crate::symbols::NamespaceDeclarations,
    pub(crate) pending_static_module_calls: Vec<(
        symbol_resolved_trees::expression::ExpressionHandle,
        Vec<symbol_resolved_trees::name::DiagnosticName>,
    )>,
    pub(crate) pending_static_module_statement_calls: Vec<(
        source::SourceSpan,
        Vec<symbol_resolved_trees::name::DiagnosticName>,
    )>,
    pub(crate) symbol_resolved_trees: SymbolResolvedTrees,
    /// Authored machine `reaches` clauses retained until symbol assignment
    /// binds every member occurrence to its exact boundary-trait identity.
    pub(crate) pending_machine_service_reaches:
        Vec<crate::selection::service_reaches::PendingAuthoredServiceReach>,
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
    pub(crate) pending_const_argument_selections: Vec<PendingConstArgumentSelection>,
    pub(crate) pending_const_argument_slots: Vec<PendingConstArgumentSlot>,
    pub(crate) derived_const_argument_origins: Vec<syntax_trees::types::ConstArgumentOrigin>,
    pub(crate) derived_const_argument_expressions: Vec<syntax_trees::expression::ExpressionHandle>,
    pub(crate) pending_const_argument_expressions: Vec<ExpressionHandle>,
    pub(crate) derived_const_argument_builtin_operators: Vec<source::SourceSpan>,
    pub(crate) pending_const_selections: Vec<PendingConstSelection>,
    /// Newly authored initializer roots awaiting declaration-side resolution.
    /// Retained base roots are already resolved and must not enter this list.
    pub(crate) pending_const_values: Vec<ExpressionHandle>,
    pub(crate) pending_const_initializers:
        Vec<crate::constant::initializer_normalization::PendingInitializer>,
    /// Outcome paths are validated against the declared result sum during
    /// lowering, then stamped with exact declaration symbols after the shared
    /// symbol-assignment pass has minted those handles.
    pub(crate) pending_outcome_specific_contracts: Vec<PendingOutcomeSpecificContract>,
    pub(crate) current_authored_expression_exposure: Option<AuthoredDeclarationSelectionExposure>,
    /// Transient partition for one compiler-instantiated trait-default
    /// application. It separates copied authored selections that may resolve
    /// differently in distinct conformances and is cleared between machines.
    pub(crate) current_compiler_selection_partition:
        Option<language_semantics::declaration_selection::CompilerDerivedSelectionPartition>,
    pub(crate) sources: Option<Arc<SourceMap>>,
    pub(crate) source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
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
    /// Authored state names remain transition candidates until resolution.
    pub(crate) current_machine_state_names: Vec<String>,
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
    pub(crate) current_state_parameters:
        Vec<(String, symbol_resolved_trees::types::TypeReference, bool)>,
    /// The CURRENT state's explicit `self` parameter, retained so an
    /// arm-selected synthesized continuation can carry the same receiver.
    pub(crate) current_state_self_parameter:
        Option<symbol_resolved_trees::signature::StateParameter>,
    /// Explicitly typed locals declared so far in the CURRENT state. A
    /// guarded arm continuation may carry them across its generated edge.
    pub(crate) current_state_locals:
        Vec<(String, symbol_resolved_trees::types::TypeReference, bool)>,
    /// The CURRENT state's declared return type -- the synthesized
    /// continuation state returns the same type. Overwritten at each state.
    pub(crate) current_state_return_type: Option<symbol_resolved_trees::types::TypeReference>,
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
    /// Present when this lowerer extends a retained base.
    pub(crate) seed: Option<BaseSeed>,
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct RootWatermarks {
    pub(crate) const_declarations: usize,
    pub(crate) data_definitions: usize,
    pub(crate) domain_definitions: usize,
    pub(crate) machines: usize,
    pub(crate) operators: usize,
    pub(crate) measures: usize,
    pub(crate) propositions: usize,
    pub(crate) mathematical_definitions: usize,
    pub(crate) traits: usize,
    pub(crate) conformances: usize,
    pub(crate) wire_schemas: usize,
}

impl RootWatermarks {
    pub(crate) fn capture(program: &SymbolResolvedTrees) -> Self {
        Self {
            const_declarations: program.const_declarations.len(),
            data_definitions: program.data_definitions.len(),
            domain_definitions: program.domain_definitions.len(),
            machines: program.machines.len(),
            operators: program.operators.len(),
            measures: program.measures.len(),
            propositions: program.propositions.len(),
            mathematical_definitions: program.mathematical_definitions.len(),
            traits: program.traits.len(),
            conformances: program.conformances.len(),
            wire_schemas: program.wire_schemas.len(),
        }
    }
}
/// What a seeded lowerer keeps from its base until finishing consumes it.
pub(crate) struct BaseSeed {
    pub(crate) roots: RootWatermarks,
    pub(crate) service_reaches: language_semantics::ServiceReachTable,
    pub(crate) service_reach_rows: language_semantics::ServiceReachRowTable,
}

/// One continuation state the guarded-arm value-call rewrite synthesizes.
pub(crate) struct SynthesizedArmState {
    pub(crate) name: String,
    pub(crate) parameters: Vec<(String, symbol_resolved_trees::types::TypeReference)>,
    pub(crate) return_type: symbol_resolved_trees::types::TypeReference,
    /// The original call expression -- its Name arguments resolve against
    /// the synthesized state's SAME-named parameters.
    pub(crate) call: symbol_resolved_trees::expression::ExpressionHandle,
}

/// One arm-selected continuation that materializes direct value-call target
/// arguments, then performs the original named transition.
pub(crate) struct SynthesizedTransitionArgumentState {
    pub(crate) name: String,
    pub(crate) self_parameter: Option<symbol_resolved_trees::signature::StateParameter>,
    pub(crate) parameters: Vec<(String, symbol_resolved_trees::types::TypeReference, bool)>,
    pub(crate) return_type: Option<symbol_resolved_trees::types::TypeReference>,
    pub(crate) target: symbol_resolved_trees::statement::NamedTransitionTarget,
    pub(crate) calls: Vec<symbol_resolved_trees::expression::ExpressionHandle>,
}
impl Lowerer {
    pub(crate) fn new(
        sources: Option<Arc<SourceMap>>,
        source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    ) -> Self {
        Self {
            const_resolution_mode: ConstResolutionMode::Complete,
            constant_selection: None,
            namespace_declarations: crate::symbols::NamespaceDeclarations::default(),
            pending_static_module_calls: Vec::new(),
            pending_static_module_statement_calls: Vec::new(),
            symbol_resolved_trees: SymbolResolvedTrees::default(),
            pending_machine_service_reaches: Vec::new(),
            pending_signature_service_reaches: Vec::new(),
            pending_authored_expressions: Vec::new(),
            pending_authored_proof_memberships: Vec::new(),
            pending_const_declarations: Vec::new(),
            pending_const_selections: Vec::new(),
            pending_const_values: Vec::new(),
            pending_const_initializers: Vec::new(),
            pending_outcome_specific_contracts: Vec::new(),
            current_authored_expression_exposure: None,
            pending_const_argument_selections: Vec::new(),
            pending_const_argument_slots: Vec::new(),
            derived_const_argument_origins: Vec::new(),
            derived_const_argument_expressions: Vec::new(),
            pending_const_argument_expressions: Vec::new(),
            derived_const_argument_builtin_operators: Vec::new(),
            current_compiler_selection_partition: None,
            sources,
            source_scoped_top_level_bindings,
            hoist_counter: 0,
            reference_struct_parameters: Vec::new(),
            current_state_parameter_names: Vec::new(),
            current_machine_is_boundary: false,
            current_machine_root_index: None,
            current_machine_name: None,
            current_machine_state_names: Vec::new(),
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
            seed: None,
        }
    }

    /// Extend a retained base: its source frontier must be the exact prefix of
    /// this lowerer's sources, and its declarations become the pending
    /// sidecars a later stratum resolves against.
    pub(crate) fn seed_resolved_base(
        &mut self,
        base: SymbolResolvedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(sources) = &self.sources else {
            return Err(vec![Diagnostic::error(
                "seeded symbol resolution requires retained source custody",
            )]);
        };
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
        self.seed = Some(BaseSeed {
            roots: RootWatermarks::capture(&base),
            service_reaches: base.service_reaches.clone(),
            service_reach_rows: base.service_reach_rows.clone(),
        });
        self.pending_const_declarations = base
            .const_declarations
            .iter()
            .map(|declaration| PendingConstDeclaration {
                scope: syntax_trees::identifier::Identifier::generated(""),
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
            let symbol_resolved_trees::data::TypeParameterKind::Machine { contract } =
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
        Ok(())
    }

    pub(crate) fn source_reference_can_see_declaration(
        &self,
        reference: source::SourceSpan,
        declaration: source::SourceSpan,
    ) -> bool {
        self.sources
            .as_deref()
            .is_none_or(|sources| sources.reference_can_see_declaration(reference, declaration))
    }

    pub(crate) fn source_resolution_strata_separate(
        &self,
        left: source::SourceSpan,
        right: source::SourceSpan,
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

    /// The constant selector lowering ran under, handed back before finishing.
    pub(crate) fn take_constant_selection(
        &mut self,
    ) -> Result<
        crate::preparation::generic_data::constant_selection::ConstantSelection<'static>,
        Vec<Diagnostic>,
    > {
        self.constant_selection.take().ok_or_else(|| {
            vec![Diagnostic::error(
                "constant preparation lost its source-aware selector",
            )]
        })
    }

    /// The finished trees: tables rebuilt from the lowered roots, with the
    /// interned semantic rows and domains built during lowering kept.
    pub(crate) fn into_trees(self) -> SymbolResolvedTrees {
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
        trees.service_reaches = service_reaches;
        trees.service_reach_rows = service_reach_rows;
        trees.authored_service_reach_rows = authored_service_reach_rows;
        trees.semantic_domains = semantic_domains;
        trees.external_bindings = external_bindings;
        trees.evidence_forwardings = evidence_forwardings;
        trees
    }

    /// Pair every root machine and pending signature with its authored
    /// service-reach names once their symbols exist.
    pub(crate) fn pending_service_reaches(
        &self,
    ) -> (
        Vec<(
            symbols::SymbolHandle,
            crate::selection::service_reaches::PendingAuthoredServiceReach,
        )>,
        Vec<crate::selection::service_reaches::PendingSignatureServiceReach>,
    ) {
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
                crate::selection::service_reaches::PendingSignatureServiceReach {
                    symbol,
                    owner: pending.owner.clone(),
                    keyword_source_spans: pending.keyword_source_spans.clone(),
                    authored: pending.authored.clone(),
                }
            })
            .collect::<Vec<_>>();
        (
            pending_machine_service_reaches,
            pending_signature_service_reaches,
        )
    }
}

fn pending_service_reach_for(
    program: &SymbolResolvedTrees,
    owner: symbols::SymbolHandle,
) -> crate::selection::service_reaches::PendingAuthoredServiceReach {
    let Some(row) = program
        .authored_service_reach_rows
        .iter()
        .find(|row| row.owner == owner)
    else {
        return crate::selection::service_reaches::PendingAuthoredServiceReach {
            keyword_source_spans: Vec::new(),
            authored: Vec::new(),
        };
    };
    crate::selection::service_reaches::PendingAuthoredServiceReach {
        keyword_source_spans: row.keyword_source_spans.clone(),
        authored: row
            .targets
            .iter()
            .map(|target| {
                symbol_resolved_trees::name::DiagnosticName::from_str(
                    program.symbols.name(target.service),
                    target.source_span,
                )
            })
            .collect(),
    }
}

fn handle_at_offset<T>(span: arena::HandleSpan<T>, offset: usize) -> arena::Handle<T> {
    arena::Handle::from_parts(
        span.start()
            .arena_index()
            .checked_add(u32::try_from(offset).expect("handle-span offset fits u32"))
            .expect("handle-span offset overflow"),
        span.start().generation(),
    )
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PendingSignatureLocation {
    Trait(arena::Handle<symbol_resolved_trees::signature::StateSignature>),
    MachineParameter(arena::Handle<symbol_resolved_trees::data::TypeParameter>),
}

#[derive(Debug, Clone)]
pub(crate) enum PendingSignatureOwner {
    Trait(symbol_resolved_trees::name::DiagnosticName),
    Requirement(symbol_resolved_trees::name::DiagnosticName),
}

#[derive(Debug, Clone)]
pub(crate) struct PendingSignatureServiceReach {
    pub(crate) location: PendingSignatureLocation,
    pub(crate) owner: PendingSignatureOwner,
    pub(crate) keyword_source_spans: Vec<source::SourceSpan>,
    pub(crate) authored: Vec<symbol_resolved_trees::name::DiagnosticName>,
}
