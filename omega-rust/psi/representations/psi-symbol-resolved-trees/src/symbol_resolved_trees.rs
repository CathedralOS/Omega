use crate::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelectionOccurrenceId,
    AuthoredDeclarationSelectionRecordError, AuthoredDeclarationSelections, data, domain,
    expression, measure, operator, proposition, signature, snapshot, state, statement, tables,
    types, wire,
};
use psi_arena::{Arena, Handle, HandleSpan, OrderedRootArena};
use psi_diagnostics::PhaseSnapshot;
use psi_language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionSuffixRebase, AuthoredDeclarationSelectionSuffixRebaseError,
};
use psi_source::SourceSpan;
use psi_symbols::SymbolTable;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedTrees {
    pub roots: SymbolResolvedRoots,
    pub tables: SymbolResolvedTableStorage,
    pub symbols: SymbolTable,
    /// EFX: boundary-trait symbols normalized into service identities after
    /// symbol assignment. This is the source of truth for service reach.
    pub service_reaches: psi_language_semantics::ServiceReachTable,
    pub service_reach_rows: psi_language_semantics::ServiceReachRowTable,
    /// Exact source-backed `reaches` occurrences, retained separately from
    /// normalized semantic rows so explicit empty ceilings survive.
    pub authored_service_reach_rows: Vec<signature::AuthoredServiceReachRow>,
    /// STR4 checked plans, slice 1: the deterministic semantic-domain
    /// interner (declared-name identity, declaration order).
    pub semantic_domains: psi_language_semantics::SemanticDomainTable,
    /// PRV4: normalized `via` bindings, interned once at lowering.
    pub external_bindings: psi_language_semantics::ExternalBindingTable,
    /// Erased evidence assignments removed from runtime statement spans.
    pub evidence_forwardings: Vec<statement::EvidenceForwarding>,
}

