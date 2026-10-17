use crate::expression::ExpressionHandle;
use crate::name::Identifier;
use crate::signature::{AuthoredInvocation, SignatureContract};
use crate::state::State;
use crate::types::TypeReferenceHandle;
use arena::HandleSpan;
use language_core::operator_spelling::OperatorSpelling;
use symbols::SymbolHandle;

impl crate::TypedTrees {
    /// Authored target membership, including the canonical member whose source
    /// spelling has no sibling suffix. Selection never changes this identity.
    pub fn machine_authored_target<'program>(
        &'program self,
        machine: &'program Machine,
    ) -> Option<&'program str> {
        machine.authored_target.as_ref().map(Identifier::as_str)
    }
}

/// Whether this declaration's implementation has entered typed storage.
/// Deferred states retain their real interfaces, not executable empty bodies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MachineBodyStatus {
    Deferred,
    #[default]
    Complete,
}

/// The consumer of this checked program. Declaration programs produce Build
/// selections; their calls are not executable operations in the product.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MachinePurpose {
    #[default]
    Runtime,
    ProviderDefaults,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub attached_data: Option<Identifier>,
    /// Exact nominal declaration named by `attached_data`, retained from
    /// resolution so cleanup and method semantics never reselect by spelling.
    pub attached_data_symbol: SymbolHandle,
    /// Exact attached owner application resolved in this method's binder
    /// scope. NULL means that the attachment is not generic.
    pub attached_data_application: TypeReferenceHandle,
    /// Exact authored method behind a validated generic-data clone. Invalid
    /// for authored machines; application checking retains the template bounds.
    pub generic_data_template: SymbolHandle,
    /// The fixed operator token bound by this declaration head, copied from
    /// symbol-resolved trees; `None` is the ordinary named form. Checked
    /// consumers read this binding instead of guessing from the leaf name.
    pub spelling: Option<OperatorSpelling>,
    /// Retained source-level package visibility, independent of supply mode.
    pub is_public: bool,
    /// Exact structural exposure retained from resolution. External supply does
    /// not erase a boundary interface or promote a private ordinary external.
    pub interface_exposure:
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    /// Copied from symbol-resolved trees; semantic consumers must not
    /// reconstruct supply from source spelling or body presence.
    pub supply_mode: language_semantics::MachineSupplyMode,
    /// Normalized by typing together with declaration-call custody. Later
    /// stages consume this purpose rather than recognize a source name.
    pub purpose: MachinePurpose,
    /// Copied from symbol-resolved trees: the target of a sibling body that
    /// is checked here and retained through product lowering; `None` for the selected
    /// declaration and every ordinary machine.
    pub target: Option<Identifier>,
    /// Authored target identity, independent of canonical family naming or
    /// Build execution selection. Consumers never reconstruct it from names.
    pub authored_target: Option<Identifier>,
    /// Independent generated Requirement of a product target family. Its
    /// callable contract has no executable body or provider authority.
    pub target_family_interface: bool,
    /// Exact source-body presence copied from symbol-resolved trees. Boundary
    /// supply can be either bodyless or a checked adapter.
    pub body_is_present: bool,
    /// Independent from authored body presence and supply mode. Execution and
    /// proof over the implementation require completed body custody.
    pub body_status: MachineBodyStatus,
    /// Source structural equations are not yet discharged; execution must reject this machine.
    pub structural_type_equations_pending: bool,
    /// TPR2 (decision 23): the normalized termination plan (published
    /// guarantee vs private ranking witness), populated ONCE at the
    /// syntax->resolved lowering and COPIED here -- never re-derived.
    pub termination_plan: language_semantics::MachineTerminationPlan,
    /// EFX: normalized symbol-resolved boundary-service row.
    pub service_reach_row: language_semantics::ServiceReachRowId,
    /// The published row is an installation-selected upper bound rather than
    /// a fixed callable ceiling.
    pub service_reach_is_installation_bound: bool,
    /// Authored same-stack demand of a bodyless external realization, in bytes.
    /// Zero means the source wrote no `stack N bytes` clause. Copied from
    /// symbol-resolved trees; later stages must not remeasure the callee.
    pub admitted_same_stack_bytes: u64,
    /// Exact authored operational-clause keyword occurrences, retained for
    /// package-review source custody and excluded from semantic identity.
    pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
    pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
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

