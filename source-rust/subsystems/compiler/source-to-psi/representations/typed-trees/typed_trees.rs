//! Typed Omega declarations and bodies after type checking.
//! Ordered roots select declarations from the tables. The symbol table, source
//! selections and retained type identities belong to the same checked forest.
#![forbid(unsafe_code)]
#![allow(
    clippy::obfuscated_if_else,
    reason = "valid-handle copy expressions use one uniform then/fallback idiom"
)]

pub use crate as source_state;
use crate::product_operand::ProductOperandApplication;
use crate::source_symbols::SymbolTable;
use arena::{Handle, OwnedInterner};
use source_declarations::{declaration_selection, source_selections};
use source_symbol_table::{source_inputs, source_symbols, source_tables};
use symbols::SymbolHandle;

#[derive(Debug, Default, PartialEq, Eq, record_storage::Record)]
#[record(
    format = b"OmegaTypedTrees1",
    write_scope = crate::source_state::type_identity::write_under_own_numbering
)]
pub struct TypedTrees {
    pub identity: ProgramIdentity,
    pub excluded_authored_selection_occurrences: AuthoredSelectionExclusions,
    pub roots: TypedTreeRoots,
    pub tables: TypedTreeTables,
    pub symbols: SymbolTable,
    pub service_reaches: crate::source_tables::ServiceReachTable<
        crate::source_state::trait_definition::RetainedTraitScope,
    >,
    pub service_reach_rows: crate::source_tables::ServiceReachRowTable,
    pub authored_service_reach_rows: arena::RetainedRows<signature::AuthoredServiceReachRow>,
    pub semantic_domains: crate::source_tables::SemanticDomainTable,
    pub applied_domain_applications: domain::AppliedDomainApplications,
    pub applied_data_cleanup_selections: Vec<domain::AppliedDataCleanupSelection>,
    /// Build's admitted carriers, ordered by the opaque declaration's symbol.
    pub opaque_representation_selections:
        Vec<opaque_representations::OpaqueRepresentationSelection>,
    pub external_bindings: crate::source_tables::ExternalBindingTable,
    pub plan_laid_layouts: Vec<PlanLaidLayout>,
    pub plan_laid_target_layouts: arena::RootedArena<PlanLaidTargetLayout>,
    pub plan_laid_schema_fields: arena::RootedArena<PlanLaidSchemaField>,
    pub plan_laid_retired_field_identities: arena::RootedArena<PlanLaidRetiredFieldIdentity>,
    pub access_view_plans: Vec<AccessViewPlan>,
    pub access_lifecycle_plans: arena::RootedArena<AccessLifecyclePlan>,
    pub access_policy_invocations: arena::RootedArena<source_text::SourceSpan>,
    pub access_borrow_factories: arena::RetainedRows<expression::AccessBorrowOperationRequest>,
    pub wire_placements: arena::RootedArena<wire::WirePlacement>,
    pub wire_encode_obligations: arena::RootedArena<wire::WireEncodeObligation>,
    pub wire_schema_plans: Vec<wire::WireSchemaPlan>,
    pub machine_specializations: MachineSpecializations,
    pub captured_machine_applications: arena::RootedArena<CapturedMachineApplication>,
    pub captured_machine_states: arena::RootedArena<CapturedMachineState>,
    pub trait_implementation_applications: arena::RootedArena<TraitImplementationApplication>,
    pub data_implementation_applications: arena::RootedArena<DataImplementationApplication>,
    pub target_machine_families: Vec<crate::source_state::machine::TargetMachineFamily>,
    /// Requests no pass served, on the declarations that hold them, ordered
    /// by declaration (`deferred_refusals.rs`).
    pub deferred_refusals: arena::RetainedRows<deferred_refusals::DeferredRefusal>,
    pub evidence_forwardings: Vec<EvidenceForwarding>,
    pub proof_output_calls: Vec<ProofOutputCall>,
    pub declaration_indexes: declaration_index::DeclarationIndexes,
    pub ranking_expression_custody:
        arena::RetainedRows<crate::source_state::ranking::RankingExpressionCustody>,
    pub pending_const_range_endpoints:
        std::collections::HashSet<crate::source_state::expression::ExpressionHandle>,
    pub constant_recipes: arena::RootedArena<constant_recipes::RetainedConstantRecipe>,
    pub constant_expression_contexts:
        arena::RootedArena<constant_recipes::ConstantExpressionContext>,
    pub constant_recipe_closure: constant_recipes::ConstantRecipeClosure,
    pub callable_identities:
        OwnedInterner<crate::source_state::type_identity::CallableIdentityTable>,
    pub type_identities: crate::source_state::type_identity::TypeIdentityTable,
}

#[macro_use]
mod table_macros;
pub(crate) use table_macros::{declaration_columns, span_columns, typed_tree_table};
mod callables;
mod definitions;
mod exports;
mod expressions;
mod proofs;
mod storage;
mod type_records;
pub use exports::*;

#[derive(Debug, Clone, Copy, Eq)]
pub struct ProgramIdentity(u64);

impl ProgramIdentity {
    pub fn get(self) -> u64 {
        self.0
    }
}

impl Default for ProgramIdentity {
    fn default() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

impl PartialEq for ProgramIdentity {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl record_storage::Record for ProgramIdentity {
    fn write(
        &self,
        _: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        Ok(())
    }
    fn read<Context>(
        _: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
        Ok(Self::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct EvidenceForwarding {
    pub machine_symbol: symbols::SymbolHandle,
    pub state_symbol: symbols::SymbolHandle,
    pub statement_index: usize,
    pub source_statement_index: usize,
    pub target: crate::source_state::name::Identifier,
    pub source: crate::source_state::name::Identifier,
    pub source_conformance: Option<symbols::SymbolHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct ProofOutputCall {
    pub machine_symbol: symbols::SymbolHandle,
    pub state_symbol: symbols::SymbolHandle,
    pub statement_index: usize,
    pub source_statement_index: usize,
    pub runtime_call_statement_index: Option<usize>,
    pub bindings: Box<[ProofOutputSelector]>,
    pub call: crate::source_state::expression::ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct ProofOutputSelector {
    pub output_field: crate::source_state::name::Identifier,
    pub binding: crate::source_state::name::Identifier,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct DataApplicationView {
    definition: Handle<data::DataDefinition>,
    is_closed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct DesignatedStaticTypeArgument {
    pub owner: symbols::SymbolHandle,
    pub reference: types::TypeReferenceHandle,
    pub source_span: source_text::SourceSpan,
}

fn proof_fact_source_span_index(handle: Handle<domain::ProofFact>) -> usize {
    usize::try_from(handle.arena_index())
        .expect("proof fact source-span index exceeds usize")
        .checked_sub(1)
        .expect("proof fact source-span handle must be valid")
}

pub(crate) fn remapped<'scope>(
    symbol: SymbolHandle,
    symbols: impl Into<SymbolRebinding<'scope>>,
) -> SymbolHandle {
    symbols.into().lookup(symbol).unwrap_or(symbol)
}

pub(crate) fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '\\' | '(' | ')' | ',') {
            out.push('\\');
        }
        out.push(character);
    }
    out
}

pub mod typed_trees {
    pub use crate::source_state::*;
}
