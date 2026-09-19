//! The current typed trees program and its concept-owned storage.
//!
//! Declarations, executable control flow, values, type references and retained
//! inspection data are subordinate to this root, not separate pipeline outputs.
//!
//! This file owns the typed trees carrier, its roots and tables.
//! `declaration_tables.rs`, `trait_and_machine_tables.rs` and
//! `wire_schema_queries.rs` hold the table accessors,
//! `type_reference_queries.rs` answers type reference queries, `placed_plans.rs`
//! holds placed and plan-laid plans and `commitments.rs` the commitments.

pub mod calls;
mod commitments;
pub mod control_flow;
mod declaration_tables;
pub mod declarations;
pub mod evidence;
pub mod inspection;
pub mod names;
mod placed_plans;
pub(crate) mod retained_prefix;
#[cfg(test)]
mod tests;
mod trait_and_machine_tables;
mod type_reference_queries;
pub mod type_system;
pub mod values;
mod wire_schema_queries;

pub use commitments::{
    BoundaryCallingPlanCommitment, BoundaryCallingPlanIdentity, ClosedConformanceApplication,
    ClosedConformanceApplicationCommitment, ClosedConformanceConstArgument,
    ClosedConformanceRowIdentity, FusedServiceErasureAuthorization, MachineSpecialization,
    MachineSpecializationCommitment, MachineTemplateCommitment, StaticRequirementDispatch,
};
pub use placed_plans::{
    PlacedAccessorTarget, PlacedAtomicObservingResultContract, PlacedAtomicResidentContract,
    PlacedFieldPlan, PlacedViewPlan, PlanLaidBitField, PlanLaidBitFragment, PlanLaidIntegerField,
    PlanLaidLayout, PlanLaidRepeatedField,
};