impl Machine {
    /// Type/const/machine and conformance telescopes both require substitution.
    /// Invocation lifetimes alone erase without producing a separate body.
    pub fn has_specialization_parameters(&self) -> bool {
        !self.type_parameters.is_empty() || !self.conformance_bounds.is_empty()
    }

    /// Whether a caller reaches this declaration's authored body as an
    /// ordinary checked callee.
    ///
    /// A bodied `boundary machine` is a checked adapter, not a seam: its body
    /// is authored in-package and competes as an ordinary candidate, while
    /// only the bodyless declaration is the boundary crossing. Consumers that
    /// re-derive this as `supply_mode == CheckedBody` silently refuse every
    /// adapter; ask here instead.
    pub fn supplies_checked_body(&self) -> bool {
        match self.supply_mode {
            language_semantics::MachineSupplyMode::CheckedBody => true,
            language_semantics::MachineSupplyMode::Boundary => self.body_is_present,
            _ => false,
        }
    }

    /// An opaque callable carries its signature without supplying a body.
    /// This includes Requirements, not just host-boundary trust tiers. An
    /// unfinished CheckedBody never becomes opaque merely because its body
    /// is absent; missing executable planning remains an error.
    pub fn has_bodyless_signature(&self) -> bool {
        !self.supplies_checked_body() && !self.body_is_present
    }

    /// The spelling of this machine's symbol in the symbol table: the
    /// authored name, or `<name>::<target>` for a target sibling, whose
    /// symbol is kept apart from the realized declaration's.
    pub fn symbol_spelling(&self) -> std::borrow::Cow<'_, str> {
        match &self.target {
            Some(target) => {
                std::borrow::Cow::Owned(format!("{}::{}", self.name.as_str(), target.as_str()))
            }
            None => std::borrow::Cow::Borrowed(self.name.as_str()),
        }
    }
}

/// One target-scoped machine family: the authored contract path plus every
/// target's checked body.
///
/// The frontend places one canonical member at the authored path and gives
/// siblings distinct symbols. Those roles remain stable across realizations.
/// This roster, not the presence of a sibling suffix, owns target membership.
/// A canonical body is not a portable contract: direct portable references
/// remain unsupported until checking carries that independent contract.
/// Families also retain deployment tags unknown to the native target catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetMachineFamily {
    /// The authored contract path every member implements
    /// (`StatLayout::plan`); equal to each member machine's `name`.
    pub name: Identifier,
    /// The machine the frontend placed at the family path: the canonical
    /// body, selected in target-name order independently of the native catalog.
    /// Portable references initially resolve here, but that resolution alone
    /// never authorizes using this member's body as a family-wide guarantee.
    pub canonical: SymbolHandle,
    /// All members share the declaration's execution phase.
    pub purpose: MachinePurpose,
    /// Every authored body keyed by its authored target name, ordered by
    /// target name. The canonical body appears under its own authored
    /// target.
    pub members: Vec<TargetMachineFamilyMember>,
}

/// One target's body within a [`TargetMachineFamily`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetMachineFamilyMember {
    /// The authored target name (`linux_x86_64`, `demo_target`, ...).
    pub target: Identifier,
    /// The typed machine declaration holding this target's body.
    pub machine: SymbolHandle,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            attached_data: None,
            attached_data_symbol: SymbolHandle::invalid(),
            attached_data_application: TypeReferenceHandle::invalid(),
            generic_data_template: SymbolHandle::invalid(),
            spelling: None,
            is_public: false,
            interface_exposure: Default::default(),
            supply_mode: language_semantics::MachineSupplyMode::CheckedBody,
            purpose: MachinePurpose::Runtime,
            target: None,
            authored_target: None,
            target_family_interface: false,
            body_is_present: true,
            body_status: MachineBodyStatus::Complete,
            structural_type_equations_pending: false,
            termination_plan: language_semantics::MachineTerminationPlan::default(),
            service_reach_row: language_semantics::ServiceReachRowId::NULL,
            service_reach_is_installation_bound: false,
            admitted_same_stack_bytes: 0,
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

