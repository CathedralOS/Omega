use crate::item::lower_item;
use psi_diagnostics::Diagnostic;
use psi_source::SourceMap;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
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

pub fn lower_syntax_trees(
    syntax_trees: &SyntaxTrees,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    lower_syntax_trees_with_optional_sources(syntax_trees, None)
}

pub fn lower_syntax_trees_with_sources(
    syntax_trees: &SyntaxTrees,
    sources: Arc<SourceMap>,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    lower_syntax_trees_with_optional_sources(syntax_trees, Some(sources))
}

fn lower_syntax_trees_with_optional_sources(
    syntax_trees: &SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    let mut syntax_trees = syntax_trees.clone();
    crate::trait_defaults::synthesize_trait_defaults(&mut syntax_trees)?;
    let mut lowerer = Lowerer::new(sources);

    for item in syntax_trees.root_items() {
        lower_item(&mut lowerer, &syntax_trees, item).map_err(|diagnostic| vec![diagnostic])?;
    }

    lowerer.finish()
}

pub(crate) struct Lowerer {
    pub(crate) symbol_resolved_trees: SymbolResolvedTrees,
    /// Authored machine `reaches` names retained only until symbol assignment
    /// builds the canonical service rows. This vector is parallel to the root
    /// machine arena and never enters the published symbol-resolved trees.
    pub(crate) pending_machine_service_reaches:
        Vec<Vec<psi_symbol_resolved_trees::name::DiagnosticName>>,
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
    pub(crate) current_authored_expression_exposure: Option<AuthoredDeclarationSelectionExposure>,
    sources: Option<Arc<SourceMap>>,
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
    fn new(sources: Option<Arc<SourceMap>>) -> Self {
        Self {
            symbol_resolved_trees: SymbolResolvedTrees::default(),
            pending_machine_service_reaches: Vec::new(),
            pending_signature_service_reaches: Vec::new(),
            pending_authored_expressions: Vec::new(),
            pending_authored_proof_memberships: Vec::new(),
            pending_const_declarations: Vec::new(),
            pending_const_selections: Vec::new(),
            current_authored_expression_exposure: None,
            sources,
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

    pub(crate) fn finish(mut self) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
        crate::domain_operator_homes::normalize_domain_operator_homes(
            &mut self.symbol_resolved_trees,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        crate::symbols::assign_symbols(
            &mut self.symbol_resolved_trees,
            self.sources,
            &self.pending_const_declarations,
        );
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
        crate::service_reaches::normalize_service_reaches(
            &mut self.symbol_resolved_trees,
            &pending_machine_service_reaches,
            &pending_signature_service_reaches,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        self.symbol_resolved_trees.rebuild_tables();
        crate::conformance_blocks::route_inline_member_calls(&mut self.symbol_resolved_trees);
        let SymbolResolvedTrees {
            roots,
            tables,
            symbols,
            service_reaches,
            service_reach_rows,
            semantic_domains,
            external_bindings,
            evidence_forwardings,
        } = self.symbol_resolved_trees;

        let mut trees = SymbolResolvedTrees::with_roots(roots, tables, symbols);
        // The interned semantic rows/domains built during lowering survive
        // the rebuild.
        trees.service_reaches = service_reaches;
        trees.service_reach_rows = service_reach_rows;
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
    pub(crate) authored: Vec<psi_symbol_resolved_trees::name::DiagnosticName>,
}

#[cfg(test)]
mod tests;
