use crate::expression::ExpressionHandle;
use crate::name::DiagnosticName;
use crate::signature::SignatureContract;
use crate::state::State;
use crate::types::TypeReference;
use arena::{Handle, HandleSpan};
use language_core::operator_spelling::OperatorSpelling;
use std::ops::{Deref, DerefMut};
use symbols::SymbolHandle;

/// Frontend completion, distinct from whether source authors a body or which
/// supply rule it declares. Deferred declarations cannot supply execution.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MachineBodyStatus {
    Deferred,
    #[default]
    Complete,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Machine {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub generic_data_origin: GenericDataMachineOrigin,
    pub trait_default_origin: TraitDefaultOrigin,
    pub attached_data: Option<DiagnosticName>,
    /// Exact nominal declaration named by `attached_data`. Spelling remains
    /// diagnostic only; semantic consumers must use this symbol.
    pub attached_data_symbol: SymbolHandle,
    /// The attached owner's ordinary generic application in this method's
    /// binder scope. Its arguments retain declaration identity without
    /// changing the nominal `Self` type used by receiver contracts.
    pub attached_data_application: Option<TypeReference>,
    /// The fixed operator token written immediately after `machine`
    /// (`machine + Vec2::add(...)`), copied once from the syntax declaration.
    /// `None` is the ordinary named form. Symbol resolution rejects a second
    /// direct binding of one token for the same owner and operand shape;
    /// operand-directed selection, cross-package semantic-home ownership, and
    /// executable-supply wiring consume this downstream
    /// (OPERATOR-MACHINE-SUPPLY).
    pub spelling: Option<OperatorSpelling>,
    /// Retained source-level package visibility. Public checked bodies publish
    /// strict effect and operational ceilings without changing supply mode.
    pub is_public: bool,
    /// Structural package exposure, independent of name visibility and supply.
    /// An external boundary retains its exported contract even when its supply
    /// is ExternalRealization rather than Boundary. Authored once in resolution.
    pub interface_exposure:
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    /// Populated once at syntax-to-resolved lowering and copied downstream;
    /// semantic consumers must not reconstruct supply from source spelling or
    /// body presence.
    pub supply_mode: language_semantics::MachineSupplyMode,
    /// Exact authored implementation target. Every product member retains
    /// this identity; its independent portable interface and ordinary machines
    /// have no target. Psi checks all members without choosing a deployment.
    pub target: Option<DiagnosticName>,
    /// Authored target head, independent of the transitional sibling marker.
    pub authored_target: Option<DiagnosticName>,
    /// Exact independent interface of this product-scope target family,
    /// including closed generic attachments. Invalid for ordinary and Build
    /// declarations. This handle supplies no executable implementation.
    pub target_family: SymbolHandle,
    /// Generated Requirement owning the portable callable identity. Target
    /// implementations refine this head; none is its executable definition.
    pub target_family_interface: bool,
    /// Declaration preparation checkpoint. Completing authored bodies must
    /// preserve these already resolved interface clauses and their handles.
    pub target_family_contracts_prepared: bool,
    /// A sibling's symbol spelling, `<path>::<target>`, so the symbol table
    /// keeps one root machine per authored path for name lookups while the
    /// record's `name` stays the authored family path. Empty for every other
    /// machine.
    pub target_symbol_name: DiagnosticName,
    /// Exact source-body presence retained independently from supply mode.
    /// Boundary supply admits both bodyless host declarations and checked
    /// adapters, so downstream review must not reconstruct this distinction
    /// from synthesized state rows.
    pub body_is_present: bool,
    pub body_status: MachineBodyStatus,
    /// Source structural equations are not yet discharged; execution must reject this machine.
    pub structural_type_equations_pending: bool,
    /// Retained template obligation. Without its syntax equations, a later
    /// extension must reject a new application rather than assume discharge.
    pub has_structural_type_equations: bool,
    /// TPR2 (decision 23): the normalized termination plan -- the authored
    /// PUBLIC guarantee and the PRIVATE ranking witness as separate fields.
    /// Populated ONCE at the syntax->resolved lowering (bare `terminates;`
    /// -> published guarantee; `terminates by ...` -> witness subjects +
    /// explicit view, canonical defaults elaborated where the root-state
    /// parameter type determines them), copied -- never re-derived --
    /// downstream. `checked_summary` stays `NoGuarantee` until the cycle
    /// checker establishes it. Ranking subjects remain private witness
    /// material in the storage below.
    pub termination_plan: language_semantics::MachineTerminationPlan,
    /// EFX: normalized boundary-service row, populated after symbol
    /// assignment. Every member is a resolved boundary trait identity.
    pub service_reach_row: language_semantics::ServiceReachRowId,
    /// The published row is an installation-selected upper bound rather than
    /// a fixed callable ceiling.
    pub service_reach_is_installation_bound: bool,
    /// Authored same-stack demand of a bodyless external realization, in bytes.
    /// Zero means the source wrote no `stack N bytes` clause.
    pub admitted_same_stack_bytes: u64,
    /// Exact authored operational-clause keyword occurrences. These explain
    /// the semantic booleans below without making source coordinates part of
    /// contract identity.
    pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
    pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
    /// Transient application partition for a compiler-instantiated trait
    /// default. This separates shared authored source from the distinct exact
    /// realization selected by each conformance and is never package identity.
    pub compiler_selection_partition:
        Option<language_semantics::declaration_selection::CompilerDerivedSelectionPartition>,
    pub storage: MachineStorage,
}