/// A generic evidence binder `Order satisfies Ranked<Element>`. The binder is
/// an implementation parameter; its argument is one named conformance whose
/// trait application equals this one after substitution. No parameter is a
/// data subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericConformanceBound {
    /// The binder's `ConformanceParameter` symbol.
    pub binder: SymbolHandle,
    pub binder_name: Identifier,
    /// The bound trait, or a transparent refinement of one.
    pub trait_symbol: SymbolHandle,
    pub trait_name: Identifier,
    pub arguments: Vec<TypeReferenceHandle>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnedData {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
    pub initial_value: ExpressionHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraitConformance {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    /// Raw declaration-order ordinals into the realizing machine's lifetime
    /// telescope, one per target-trait lifetime parameter. Public edge
    /// identity first-occurrence-normalizes this vector instead of exposing it.
    pub trait_lifetime_arguments: Vec<u32>,
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
    pub requirement_source_span: Option<source::SourceSpan>,
    pub alias: Option<Identifier>,
    /// Exact interned identity of the external leaf's structured `via`
    /// bootstrap binding. New source instead retains `via_expression` until
    /// hermetic evaluation produces the normalized Omega-owned binding.
    pub external_binding: Option<language_semantics::ExternalBindingId>,
    /// Ordinary typed expression after `via`. The admitted source-evaluation
    /// rung accepts one exact zero-argument direct machine call. Invalid for
    /// bootstrap bindings and ordinary checked satisfiers.
    pub via_expression: crate::expression::ExpressionHandle,
    /// Exact authored `via` keyword occurrence retained separately from the
    /// semantic binding identity for package-review source custody.
    pub external_binding_source_span: Option<source::SourceSpan>,
}

#[derive(Debug, Clone, Copy)]
pub enum SatisfiedDeclaration<'program> {
    Trait {
        definition: &'program crate::trait_definition::TraitDefinition,
        requirement: &'program crate::signature::StateSignature,
    },
    TopLevelRequirement(&'program Machine),
    Operator(&'program crate::operator::OperatorDefinition),
}

impl SatisfiedDeclaration<'_> {
    pub fn symbol(self) -> SymbolHandle {
        match self {
            Self::Trait { requirement, .. } => requirement.symbol,
            Self::TopLevelRequirement(requirement) => requirement.symbol,
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
    if let Some(definition) = program.trait_definition_by_symbol(conformance.symbol) {
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

    // An explicit non-token top-level requirement is already one exact root-machine
    // declaration. Symbol resolution selects that declaration; typed
    // settlement only rejoins the retained symbol to the complete machine and
    // verifies its supply kind. Authored owner/member spellings remain
    // diagnostic material and cannot repair a missing or wrong-kind symbol.
    if let Some(requirement) = program.machines().iter().find(|requirement| {
        requirement.symbol == conformance.symbol
            && requirement.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
            && requirement.spelling.is_none()
    }) {
        return Some(SatisfiedDeclaration::TopLevelRequirement(requirement));
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
    let operator = crate::operator::resolve_satisfied_checked_operator_for_conformance(
        program,
        machine,
        conformance,
    )?;
    // Token-bearing top-level requirements share the operator signature
    // matcher, but retain their machine declaration as the satisfaction target.
    if let Some(requirement) = program.machines().iter().find(|requirement| {
        requirement.symbol == operator.symbol
            && requirement.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
    }) {
        return Some(SatisfiedDeclaration::TopLevelRequirement(requirement));
    }
    Some(SatisfiedDeclaration::Operator(operator))
}

/// The machine keyed `symbol`, when declared. Machines are keyed by symbol
/// rather than name, so dispatch and evidence lookups resolve identity
/// directly.
pub fn machine_by_symbol(machines: &[Machine], symbol: SymbolHandle) -> Option<&Machine> {
    machines.iter().find(|machine| machine.symbol == symbol)
}