use crate::{
    data, domain, expression, machine, mathematical, measure, proposition, signature, snapshot,
    trait_definition, types, wire,
};
use arena::{Arena, Handle, HandleSpan};
use diagnostics::PhaseSnapshot;
use std::ops::{Deref, DerefMut};
use symbols::SymbolTable;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTrees {
    pub roots: TypedTreeRoots,
    pub tables: TypedTreeTables,
    pub symbols: SymbolTable,
    /// EFX: normalized boundary-service declarations and rows, copied from
    /// symbol-resolved trees. All durable service consumers migrate here.
    pub service_reaches: language_semantics::ServiceReachTable,
    pub service_reach_rows: language_semantics::ServiceReachRowTable,
    /// Exact source-backed `reaches` occurrences, retained separately from
    /// normalized semantic rows so explicit empty ceilings survive.
    pub authored_service_reach_rows: Vec<signature::AuthoredServiceReachRow>,
    /// STR4 checked plans, slice 1: the semantic-domain interner, copied
    /// verbatim from the resolved trees.
    pub semantic_domains: language_semantics::SemanticDomainTable,
    /// Exact structured identities for external realization bindings. Typed
    /// conformance and supply rows retain ids into this table, so downstream
    /// consumers never need to rescan source syntax to recover the binding.
    pub external_bindings: language_semantics::ExternalBindingTable,
    /// Validated layout plans for PLAN-LAID VALUE TYPES (`gdt: CLayout<Gdt>`
    /// in type position; programmable-layouts L4). Populated by the compiler
    /// pipeline AFTER build-time plan evaluation + validation; the native
    /// layout builder places the named data definitions at these offsets
    /// instead of running its own packing. Empty for programs with no
    /// plan-laid fields.
    pub plan_laid_layouts: Vec<PlanLaidLayout>,
    /// Canonical source `Placed<P, T>` derivations. Each record binds the
    /// synthetic view/accessor types to the validated placement plan that
    /// selected them, so checking and lowering never reconstruct permissions
    /// from generated names.
    pub placed_view_plans: Vec<PlacedViewPlan>,
    /// Derived wire placements (mint arc rung 2a): one arena for every
    /// schema's placements, referenced by span from `wire_schema_plans` --
    /// arena-backed storage, HandleSpan ownership.
    pub wire_placements: Arena<wire::WirePlacement>,
    pub wire_encode_obligations: Arena<wire::WireEncodeObligation>,
    pub wire_schema_plans: Vec<wire::WireSchemaPlan>,
    /// MP4: deterministic records of generic-machine specializations applied
    /// before checked lowering. The template keeps its declaration symbol;
    /// this record is the cache/audit identity of the concrete argument tuple.
    pub machine_specializations: Vec<MachineSpecialization>,
    /// Canonical calling-policy identities evaluated for concrete boundary
    /// requirements. The key is semantic (boundary trait + requirement
    /// machine), while the policy type/source body is deliberately absent:
    /// only the validated plan fingerprint is public contract material.
    pub boundary_calling_plans: Vec<BoundaryCallingPlanIdentity>,
    /// Compiler-owned authorizations for erasing one exact routed service
    /// carrier after a Fused provider selection. Ordinary Psi lowering keeps
    /// this empty and therefore cannot erase `Service<R>` by type shape alone.
    pub fused_service_erasures: Vec<FusedServiceErasureAuthorization>,
    /// PDI3 exact operation/algebra selections for proof-static open index
    /// expressions. The expression tree remains the canonical structural
    /// input; these records bind each operator node to the public operation
    /// contract and proved algebra instance that license normalization.
    pub open_index_normalizations: Vec<OpenIndexNormalization>,
    /// Exact owner identity and authored names for erased evidence forwarding;
    /// checked lowering binds both names to checked evidence-term handles.
    pub evidence_forwardings: Vec<EvidenceForwarding>,
    /// Calls whose proof-output lane is bound immediately. The
    /// group itself is proof metadata; a contextual scalar `value` separately
    /// names its corresponding ordinary runtime local/call statement.
    pub proof_output_calls: Vec<ProofOutputCall>,
    /// Exact typed roots for private ranking witnesses. The normalized
    /// termination plan retains stable semantic text; checked consumers join
    /// through these handles instead of rediscovering expressions by spelling.
    pub ranking_expression_custody: Vec<crate::ranking::RankingExpressionCustody>,
    /// Range-endpoint call expressions whose const evaluation deferred to
    /// selected execution. The build-time evaluation owner marks every
    /// still-authored endpoint its continuation will fold; interim checking
    /// treats exactly those marked bounds as pending constants rather than
    /// non-constant or dependent ranges. The evaluator removes each mark as
    /// its endpoint folds, and checked lowering refuses any mark that
    /// survives to it -- a surviving mark is a lost continuation, never an
    /// unbounded range.
    pub pending_const_range_endpoints:
        std::collections::HashSet<crate::expression::ExpressionHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceForwarding {
    pub machine_symbol: symbols::SymbolHandle,
    pub state_symbol: symbols::SymbolHandle,
    pub statement_index: usize,
    /// Original pre-erasure source coordinate for lexical evidence scope.
    pub source_statement_index: usize,
    pub target: crate::name::Identifier,
    pub source: crate::name::Identifier,
    pub source_conformance: Option<symbols::SymbolHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofOutputCall {
    pub machine_symbol: symbols::SymbolHandle,
    pub state_symbol: symbols::SymbolHandle,
    pub statement_index: usize,
    /// Pre-erasure coordinate used only to normalize other erased metadata.
    pub source_statement_index: usize,
    /// Exact runtime local statement synthesized for a contextual `value`
    /// result. Proof-only calls have no runtime statement.
    pub runtime_call_statement_index: Option<usize>,
    pub bindings: Box<[ProofOutputSelector]>,
    pub call: crate::expression::ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofOutputSelector {
    pub output_field: crate::name::Identifier,
    pub binding: crate::name::Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenIndexNormalization {
    pub expression: crate::expression::ExpressionHandle,
    pub index_type: crate::types::TypeReferenceHandle,
    pub operations: Vec<OpenIndexOperationSelection>,
    /// Artifact provenance only. It never enters semantic type identity.
    pub normalizer_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenIndexOperationSelection {
    pub expression: crate::expression::ExpressionHandle,
    pub spelling: language_core::operator_spelling::OperatorSpelling,
    pub operator: symbols::SymbolHandle,
    pub operation_contract_identity: String,
    pub provider: symbols::SymbolHandle,
    pub algebra_trait: symbols::SymbolHandle,
    pub algebra_requirement: String,
    pub algebra_alias: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTreeTables {
    /// Package-agnostic authored-selection custody carried verbatim from
    /// symbol resolution. Checked facts join late selections through the
    /// opaque occurrence identities in this ledger, never source rendering.
    authored_declaration_selections:
        language_semantics::declaration_selection::AuthoredDeclarationSelections,
    pub const_declarations: Arena<crate::constant::ConstDeclaration>,
    pub data_definitions: Arena<data::DataDefinition>,
    pub data_type_parameters: Arena<data::TypeParameter>,
    pub data_members: Arena<data::DataMember>,
    pub data_payload_fields: Arena<data::DataField>,
    pub domain_definitions: Arena<domain::DomainDefinition>,
    pub proof_facts: Arena<domain::ProofFact>,
    proof_fact_source_spans: Vec<Option<source::SourceSpan>>,
    pub propositions: Arena<proposition::PropositionDefinition>,
    pub proposition_binders: Arena<proposition::PropositionBinder>,
    /// Typed mathematical `let`/`boundary let` declarations
    /// (PROOF-CONTRACT-MIGRATION) with their telescope and declaration-local
    /// mathematical type grammar.
    pub mathematical_definitions: Arena<mathematical::MathematicalDefinition>,
    pub mathematical_parameters: Arena<mathematical::MathematicalParameter>,
    pub mathematical_types: Arena<mathematical::MathematicalType>,
    pub domain_path_members: Arena<crate::name::Identifier>,
    pub operator_path_members: Arena<crate::name::Identifier>,
    pub machines: Arena<machine::Machine>,
    pub measures: Arena<measure::MeasureDefinition>,
    pub measure_path_members: Arena<crate::name::Identifier>,
    pub operators: Arena<crate::operator::OperatorDefinition>,
    pub machine_owned_data: Arena<machine::OwnedData>,
    pub machine_trait_conformances: Arena<machine::TraitConformance>,
    pub machine_states: Arena<crate::state::State>,
    pub state_parameters: Arena<signature::StateParameter>,
    pub traits: Arena<trait_definition::TraitDefinition>,
    pub conformances: Arena<trait_definition::Conformance>,
    pub trait_requirements: Arena<trait_definition::TraitRequirement>,
    pub trait_machine_signatures: Arena<signature::StateSignature>,
    pub signature_invokes: Arena<signature::AuthoredInvocation>,
    pub signature_contracts: Arena<signature::SignatureContract>,
    pub expression_table: expression::ExpressionTable,
    pub statement_table: crate::statement::StatementTable,
    pub type_reference_table: types::TypeReferenceTable,
    pub wire_schemas: Arena<wire::WireSchema>,
    pub wire_members: Arena<wire::WireMember>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTreeRoots {
    pub const_declarations: HandleSpan<crate::constant::ConstDeclaration>,
    pub data_definitions: HandleSpan<data::DataDefinition>,
    pub domain_definitions: HandleSpan<domain::DomainDefinition>,
    pub machines: HandleSpan<machine::Machine>,
    pub measures: HandleSpan<measure::MeasureDefinition>,
    pub operators: HandleSpan<crate::operator::OperatorDefinition>,
    /// Operator-signature views of token-bearing machines (`machine + Vec2::add`),
    /// one per binding, stored in the same operator table as the authored
    /// `operator` declarations but rooted separately so `operators()` keeps
    /// enumerating authored declarations only. See
    /// [`crate::operator::resolve_spelling`] for why the view exists.
    pub machine_token_bindings: HandleSpan<crate::operator::OperatorDefinition>,
    pub propositions: HandleSpan<proposition::PropositionDefinition>,
    /// Typed mathematical `let`/`boundary let` declarations
    /// (PROOF-CONTRACT-MIGRATION): a distinct root category carrying a
    /// dependent result type and a transparent term or named assumption, with
    /// no executable body.
    pub mathematical_definitions: HandleSpan<mathematical::MathematicalDefinition>,
    pub traits: HandleSpan<trait_definition::TraitDefinition>,
    pub conformances: HandleSpan<trait_definition::Conformance>,
    pub wire_schemas: HandleSpan<wire::WireSchema>,
}

impl TypedTreeRoots {
    pub fn with_roots(
        data_definitions: HandleSpan<data::DataDefinition>,
        domain_definitions: HandleSpan<domain::DomainDefinition>,
        machines: HandleSpan<machine::Machine>,
        operators: HandleSpan<crate::operator::OperatorDefinition>,
        traits: HandleSpan<trait_definition::TraitDefinition>,
    ) -> Self {
        Self {
            const_declarations: HandleSpan::default(),
            data_definitions,
            domain_definitions,
            machines,
            measures: HandleSpan::default(),
            operators,
            machine_token_bindings: HandleSpan::default(),
            propositions: HandleSpan::default(),
            mathematical_definitions: HandleSpan::default(),
            traits,
            conformances: HandleSpan::default(),
            wire_schemas: HandleSpan::default(),
        }
    }
}

impl TypedTrees {
    pub fn authored_service_reach_rows_for(
        &self,
        owner: symbols::SymbolHandle,
    ) -> impl Iterator<Item = &signature::AuthoredServiceReachRow> {
        self.authored_service_reach_rows
            .iter()
            .filter(move |row| row.owner == owner)
    }

    pub fn with_roots(
        roots: TypedTreeRoots,
        tables: TypedTreeTables,
        symbols: SymbolTable,
    ) -> Self {
        Self {
            roots,
            tables,
            symbols,
            service_reaches: language_semantics::ServiceReachTable::default(),
            service_reach_rows: language_semantics::ServiceReachRowTable::default(),
            authored_service_reach_rows: Vec::new(),
            semantic_domains: language_semantics::SemanticDomainTable::default(),
            external_bindings: language_semantics::ExternalBindingTable::default(),
            plan_laid_layouts: Vec::new(),
            placed_view_plans: Vec::new(),
            wire_placements: Arena::new(),
            wire_encode_obligations: Arena::new(),
            wire_schema_plans: Vec::new(),
            machine_specializations: Vec::new(),
            boundary_calling_plans: Vec::new(),
            fused_service_erasures: Vec::new(),
            open_index_normalizations: Vec::new(),
            evidence_forwardings: Vec::new(),
            proof_output_calls: Vec::new(),
            ranking_expression_custody: Vec::new(),
            pending_const_range_endpoints: std::collections::HashSet::new(),
        }
    }

    pub fn bind_fused_service_erasures(
        &mut self,
        mut authorizations: Vec<FusedServiceErasureAuthorization>,
    ) -> Result<(), String> {
        authorizations.sort_by_key(|authorization| {
            (
                authorization.requirement.arena_index(),
                authorization.requirement.generation(),
            )
        });
        for authorization in &authorizations {
            if authorization.provider_plan_digest == [0; 32]
                || !self.traits().iter().any(|definition| {
                    definition.is_boundary && definition.symbol == authorization.requirement
                })
            {
                return Err(
                    "fused service erasure authorization lacks an exact boundary requirement or selected-plan digest"
                        .to_owned(),
                );
            }
        }
        if authorizations
            .windows(2)
            .any(|pair| pair[0].requirement == pair[1].requirement)
        {
            return Err(
                "fused service erasure authorizations contain duplicate boundary requirements"
                    .to_owned(),
            );
        }
        self.fused_service_erasures = authorizations;
        Ok(())
    }

    pub fn fused_service_erasure(
        &self,
        requirement: symbols::SymbolHandle,
    ) -> Option<FusedServiceErasureAuthorization> {
        self.fused_service_erasures
            .iter()
            .find(|authorization| authorization.requirement == requirement)
            .copied()
    }

    pub fn ranking_expression_custody_for(
        &self,
        machine: symbols::SymbolHandle,
    ) -> Option<&crate::ranking::RankingExpressionCustody> {
        self.ranking_expression_custody
            .iter()
            .find(|custody| custody.machine == machine)
    }

    pub fn snapshot(&self) -> snapshot::TypedTreesSnapshot {
        snapshot::TypedTreesSnapshot::from_typed_trees(self)
    }

    pub fn snapshot_json(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json()
    }

    pub fn snapshot_json_pretty(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json_pretty()
    }
}

fn proof_fact_source_span_index(handle: Handle<domain::ProofFact>) -> usize {
    usize::try_from(handle.arena_index())
        .expect("proof fact source-span index exceeds usize")
        .checked_sub(1)
        .expect("proof fact source-span handle must be valid")
}

/// The static const spelling one machine argument selects, mirroring the
/// specialization pipeline's argument spelling exactly: decimal for integer
/// literals and integer-valued const declarations, the canonical atom for
/// structured const values, and `true`/`false` for Boolean literals.
fn static_const_argument_spelling(
    program: &TypedTrees,
    argument: &expression::StaticMachineArgument,
) -> Option<String> {
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};

    if argument.application.is_some() || argument.evidence_projection.is_some() {
        return None;
    }
    if let Some(literal) = &argument.const_literal {
        return Some(
            literal
                .value_i64()
                .map(i128::from)
                .or_else(|| literal.value_u64().map(i128::from))
                .map_or_else(|| literal.text().to_owned(), |value| value.to_string()),
        );
    }
    if argument.symbol.is_valid() {
        let declaration = program
            .const_declarations()
            .iter()
            .find(|declaration| declaration.symbol == argument.symbol)?;
        let value = CanonicalConstValue::new(
            program.display_type_reference(declaration.declared_type),
            declaration.canonical_value_encoding.as_ref()?.clone(),
            argument.display_name(),
        );
        return Some(match value.decode_encoding()? {
            DecodedCanonicalConstValue::Integer { value, .. } => value.to_string(),
            _ => value.atom(),
        });
    }
    let [name] = argument.path.as_ref() else {
        return None;
    };
    match name.as_str() {
        "true" => Some(CanonicalConstValue::boolean(true).atom()),
        "false" => Some(CanonicalConstValue::boolean(false).atom()),
        // A previous specialization may forward a compiler-created atom.
        spelling => CanonicalConstValue::from_atom(spelling).map(|value| value.atom()),
    }
}

/// Wrap one static const spelling in the normalized `Named` type identity the
/// specialization binding records: `named(integer-const(v))` for decimal
/// spellings, `named(canonical-const(type(T),encoding(E)))` for canonical
/// atoms. The escaping rules mirror `type_identity`'s atom/compound leaves.
fn static_const_identity_from_spelling(spelling: &str) -> String {
    fn escape(value: &str) -> String {
        let mut out = String::with_capacity(value.len());
        for character in value.chars() {
            if matches!(character, '\\' | '(' | ')' | ',') {
                out.push('\\');
            }
            out.push(character);
        }
        out
    }
    if let Some(value) = language_semantics::const_value::CanonicalConstValue::from_atom(spelling) {
        return format!(
            "named(canonical-const(type({}),encoding({})))",
            escape(&value.type_name),
            escape(&value.encoding)
        );
    }
    if let Ok(value) = spelling.parse::<i128>() {
        return format!("named(integer-const({value}))");
    }
    format!("named(name({}))", escape(spelling))
}

impl PhaseSnapshot for TypedTrees {
    type Snapshot = snapshot::TypedTreesSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        TypedTrees::snapshot(self)
    }
}

impl Deref for TypedTrees {
    type Target = TypedTreeTables;

    fn deref(&self) -> &Self::Target {
        &self.tables
    }
}

impl DerefMut for TypedTrees {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tables
    }
}