impl Machine {
    /// The spelling of this machine's symbol in the symbol table: the
    /// authored name, or `<name>::<target>` for a target sibling.
    pub fn symbol_spelling(&self) -> &str {
        if self.target.is_some() {
            self.target_symbol_name.as_str()
        } else {
            self.name.as_str()
        }
    }
}

/// Compiler-owned derivation, separate from trait-default selection partitions.
/// The declaration token settles to one original template and one closed data
/// owner before typed lowering can suppress substituted signature occurrences.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GenericDataMachineOrigin {
    pub template_source: DiagnosticName,
    pub template: SymbolHandle,
    pub closed_owner: SymbolHandle,
}

/// Compiler-owned derivation of one trait default instantiation, separate
/// from its selection partition. The instantiated body was authored inside
/// `declaring_trait`, so bare requirement names in it keep that trait's
/// lexical scope instead of the instantiating carrier's. Both fields are empty
/// on authored machines and other synthesis families.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraitDefaultOrigin {
    /// Logical trait path spelled by default synthesis.
    pub trait_source: DiagnosticName,
    /// Exact trait selected by `trait_source` at declaration assignment.
    pub declaring_trait: SymbolHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineStorage {
    pub lifetime_parameters: Vec<DiagnosticName>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub owned_data: HandleSpan<OwnedData>,
    pub satisfies: HandleSpan<TraitConformance>,
    pub conformance_bounds: Vec<GenericConformanceBound>,
    pub ranking_subjects: HandleSpan<ExpressionHandle>,
    pub ranking_view: HandleSpan<DiagnosticName>,
    /// TPR3: argumented-view arguments (`-> Nat::IncreasingTo(limit)`).
    pub ranking_view_arguments: HandleSpan<ExpressionHandle>,
    /// TPR3: the optional `in <range>` rank constraint (a Range expression;
    /// invalid = absent). The checker verifies it structurally.
    pub ranking_range: ExpressionHandle,
    pub invokes: HandleSpan<DiagnosticName>,
    /// Authored operational ceilings, independent from the service row.
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: HandleSpan<SignatureContract>,
    pub states: HandleSpan<Handle<State>>,
}

/// A generic evidence binder `Order satisfies Ranked<Element>`. The binder is
/// an implementation parameter; its argument is one named conformance whose
/// trait application equals this one after substitution. No parameter is a
/// data subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericConformanceBound {
    /// The binder's `ConformanceParameter` symbol.
    pub binder: SymbolHandle,
    pub binder_name: DiagnosticName,
    /// The bound trait, or a transparent refinement of one.
    pub trait_symbol: SymbolHandle,
    pub trait_name: DiagnosticName,
    pub arguments: HandleSpan<TypeReference>,
}

