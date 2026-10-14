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

macro_rules! typed_tree_table {
    ($( $field:ident => $get:ident ($ty:ty) $( [ $push:ident ] )? $(, $inline:ident)? ;)*) => {
        $(
            $( pub fn $push(&mut self, value: $ty) {
                self.tables
                    .$field
                    .append_to_span(&mut self.roots.$field, value);
            } )?

            $( #[ $inline ] )?
            pub fn $get(&self) -> &[$ty] {
                self.tables
                    .$field
                    .span_or_empty(self.roots.$field)
            }
        )*
    };
}
pub(crate) use typed_tree_table;

mod authored_targets;
mod callable_applications;
pub use callable_applications::{
    AppliedMachineCallable, CallableSubstitution, ClosedRequirementApplication,
};
pub mod calls;
mod commitments;
pub mod constant_recipes;
mod frozen;
pub use frozen::FrozenTypedTrees;
pub mod control_flow;
pub mod declaration_index;
mod declaration_tables;
pub mod declarations;
pub mod evidence;
pub mod inspection;
pub mod names;
mod placed_plans;
pub(crate) mod retained_prefix;
pub mod sum_reachability;
mod trait_and_machine_tables;
mod type_reference_queries;
pub mod type_system;
pub mod values;
mod wire_schema_queries;

pub use commitments::{
    CapturedMachineApplication, CapturedMachineState, ClosedConformanceApplication,
    ClosedConformanceApplicationCommitment, ClosedConformanceApplicationSite,
    ClosedConformanceConstArgument, ClosedConformanceRowIdentity, MachineSpecialization,
    MachineSpecializationCommitment, MachineTemplateCommitment, StaticRequirementDispatch,
    closed_conformance_applications_of, machine_specialization_matches_template_identity,
};
pub use placed_plans::{
    PlacedAccessorTarget, PlacedAtomicObservingResultContract, PlacedAtomicResidentContract,
    PlacedFieldPlan, PlacedScalarAccessorDeclaration, PlacedScalarAccessorOperation,
    PlacedViewPlan, PlanLaidBitField, PlanLaidBitFragment, PlanLaidGeometry, PlanLaidIntegerField,
    PlanLaidLayout, PlanLaidRealizations, PlanLaidRepeatedField, PlanLaidSchemaArgument,
    PlanLaidSchemaField, PlanLaidSchemaFieldKind, PlanLaidTargetLayout,
};

use crate::{
    data, domain, expression, machine, mathematical, measure, proposition, signature, snapshot,
    trait_definition, types, wire,
};
use arena::{Arena, Handle, HandleSpan, OwnedIndex, OwnedInterner};
use diagnostics::PhaseSnapshot;
use std::ops::{Deref, DerefMut};
use symbols::{SymbolHandle, SymbolTable};

/// A process-unique tag for one `TypedTrees` instance.
///
/// Memo caches outside this crate key their entries on the program they were
/// computed for. An address cannot serve as that key: when a program is
/// dropped the allocator is free to place the next one at the same address,
/// and a cache that trusts the address then answers one program's queries with
/// another's verdicts. Mixing arena base pointers into a fingerprint does not
/// close that, because those blocks are recycled together with the program.
///
/// `Default` mints the next tag, so every `TypedTrees` built by
/// `Default::default()` or a `..Default::default()` literal carries its own.
/// Equality is deliberately vacuous: the tag identifies an instance, not
/// program content, and `TypedTrees` equality compares content.
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTrees {
    /// This instance's cache key; see `ProgramIdentity`. A clone keeps the
    /// tag it was cloned from: its content is identical at that moment, and a
    /// later mutation moves the arena lengths a consumer's fingerprint reads.
    pub identity: ProgramIdentity,
    /// Immutable declaration frontier for designated product operands during
    /// Build checking and execution. Ordinary declaration/body lookup uses this
    /// program's roots, never this view. The original frontier retains exact
    /// symbol and interface handles without granting its bodies execution.
    pub product_declaration_frontier: Option<std::sync::Arc<TypedTrees>>,
    /// Sorted exact ledger IDs proven to belong only to projected-away roots.
    /// The ledger itself remains intact; unknown or orphan IDs are never
    /// excluded by root projection and still require ordinary finalization.
    pub excluded_authored_selection_occurrences:
        Vec<crate::AuthoredDeclarationSelectionOccurrenceId>,
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
    /// Guarded exact policy-member geometries owned by application spans.
    pub plan_laid_target_layouts: Arena<PlanLaidTargetLayout>,
    /// Exact active Schema argument rows retained by family applications.
    pub plan_laid_schema_fields: Arena<PlanLaidSchemaField>,
    /// Canonical source `Placed<P, T>` derivations. Each record binds the
    /// synthetic view/accessor types to the validated placement plan that
    /// selected them, so checking and lowering never reconstruct permissions
    /// from generated names.
    pub placed_view_plans: Vec<PlacedViewPlan>,
    /// Exact sealed borrowed-view prototypes selected during typing. These
    /// declaration rows supply no memory, resource profile or loan authority.
    pub placed_borrow_factories: Vec<expression::PlacedBorrowOperationRequest>,
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
    /// Capture applications precede, and never impersonate, method invocation
    /// specialization. Referenced realizers have no row in this arena.
    pub captured_machine_applications: Arena<CapturedMachineApplication>,
    pub captured_machine_states: Arena<CapturedMachineState>,
    /// Requirements instantiated under the `dyn Trait<Args>` applications
    /// that select or call them. Checking appends one record per
    /// (selector application, requirement) after specialization converges.
    pub dynamic_slots: Vec<crate::dynamic_traits::DynamicSlotSignature>,
    /// Product-scope target-machine families: one record per authored
    /// contract name implemented under target heads, keying each target's
    /// body. Typing produces it from resolved family handles before the
    /// per-target swap and check run. Product pruning preserves every member;
    /// runtime families lower into Terminal and declaration programs retain
    /// Build effects separately. Early portable-call retargeting remains a
    /// transitional consumer. Empty for programs without admitted
    /// product-scope target-scoped declarations.
    /// Seeded typing preserves the base prefix and appends new families in
    /// name order within the extension cohort.
    pub target_machine_families: Vec<crate::machine::TargetMachineFamily>,
    /// Exact source semantic-role inputs supplied by package admission or an
    /// explicitly proposed review role. These describe callable completion;
    /// they grant no selected provider or execution authority.
    pub boundary_completion_declarations: Arena<BoundaryCompletionDeclaration>,
    /// Exact owner identity and authored names for erased evidence forwarding;
    /// checked lowering binds both names to checked evidence-term handles.
    pub evidence_forwardings: Vec<EvidenceForwarding>,
    /// Calls whose proof-output lane is bound immediately. The
    /// group itself is proof metadata; a contextual scalar `value` separately
    /// names its corresponding ordinary runtime local/call statement.
    pub proof_output_calls: Vec<ProofOutputCall>,
    /// Symbol-to-declaration indexes owned by this program. Each slot pairs
    /// its index with the revision of the arenas it was built from, so a
    /// table mutation — through the `push_*` methods or any other mutating
    /// call — rebuilds the index on the next query. Declaration mutation goes
    /// through methods; the revisions keep the indexes honest even where a
    /// table remains reachable through a `pub` field.
    pub declaration_indexes: declaration_index::DeclarationIndexes,
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
    /// Constant positions whose evaluation waits for a selected realization.
    /// The shared program keeps their authored symbolic nodes; checking
    /// records what it needs from each; a closed copy carries the values.
    pub constant_recipes: Arena<constant_recipes::RetainedConstantRecipe>,
    /// Every recipe's value under this copy's target capsule. Empty in the
    /// shared program: no consumer may guess a value the capsule has not fixed.
    pub constant_recipe_closure: constant_recipes::ConstantRecipeClosure,
    /// Durable named-callable structural keys. Checked plans retain these
    /// handles across arena revisions; historical keys stay renderable.
    pub callable_identities: OwnedInterner<crate::type_identity::CallableIdentityTable>,
    /// Current declarations and reverse lookup, rebuilt on structural revision.
    pub callable_identity_memo: OwnedIndex<crate::type_identity::CallableIdentityMemo>,
    /// The hash-consed structural type keys minted on demand by the type
    /// identity queries. A handle minted here compares by integer; its text
    /// stays inside the table for diagnostics and artifact encoding.
    pub type_identities: OwnedInterner<crate::type_identity::TypeIdentityTable>,
}

/// A package-scoped declaration whose consumer role supplies its completion
/// contract before checking. Final provider admission independently rejoins
/// the accepted role's complete schema and selected-plan commitments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoundaryCompletionDeclaration {
    /// Absent only for the arena's invalid sentinel; every admitted row must
    /// retain its nonzero package identity.
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub owner: symbols::SymbolHandle,
    pub owner_path: String,
    pub requirement: symbols::SymbolHandle,
    pub requirement_path: String,
    pub completion: semantic_vocabulary::BoundaryCompletion,
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

/// Producer-retained fields for one application and its separate closure
/// status. Substitution alone grants no concrete storage or checking authority.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataApplicationView {
    definition: Handle<data::DataDefinition>,
    is_closed: bool,
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
    /// Applied field views, indexed by the exact type-reference row. A symbolic
    /// view retains its caller's binders without granting closed storage.
    /// Their definitions live in the declaration arena but not the authored
    /// root roster: substitution creates no new nominal or field identity.
    pub data_application_definitions: Arena<DataApplicationView>,
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
    pub(crate) applied_callable_signatures: Arena<callable_applications::AppliedCallableSignature>,
    pub(crate) callable_substitutions: Arena<callable_applications::CallableSubstitution>,
    pub(crate) trait_application_frames: Arena<trait_definition::RetainedTraitApplicationFrame>,
    pub(crate) callable_argument_identities: Arena<crate::type_identity::NormalizedTypeIdentity>,
    pub(crate) callable_lifetime_substitutions:
        Arena<crate::static_applications::StaticMachineLifetimeBinding>,
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
    /// Declaration input for designated product-description operations only.
    /// An ordinary full program already owns that same declaration frontier.
    pub fn product_declarations(&self) -> &TypedTrees {
        self.product_declaration_frontier.as_deref().unwrap_or(self)
    }

    pub fn plan_laid_layout(&self, data_symbol: symbols::SymbolHandle) -> Option<&PlanLaidLayout> {
        self.plan_laid_layouts
            .iter()
            .find(|layout| layout.data_symbol == data_symbol)
    }

    pub fn machine_specialization(
        &self,
        instance: symbols::SymbolHandle,
    ) -> Option<&MachineSpecialization> {
        self.machine_specializations
            .iter()
            .find(|specialization| specialization.instance == instance)
    }

    /// Whether an authored occurrence belongs to this checking context.
    /// A Build view retains descriptor ledger IDs, but does not judge the
    /// Product-authored declarations those IDs describe.
    pub fn checks_authored_selection(
        &self,
        selection: &language_semantics::declaration_selection::AuthoredDeclarationSelection,
    ) -> bool {
        self.excluded_authored_selection_occurrences
            .binary_search(&selection.occurrence_id())
            .is_err()
            && (self.product_declaration_frontier.is_none()
                || self
                    .symbols
                    .source_file(selection.source_span())
                    .is_none_or(|file| {
                        file.origin == source::SourceOrigin::Toolchain
                            || file.dependency_scope == source::DependencyScope::Build
                    }))
    }

    /// The path of `symbol`'s source file relative to its package root when
    /// that source is toolchain-provided; `None` for authored sources.
    pub fn symbol_toolchain_relative_path(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<&std::path::Path> {
        let span = self.symbols.symbol_source_span(symbol)?;
        let file = self.symbols.source_file(span)?;
        if file.origin != source::SourceOrigin::Toolchain {
            return None;
        }
        file.path.strip_prefix(&file.package_root).ok()
    }

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
            identity: ProgramIdentity::default(),
            product_declaration_frontier: None,
            excluded_authored_selection_occurrences: Vec::new(),
            roots,
            tables,
            symbols,
            service_reaches: language_semantics::ServiceReachTable::default(),
            service_reach_rows: language_semantics::ServiceReachRowTable::default(),
            authored_service_reach_rows: Vec::new(),
            semantic_domains: language_semantics::SemanticDomainTable::default(),
            external_bindings: language_semantics::ExternalBindingTable::default(),
            plan_laid_layouts: Vec::new(),
            plan_laid_target_layouts: Arena::new(),
            plan_laid_schema_fields: Arena::new(),
            placed_view_plans: Vec::new(),
            placed_borrow_factories: Vec::new(),
            wire_placements: Arena::new(),
            wire_encode_obligations: Arena::new(),
            wire_schema_plans: Vec::new(),
            machine_specializations: Vec::new(),
            captured_machine_applications: Arena::new(),
            captured_machine_states: Arena::new(),
            dynamic_slots: Vec::new(),
            target_machine_families: Vec::new(),
            boundary_completion_declarations: Arena::default(),
            evidence_forwardings: Vec::new(),
            proof_output_calls: Vec::new(),
            declaration_indexes: declaration_index::DeclarationIndexes::default(),
            ranking_expression_custody: Vec::new(),
            pending_const_range_endpoints: std::collections::HashSet::new(),
            constant_recipes: Arena::new(),
            constant_recipe_closure: constant_recipes::ConstantRecipeClosure::default(),
            callable_identities: OwnedInterner::default(),
            callable_identity_memo: OwnedIndex::default(),
            type_identities: OwnedInterner::default(),
        }
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
pub fn static_const_argument_spelling(
    program: &TypedTrees,
    argument: &expression::StaticMachineArgument,
) -> Option<String> {
    use language_semantics::const_value::{
        CanonicalConstValue, DecodedCanonicalConstValue, boolean_literal_spelling,
    };

    if argument.type_reference.is_valid()
        || argument.application.is_some()
        || argument.evidence_projection.is_some()
    {
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
        let declaration = program.const_declaration_by_symbol(argument.symbol)?;
        let value = CanonicalConstValue::new(
            program.display_type_reference(declaration.declared_type),
            declaration.evaluated_value.as_ref()?.encoding()?.to_owned(),
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
    if let Some(value) = boolean_literal_spelling(name.as_str()) {
        return Some(CanonicalConstValue::boolean(value).atom());
    }
    // A previous specialization may forward a compiler-created atom.
    CanonicalConstValue::from_atom(name.as_str()).map(|value| value.atom())
}

/// Wrap one static const spelling in the normalized `Named` type identity the
/// specialization binding records: `named(integer-const(v))` for decimal
/// spellings, `named(canonical-const(type(T),encoding(E)))` for canonical
/// atoms. The escaping rules mirror `type_identity`'s atom/compound leaves.
fn static_const_identity_from_spelling(spelling: &str) -> String {
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

/// Symbol-remap lookup shared by the typed-tree copy paths: symbols map
/// through the pair table, unmapped symbols stay themselves.
pub(crate) fn remapped(
    symbol: SymbolHandle,
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> SymbolHandle {
    symbols
        .iter()
        .find_map(|(source, target)| (*source == symbol).then_some(*target))
        .unwrap_or(symbol)
}

/// Escape `\`, `(`, `)` and `,` with `\` for the normalized type-identity
/// spelling format shared by const-identity and call-family identity code.
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
