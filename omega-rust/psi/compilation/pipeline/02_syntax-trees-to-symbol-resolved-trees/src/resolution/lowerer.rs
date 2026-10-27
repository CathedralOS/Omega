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
use source::{SourceMap, SourceSpan};
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

/// An arm's lexical aliases can only read their own enclosing capture and
/// exact structural head. This stack is construction scratch, not a name map.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MatchArmCapture {
    pub(crate) syntax_capture: arena::Handle<syntax_trees::expression::MatchSubjectCapture>,
    pub(crate) syntax_head: syntax_trees::expression::ExpressionHandle,
    pub(crate) resolved_capture:
        arena::Handle<symbol_resolved_trees::expression::MatchSubjectCapture>,
    pub(crate) resolved_head: ExpressionHandle,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingConstDeclaration {
    pub(crate) declaration: arena::Handle<symbol_resolved_trees::constant::ConstDeclaration>,
    pub(crate) scope: syntax_trees::identifier::Identifier,
    pub(crate) semantic_name: String,
    pub(crate) source_span: source::SourceSpan,
    pub(crate) is_public: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingConstSelection {
    pub(crate) expression: ExpressionHandle,
    pub(crate) source_span: source::SourceSpan,
    pub(crate) declaration: symbols::SymbolHandle,
    pub(crate) exposure: AuthoredDeclarationSelectionExposure,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingConstArgumentSelection {
    pub(crate) origin: syntax_trees::types::ConstArgumentOrigin,
    /// Copying into a generated instance retains semantic custody without
    /// creating a new authored interface occurrence.
    pub(crate) exposure: Option<AuthoredDeclarationSelectionExposure>,
    /// The selected declaration supplies the complete argument at this
    /// occurrence. Other retained origins are dependencies of its computation;
    /// their declared carriers do not become the receiving parameter's type.
    pub(crate) is_argument_value: bool,
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ConstResolutionMode {
    #[default]
    Complete,
    ArgumentSelection,
    InitializerSelection,
}
pub(crate) struct Lowerer {
    pub(crate) defer_machine_bodies: bool,
    pub(crate) pending_machine_bodies: Vec<crate::lowering::machine::PendingMachineBody>,
    pub(crate) pending_type_inputs: Vec<super::type_inputs::LoweredTypeInputs>,
    pub(crate) supplied_operand_owners: Vec<(
        syntax_trees::item::ItemHandle,
        symbol_resolved_trees::types::OperandLifetimeOwnerHandle,
    )>,
    pub(crate) conformance_inputs: Vec<(syntax_trees::item::ItemHandle, usize)>,
    pub(crate) encountered_pending_type_arguments: Vec<syntax_trees::types::TypeReferenceHandle>,
    pub(crate) equation_sources: Option<crate::preparation::machine_equations::SourceLinks>,
    pub(crate) const_resolution_mode: ConstResolutionMode,
    /// Constant values may be normalized before structural equations have
    /// their canonical range inputs. Only the complete route clears this.
    pub(crate) structural_type_equations_pending: bool,
    pub(crate) constant_selection:
        Option<crate::preparation::declaration_selection::ConstantSelection<'static>>,
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
    /// Origin rows before this frontier already own exact declaration symbols.
    /// Body completion must bind its suffix without reinterpreting that prefix.
    pub(crate) bound_generic_application_origins: usize,
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
    /// Owners whose previously deferred bodies this producer completed.
    pub(crate) completed_machines: Vec<symbols::SymbolHandle>,
    pub(crate) pending_const_declarations: Vec<PendingConstDeclaration>,
    pub(crate) pending_const_argument_selections: Vec<PendingConstArgumentSelection>,
    pub(crate) pending_const_argument_slots: Vec<PendingConstArgumentSlot>,
    /// Resolved declaration-construction owners from deferred lexical probes.
    /// Their Generic parents must outlive the pending const argument slots;
    /// they are never installed as executable statements in the resolved tree.
    pub(crate) resolved_lexical_inputs: Vec<crate::lowering::state::StateLexicalInputs>,
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
    /// temporaries hoisted out of operands, calls and match subjects (see
    /// `statement::hoist_temps`). `__hoist_` prefixed so the generated names
    /// cannot collide with source identifiers.
    pub(crate) hoist_counter: u32,
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
    /// Maps a match SUBJECT syntax expression handle to the name and diagnostic
    /// source span of its single hoisted temp. All arms of one enum-variant match share the same
    /// syntax subject handle (the parser reuses it across arms), so the first
    /// arm mints `let __hoist_N = <subject>` and the siblings reuse the name --
    /// keeping ONE shared subject so match exhaustiveness still groups the arms
    /// (`statement::hoist_membership_match_subject`).
    match_subject_temps:
        std::collections::HashMap<syntax_trees::expression::ExpressionHandle, (String, SourceSpan)>,
    pub(crate) expression_match_captures: Vec<MatchArmCapture>,
    /// The CURRENT state's parameters (name, resolved type,
    /// mutability). The scalar-computation classification of transition
    /// values reads them. Overwritten at each state.
    pub(crate) current_state_parameters:
        Vec<(String, symbol_resolved_trees::types::TypeReference, bool)>,
    /// The CURRENT state's declared return type. Overwritten at each state.
    pub(crate) current_state_return_type: Option<symbol_resolved_trees::types::TypeReference>,
    /// Present when this lowerer extends a retained base.
    pub(crate) seed: Option<BaseSeed>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RootWatermarks {
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
    pub(crate) source: Arc<SymbolResolvedTrees>,
}

impl Lowerer {
    pub(crate) fn new(
        sources: Option<Arc<SourceMap>>,
        source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    ) -> Self {
        Self {
            defer_machine_bodies: false,
            pending_machine_bodies: Vec::new(),
            pending_type_inputs: Vec::new(),
            supplied_operand_owners: Vec::new(),
            conformance_inputs: Vec::new(),
            encountered_pending_type_arguments: Vec::new(),
            equation_sources: None,
            const_resolution_mode: ConstResolutionMode::Complete,
            structural_type_equations_pending: true,
            constant_selection: None,
            namespace_declarations: crate::symbols::NamespaceDeclarations::default(),
            pending_static_module_calls: Vec::new(),
            pending_static_module_statement_calls: Vec::new(),
            symbol_resolved_trees: SymbolResolvedTrees::default(),
            bound_generic_application_origins: 0,
            pending_machine_service_reaches: Vec::new(),
            pending_signature_service_reaches: Vec::new(),
            pending_authored_expressions: Vec::new(),
            pending_authored_proof_memberships: Vec::new(),
            completed_machines: Vec::new(),
            pending_const_declarations: Vec::new(),
            pending_const_selections: Vec::new(),
            pending_const_values: Vec::new(),
            pending_const_initializers: Vec::new(),
            pending_outcome_specific_contracts: Vec::new(),
            current_authored_expression_exposure: None,
            pending_const_argument_selections: Vec::new(),
            pending_const_argument_slots: Vec::new(),
            resolved_lexical_inputs: Vec::new(),
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
            current_state_name: None,
            current_evidence_term_names: Vec::new(),
            match_subject_temps: std::collections::HashMap::new(),
            expression_match_captures: Vec::new(),
            current_state_parameters: Vec::new(),
            current_state_return_type: None,
            seed: None,
        }
    }

    /// Extend a retained base: its source frontier must be the exact prefix of
    /// this lowerer's sources, and its declarations become the pending
    /// sidecars a later stratum resolves against.
    pub(crate) fn seed_resolved_base(
        &mut self,
        base: Arc<SymbolResolvedTrees>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(sources) = &self.sources else {
            return Err(vec![Diagnostic::error(
                "seeded symbol resolution requires retained source custody",
            )]);
        };
        if base.symbols.source_map().is_some_and(|retained| {
            !sources.retains_source_storage(&retained)
                || retained
                    .selected_files()
                    .any(|file| !sources.source_is_selected(file.source_id))
        }) {
            return Err(vec![Diagnostic::error(
                "seeded symbol resolution source map does not retain the exact base frontier",
            )]);
        }
        self.seed = Some(BaseSeed {
            roots: RootWatermarks::capture(&base),
            source: Arc::clone(&base),
        });
        // The retained constant rows are lookup, not new declaration work.
        // Lowering registers each fresh declaration by its allocation handle.
        self.pending_machine_service_reaches = base
            .machines
            .iter()
            .map(|machine| {
                retained_service_reach_for(&base, machine.symbol, machine.service_reach_row)
            })
            .collect();
        for definition in &base.traits {
            for offset in 0..definition.machines.len() {
                let handle = handle_at_offset(definition.machines, offset);
                let signature = base
                    .tables
                    .declarations
                    .trait_machine_signatures
                    .get(handle);
                let reaches = retained_service_reach_for(
                    &base,
                    signature.symbol,
                    signature.service_reach_row,
                );
                self.pending_signature_service_reaches
                    .push(PendingSignatureServiceReach {
                        location: PendingSignatureLocation::Trait(handle),
                        owner: PendingSignatureOwner::Trait(definition.name.clone()),
                        reaches,
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
            let reaches =
                retained_service_reach_for(&base, signature.symbol, signature.service_reach_row);
            self.pending_signature_service_reaches
                .push(PendingSignatureServiceReach {
                    location: PendingSignatureLocation::MachineParameter(handle),
                    owner: PendingSignatureOwner::Requirement(parameter.name.clone()),
                    reaches,
                });
        }
        self.bound_generic_application_origins =
            base.tables.types.generic_application_origins.len();
        self.symbol_resolved_trees = base.as_ref().clone();
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

    /// The shared hoist-temp name and source occurrence, if the first arm
    /// already captured this exact match subject.
    pub(crate) fn match_subject_temp(
        &self,
        subject: syntax_trees::expression::ExpressionHandle,
    ) -> Option<(String, SourceSpan)> {
        self.match_subject_temps.get(&subject).cloned()
    }

    pub(crate) fn record_match_subject_temp(
        &mut self,
        subject: syntax_trees::expression::ExpressionHandle,
        name: String,
        source_span: SourceSpan,
    ) {
        self.match_subject_temps
            .insert(subject, (name, source_span));
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
        crate::preparation::declaration_selection::ConstantSelection<'static>,
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
            // Derived lookup state does not travel with the fields; the
            // rebuilt program's indexes fill lazily from its own tables.
            resolution_indexes: _,
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
    ) -> Result<
        (
            Vec<(
                symbols::SymbolHandle,
                crate::selection::service_reaches::PendingAuthoredServiceReach,
            )>,
            Vec<crate::selection::service_reaches::PendingSignatureServiceReach>,
        ),
        Vec<Diagnostic>,
    > {
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
                        let parameter = self.symbol_resolved_trees
                            .tables
                            .declarations
                            .data_type_parameters
                            .get(handle);
                        let symbol_resolved_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
                            return Err(vec![Diagnostic::error("pending service reach no longer names a machine parameter")]);
                        };
                        contract.structural().ok_or_else(|| vec![Diagnostic::error("pending service reach lost its structural machine-parameter signature")])?.symbol
                    }
                };
                Ok(crate::selection::service_reaches::PendingSignatureServiceReach {
                    symbol,
                    owner: pending.owner.clone(),
                    reaches: pending.reaches.clone(),
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        Ok((
            pending_machine_service_reaches,
            pending_signature_service_reaches,
        ))
    }
}

fn retained_service_reach_for(
    program: &SymbolResolvedTrees,
    owner: symbols::SymbolHandle,
    row: language_semantics::ServiceReachRowId,
) -> crate::selection::service_reaches::PendingAuthoredServiceReach {
    crate::selection::service_reaches::PendingAuthoredServiceReach::Resolved {
        row,
        authored: program
            .authored_service_reach_rows
            .iter()
            .find(|row| row.owner == owner)
            .cloned(),
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
    pub(crate) reaches: crate::selection::service_reaches::PendingAuthoredServiceReach,
}