deref_to_storage!(Machine => MachineStorage);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OwnedData {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub type_reference: TypeReference,
    pub initial_value: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TraitConformance {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    /// Explicit target-trait lifetime arguments, still named until typed
    /// lowering resolves them against the realizing machine telescope.
    pub lifetime_arguments: Vec<DiagnosticName>,
    pub arguments: HandleSpan<crate::types::TypeReference>,
    /// The exact requirement binding (`satisfies Trait::requirement`). Source
    /// lowering always supplies `Some`; `alias` names the satisfier
    /// (`as Name`) for plural algebras / signature collisions.
    pub requirement: Option<DiagnosticName>,
    pub alias: Option<DiagnosticName>,
    /// Exact interned identity of the external leaf's structured `via`
    /// bootstrap binding. New source instead retains `via_expression` until
    /// hermetic evaluation produces the normalized Omega-owned binding.
    pub external_binding: Option<language_semantics::ExternalBindingId>,
    /// Ordinary authored expression after `via`, lowered through the same
    /// name-resolution table as every other expression. Invalid for the
    /// segregated bootstrap binding and for ordinary checked satisfiers.
    pub via_expression: ExpressionHandle,
    /// Exact authored `via` keyword occurrence retained separately from the
    /// semantic binding identity for package-review source custody.
    pub external_binding_source_span: Option<source::SourceSpan>,
}

#[cfg(test)]
/// One guarded-transition state in a fresh program: the snapshot and
/// table-rebuild tests start from the same fixture.
#[cfg(test)]
pub(crate) fn test_program_with_guarded_transition()
-> (crate::SymbolResolvedTrees, HandleSpan<Handle<State>>) {
    let mut program = crate::SymbolResolvedTrees::default();
    let guard =
        program
            .tables
            .bodies
            .expressions
            .insert(crate::expression::ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(1),
            ));
    let statements = program.tables.declarations.state_statements.insert_many([
        crate::statement::Statement::Transition(crate::statement::Transition {
            target: crate::statement::TransitionTarget::Terminal,
            continuation: None,
            guard: crate::statement::TransitionGuard::When(guard),
            proof_selectors: Box::default(),
            exit: Default::default(),
            ends_dispatch: true,
            source_span: Default::default(),
        }),
    ]);
    let state = program
        .tables
        .declarations
        .machine_states
        .append(crate::state::test_state(statements));
    let states = program
        .tables
        .declarations
        .machine_state_handles
        .insert_many([state]);
    (program, states)
}

#[cfg(test)]
pub(crate) fn test_machine(states: HandleSpan<Handle<State>>) -> Machine {
    Machine {
        symbol: SymbolHandle::invalid(),
        generic_data_origin: Default::default(),
        trait_default_origin: Default::default(),
        name: DiagnosticName::generated("main"),
        attached_data: None,
        attached_data_symbol: SymbolHandle::invalid(),
        attached_data_application: None,
        spelling: None,
        is_public: false,
        supply_mode: Default::default(),
        target: None,
        target_symbol_name: Default::default(),
        authored_target: None,
        target_family: SymbolHandle::invalid(),
        target_family_interface: false,
        interface_exposure: Default::default(),
        target_family_contracts_prepared: false,
        body_is_present: true,
        body_status: crate::machine::MachineBodyStatus::Complete,
        has_structural_type_equations: false,
        structural_type_equations_pending: false,
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        admitted_same_stack_bytes: 0,
        suspends_keyword_source_spans: Vec::new(),
        blocks_keyword_source_spans: Vec::new(),
        compiler_selection_partition: None,
        storage: MachineStorage {
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            ranking_subjects: HandleSpan::empty(),
            ranking_view: HandleSpan::empty(),
            ranking_view_arguments: HandleSpan::empty(),
            ranking_range: Default::default(),
            invokes: HandleSpan::empty(),
            suspends: false,
            blocks: false,
            contracts: HandleSpan::empty(),
            states,
        },
    }
}