/// Exact append boundary for every symbol-resolved store which can retain an
/// authored declaration-selection occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthoredSelectionExtensionFrontier {
    selections: usize,
    expressions: usize,
    tree_statements: usize,
    statements: usize,
    transition_targets: usize,
    proof_facts: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredSelectionOccurrenceStore {
    Expressions,
    TreeStatements,
    Statements,
    TransitionTargets,
    ProofFacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredSelectionExtensionRebaseError {
    Ledger(AuthoredDeclarationSelectionSuffixRebaseError),
    StoreFrontierOutOfRange(AuthoredSelectionOccurrenceStore),
    OccurrenceOutsideOwnerFrontier(AuthoredSelectionOccurrenceStore),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedRoots {
    pub const_declarations: OrderedRootArena<crate::constant::ConstDeclaration>,
    pub data_definitions: OrderedRootArena<crate::data::DataDefinition>,
    pub domain_definitions: OrderedRootArena<crate::domain::DomainDefinition>,
    pub machines: OrderedRootArena<crate::machine::Machine>,
    pub measures: OrderedRootArena<measure::MeasureDefinition>,
    pub operators: OrderedRootArena<operator::OperatorDefinition>,
    pub propositions: OrderedRootArena<proposition::PropositionDefinition>,
    pub traits: OrderedRootArena<crate::trait_definition::TraitDefinition>,
    pub conformances: OrderedRootArena<crate::trait_definition::Conformance>,
    pub wire_schemas: OrderedRootArena<wire::WireSchema>,
}

impl SymbolResolvedRoots {
    pub fn with_roots(
        data_definitions: OrderedRootArena<crate::data::DataDefinition>,
        domain_definitions: OrderedRootArena<crate::domain::DomainDefinition>,
        machines: OrderedRootArena<crate::machine::Machine>,
        operators: OrderedRootArena<operator::OperatorDefinition>,
        traits: OrderedRootArena<crate::trait_definition::TraitDefinition>,
    ) -> Self {
        Self {
            const_declarations: OrderedRootArena::default(),
            data_definitions,
            domain_definitions,
            machines,
            measures: OrderedRootArena::default(),
            operators,
            propositions: OrderedRootArena::default(),
            traits,
            conformances: OrderedRootArena::default(),
            wire_schemas: OrderedRootArena::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedTableStorage {
    pub declarations: SymbolResolvedDeclarationStorage,
    pub bodies: SymbolResolvedBodyStorage,
    pub types: SymbolResolvedTypeStorage,
    authored_declaration_selections: AuthoredDeclarationSelections,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedDeclarationStorage {
    pub data_members: Arena<data::DataMember>,
    pub data_payload_fields: Arena<data::DataField>,
    pub data_type_parameters: Arena<data::TypeParameter>,
    pub proof_facts: Arena<domain::ProofFact>,
    proof_fact_source_spans: Vec<Option<psi_source::SourceSpan>>,
    pub domain_path_members: Arena<crate::name::DiagnosticName>,
    pub operator_path_members: Arena<crate::name::DiagnosticName>,
    pub operator_definitions: Arena<operator::OperatorDefinition>,
    pub proposition_binders: Arena<proposition::PropositionBinder>,
    pub machine_owned_data: Arena<crate::machine::OwnedData>,
    pub machine_trait_conformances: Arena<crate::machine::TraitConformance>,
    pub machine_state_handles: Arena<Handle<state::State>>,
    pub machine_states: Arena<state::State>,
    pub trait_requirements: Arena<crate::trait_definition::TraitRequirement>,
    pub trait_machine_signatures: Arena<signature::StateSignature>,
    pub ranking_views: Arena<crate::name::DiagnosticName>,
    pub signature_invokes: Arena<crate::name::DiagnosticName>,
    pub signature_contracts: Arena<signature::SignatureContract>,
    pub state_parameters: Arena<signature::StateParameter>,
    pub statement_path_members: Arena<crate::name::DiagnosticName>,
    pub state_statements: Arena<statement::Statement>,
    pub child_type_references: Arena<types::TypeReference>,
    pub wire_members: Arena<wire::WireMember>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedTypeStorage {
    pub constraints: Arena<types::TypeConstraint>,
    pub references: types::TypeReferenceTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedBodyStorage {
    pub expressions: expression::ExpressionTable,
    pub statements: statement::StatementTable,
}

impl SymbolResolvedTrees {
    pub fn authored_selection_extension_frontier(&self) -> AuthoredSelectionExtensionFrontier {
        AuthoredSelectionExtensionFrontier {
            selections: self.tables.authored_declaration_selections.len(),
            expressions: self.tables.bodies.expressions.expression_count(),
            tree_statements: self.tables.declarations.state_statements.len(),
            statements: self.tables.bodies.statements.statement_count(),
            transition_targets: self.tables.bodies.statements.transition_target_count(),
            proof_facts: self.tables.declarations.proof_facts.len(),
        }
    }

    /// Replace the retained authored-selection ledger with an exact
    /// later-phase base, preserve appended clones of base occurrences, and
    /// shift only suffix occurrence identities. Failure returns the original
    /// trees intact.
    pub fn rebase_authored_selection_extension(
        self,
        frontier: AuthoredSelectionExtensionFrontier,
        destination_base: &AuthoredDeclarationSelections,
    ) -> Result<Self, (Self, AuthoredSelectionExtensionRebaseError)> {
        let (ledger, rebase) = match self
            .tables
            .authored_declaration_selections
            .replace_prefix_and_rebase_suffix(frontier.selections, destination_base)
        {
            Ok(joined) => joined,
            Err(error) => {
                return Err((self, AuthoredSelectionExtensionRebaseError::Ledger(error)));
            }
        };
        let mut candidate = self.clone();
        if let Err(error) = candidate.rebase_occurrence_stores(frontier, rebase) {
            return Err((self, error));
        }
        candidate.tables.authored_declaration_selections = ledger;
        Ok(candidate)
    }

    fn rebase_occurrence_stores(
        &mut self,
        frontier: AuthoredSelectionExtensionFrontier,
        rebase: AuthoredDeclarationSelectionSuffixRebase,
    ) -> Result<(), AuthoredSelectionExtensionRebaseError> {
        if frontier.expressions > self.tables.bodies.expressions.expression_count() {
            return Err(
                AuthoredSelectionExtensionRebaseError::StoreFrontierOutOfRange(
                    AuthoredSelectionOccurrenceStore::Expressions,
                ),
            );
        }
        self.tables
            .bodies
            .expressions
            .rebase_authored_selection_extension(frontier.expressions, rebase)
            .map_err(|()| {
                AuthoredSelectionExtensionRebaseError::OccurrenceOutsideOwnerFrontier(
                    AuthoredSelectionOccurrenceStore::Expressions,
                )
            })?;

        self.rebase_tree_statement_occurrences(frontier.tree_statements, rebase)?;

        if frontier.statements > self.tables.bodies.statements.statement_count() {
            return Err(
                AuthoredSelectionExtensionRebaseError::StoreFrontierOutOfRange(
                    AuthoredSelectionOccurrenceStore::Statements,
                ),
            );
        }
        if frontier.transition_targets > self.tables.bodies.statements.transition_target_count() {
            return Err(
                AuthoredSelectionExtensionRebaseError::StoreFrontierOutOfRange(
                    AuthoredSelectionOccurrenceStore::TransitionTargets,
                ),
            );
        }
        self.tables
            .bodies
            .statements
            .rebase_authored_selection_extension(
                frontier.statements,
                frontier.transition_targets,
                rebase,
            )
            .map_err(|store| {
                AuthoredSelectionExtensionRebaseError::OccurrenceOutsideOwnerFrontier(match store {
                    statement::AuthoredSelectionStatementStore::Statements => {
                        AuthoredSelectionOccurrenceStore::Statements
                    }
                    statement::AuthoredSelectionStatementStore::TransitionTargets => {
                        AuthoredSelectionOccurrenceStore::TransitionTargets
                    }
                })
            })?;

        self.rebase_proof_fact_occurrences(frontier.proof_facts, rebase)
    }

    fn rebase_tree_statement_occurrences(
        &mut self,
        frontier: usize,
        rebase: AuthoredDeclarationSelectionSuffixRebase,
    ) -> Result<(), AuthoredSelectionExtensionRebaseError> {
        let statements = &mut self.tables.declarations.state_statements;
        if frontier > statements.len() {
            return Err(
                AuthoredSelectionExtensionRebaseError::StoreFrontierOutOfRange(
                    AuthoredSelectionOccurrenceStore::TreeStatements,
                ),
            );
        }
        for (index, (_, statement)) in statements.iter().enumerate() {
            for occurrence in tree_statement_occurrences(statement) {
                let valid = if index < frontier {
                    rebase.retain_base(occurrence)
                } else {
                    rebase.rebase_appended(occurrence)
                };
                if valid.is_none() {
                    return Err(
                        AuthoredSelectionExtensionRebaseError::OccurrenceOutsideOwnerFrontier(
                            AuthoredSelectionOccurrenceStore::TreeStatements,
                        ),
                    );
                }
            }
        }
        statements.for_each_mut(|handle, statement| {
            if usize::try_from(handle.arena_index()).expect("tree statement index overflow")
                > frontier
            {
                rebase_tree_statement_occurrences(statement, rebase);
            }
        });
        Ok(())
    }

    fn rebase_proof_fact_occurrences(
        &mut self,
        frontier: usize,
        rebase: AuthoredDeclarationSelectionSuffixRebase,
    ) -> Result<(), AuthoredSelectionExtensionRebaseError> {
        let facts = &mut self.tables.declarations.proof_facts;
        if frontier > facts.len() {
            return Err(
                AuthoredSelectionExtensionRebaseError::StoreFrontierOutOfRange(
                    AuthoredSelectionOccurrenceStore::ProofFacts,
                ),
            );
        }
        for (index, (_, fact)) in facts.iter().enumerate() {
            let domain::ProofFact::Membership(membership) = fact else {
                continue;
            };
            let Some(occurrence) = membership.authored_domain_selection else {
                continue;
            };
            let valid = if index < frontier {
                rebase.retain_base(occurrence)
            } else {
                rebase.rebase_appended(occurrence)
            };
            if valid.is_none() {
                return Err(
                    AuthoredSelectionExtensionRebaseError::OccurrenceOutsideOwnerFrontier(
                        AuthoredSelectionOccurrenceStore::ProofFacts,
                    ),
                );
            }
        }
        facts.for_each_mut(|handle, fact| {
            if usize::try_from(handle.arena_index()).expect("proof fact index overflow") <= frontier
            {
                return;
            }
            let domain::ProofFact::Membership(membership) = fact else {
                return;
            };
            let Some(occurrence) = membership.authored_domain_selection else {
                return;
            };
            membership.authored_domain_selection = Some(
                rebase
                    .rebase_appended(occurrence)
                    .expect("proof membership occurrence was validated before mutation"),
            );
        });
        Ok(())
    }

    pub fn authored_service_reach_rows_for(
        &self,
        owner: psi_symbols::SymbolHandle,
    ) -> impl Iterator<Item = &signature::AuthoredServiceReachRow> {
        self.authored_service_reach_rows
            .iter()
            .filter(move |row| row.owner == owner)
    }

    pub fn with_roots(
        roots: SymbolResolvedRoots,
        tables: SymbolResolvedTableStorage,
        symbols: SymbolTable,
    ) -> Self {
        Self {
            roots,
            tables,
            symbols,
            service_reaches: psi_language_semantics::ServiceReachTable::default(),
            service_reach_rows: psi_language_semantics::ServiceReachRowTable::default(),
            authored_service_reach_rows: Vec::new(),
            semantic_domains: psi_language_semantics::SemanticDomainTable::default(),
            external_bindings: psi_language_semantics::ExternalBindingTable::default(),
            evidence_forwardings: Vec::new(),
        }
    }

    pub fn data_members(&self, span: HandleSpan<data::DataMember>) -> &[data::DataMember] {
        self.tables.declarations.data_members.span_or_empty(span)
    }

    pub fn record_resolved_authored_declaration_selection(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        selected_symbol: psi_symbols::SymbolHandle,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        self.tables.authored_declaration_selections.record_resolved(
            source_span,
            exposure,
            kind,
            selected_symbol,
        )
    }

    pub fn record_resolved_authored_declaration_selection_in_partition(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        compiler_partition: Option<
            psi_language_semantics::declaration_selection::CompilerDerivedSelectionPartition,
        >,
        selected_symbol: psi_symbols::SymbolHandle,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        self.tables
            .authored_declaration_selections
            .record_resolved_in_partition(
                source_span,
                exposure,
                kind,
                compiler_partition,
                selected_symbol,
            )
    }

    pub fn record_late_bound_authored_declaration_selection(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        late_binding: AuthoredDeclarationSelectionLateBinding,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        self.tables
            .authored_declaration_selections
            .record_late_bound(source_span, exposure, kind, late_binding)
    }

    pub fn record_late_bound_authored_declaration_selection_in_partition(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        compiler_partition: Option<
            psi_language_semantics::declaration_selection::CompilerDerivedSelectionPartition,
        >,
        late_binding: AuthoredDeclarationSelectionLateBinding,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        self.tables
            .authored_declaration_selections
            .record_late_bound_in_partition(
                source_span,
                exposure,
                kind,
                compiler_partition,
                late_binding,
            )
    }

    pub fn authored_declaration_selections(&self) -> &AuthoredDeclarationSelections {
        &self.tables.authored_declaration_selections
    }

    pub fn data_payload_fields(&self, span: HandleSpan<data::DataField>) -> &[data::DataField] {
        self.tables
            .declarations
            .data_payload_fields
            .span_or_empty(span)
    }

    pub fn data_type_parameters(
        &self,
        span: HandleSpan<data::TypeParameter>,
    ) -> &[data::TypeParameter] {
        self.tables
            .declarations
            .data_type_parameters
            .span_or_empty(span)
    }

    pub fn proof_facts(&self, span: HandleSpan<domain::ProofFact>) -> &[domain::ProofFact] {
        self.tables.declarations.proof_facts.span_or_empty(span)
    }

    pub fn proof_fact_source_span(
        &self,
        handle: Handle<domain::ProofFact>,
    ) -> Option<psi_source::SourceSpan> {
        self.tables
            .declarations
            .proof_fact_source_spans
            .get(proof_fact_source_span_index(handle))
            .copied()
            .flatten()
    }

    pub fn set_proof_fact_source_span(
        &mut self,
        handle: Handle<domain::ProofFact>,
        source_span: psi_source::SourceSpan,
    ) {
        let index = proof_fact_source_span_index(handle);
        self.tables
            .declarations
            .proof_fact_source_spans
            .resize(index + 1, None);
        self.tables.declarations.proof_fact_source_spans[index] = Some(source_span);
    }

    pub fn domain_path_members(
        &self,
        span: HandleSpan<crate::name::DiagnosticName>,
    ) -> &[crate::name::DiagnosticName] {
        self.tables
            .declarations
            .domain_path_members
            .span_or_empty(span)
    }

    pub fn operator_path_members(
        &self,
        span: HandleSpan<crate::name::DiagnosticName>,
    ) -> &[crate::name::DiagnosticName] {
        self.tables
            .declarations
            .operator_path_members
            .span_or_empty(span)
    }

    pub fn operator_definitions(
        &self,
        span: HandleSpan<operator::OperatorDefinition>,
    ) -> &[operator::OperatorDefinition] {
        self.tables
            .declarations
            .operator_definitions
            .span_or_empty(span)
    }

    pub fn measure_path_members(
        &self,
        span: HandleSpan<crate::name::DiagnosticName>,
    ) -> &[crate::name::DiagnosticName] {
        self.tables
            .declarations
            .operator_path_members
            .span_or_empty(span)
    }

    pub fn trait_machine_signatures(
        &self,
        span: HandleSpan<signature::StateSignature>,
    ) -> &[signature::StateSignature] {
        self.tables
            .declarations
            .trait_machine_signatures
            .span_or_empty(span)
    }

    /// Project a machine-parameter contract to its single published signature
    /// while retaining whether that signature is structurally owned or an
    /// exact nominal trait requirement. Nominal ownership is revalidated here
    /// so a mismatched symbol pair fails closed.
    pub fn machine_parameter_contract_view<'program>(
        &'program self,
        contract: &'program data::MachineParameterContract,
    ) -> Option<data::MachineParameterContractView<'program>> {
        match contract {
            data::MachineParameterContract::RequirementIdentity => None,
            data::MachineParameterContract::Structural(signature) => {
                Some(data::MachineParameterContractView::Structural(signature))
            }
            data::MachineParameterContract::AuthoredNominal { .. } => None,
            data::MachineParameterContract::Nominal {
                trait_definition,
                requirement,
                ..
            } => {
                let trait_definition = self
                    .traits
                    .iter()
                    .find(|candidate| candidate.symbol == *trait_definition)?;
                let requirement = self
                    .trait_machine_signatures(trait_definition.machines)
                    .iter()
                    .find(|candidate| candidate.symbol == *requirement)?;
                Some(data::MachineParameterContractView::Nominal {
                    trait_definition,
                    requirement,
                })
            }
        }
    }

    pub fn trait_type_parameters(
        &self,
        trait_definition: &crate::trait_definition::TraitDefinition,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters(trait_definition.type_parameters)
    }

    pub fn trait_requirements(
        &self,
        span: HandleSpan<crate::trait_definition::TraitRequirement>,
    ) -> &[crate::trait_definition::TraitRequirement] {
        self.tables
            .declarations
            .trait_requirements
            .span_or_empty(span)
    }

    pub fn state_parameters(
        &self,
        span: HandleSpan<signature::StateParameter>,
    ) -> &[signature::StateParameter] {
        self.tables
            .declarations
            .state_parameters
            .span_or_empty(span)
    }

    pub fn signature_invokes(
        &self,
        span: HandleSpan<crate::name::DiagnosticName>,
    ) -> &[crate::name::DiagnosticName] {
        self.tables
            .declarations
            .signature_invokes
            .span_or_empty(span)
    }

    pub fn signature_contracts(
        &self,
        span: HandleSpan<signature::SignatureContract>,
    ) -> &[signature::SignatureContract] {
        self.tables
            .declarations
            .signature_contracts
            .span_or_empty(span)
    }

    pub fn machine_invokes(
        &self,
        machine: &crate::machine::Machine,
    ) -> &[crate::name::DiagnosticName] {
        self.signature_invokes(machine.invokes)
    }

    pub fn machine_type_parameters(
        &self,
        machine: &crate::machine::Machine,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters(machine.type_parameters)
    }

    pub fn machine_ranking_view(
        &self,
        span: HandleSpan<crate::name::DiagnosticName>,
    ) -> &[crate::name::DiagnosticName] {
        self.tables.declarations.ranking_views.span_or_empty(span)
    }

    pub fn machine_contracts(
        &self,
        machine: &crate::machine::Machine,
    ) -> &[signature::SignatureContract] {
        self.signature_contracts(machine.contracts)
    }

    pub fn machine_state_handles(
        &self,
        span: HandleSpan<Handle<state::State>>,
    ) -> &[Handle<state::State>] {
        self.tables
            .declarations
            .machine_state_handles
            .span_or_empty(span)
    }

    pub fn machine_state(&self, handle: Handle<state::State>) -> &state::State {
        self.tables.declarations.machine_states.get(handle)
    }

    pub fn machine_owned_data(
        &self,
        span: HandleSpan<crate::machine::OwnedData>,
    ) -> &[crate::machine::OwnedData] {
        self.tables
            .declarations
            .machine_owned_data
            .span_or_empty(span)
    }

    pub fn machine_trait_conformances(
        &self,
        span: HandleSpan<crate::machine::TraitConformance>,
    ) -> &[crate::machine::TraitConformance] {
        self.tables
            .declarations
            .machine_trait_conformances
            .span_or_empty(span)
    }

    pub fn state_statements(
        &self,
        span: HandleSpan<statement::Statement>,
    ) -> &[statement::Statement] {
        self.tables
            .declarations
            .state_statements
            .span_or_empty(span)
    }

    pub fn child_type_references(
        &self,
        span: HandleSpan<types::TypeReference>,
    ) -> &[types::TypeReference] {
        self.tables
            .declarations
            .child_type_references
            .span_or_empty(span)
    }

    pub fn child_type_reference(
        &self,
        handle: Handle<types::TypeReference>,
    ) -> &types::TypeReference {
        self.tables.declarations.child_type_references.get(handle)
    }

    pub fn wire_members(&self, span: HandleSpan<wire::WireMember>) -> &[wire::WireMember] {
        self.tables.declarations.wire_members.span_or_empty(span)
    }

    pub fn rebuild_tables(&mut self) {
        let tables =
            tables::SymbolResolvedTreeTables::from_symbol_resolved_trees_with_state_spans(self);
        self.tables.bodies.expressions = tables.bodies.expressions;
        self.tables.bodies.statements = tables.bodies.statements;
        self.tables.types.references = tables.types.references;
    }

    pub fn snapshot(&self) -> snapshot::SymbolResolvedTreesSnapshot {
        snapshot::SymbolResolvedTreesSnapshot::from_symbol_resolved_trees(self)
    }

    pub fn snapshot_json(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json()
    }

    pub fn snapshot_json_pretty(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json_pretty()
    }
}

fn tree_statement_occurrences(
    statement: &statement::Statement,
) -> impl Iterator<Item = AuthoredDeclarationSelectionOccurrenceId> {
    let mut occurrences = [None, None];
    match statement {
        statement::Statement::Call(call) => occurrences[0] = call.authored_call_selection,
        statement::Statement::Transition(transition) => {
            if let statement::TransitionTarget::Named(target) = &transition.target {
                occurrences[0] = target.authored_call_selection;
            }
            if let Some(statement::TransitionTarget::Named(target)) = &transition.continuation {
                occurrences[1] = target.authored_call_selection;
            }
        }
        _ => {}
    }
    occurrences.into_iter().flatten()
}

fn rebase_tree_statement_occurrences(
    statement: &mut statement::Statement,
    rebase: AuthoredDeclarationSelectionSuffixRebase,
) {
    match statement {
        statement::Statement::Call(call) => {
            if let Some(occurrence) = call.authored_call_selection {
                call.authored_call_selection = Some(
                    rebase
                        .rebase_appended(occurrence)
                        .expect("tree call occurrence was validated before mutation"),
                );
            }
        }
        statement::Statement::Transition(transition) => {
            if let statement::TransitionTarget::Named(target) = &mut transition.target
                && let Some(occurrence) = target.authored_call_selection
            {
                target.authored_call_selection = Some(
                    rebase
                        .rebase_appended(occurrence)
                        .expect("tree transition occurrence was validated before mutation"),
                );
            }
            if let Some(statement::TransitionTarget::Named(target)) = &mut transition.continuation
                && let Some(occurrence) = target.authored_call_selection
            {
                target.authored_call_selection = Some(
                    rebase
                        .rebase_appended(occurrence)
                        .expect("tree continuation occurrence was validated before mutation"),
                );
            }
        }
        _ => {}
    }
}

fn proof_fact_source_span_index(handle: Handle<domain::ProofFact>) -> usize {
    usize::try_from(handle.arena_index())
        .expect("proof fact source-span index exceeds usize")
        .checked_sub(1)
        .expect("proof fact source-span handle must be valid")
}

impl PhaseSnapshot for SymbolResolvedTrees {
    type Snapshot = snapshot::SymbolResolvedTreesSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        SymbolResolvedTrees::snapshot(self)
    }
}

impl Deref for SymbolResolvedTrees {
    type Target = SymbolResolvedRoots;

    fn deref(&self) -> &Self::Target {
        &self.roots
    }
}

impl DerefMut for SymbolResolvedTrees {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.roots
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        AuthoredSelectionExtensionRebaseError, AuthoredSelectionOccurrenceStore,
        SymbolResolvedRoots, SymbolResolvedTableStorage, SymbolResolvedTrees, data, domain,
        expression, machine, name::DiagnosticName, operator, statement, trait_definition,
    };
    use psi_arena::{HandleSpan, OrderedRootArena};
    use psi_source::{SourceId, SourceSpan, Span};
    use psi_symbols::{SymbolHandle, SymbolTable};

    #[test]
    fn symbol_resolved_roots_constructor_keeps_top_level_roots_explicit() {
        let data_definitions = OrderedRootArena::<data::DataDefinition>::default();
        let domain_definitions = OrderedRootArena::<domain::DomainDefinition>::default();
        let machines = OrderedRootArena::<machine::Machine>::default();
        let operators = OrderedRootArena::<operator::OperatorDefinition>::default();
        let traits = OrderedRootArena::<trait_definition::TraitDefinition>::default();

        let roots = SymbolResolvedRoots::with_roots(
            data_definitions.clone(),
            domain_definitions.clone(),
            machines.clone(),
            operators.clone(),
            traits.clone(),
        );

        assert_eq!(roots.data_definitions, data_definitions);
        assert_eq!(roots.domain_definitions, domain_definitions);
        assert_eq!(roots.machines, machines);
        assert_eq!(roots.operators, operators);
        assert_eq!(roots.traits, traits);
    }

    #[test]
    fn symbol_resolved_trees_constructor_keeps_roots_tables_and_symbols_explicit() {
        let roots = SymbolResolvedRoots::default();
        let tables = SymbolResolvedTableStorage::default();
        let symbols = SymbolTable::default();

        let trees = SymbolResolvedTrees::with_roots(roots.clone(), tables.clone(), symbols.clone());

        assert_eq!(trees.roots, roots);
        assert_eq!(trees.tables, tables);
        assert_eq!(trees.symbols, symbols);
        assert!(trees.authored_declaration_selections().is_empty());
    }

    #[test]
    fn symbol_resolved_trees_owns_authored_declaration_selections() {
        let mut trees = SymbolResolvedTrees::default();
        let occurrence_id = trees
            .record_resolved_authored_declaration_selection(
                SourceSpan::new(SourceId(4), Span::new(8, 13)),
                AuthoredDeclarationSelectionExposure::PublicInterface,
                AuthoredDeclarationSelectionKind::TypeReference,
                SymbolHandle::from_arena_index(9),
            )
            .expect("valid selected symbol");
        let selection = *trees
            .authored_declaration_selections()
            .get(occurrence_id)
            .expect("recorded selection");
        let copied_trees = trees.clone();
        trees.rebuild_tables();

        assert_eq!(
            trees.authored_declaration_selections().get(occurrence_id),
            Some(&selection)
        );
        assert_eq!(
            copied_trees.authored_declaration_selections(),
            trees.authored_declaration_selections()
        );
    }

    #[test]
    fn ranking_views_and_signature_invokes_use_independent_arenas() {
        let mut trees = SymbolResolvedTrees::default();
        let mut ranking_view = HandleSpan::empty();
        let mut signature_invokes = HandleSpan::empty();

        trees
            .tables
            .declarations
            .ranking_views
            .append_to_span(&mut ranking_view, DiagnosticName::generated("remaining"));
        trees.tables.declarations.signature_invokes.append_to_span(
            &mut signature_invokes,
            DiagnosticName::generated("Console.write"),
        );

        assert_eq!(
            trees.machine_ranking_view(ranking_view)[0].as_str(),
            "remaining"
        );
        assert_eq!(
            trees.signature_invokes(signature_invokes)[0].as_str(),
            "Console.write"
        );
    }

    fn record_selection(
        trees: &mut SymbolResolvedTrees,
        start: usize,
        symbol: u32,
    ) -> crate::AuthoredDeclarationSelectionOccurrenceId {
        trees
            .record_resolved_authored_declaration_selection(
                SourceSpan::new(SourceId(4), Span::new(start, start + 1)),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                SymbolHandle::from_arena_index(symbol),
            )
            .expect("valid selected symbol")
    }

    fn tree_call(
        occurrence: crate::AuthoredDeclarationSelectionOccurrenceId,
    ) -> statement::Statement {
        statement::Statement::Call(statement::Call {
            receiver_symbol: SymbolHandle::invalid(),
            target_symbol: SymbolHandle::from_arena_index(8),
            target: DiagnosticName::generated("invoke"),
            storage: statement::CallStorage {
                authored_call_selection: Some(occurrence),
                ..Default::default()
            },
        })
    }

    fn named_target(
        occurrence: crate::AuthoredDeclarationSelectionOccurrenceId,
    ) -> statement::TransitionTarget {
        statement::TransitionTarget::Named(statement::NamedTransitionTarget {
            head_symbol: SymbolHandle::from_arena_index(8),
            symbol: SymbolHandle::from_arena_index(9),
            storage: statement::NamedTransitionTargetStorage {
                authored_call_selection: Some(occurrence),
                ..Default::default()
            },
        })
    }

    fn tree_transition(
        occurrence: crate::AuthoredDeclarationSelectionOccurrenceId,
    ) -> statement::Statement {
        statement::Statement::Transition(statement::Transition {
            target: named_target(occurrence),
            continuation: Some(named_target(occurrence)),
            guard: statement::TransitionGuard::Always,
            proof_selectors: Box::default(),
            exit: statement::TransitionExit::Ordinary,
            source_span: SourceSpan::default(),
        })
    }

    #[test]
    fn extension_rebase_shifts_every_extension_owned_occurrence_store() {
        let mut trees = SymbolResolvedTrees::default();
        let base_occurrence = record_selection(&mut trees, 1, 3);
        let base_expression = trees
            .tables
            .bodies
            .expressions
            .insert(expression::ExpressionNode::default());
        trees
            .tables
            .bodies
            .expressions
            .attach_authored_selection_occurrences(base_expression, [base_occurrence]);
        trees
            .tables
            .declarations
            .state_statements
            .append(tree_call(base_occurrence));
        trees
            .tables
            .bodies
            .statements
            .insert(statement::StatementNode::Call(statement::TableCall {
                authored_call_selection: Some(base_occurrence),
                ..Default::default()
            }));
        trees
            .tables
            .declarations
            .proof_facts
            .append(domain::ProofFact::Membership(domain::ProofMembershipFact {
                authored_domain_selection: Some(base_occurrence),
                ..Default::default()
            }));
        let frontier = trees.authored_selection_extension_frontier();

        let extension_occurrence = record_selection(&mut trees, 3, 4);
        let extension_expression = trees
            .tables
            .bodies
            .expressions
            .insert(expression::ExpressionNode::default());
        trees
            .tables
            .bodies
            .expressions
            .attach_authored_selection_occurrences(extension_expression, [extension_occurrence]);
        let extension_tree_statement = trees
            .tables
            .declarations
            .state_statements
            .append(tree_call(extension_occurrence));
        let extension_tree_transition = trees
            .tables
            .declarations
            .state_statements
            .append(tree_transition(extension_occurrence));
        let extension_table_statement =
            trees
                .tables
                .bodies
                .statements
                .insert(statement::StatementNode::Call(statement::TableCall {
                    authored_call_selection: Some(extension_occurrence),
                    ..Default::default()
                }));
        let extension_fact =
            trees
                .tables
                .declarations
                .proof_facts
                .append(domain::ProofFact::Membership(domain::ProofMembershipFact {
                    authored_domain_selection: Some(extension_occurrence),
                    ..Default::default()
                }));

        let mut destination = crate::AuthoredDeclarationSelections::default();
        destination
            .record_resolved(
                SourceSpan::new(SourceId(4), Span::new(1, 2)),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                SymbolHandle::from_arena_index(3),
            )
            .expect("same retained base row");
        destination
            .record_resolved(
                SourceSpan::new(SourceId(4), Span::new(2, 3)),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess,
                SymbolHandle::from_arena_index(7),
            )
            .expect("later-phase-only retained row");

        let rebased = trees
            .rebase_authored_selection_extension(frontier, &destination)
            .expect("exact append frontier");
        let shifted = rebased.authored_declaration_selections().as_slice()[2].occurrence_id();
        assert_eq!(
            rebased
                .tables
                .bodies
                .expressions
                .authored_selection_occurrences(extension_expression)
                .collect::<Vec<_>>(),
            vec![shifted]
        );
        let statement::Statement::Call(call) = rebased
            .tables
            .declarations
            .state_statements
            .get(extension_tree_statement)
        else {
            panic!("extension tree call")
        };
        assert_eq!(call.authored_call_selection, Some(shifted));
        let statement::Statement::Transition(transition) = rebased
            .tables
            .declarations
            .state_statements
            .get(extension_tree_transition)
        else {
            panic!("extension tree transition")
        };
        let statement::TransitionTarget::Named(target) = &transition.target else {
            panic!("named target")
        };
        assert_eq!(target.authored_call_selection, Some(shifted));
        let Some(statement::TransitionTarget::Named(continuation)) = &transition.continuation
        else {
            panic!("named continuation")
        };
        assert_eq!(continuation.authored_call_selection, Some(shifted));
        let statement::StatementNode::Call(call) = rebased
            .tables
            .bodies
            .statements
            .statement(extension_table_statement)
        else {
            panic!("extension table call")
        };
        assert_eq!(call.authored_call_selection, Some(shifted));
        let domain::ProofFact::Membership(fact) =
            rebased.tables.declarations.proof_facts.get(extension_fact)
        else {
            panic!("extension proof membership")
        };
        assert_eq!(fact.authored_domain_selection, Some(shifted));
        assert_eq!(
            &rebased.authored_declaration_selections().as_slice()[..2],
            destination.as_slice()
        );
    }

    #[test]
    fn extension_rebase_failure_returns_the_original_trees_intact() {
        let mut trees = SymbolResolvedTrees::default();
        record_selection(&mut trees, 1, 3);
        let frontier = trees.authored_selection_extension_frontier();
        let mut destination = trees.authored_declaration_selections().clone();
        let destination_only_occurrence = destination
            .record_resolved(
                SourceSpan::new(SourceId(4), Span::new(3, 4)),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess,
                SymbolHandle::from_arena_index(4),
            )
            .expect("destination-only selection");
        trees
            .tables
            .declarations
            .state_statements
            .append(tree_call(destination_only_occurrence));
        let expected = trees.clone();

        let (returned, error) = trees
            .rebase_authored_selection_extension(frontier, &destination)
            .expect_err("appended store cannot claim a destination-only occurrence");

        assert_eq!(returned, expected);
        assert_eq!(
            error,
            AuthoredSelectionExtensionRebaseError::OccurrenceOutsideOwnerFrontier(
                AuthoredSelectionOccurrenceStore::TreeStatements
            )
        );
    }

    #[test]
    fn extension_rebase_preserves_appended_clones_of_base_occurrences() {
        let mut trees = SymbolResolvedTrees::default();
        let base_occurrence = record_selection(&mut trees, 1, 3);
        let frontier = trees.authored_selection_extension_frontier();
        let appended_expression = trees
            .tables
            .bodies
            .expressions
            .insert(expression::ExpressionNode::default());
        trees
            .tables
            .bodies
            .expressions
            .attach_authored_selection_occurrences(appended_expression, [base_occurrence]);
        let appended_clone = trees
            .tables
            .declarations
            .state_statements
            .append(tree_call(base_occurrence));
        let destination = trees.authored_declaration_selections().clone();

        let rebased = trees
            .rebase_authored_selection_extension(frontier, &destination)
            .expect("compiler-generated clone retains its authored base identity");
        let statement::Statement::Call(call) = rebased
            .tables
            .declarations
            .state_statements
            .get(appended_clone)
        else {
            panic!("appended call clone")
        };
        assert_eq!(call.authored_call_selection, Some(base_occurrence));
        assert_eq!(
            rebased
                .tables
                .bodies
                .expressions
                .authored_selection_occurrences(appended_expression)
                .collect::<Vec<_>>(),
            vec![base_occurrence]
        );
    }
}
