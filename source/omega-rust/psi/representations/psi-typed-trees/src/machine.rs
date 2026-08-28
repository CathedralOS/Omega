use crate::expression::ExpressionHandle;
use crate::name::Identifier;
use crate::signature::{AuthoredInvocation, SignatureContract};
use crate::state::State;
use crate::types::TypeReferenceHandle;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub attached_data: Option<Identifier>,
    /// Exact nominal declaration named by `attached_data`, retained from
    /// resolution so cleanup and method semantics never reselect by spelling.
    pub attached_data_symbol: SymbolHandle,
    /// Retained source-level package visibility, independent of supply mode.
    pub is_public: bool,
    /// Copied from symbol-resolved trees; semantic consumers must not
    /// reconstruct supply from source spelling or body presence.
    pub supply_mode: psi_language_semantics::MachineSupplyMode,
    /// Exact source-body presence copied from symbol-resolved trees. Boundary
    /// supply can be either bodyless or a checked adapter.
    pub body_is_present: bool,
    /// TPR2 (decision 23): the normalized termination plan (published
    /// guarantee vs private ranking witness), populated ONCE at the
    /// syntax->resolved lowering and COPIED here -- never re-derived.
    pub termination_plan: psi_language_semantics::MachineTerminationPlan,
    /// EFX: normalized symbol-resolved boundary-service row.
    pub service_reach_row: psi_language_semantics::ServiceReachRowId,
    /// The published row is an installation-selected upper bound rather than
    /// a fixed callable ceiling.
    pub service_reach_is_installation_bound: bool,
    /// Exact authored operational-clause keyword occurrences, retained for
    /// package-review source custody and excluded from semantic identity.
    pub suspends_keyword_source_spans: Vec<psi_source::SourceSpan>,
    pub blocks_keyword_source_spans: Vec<psi_source::SourceSpan>,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub owned_data: HandleSpan<OwnedData>,
    pub satisfies: HandleSpan<TraitConformance>,
    pub conformance_bounds: Vec<GenericConformanceBound>,
    pub invokes: HandleSpan<AuthoredInvocation>,
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: HandleSpan<SignatureContract>,
    pub states: HandleSpan<State>,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            attached_data: None,
            attached_data_symbol: SymbolHandle::invalid(),
            is_public: false,
            supply_mode: psi_language_semantics::MachineSupplyMode::CheckedBody,
            body_is_present: true,
            termination_plan: psi_language_semantics::MachineTerminationPlan::default(),
            service_reach_row: psi_language_semantics::ServiceReachRowId::NULL,
            service_reach_is_installation_bound: false,
            suspends_keyword_source_spans: Vec::new(),
            blocks_keyword_source_spans: Vec::new(),
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            invokes: HandleSpan::empty(),
            suspends: false,
            blocks: false,
            contracts: HandleSpan::empty(),
            states: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericConformanceBound {
    pub binder: Option<SymbolHandle>,
    pub binder_name: Option<Identifier>,
    pub subject: SymbolHandle,
    pub subject_name: Identifier,
    pub carrier: SymbolHandle,
    pub carrier_name: Identifier,
    pub arguments: Vec<TypeReferenceHandle>,
    /// Exact selected declaration and its recursively delimited application.
    /// The selected symbol is never reconstructed from subject/trait shape.
    pub selected_conformance: Option<crate::expression::StaticMachineArgument>,
}

impl GenericConformanceBound {
    pub fn selected_conformance_symbol(&self) -> Option<SymbolHandle> {
        self.selected_conformance
            .as_ref()
            .map(|selected| selected.symbol)
    }

    pub fn selected_conformance_name(&self) -> Option<&Identifier> {
        self.selected_conformance
            .as_ref()
            .and_then(|selected| selected.path.last())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedData {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
    pub initial_value: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitConformance {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    /// The exact requirement binding (`satisfies Trait::requirement`). Source
    /// lowering always supplies `Some`; `alias` names the satisfier
    /// (`as Name`) for plural algebras / signature collisions.
    pub requirement: Option<Identifier>,
    /// Exact overload selected by the implementation entry signature. Trait
    /// requirements retain their state symbol; operator realizations retain
    /// the exact operator declaration symbol.
    pub requirement_symbol: SymbolHandle,
    /// Exact authored requirement-token occurrence retained because typed
    /// identifiers deliberately own text only.
    pub requirement_source_span: Option<psi_source::SourceSpan>,
    pub alias: Option<Identifier>,
    /// Exact interned identity of the external leaf's structured `via`
    /// binding. Absence means this is an ordinary checked satisfier.
    pub external_binding: Option<psi_language_semantics::ExternalBindingId>,
    /// Exact authored `via` keyword occurrence retained separately from the
    /// semantic binding identity for package-review source custody.
    pub external_binding_source_span: Option<psi_source::SourceSpan>,
}

impl Default for TraitConformance {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            arguments: HandleSpan::empty(),
            requirement: None,
            requirement_symbol: SymbolHandle::invalid(),
            requirement_source_span: None,
            alias: None,
            external_binding: None,
            external_binding_source_span: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SatisfiedDeclaration<'program> {
    Trait {
        definition: &'program crate::trait_definition::TraitDefinition,
        requirement: &'program crate::signature::StateSignature,
    },
    Operator(&'program crate::operator::OperatorDefinition),
}

impl SatisfiedDeclaration<'_> {
    pub fn symbol(self) -> SymbolHandle {
        match self {
            Self::Trait { requirement, .. } => requirement.symbol,
            Self::Operator(operator) => operator.symbol,
        }
    }
}

/// Resolve one authored `satisfies Namespace::requirement` edge from the
/// complete typed program. Result-dispatch identity settles overloaded trait
/// requirements; exact signature identity settles operators.
pub fn resolve_satisfied_declaration<'program>(
    program: &'program crate::TypedTrees,
    machine: &'program Machine,
    conformance: &'program TraitConformance,
) -> Option<SatisfiedDeclaration<'program>> {
    let requirement_name = conformance.requirement.as_ref()?;
    if let Some(definition) = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == conformance.symbol)
    {
        let entry = program.machine_states(machine).first()?;
        let implementation_dispatch = program.normalized_result_dispatch_set(entry.return_type);
        let named = program
            .trait_machine_signatures(definition)
            .iter()
            .filter(|requirement| requirement.name == *requirement_name)
            .collect::<Vec<_>>();
        let matching = if named.len() == 1 {
            named
        } else {
            named
                .into_iter()
                .filter(|requirement| {
                    program.normalized_result_dispatch_set(requirement.return_type)
                        == implementation_dispatch
                })
                .collect()
        };
        let [requirement] = matching.as_slice() else {
            return None;
        };
        return Some(SatisfiedDeclaration::Trait {
            definition,
            requirement,
        });
    }

    if !program
        .type_reference_table
        .type_reference_handles(conformance.arguments)
        .is_empty()
    {
        return None;
    }
    // Resolve the declaration the source selected before applying supply-mode
    // policy. External supply does not turn an ordinary operator into a
    // boundary operator; validation and package admission reject that
    // unsupported association independently while retaining its exact subject.
    let operator = crate::operator::resolve_satisfied_checked_operator(
        program,
        machine,
        conformance.name.as_str(),
        requirement_name.as_str(),
    )?;
    Some(SatisfiedDeclaration::Operator(operator))
}

impl Default for OwnedData {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            type_reference: TypeReferenceHandle::invalid(),
            initial_value: ExpressionHandle::invalid(),
        }
    }
}
